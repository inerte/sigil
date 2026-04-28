use serde_json::Value;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .to_path_buf()
}

fn sigil_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sigil"))
}

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = repo_root().join(".local").join(format!(
        "sigil-cli-run-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_program(dir: &Path, name: &str, source: &str) -> PathBuf {
    let file = dir.join(name);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&file, source).unwrap();
    file
}

fn compile_runner(file: &Path) -> PathBuf {
    compile_runner_with_env(file, None)
}

fn compile_runner_with_env(file: &Path, selected_env: Option<&str>) -> PathBuf {
    let mut compile = Command::new(sigil_bin());
    compile.current_dir(repo_root()).arg("compile");
    if let Some(env_name) = selected_env {
        compile.arg("--env").arg(env_name);
    }
    let output = compile.arg(file).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let compile_json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let root_ts = PathBuf::from(
        compile_json["data"]["outputs"]["rootTs"]
            .as_str()
            .expect("compile output should include rootTs"),
    );
    let runner = root_ts.with_extension("run.mjs");
    let mut warm = Command::new(sigil_bin());
    warm.current_dir(repo_root()).arg("run");
    if let Some(env_name) = selected_env {
        warm.arg("--env").arg(env_name);
    }
    let warm_output = warm.arg(file).arg("--").arg("--help").output().unwrap();
    assert!(
        warm_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&warm_output.stdout),
        String::from_utf8_lossy(&warm_output.stderr)
    );
    assert!(runner.exists(), "expected runner at {}", runner.display());
    runner
}

fn run_compiled_runner(runner: &Path, args: &[&str]) -> std::process::Output {
    Command::new("node")
        .current_dir(repo_root())
        .arg(runner)
        .args(args)
        .output()
        .unwrap()
}

fn modified_time(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

fn collect_cached_module_outputs(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_cached_module_outputs(&path, files);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("mjs") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if file_name.ends_with(".run.mjs") {
            continue;
        }
        files.push(path);
    }
}

fn topology_cached_outputs(root: &Path) -> Vec<PathBuf> {
    let local_dir = root.join(".local");
    let mut files = Vec::new();
    collect_cached_module_outputs(&local_dir, &mut files);
    files.sort();
    files
}

fn write_topology_project(root: &Path) -> PathBuf {
    write_program(
        root,
        "sigil.json",
        "{\n  \"name\": \"topologyCache\",\n  \"version\": \"2026-04-15T00-00-00Z\"\n}\n",
    );
    write_program(
        root,
        "src/topology.lib.sigil",
        "c test=(§topology.environment(\"test\"):§topology.Environment)\n",
    );
    write_program(
        root,
        "config/test.lib.sigil",
        concat!(
            "c world=(†runtime.world(\n",
            "  †clock.systemClock(),\n",
            "  †fs.real(),\n",
            "  †fsWatch.real(),\n",
            "  [],\n",
            "  †log.capture(),\n",
            "  †process.real(),\n",
            "  †pty.real(),\n",
            "  †random.seeded(7),\n",
            "  †sql.deny(),\n",
            "  †stream.live(),\n",
            "  †task.real(),\n",
            "  [],\n",
            "  †timer.virtual(),\n",
            "  †websocket.real()\n",
            "):†runtime.World)\n",
        ),
    );
    write_program(root, "src/main.sigil", "λmain()=>String=\"cache ok\"\n")
}

fn node_has_global_websocket() -> bool {
    Command::new("node")
        .arg("-e")
        .arg("process.exit(typeof WebSocket === 'function' ? 0 : 1)")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn write_bridge_subscription_project(root: &Path) -> PathBuf {
    write_program(
        root,
        "sigil.json",
        "{\n  \"name\": \"bridgeRuntime\",\n  \"version\": \"2026-04-17T00-00-00Z\"\n}\n",
    );
    write_program(
        root,
        "bridges/subscriptionProbe.mjs",
        "export function tick(emit) {\n  emit('ready');\n  return () => {};\n}\n",
    );
    write_program(
        root,
        "src/main.sigil",
        concat!(
            "e bridge::subscriptionProbe:{tick: subscribes λ()=>String}\n\n",
            "λmain()=>!Stream String={\n",
            "  using source=bridge::subscriptionProbe.tick(){\n",
            "    match §stream.next(source){\n",
            "      §stream.Item(text)=>text|\n",
            "      §stream.Done()=>\"done\"\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    )
}

fn write_http_websocket_bridge_project(root: &Path) -> PathBuf {
    write_program(
        root,
        "sigil.json",
        "{\n  \"name\": \"httpWsBridge\",\n  \"version\": \"2026-04-17T00-00-00Z\"\n}\n",
    );
    write_program(
        root,
        "src/topology.lib.sigil",
        concat!(
            "c liveUpdates=(§topology.websocketHandle(\"liveUpdates\"):§topology.WebSocketHandle)\n\n",
            "c test=(§topology.environment(\"test\"):§topology.Environment)\n",
        ),
    );
    write_program(
        root,
        "config/test.lib.sigil",
        concat!(
            "c world=(†runtime.withWebSocketHandles(\n",
            "  [†websocket.realHandle(•topology.liveUpdates)],\n",
            "  †runtime.world(\n",
            "    †clock.systemClock(),\n",
            "    †fs.real(),\n",
            "    †fsWatch.real(),\n",
            "    [],\n",
            "    †log.capture(),\n",
            "    †process.real(),\n",
            "    †pty.real(),\n",
            "    †random.seeded(7),\n",
            "    †sql.deny(),\n",
            "    †stream.live(),\n",
            "    †task.real(),\n",
            "    [],\n",
            "    †timer.virtual(),\n",
            "    †websocket.real()\n",
            "  )\n",
            "):†runtime.World)\n",
        ),
    );
    write_program(root, "src/types.lib.sigil", "t WsClient={id:String}\n");
    write_program(
        root,
        "bridges/wsClient.mjs",
        concat!(
            "export async function roundTrip(url) {\n",
            "  const socket = new WebSocket(String(url));\n",
            "  await new Promise((resolve, reject) => {\n",
            "    const onOpen = () => { cleanup(); resolve(); };\n",
            "    const onError = (event) => {\n",
            "      cleanup();\n",
            "      reject(new Error(String(event?.message ?? 'websocket open failed')));\n",
            "    };\n",
            "    const cleanup = () => {\n",
            "      socket.removeEventListener('open', onOpen);\n",
            "      socket.removeEventListener('error', onError);\n",
            "    };\n",
            "    socket.addEventListener('open', onOpen);\n",
            "    socket.addEventListener('error', onError);\n",
            "  });\n",
            "  const reply = await new Promise((resolve, reject) => {\n",
            "    const onMessage = (event) => { cleanup(); resolve(String(event?.data ?? '')); };\n",
            "    const onError = (event) => { cleanup(); reject(new Error(String(event?.message ?? 'websocket message failed'))); };\n",
            "    const onClose = () => { cleanup(); resolve('closed'); };\n",
            "    const cleanup = () => {\n",
            "      socket.removeEventListener('message', onMessage);\n",
            "      socket.removeEventListener('error', onError);\n",
            "      socket.removeEventListener('close', onClose);\n",
            "    };\n",
            "    socket.addEventListener('message', onMessage);\n",
            "    socket.addEventListener('error', onError);\n",
            "    socket.addEventListener('close', onClose);\n",
            "    socket.send('ping');\n",
            "  });\n",
            "  try {\n",
            "    socket.close();\n",
            "  } catch {\n",
            "    // best effort\n",
            "  }\n",
            "  return reply;\n",
            "}\n",
        ),
    );
    write_program(
        root,
        "src/main.sigil",
        concat!(
            "e bridge::wsClient:{roundTrip:λ(String)=>String}\n\n",
            "λmain()=>!Http!Stream!Task String={\n",
            "  using server=§httpServer.listenWithWebSockets(\n",
            "    0,\n",
            "    [§httpServer.websocketRoute(\n",
            "      •topology.liveUpdates,\n",
            "      \"/ws\"\n",
            "    )]\n",
            "  ){\n",
            "    {\n",
            "      using clientTask=(§task.spawn(λ()=>String=bridge::wsClient.roundTrip((\"ws://127.0.0.1:\"\n",
            "        ++§string.intToString(§httpServer.port(server))\n",
            "        ++\"/ws\":String))):Owned[§task.Task[String]]){\n",
            "        {\n",
            "          using clients=§httpServer.websocketConnections(\n",
            "            •topology.liveUpdates,\n",
            "            server\n",
            "          ){\n",
            "            match §stream.next(clients){\n",
            "              §stream.Item(client)=>{\n",
            "                using messages=§httpServer.websocketMessages(client){\n",
            "                  l received=(§stream.next(messages):§stream.Next[String]);\n",
            "                  l _=(§httpServer.websocketSend(\n",
            "                    client,\n",
            "                    \"pong\"\n",
            "                  ):Unit);\n",
            "                  l _=(§httpServer.websocketClose(client):Unit);\n",
            "                  match received{\n",
            "                    §stream.Item(text)=>match (§task.wait(clientTask):§task.TaskResult[String]){\n",
            "                      §task.Succeeded(reply)=>match text=\"ping\" and reply=\"pong\"{\n",
            "                        true=>\"ok\"|\n",
            "                        false=>\"mismatch\"\n",
            "                      }|\n",
            "                      §task.Failed(message)=>message|\n",
            "                      §task.Cancelled()=>\"cancelled\"\n",
            "                    }|\n",
            "                    §stream.Done()=>\"no-message\"\n",
            "                  }\n",
            "                }\n",
            "              }|\n",
            "              §stream.Done()=>\"no-client\"\n",
            "            }\n",
            "          }\n",
            "        }\n",
            "      }\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    )
}

fn parse_json(text: &[u8]) -> Value {
    serde_json::from_slice(text).unwrap()
}

fn parse_replay_artifact(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn replay_event<'a>(artifact: &'a Value, family: &str, operation: &str) -> &'a Value {
    artifact["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["family"] == family && event["operation"] == operation)
        .unwrap()
}

fn line_break_selector(file: &Path, line: usize) -> String {
    format!("{}:{}", file.to_string_lossy(), line)
}

fn path_with_shadowed_pnpm(dir: &Path) -> OsString {
    let bin_dir = dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let shim_path = if cfg!(windows) {
        bin_dir.join("pnpm.cmd")
    } else {
        bin_dir.join("pnpm")
    };
    let shim_source = if cfg!(windows) {
        "@echo off\r\necho shadowed pnpm>&2\r\nexit /b 97\r\n"
    } else {
        "#!/bin/sh\necho shadowed pnpm >&2\nexit 97\n"
    };
    fs::write(&shim_path, shim_source).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&shim_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shim_path, permissions).unwrap();
    }

    let current_path = env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir];
    paths.extend(env::split_paths(&current_path));
    env::join_paths(paths).unwrap()
}

#[test]
fn run_streams_raw_stdout_by_default() {
    let dir = temp_dir("raw-success");
    let file = write_program(
        &dir,
        "main.sigil",
        "e console:{log:λ(String)=>!Log Unit}\n\nλmain()=>!Log Unit=console.log(\"raw ok\")\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "raw ok\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn run_succeeds_when_pnpm_is_shadowed() {
    let dir = temp_dir("shadowed-pnpm");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .env("PATH", path_with_shadowed_pnpm(&dir))
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn run_cli_parses_many_options_and_double_dash() {
    let dir = temp_dir("cli-many-options");
    let file = write_program(
        &dir,
        "main.sigil",
        concat!(
            "λcliProgram()=>§cli.Program[String]=§cli.program(\n",
            "  \"Exercise canonical CLI parsing.\",\n",
            "  \"cliTest\",\n",
            "  Some(§cli.root3(\n",
            "    §cli.flag(\n",
            "      \"Enable verbose mode.\",\n",
            "      \"verbose\",\n",
            "      Some(\"v\")\n",
            "    ),\n",
            "    §cli.manyOption(\n",
            "      \"Select a check id.\",\n",
            "      \"check\",\n",
            "      Some(\"c\"),\n",
            "      \"ID\"\n",
            "    ),\n",
            "    §cli.manyPositionals(\n",
            "      \"Input paths.\",\n",
            "      \"PATH\"\n",
            "    ),\n",
            "    λ(verbose:Bool,checks:[String],paths:[String])=>String=(match verbose{\n",
            "      true=>\"true\"|\n",
            "      false=>\"false\"\n",
            "    })\n",
            "      ++\"|\"\n",
            "      ++§string.join(\n",
            "        \",\",\n",
            "        checks\n",
            "      )\n",
            "      ++\"|\"\n",
            "      ++§string.join(\n",
            "        \",\",\n",
            "        paths\n",
            "      ),\n",
            "    \"Accept a flag, repeated options, and trailing paths.\"\n",
            "  )),\n",
            "  []\n",
            ")\n",
            "\n",
            "λmain()=>!Log!Process String=§cli.run(\n",
            "  §process.argv(),\n",
            "  cliProgram()\n",
            ")\n",
        ),
    );

    let runner = compile_runner(&file);
    let output = run_compiled_runner(
        &runner,
        &[
            "--verbose",
            "--check",
            "repo-compile",
            "--check=docs-drift",
            "--",
            "alpha",
            "--check",
        ],
    );

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "true|repo-compile,docs-drift|alpha,--check\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn run_cli_help_exits_zero_and_prints_usage() {
    let dir = temp_dir("cli-help");
    let file = write_program(
        &dir,
        "main.sigil",
        concat!(
            "λmain()=>!Log!Process String=§cli.run(\n",
            "  §process.argv(),\n",
            "  §cli.program(\n",
            "    \"Exercise canonical help output.\",\n",
            "    \"cliHelp\",\n",
            "    Some(§cli.root2(\n",
            "      §cli.option(\n",
            "        \"Optional name.\",\n",
            "        \"name\",\n",
            "        Some(\"n\"),\n",
            "        \"TEXT\"\n",
            "      ),\n",
            "      §cli.manyPositionals(\n",
            "        \"Input paths.\",\n",
            "        \"PATH\"\n",
            "      ),\n",
            "      λ(name:Option[String],paths:[String])=>String=§string.intToString(#paths),\n",
            "      \"Accept one option and trailing paths.\"\n",
            "    )),\n",
            "    []\n",
            "  )\n",
            ")\n",
        ),
    );

    let runner = compile_runner(&file);
    let output = run_compiled_runner(&runner, &["--help"]);

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Exercise canonical help output."));
    assert!(stdout.contains("Usage: cliHelp [--name TEXT] [PATH ...]"));
    assert!(stdout.contains("--name, -n TEXT  Optional name."));
    assert!(stdout.contains("[PATH ...]  Input paths."));
    assert!(output.stderr.is_empty());
}

#[test]
fn run_cli_parse_errors_exit_two_and_print_usage() {
    let dir = temp_dir("cli-parse-error");
    let file = write_program(
        &dir,
        "main.sigil",
        concat!(
            "λmain()=>!Log!Process String=§cli.run(\n",
            "  §process.argv(),\n",
            "  §cli.program(\n",
            "    \"Exercise canonical parse failures.\",\n",
            "    \"cliParse\",\n",
            "    Some(§cli.root2(\n",
            "      §cli.flag(\n",
            "        \"Enable verbose mode.\",\n",
            "        \"verbose\",\n",
            "        Some(\"v\")\n",
            "      ),\n",
            "      §cli.option(\n",
            "        \"Optional name.\",\n",
            "        \"name\",\n",
            "        Some(\"n\"),\n",
            "        \"TEXT\"\n",
            "      ),\n",
            "      λ(verbose:Bool,name:Option[String])=>String=\"ok\",\n",
            "      \"Accept one flag and one optional option.\"\n",
            "    )),\n",
            "    []\n",
            "  )\n",
            ")\n",
        ),
    );

    let runner = compile_runner(&file);
    let output = run_compiled_runner(&runner, &["-v", "-v"]);

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error: option '-v' may only appear once"));
    assert!(stderr.contains("Usage: cliParse [--verbose] [--name TEXT]"));
    assert!(output.stdout.is_empty());
}

#[test]
fn run_repo_audit_help_and_parse_failures_use_canonical_cli_output() {
    let repo_audit = repo_root().join("projects/repoAudit/src/main.sigil");
    let help_runner = compile_runner(&repo_audit);
    let help = run_compiled_runner(&help_runner, &["--help"]);

    assert!(help.status.success(), "{help:?}");
    let help_stdout = String::from_utf8_lossy(&help.stdout);
    assert!(help_stdout.contains("Audit first-party repository invariants."));
    assert!(help_stdout.contains("Usage: repoAudit [--check ID ...]"));
    assert!(help_stdout
        .contains("--check, -c ID  Run only the named check. Repeat to select multiple checks."));
    assert!(help.stderr.is_empty());

    let parse_error = run_compiled_runner(&help_runner, &["--check"]);

    assert_eq!(parse_error.status.code(), Some(2), "{parse_error:?}");
    let parse_stderr = String::from_utf8_lossy(&parse_error.stderr);
    assert!(parse_stderr.contains("error: expected value after '--check'"));
    assert!(parse_stderr.contains("Usage: repoAudit [--check ID ...]"));
    assert!(!parse_stderr.trim_start().starts_with('{'));
    assert!(parse_error.stdout.is_empty());
}

#[test]
fn run_topology_tcp_help_uses_canonical_cli_output() {
    let topology_tcp = repo_root().join("projects/topology-tcp/src/tcpRoundtripServer.sigil");
    let runner = compile_runner_with_env(&topology_tcp, Some("test"));
    let output = run_compiled_runner(&runner, &["--help"]);

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run the TCP roundtrip server."));
    assert!(stdout.contains("Usage: tcpRoundtripServer [port] [ready-file]"));
    assert!(stdout.contains("[port]  TCP port to listen on."));
    assert!(stdout.contains("[ready-file]  Path to write the ready port file."));
    assert!(output.stderr.is_empty());
}

#[test]
fn run_real_pty_smoke_succeeds_when_runtime_bridge_is_available() {
    if !repo_root()
        .join("language/runtime/node/node_modules/node-pty")
        .exists()
    {
        return;
    }

    let dir = temp_dir("pty-smoke");
    let file = write_program(
        &dir,
        "main.sigil",
        concat!(
            "λmain()=>!Pty!Stream Bool={\n",
            "  using session=§pty.spawn({\n",
            "    argv:[\n",
            "      \"/bin/sh\",\n",
            "      \"-lc\",\n",
            "      \"printf ready\"\n",
            "    ],\n",
            "    cols:80,\n",
            "    cwd:None(),\n",
            "    env:({↦}:{String↦String}),\n",
            "    rows:24\n",
            "  }){\n",
            "    l events=(§pty.events(session):§stream.Source[§pty.Event]);\n",
            "    l first=(§stream.next(events):§stream.Next[§pty.Event]);\n",
            "    l exitCode=(§pty.wait(session):Int);\n",
            "    match first{\n",
            "      §stream.Item(§pty.Output(text))=>§string.contains(\n",
            "        text,\n",
            "        \"ready\"\n",
            "      ) and exitCode=0|\n",
            "      _=>false\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "true\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn run_real_fswatch_smoke_succeeds_when_recursive_watch_is_available() {
    if !cfg!(target_os = "macos") {
        return;
    }

    let dir = temp_dir("fswatch-smoke");
    let file = write_program(
        &dir,
        "main.sigil",
        concat!(
            "λmain()=>!Fs!FsWatch!Stream!Timer Bool={\n",
            "  l _=(§file.makeDirs(\"watched\"):Unit);\n",
            "  l observed={\n",
            "    using watch=§fsWatch.watch(\"watched\"){\n",
            "      l _=(§time.sleepMs(100):Unit);\n",
            "      l _=(§file.writeText(\n",
            "        \"ready\",\n",
            "        §path.join(\n",
            "          \"watched\",\n",
            "          \"fresh.txt\"\n",
            "        )\n",
            "      ):Unit);\n",
            "      l events=(§fsWatch.events(watch):§stream.Source[§fsWatch.Event]);\n",
            "      l first=(§stream.next(events):§stream.Next[§fsWatch.Event]);\n",
            "      match first{\n",
            "        §stream.Item(§fsWatch.Created(path))=>path=\"fresh.txt\"|\n",
            "        §stream.Item(§fsWatch.Changed(path))=>path=\"fresh.txt\"|\n",
            "        _=>false\n",
            "      }\n",
            "    }\n",
            "  };\n",
            "  observed\n",
            "}\n",
        ),
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "true\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn run_project_bridge_subscription_succeeds() {
    let dir = temp_dir("bridge-subscription");
    let file = write_bridge_subscription_project(&dir);

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ready\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn run_http_server_websocket_shared_port_smoke_succeeds_when_runtime_bridge_is_available() {
    if !repo_root()
        .join("language/runtime/node/node_modules/ws")
        .exists()
    {
        return;
    }
    if !node_has_global_websocket() {
        return;
    }

    let dir = temp_dir("http-websocket-shared-port");
    let file = write_http_websocket_bridge_project(&dir);

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--env")
        .arg("test")
        .arg(&file)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn run_uses_standalone_local_world_when_present() {
    let dir = temp_dir("standalone-world");
    let file = write_program(
        &dir,
        "main.sigil",
        concat!(
            "c auditLog=(§topology.logSink(\"auditLog\"):§topology.LogSink)\n\n",
            "c world=(†runtime.withLogSinks(\n",
            "  [†log.captureSink(auditLog)],\n",
            "  †runtime.world(\n",
            "    †clock.systemClock(),\n",
            "    †fs.real(),\n",
            "    †fsWatch.real(),\n",
            "    [],\n",
            "    †log.capture(),\n",
            "    †process.real(),\n",
            "    †pty.real(),\n",
            "    †random.seeded(7),\n",
            "    †sql.deny(),\n",
            "    †stream.live(),\n",
            "    †task.real(),\n",
            "    [],\n",
            "    †timer.virtual(),\n",
            "    †websocket.real()\n",
            "  )\n",
            "):†runtime.World)\n\n",
            "λmain()=>!Log String={\n",
            "  l _=(§log.write(\n",
            "    \"single-file\",\n",
            "    auditLog\n",
            "  ):Unit);\n",
            "  \"done\"\n",
            "}\n",
        ),
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "done\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn run_process_wait_does_not_hang_on_wrapper_exit_with_inherited_pipes() {
    let dir = temp_dir("process-close-fallback");
    let file = write_program(
        &dir,
        "main.sigil",
        concat!(
            "λmain()=>!Process Int={\n",
            "  l result=(§process.run(§process.command([\n",
            "    \"node\",\n",
            "    \"-e\",\n",
            "    \"const { spawn } = require('child_process'); const child = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 15000)'], { detached: true, stdio: 'inherit' }); child.unref(); console.log('wrapper'); process.exit(0);\"\n",
            "  ])):§process.ProcessResult);\n",
            "  result.code\n",
            "}\n",
        ),
    );

    let started = Instant::now();
    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();
    let elapsed = started.elapsed();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
    assert!(
        elapsed.as_secs_f64() < 12.5,
        "expected wrapper exit to finish quickly, took {:?}",
        elapsed
    );
}

#[test]
fn run_project_reuses_cached_compile_outputs_when_inputs_are_unchanged() {
    let dir = temp_dir("project-cache-hit");
    let file = write_topology_project(&dir);

    let first = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--env")
        .arg("test")
        .arg(&file)
        .output()
        .unwrap();

    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stdout)
    );
    assert_eq!(String::from_utf8_lossy(&first.stdout), "cache ok\n");

    let cached_outputs = topology_cached_outputs(&dir);
    assert!(!cached_outputs.is_empty());
    let first_times = cached_outputs
        .iter()
        .map(|path| (path.clone(), modified_time(path)))
        .collect::<Vec<_>>();

    thread::sleep(Duration::from_millis(50));

    let second = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--env")
        .arg("test")
        .arg(&file)
        .output()
        .unwrap();

    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stdout)
    );
    assert_eq!(String::from_utf8_lossy(&second.stdout), "cache ok\n");

    for (path, first_time) in first_times {
        assert_eq!(
            modified_time(&path),
            first_time,
            "cached output changed: {}",
            path.display()
        );
    }
}

#[test]
fn run_feature_flag_rollout_requires_a_stable_key() {
    let dir = temp_dir("feature-flag-missing-key");
    let file = write_program(
        &dir,
        "main.sigil",
        concat!(
            "t Context={userId:Option[String]}\n\n",
            "featureFlag NewCheckout:Bool\n",
            "  createdAt \"2026-04-15T00-00-00Z\"\n",
            "  default false\n\n",
            "c flags=([§featureFlags.entry(\n",
            "  {\n",
            "    key:None(),\n",
            "    rules:[{\n",
            "      action:§featureFlags.Rollout({\n",
            "        percentage:10,\n",
            "        variants:[{\n",
            "          value:true,\n",
            "          weight:100\n",
            "        }]\n",
            "      }),\n",
            "      predicate:λ(context:Context)=>Bool=true\n",
            "    }]\n",
            "  },\n",
            "  NewCheckout\n",
            ")]:§featureFlags.Set[Context])\n\n",
            "λmain()=>Bool=§featureFlags.get(\n",
            "  {userId:Some(\"demo\")},\n",
            "  NewCheckout,\n",
            "  flags\n",
            ")\n",
        ),
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-FEATURE-FLAG");
    assert!(json["error"]["details"]["runtime"]["stderr"]
        .as_str()
        .unwrap()
        .contains("uses a rollout rule but no stable key was resolved"));
}

#[test]
fn run_feature_flag_rollout_rejects_invalid_variant_weights() {
    let dir = temp_dir("feature-flag-invalid-weights");
    let file = write_program(
        &dir,
        "main.sigil",
        concat!(
            "t Context={userId:Option[String]}\n\n",
            "featureFlag NewCheckout:Bool\n",
            "  createdAt \"2026-04-15T00-00-00Z\"\n",
            "  default false\n\n",
            "c flags=([§featureFlags.entry(\n",
            "  {\n",
            "    key:Some(λ(context:Context)=>Option[String]=context.userId),\n",
            "    rules:[{\n",
            "      action:§featureFlags.Rollout({\n",
            "        percentage:10,\n",
            "        variants:[\n",
            "          {\n",
            "            value:true,\n",
            "            weight:60\n",
            "          },\n",
            "          {\n",
            "            value:false,\n",
            "            weight:30\n",
            "          }\n",
            "        ]\n",
            "      }),\n",
            "      predicate:λ(context:Context)=>Bool=true\n",
            "    }]\n",
            "  },\n",
            "  NewCheckout\n",
            ")]:§featureFlags.Set[Context])\n\n",
            "λmain()=>Bool=§featureFlags.get(\n",
            "  {userId:Some(\"demo\")},\n",
            "  NewCheckout,\n",
            "  flags\n",
            ")\n",
        ),
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-FEATURE-FLAG");
    assert!(json["error"]["details"]["runtime"]["stderr"]
        .as_str()
        .unwrap()
        .contains("rollout variant weights must sum to 100"));
}

#[test]
fn run_json_preserves_success_envelope() {
    let dir = temp_dir("json-success");
    let file = write_program(
        &dir,
        "main.sigil",
        "e console:{log:λ(String)=>!Log Unit}\n\nλmain()=>!Log Unit=console.log(\"json ok\")\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc run");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["runtime"]["stdout"], "json ok\n");
    assert_eq!(json["data"]["runtime"]["stderr"], "");
    assert!(PathBuf::from(
        json["data"]["compile"]["spanMapFile"]
            .as_str()
            .expect("spanMapFile path")
    )
    .exists());
}

#[test]
fn run_trace_requires_json() {
    let dir = temp_dir("trace-requires-json");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--trace")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let json = parse_json(output.stderr.trim_ascii());
    assert_eq!(json["command"], "sigilc run");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "cli");
    assert_eq!(json["error"]["code"], "SIGIL-CLI-USAGE");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--json"));
}

#[test]
fn run_trace_expr_requires_trace_and_json() {
    let dir = temp_dir("trace-expr-requires-trace");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--trace-expr")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let json = parse_json(output.stderr.trim_ascii());
    assert_eq!(json["error"]["code"], "SIGIL-CLI-USAGE");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--trace-expr"));
}

#[test]
fn run_breakpoints_require_json() {
    let dir = temp_dir("break-requires-json");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--break-fn")
        .arg("main")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let json = parse_json(output.stderr.trim_ascii());
    assert_eq!(json["error"]["code"], "SIGIL-CLI-USAGE");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--json"));
}

#[test]
fn run_json_breakpoint_not_found_reports_cli_error() {
    let dir = temp_dir("break-not-found");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--break")
        .arg(line_break_selector(&file, 99))
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-CLI-BREAKPOINT-NOT-FOUND");
}

#[test]
fn run_json_breakpoint_ambiguous_function_reports_cli_error() {
    let dir = temp_dir("break-ambiguous");
    fs::write(
        dir.join("sigil.json"),
        "{\n  \"name\": \"breakAmbiguous\",\n  \"version\": \"2026-04-05T14-58-24Z\"\n}\n",
    )
    .unwrap();
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    write_program(&src_dir, "helper2.lib.sigil", "λtarget()=>Int=1\n");
    write_program(
        &src_dir,
        "helper.lib.sigil",
        "λtarget()=>Int=•helper2.target()\n",
    );
    let file = write_program(&src_dir, "main.sigil", "λmain()=>Int=•helper.target()\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--break-fn")
        .arg("target")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-CLI-BREAKPOINT-AMBIGUOUS");
}

#[test]
fn run_json_breakpoint_stop_mode_returns_successful_early_stop() {
    let dir = temp_dir("break-stop");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1+1\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--break-fn")
        .arg("main")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["breakpoints"]["enabled"], true);
    assert_eq!(json["data"]["breakpoints"]["stopped"], true);
    assert_eq!(json["data"]["breakpoints"]["totalHits"], 1);
    assert_eq!(
        json["data"]["breakpoints"]["hits"][0]["declarationLabel"],
        "main"
    );
}

#[test]
fn run_json_breakpoint_hits_include_live_let_locals() {
    let dir = temp_dir("break-let-locals");
    let file = write_program(
        &dir,
        "main.sigil",
        "t UserId=Int where value≥0\n\nλhelper(userId:UserId)=>Int={\n  l current=(userId:UserId);\n  match current=(0:UserId){\n    true=>1|\n    false=>current-current\n  }\n}\n\nλmain()=>Int=helper((1:UserId))\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--break")
        .arg(line_break_selector(&file, 7))
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    let json = parse_json(&output.stdout);
    let locals = json["data"]["breakpoints"]["hits"][0]["locals"]
        .as_array()
        .expect("locals array");
    assert_eq!(
        json["data"]["breakpoints"]["hits"][0]["spanKind"],
        "expr_identifier"
    );
    assert!(locals.iter().any(|local| {
        local["name"] == "userId"
            && local["origin"] == "param"
            && local["typeId"].as_str().unwrap().ends_with(".UserId")
            && local["value"]["typeId"]
                .as_str()
                .unwrap()
                .ends_with(".UserId")
    }));
    assert!(locals.iter().any(|local| {
        local["name"] == "current"
            && local["origin"] == "let"
            && local["typeId"].as_str().unwrap().ends_with(".UserId")
            && local["value"]["typeId"]
                .as_str()
                .unwrap()
                .ends_with(".UserId")
    }));
}

#[test]
fn run_json_breakpoint_collect_mode_truncates_hit_window() {
    let dir = temp_dir("break-collect");
    let file = write_program(
        &dir,
        "main.sigil",
        "λloop(n:Int)=>Int\ndecreases n\nmatch n=0{\n  true=>0|\n  false=>loop(n-1)\n}\n\nλmain()=>Int=loop(5)\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--break-fn")
        .arg("loop")
        .arg("--break-mode")
        .arg("collect")
        .arg("--break-max-hits")
        .arg("2")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["breakpoints"]["stopped"], false);
    assert_eq!(json["data"]["breakpoints"]["returnedHits"], 2);
    assert!(json["data"]["breakpoints"]["totalHits"].as_u64().unwrap() > 2);
    assert!(json["data"]["breakpoints"]["droppedHits"].as_u64().unwrap() > 0);
}

#[test]
fn run_json_breakpoint_failure_preserves_hits_in_error_details() {
    let dir = temp_dir("break-failure-details");
    let file = write_program(
        &dir,
        "main.sigil",
        "e boom:{explode:λ()=>Int}\n\nλhelper()=>Int=1\n\nλmain()=>Int=helper()+boom.explode()\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--break-fn")
        .arg("helper")
        .arg("--break-mode")
        .arg("collect")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-UNCAUGHT-EXCEPTION");
    assert!(
        json["error"]["details"]["breakpoints"]["totalHits"]
            .as_u64()
            .unwrap()
            >= 1
    );
}

#[test]
fn run_json_trace_success_includes_call_branch_and_effect_events() {
    let dir = temp_dir("trace-success");
    let file = write_program(
        &dir,
        "main.sigil",
        r#"λhelper(flag:Bool)=>!Random Int match flag{
  true=>§random.intBetween(
    1,
    1
  )|
  false=>0
}

λmain()=>!Random Int=helper(true)
"#,
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--trace")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc run");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["trace"]["enabled"], true);
    let events = json["data"]["trace"]["events"]
        .as_array()
        .expect("trace events array");
    assert!(events.iter().any(|event| event["kind"] == "call"));
    assert!(events.iter().any(|event| event["kind"] == "branch_match"));
    assert!(events.iter().any(|event| event["kind"] == "effect_call"));
    assert!(events.iter().any(|event| event["kind"] == "effect_result"));
    assert!(events.iter().any(|event| {
        event["kind"] == "effect_call"
            && event["effectFamily"] == "random"
            && event["operation"] == "intBetween"
    }));
}

#[test]
fn run_emits_json_error_on_compile_failure() {
    let dir = temp_dir("compile-failure");
    let file = write_program(&dir, "broken.sigil", "λmain()=>Unit={\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("\nError: "));
    let json = parse_json(output.stderr.trim_ascii());
    assert_eq!(json["command"], "sigilc run");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "parser");
}

#[test]
fn run_keeps_streamed_output_and_appends_json_on_child_failure() {
    let dir = temp_dir("runtime-failure");
    let file = write_program(
        &dir,
        "main.sigil",
        "e console:{log:λ(String)=>!Log Unit}\n\
\n\
e process:{exit:λ(Int)=>Unit}\n\
\n\
λmain()=>!Log Unit={\n  l _=(console.log(\"before exit\"):Unit);\n  process.exit(1)\n}\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "before exit\n");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("\nError: Process exited with code"));
    let json = parse_json(output.stderr.trim_ascii());
    assert_eq!(json["command"], "sigilc run");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-CHILD-EXIT");
    assert_eq!(
        json["error"]["details"]["runtime"]["stdout"],
        "before exit\n"
    );
    assert_eq!(
        json["error"]["details"]["compile"]["input"],
        file.to_string_lossy().to_string()
    );
    assert!(json["error"]["details"]["exception"].is_null());
}

#[test]
fn run_json_reports_runtime_failures_without_extra_text() {
    let dir = temp_dir("json-runtime-failure");
    let file = write_program(
        &dir,
        "main.sigil",
        "e console:{log:λ(String)=>!Log Unit}\n\
\n\
e process:{exit:λ(Int)=>Unit}\n\
\n\
λmain()=>!Log Unit={\n  l _=(console.log(\"json before exit\"):Unit);\n  process.exit(1)\n}\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc run");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-CHILD-EXIT");
    assert_eq!(
        json["error"]["details"]["runtime"]["stdout"],
        "json before exit\n"
    );
    assert_eq!(
        json["error"]["details"]["compile"]["input"],
        file.to_string_lossy().to_string()
    );
    assert!(json["error"]["details"]["exception"].is_null());
}

#[test]
fn run_json_trace_preserves_child_exit_failures_with_trace_details() {
    let dir = temp_dir("json-trace-child-exit");
    let file = write_program(
        &dir,
        "main.sigil",
        "λmain()=>!Process Unit=§process.exit(1)\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--trace")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-CHILD-EXIT");
    assert_eq!(json["error"]["details"]["trace"]["enabled"], true);
    assert!(json["error"]["details"]["trace"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["kind"] == "effect_call"));
    assert!(json["error"]["details"]["exception"].is_null());
}

#[test]
fn run_json_enriches_uncaught_runtime_exceptions() {
    let dir = temp_dir("json-runtime-exception");
    let file = write_program(
        &dir,
        "main.sigil",
        "e boom:{explode:λ()=>Unit}\n\nλmain()=>Unit=boom.explode()\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc run");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "runtime");
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-UNCAUGHT-EXCEPTION");
    assert_eq!(
        json["error"]["location"]["file"],
        file.to_string_lossy().to_string()
    );
    assert_eq!(
        json["error"]["details"]["compile"]["input"],
        file.to_string_lossy().to_string()
    );
    assert!(PathBuf::from(
        json["error"]["details"]["compile"]["spanMapFile"]
            .as_str()
            .expect("spanMapFile path")
    )
    .exists());
    assert!(json["error"]["details"]["runtime"]["stderr"]
        .as_str()
        .unwrap()
        .contains("ReferenceError"));
    assert_eq!(
        json["error"]["details"]["exception"]["name"],
        "ReferenceError"
    );
    assert_eq!(
        json["error"]["details"]["exception"]["sigilFrame"]["label"],
        "main"
    );
    assert_eq!(
        json["error"]["details"]["exception"]["sigilExpression"]["file"],
        file.to_string_lossy().to_string()
    );
    assert_ne!(
        json["error"]["details"]["exception"]["sigilExpression"]["kind"],
        "function_decl"
    );
    assert!(
        json["error"]["details"]["exception"]["sigilExpression"]["location"]["start"]["column"]
            .as_u64()
            .unwrap()
            > json["error"]["details"]["exception"]["sigilFrame"]["location"]["start"]["column"]
                .as_u64()
                .unwrap()
    );
    assert_eq!(
        json["error"]["details"]["exception"]["sigilFrame"]["kind"],
        "function_decl"
    );
    assert_eq!(
        json["error"]["details"]["exception"]["sigilFrame"]["file"],
        file.to_string_lossy().to_string()
    );
    assert!(
        json["error"]["details"]["exception"]["generatedFrame"]["file"]
            .as_str()
            .unwrap()
            .ends_with(".mjs")
    );
    assert!(
        json["error"]["details"]["exception"]["sigilFrame"]["excerpt"]["text"]
            .as_str()
            .unwrap()
            .contains("λmain()=>Unit=boom.explode()")
    );
}

#[test]
fn run_json_trace_expr_success_includes_expression_events() {
    let dir = temp_dir("json-trace-expr-success");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1+2\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--trace")
        .arg("--trace-expr")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    let events = json["data"]["trace"]["events"]
        .as_array()
        .expect("trace events");
    assert!(events.iter().any(|event| event["kind"] == "expr_enter"));
    assert!(events.iter().any(|event| event["kind"] == "expr_return"));
}

#[test]
fn run_json_trace_failure_includes_trace_details() {
    let dir = temp_dir("json-trace-runtime-exception");
    let file = write_program(
        &dir,
        "main.sigil",
        "e boom:{explode:λ()=>Int}\n\nλmain()=>Int=boom.explode()\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--trace")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-UNCAUGHT-EXCEPTION");
    assert_eq!(json["error"]["details"]["trace"]["enabled"], true);
    assert!(json["error"]["details"]["trace"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["kind"] == "call"));
}

#[test]
fn run_json_enriches_import_time_runtime_exceptions() {
    let dir = temp_dir("json-import-runtime-exception");
    let file = write_program(
        &dir,
        "main.sigil",
        "t BirthYear=Int where value>1800 and value<10000\n\ne process:{chdir:λ(String)=>BirthYear}\n\nc bad=(process.chdir(\"\"):BirthYear)\n\nλmain()=>BirthYear=bad\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-UNCAUGHT-EXCEPTION");
    assert_eq!(
        json["error"]["location"]["file"],
        file.to_string_lossy().to_string()
    );
    assert_eq!(
        json["error"]["details"]["exception"]["sigilFrame"]["label"],
        "bad"
    );
    assert_eq!(
        json["error"]["details"]["exception"]["sigilExpression"]["file"],
        file.to_string_lossy().to_string()
    );
    assert_ne!(
        json["error"]["details"]["exception"]["sigilExpression"]["kind"],
        "const_decl"
    );
    assert!(
        json["error"]["details"]["exception"]["sigilExpression"]["error"]["typeId"]
            .as_str()
            .unwrap()
            .ends_with(".BirthYear")
    );
    assert_eq!(
        json["error"]["details"]["exception"]["sigilFrame"]["kind"],
        "const_decl"
    );
    assert!(
        json["error"]["details"]["exception"]["sigilFrame"]["excerpt"]["text"]
            .as_str()
            .unwrap()
            .contains("c bad=(process.chdir(\"\"):BirthYear)")
    );
}

#[test]
fn run_json_runtime_expression_includes_live_locals_when_breakpoints_are_enabled() {
    let dir = temp_dir("json-expression-locals");
    let file = write_program(
        &dir,
        "main.sigil",
        "t UserId=Int where value≥0\n\ne boom:{explode:λ()=>Int}\n\nλhelper(userId:UserId)=>Int={\n  l current=(userId:UserId);\n  boom.explode()+(match current=(0:UserId){\n    true=>1|\n    false=>current-current\n  })\n}\n\nλmain()=>Int=helper((1:UserId))\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--break-fn")
        .arg("helper")
        .arg("--break-mode")
        .arg("collect")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    let locals = json["error"]["details"]["exception"]["sigilExpression"]["locals"]
        .as_array()
        .expect("expression locals");
    assert!(locals.iter().any(|local| {
        local["name"] == "userId"
            && local["origin"] == "param"
            && local["typeId"].as_str().unwrap().ends_with(".UserId")
            && local["value"]["typeId"]
                .as_str()
                .unwrap()
                .ends_with(".UserId")
    }));
    assert!(locals.iter().any(|local| {
        local["name"] == "current"
            && local["origin"] == "let"
            && local["typeId"].as_str().unwrap().ends_with(".UserId")
            && local["value"]["typeId"]
                .as_str()
                .unwrap()
                .ends_with(".UserId")
    }));
}

#[test]
fn run_json_trace_truncates_large_event_streams() {
    let dir = temp_dir("trace-truncation");
    let file = write_program(
        &dir,
        "main.sigil",
        "λloop(n:Int)=>Int\ndecreases n\nmatch n=0{\n  true=>0|\n  false=>loop(n-1)\n}\n\nλmain()=>Int=loop(400)\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--trace")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["trace"]["enabled"], true);
    assert_eq!(json["data"]["trace"]["truncated"], true);
    assert_eq!(json["data"]["trace"]["returnedEvents"], 256);
    assert!(
        json["data"]["trace"]["totalEvents"].as_u64().unwrap()
            > json["data"]["trace"]["returnedEvents"].as_u64().unwrap()
    );
    assert!(json["data"]["trace"]["droppedEvents"].as_u64().unwrap() > 0);
}

#[test]
fn run_json_preserves_topology_codes_for_bootstrap_failures() {
    let dir = temp_dir("json-topology-runtime-failure");
    let src_dir = dir.join("src");
    let config_dir = dir.join("config");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        dir.join("sigil.json"),
        "{\n  \"name\": \"topologyRuntimeFailure\",\n  \"version\": \"2026-04-05T14-58-24Z\"\n}\n",
    )
    .unwrap();
    let file = write_program(&src_dir, "main.sigil", "λmain()=>Int=1\n");
    write_program(
        &src_dir,
        "topology.lib.sigil",
        "c local=(§topology.environment(\"local\"):§topology.Environment)\n",
    );
    fs::write(
        config_dir.join("staging.lib.sigil"),
        concat!(
            "e process\n\n",
            "c world=(†runtime.world(\n",
            "  †clock.systemClock(),\n",
            "  †fs.real(),\n",
            "  †fsWatch.real(),\n",
            "  [],\n",
            "  †log.capture(),\n",
            "  †process.real(),\n",
            "  †pty.real(),\n",
            "  †random.seeded(1337),\n",
            "  †sql.deny(),\n",
            "  †stream.live(),\n",
            "  †task.real(),\n",
            "  [],\n",
            "  †timer.virtual(),\n",
            "  †websocket.real()\n",
            "):†runtime.World)\n",
        ),
    )
    .unwrap();

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--env")
        .arg("staging")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc run");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "topology");
    assert_eq!(json["error"]["code"], "SIGIL-TOPO-ENV-NOT-FOUND");
    assert_eq!(
        json["error"]["details"]["compile"]["input"],
        file.to_string_lossy().to_string()
    );
    assert!(json["error"]["details"]["runtime"]["stderr"]
        .as_str()
        .unwrap()
        .contains("SIGIL-TOPO-ENV-NOT-FOUND"));
    assert_eq!(json["error"]["details"]["exception"]["name"], "Error");
    assert!(
        json["error"]["details"]["exception"]["generatedFrame"]["file"]
            .as_str()
            .unwrap()
            .ends_with(".run.mjs")
    );
    assert!(json["error"]["location"].is_null());
    assert!(json["error"]["details"]["exception"]["sigilFrame"].is_null());
}

#[test]
fn run_json_record_writes_replay_artifact_on_success() {
    let dir = temp_dir("record-success");
    let file = write_program(
        &dir,
        "main.sigil",
        r#"λmain()=>!Clock!Random!Timer String={
  l now=(§time.toEpochMillis((§time.now():§time.Instant)):Int);
  l draw=(§random.intBetween(
    1,
    1
  ):Int);
  l _=(§time.sleepMs(1):Unit);
  "t="
    ++§string.intToString(now)
    ++",n="
    ++§string.intToString(draw)
}
"#,
    );
    let artifact = dir.join("success.replay.json");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--record")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["replay"]["mode"], "record");
    assert_eq!(json["data"]["replay"]["partial"], false);
    assert!(artifact.exists());

    let artifact_json = parse_replay_artifact(&artifact);
    assert_eq!(artifact_json["formatVersion"], 2);
    assert_eq!(artifact_json["kind"], "sigilRunReplay");
    assert_eq!(
        artifact_json["entry"]["sourceFile"],
        file.to_string_lossy().to_string()
    );
    assert_eq!(artifact_json["summary"]["failed"], false);
    assert!(artifact_json["events"].as_array().unwrap().len() >= 3);
    assert!(
        artifact_json["summary"]["effectCounts"]["random"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(
        artifact_json["summary"]["effectCounts"]["timer"]
            .as_u64()
            .unwrap()
            >= 2
    );
    let event = artifact_json["events"].as_array().unwrap().first().unwrap();
    assert!(event.get("request").is_some());
    assert!(event.get("outcome").is_some());
    assert!(event.get("payload").is_none());
}

#[test]
fn run_json_record_writes_partial_artifact_on_runtime_failure() {
    let dir = temp_dir("record-failure");
    let file = write_program(
        &dir,
        "main.sigil",
        r#"e boom:{explode:λ()=>Int}

λmain()=>!Random Int={
  l _=(§random.intBetween(
    1,
    1
  ):Int);
  boom.explode()
}
"#,
    );
    let artifact = dir.join("failure.replay.json");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--record")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-RUNTIME-UNCAUGHT-EXCEPTION");
    assert_eq!(json["error"]["details"]["replay"]["mode"], "record");
    assert_eq!(json["error"]["details"]["replay"]["partial"], true);
    assert!(artifact.exists());

    let artifact_json = parse_replay_artifact(&artifact);
    assert_eq!(artifact_json["formatVersion"], 2);
    assert_eq!(artifact_json["summary"]["failed"], true);
    assert_eq!(
        artifact_json["failure"]["code"],
        "SIGIL-RUNTIME-UNCAUGHT-EXCEPTION"
    );
}

#[test]
fn run_json_replay_reproduces_recorded_success() {
    let dir = temp_dir("replay-success");
    let file = write_program(
        &dir,
        "main.sigil",
        r#"λmain()=>!Clock!Random!Timer String={
  l now=(§time.toEpochMillis((§time.now():§time.Instant)):Int);
  l draw=(§random.intBetween(
    1,
    1
  ):Int);
  l _=(§time.sleepMs(1):Unit);
  "t="
    ++§string.intToString(now)
    ++",n="
    ++§string.intToString(draw)
}
"#,
    );
    let artifact = dir.join("success.replay.json");

    let recorded = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--record")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(recorded.status.success());
    let recorded_json = parse_json(&recorded.stdout);

    let replayed = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--trace")
        .arg("--replay")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(replayed.status.success());
    assert!(replayed.stderr.is_empty());

    let replayed_json = parse_json(&replayed.stdout);
    assert_eq!(
        replayed_json["data"]["runtime"]["stdout"],
        recorded_json["data"]["runtime"]["stdout"]
    );
    assert_eq!(replayed_json["data"]["replay"]["mode"], "replay");
    assert!(
        replayed_json["data"]["replay"]["consumedEvents"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(replayed_json["data"]["replay"]["remainingEvents"], 0);
    assert!(replayed_json["data"]["trace"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["kind"] == "effect_result"));
}

#[test]
fn run_json_replay_breakpoints_preserve_hit_resolution() {
    let dir = temp_dir("replay-breakpoints");
    let file = write_program(
        &dir,
        "main.sigil",
        r#"λhelper(flag:Bool)=>!Random Int match flag{
  true=>§random.intBetween(
    1,
    1
  )|
  false=>0
}

λmain()=>!Random Int=helper(true)
"#,
    );
    let artifact = dir.join("breakpoints.replay.json");

    let recorded = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--record")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(recorded.status.success());

    let replayed = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--replay")
        .arg(&artifact)
        .arg("--break-fn")
        .arg("helper")
        .arg("--break-mode")
        .arg("collect")
        .arg(&file)
        .output()
        .unwrap();

    assert!(replayed.status.success());
    assert!(replayed.stderr.is_empty());

    let json = parse_json(&replayed.stdout);
    assert_eq!(json["data"]["replay"]["mode"], "replay");
    assert_eq!(json["data"]["breakpoints"]["totalHits"], 1);
    assert_eq!(
        json["data"]["breakpoints"]["hits"][0]["declarationLabel"],
        "helper"
    );
    assert!(json["data"]["breakpoints"]["hits"][0]["spanId"]
        .as_str()
        .unwrap()
        .starts_with('s'));
}

#[test]
fn run_replay_rejects_env() {
    let dir = temp_dir("replay-env-conflict");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1\n");
    let artifact = dir.join("dummy.replay.json");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--replay")
        .arg(&artifact)
        .arg("--env")
        .arg("local")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let json = parse_json(output.stderr.trim_ascii());
    assert_eq!(json["error"]["code"], "SIGIL-CLI-USAGE");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--replay"));
}

#[test]
fn run_json_replay_rejects_binding_mismatch_on_argv() {
    let dir = temp_dir("replay-binding-argv");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1\n");
    let artifact = dir.join("argv.replay.json");

    let recorded = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--record")
        .arg(&artifact)
        .arg(&file)
        .arg("--")
        .arg("alpha")
        .output()
        .unwrap();

    assert!(recorded.status.success());

    let replayed = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--replay")
        .arg(&artifact)
        .arg(&file)
        .arg("--")
        .arg("beta")
        .output()
        .unwrap();

    assert!(!replayed.status.success());
    assert!(replayed.stderr.is_empty());

    let json = parse_json(&replayed.stdout);
    assert_eq!(
        json["error"]["code"],
        "SIGIL-RUNTIME-REPLAY-BINDING-MISMATCH"
    );
}

#[test]
fn run_json_replay_reproduces_recorded_filesystem_effects_without_touching_disk() {
    let dir = temp_dir("replay-fs-success");
    let file = write_program(
        &dir,
        "main.sigil",
        r#"λboolText(value:Bool)=>String match value{
  true=>"t"|
  false=>"f"
}

λmain()=>!Fs String={
  l root=(§file.makeTempDir("sigil-replay-fs-"):String);
  l file=(§path.join(
    root,
    "sample.txt"
  ):String);
  l _=(§file.writeText(
    "hello",
    file
  ):Unit);
  l _=(§file.appendText(
    " world",
    file
  ):Unit);
  l text=(§file.readText(file):String);
  l entries=(§file.listDir(root):[String]);
  l present=(§file.exists(file):Bool);
  l _=(§file.remove(file):Unit);
  l _=(§file.removeTree(root):Unit);
  text
    ++"|"
    ++boolText(present)
    ++"|"
    ++§string.join(
      ",",
      entries
    )
}
"#,
    );
    let artifact = dir.join("success-fs.replay.json");

    let recorded = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--record")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(recorded.status.success());
    assert!(recorded.stderr.is_empty());
    let recorded_json = parse_json(&recorded.stdout);
    let artifact_json = parse_replay_artifact(&artifact);
    assert_eq!(artifact_json["formatVersion"], 2);
    let write_event = replay_event(&artifact_json, "file", "writeText");
    assert_eq!(write_event["request"]["content"]["kind"], "textSummary");
    assert_eq!(write_event["request"]["content"]["length"], 5);
    assert!(write_event["request"]["content"]["sha256"].is_string());
    assert!(write_event["request"]["content"].get("text").is_none());
    let read_event = replay_event(&artifact_json, "file", "readText");
    assert_eq!(read_event["outcome"]["kind"], "return");
    assert_eq!(read_event["outcome"]["value"], "hello world");
    let temp_dir_event = replay_event(&artifact_json, "file", "makeTempDir");
    let recorded_root = PathBuf::from(temp_dir_event["outcome"]["value"].as_str().unwrap());
    assert!(!recorded_root.exists());

    let replayed = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--replay")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(replayed.status.success());
    assert!(replayed.stderr.is_empty());

    let replayed_json = parse_json(&replayed.stdout);
    assert_eq!(
        replayed_json["data"]["runtime"]["stdout"],
        recorded_json["data"]["runtime"]["stdout"]
    );
    assert_eq!(replayed_json["data"]["replay"]["mode"], "replay");
    assert_eq!(replayed_json["data"]["replay"]["remainingEvents"], 0);
    assert!(!recorded_root.exists());
}

#[test]
fn run_json_replay_reproduces_recorded_filesystem_failure() {
    let dir = temp_dir("replay-fs-failure");
    let missing_path = dir.join("missing.txt");
    let file = write_program(
        &dir,
        "main.sigil",
        &format!(
            "λmain()=>!Fs String=§file.readText(\"{}\")\n",
            missing_path.to_string_lossy()
        ),
    );
    let artifact = dir.join("failure-fs.replay.json");

    let recorded = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--record")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(!recorded.status.success());
    assert!(recorded.stderr.is_empty());
    let recorded_json = parse_json(&recorded.stdout);
    assert_eq!(
        recorded_json["error"]["code"],
        "SIGIL-RUNTIME-UNCAUGHT-EXCEPTION"
    );

    let artifact_json = parse_replay_artifact(&artifact);
    let read_event = replay_event(&artifact_json, "file", "readText");
    assert_eq!(read_event["outcome"]["kind"], "throw");
    assert_eq!(read_event["outcome"]["error"]["code"], "ENOENT");
    assert!(read_event["outcome"]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("ENOENT"));

    let replayed = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--replay")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(!replayed.status.success());
    assert!(replayed.stderr.is_empty());

    let replayed_json = parse_json(&replayed.stdout);
    assert_eq!(
        replayed_json["error"]["code"],
        "SIGIL-RUNTIME-UNCAUGHT-EXCEPTION"
    );
    assert_eq!(
        replayed_json["error"]["details"]["replay"]["mode"],
        "replay"
    );
    assert!(replayed_json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("ENOENT"));
    assert!(replayed_json["error"]["details"]["exception"]["sigilExpression"].is_object());
}

#[test]
fn run_json_replay_reproduces_recorded_child_exit_failure() {
    let dir = temp_dir("replay-child-exit");
    let file = write_program(
        &dir,
        "main.sigil",
        "λmain()=>!Process Unit=§process.exit(3)\n",
    );
    let artifact = dir.join("child-exit.replay.json");

    let recorded = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--record")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(!recorded.status.success());
    let recorded_json = parse_json(&recorded.stdout);
    assert_eq!(recorded_json["error"]["code"], "SIGIL-RUNTIME-CHILD-EXIT");

    let replayed = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("run")
        .arg("--json")
        .arg("--replay")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(!replayed.status.success());
    assert!(replayed.stderr.is_empty());

    let replayed_json = parse_json(&replayed.stdout);
    assert_eq!(replayed_json["error"]["code"], "SIGIL-RUNTIME-CHILD-EXIT");
    assert_eq!(
        replayed_json["error"]["details"]["replay"]["mode"],
        "replay"
    );
}
