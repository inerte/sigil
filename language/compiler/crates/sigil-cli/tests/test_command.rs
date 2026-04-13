use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
    let dir = repo_root().join("target").join(format!(
        "sigil-cli-test-{label}-{}-{unique}",
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

fn parse_json(text: &[u8]) -> Value {
    serde_json::from_slice(text).unwrap()
}

fn parse_replay_artifact(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn test_trace_expr_requires_trace() {
    let dir = temp_dir("trace-expr-requires-trace");
    let file = write_program(
        &dir,
        "tests/basic.sigil",
        "λmain()=>Unit=()\n\ntest \"basic\" {\n  true\n}\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("test")
        .arg("--trace-expr")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc test");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "cli");
    assert_eq!(json["error"]["code"], "SIGIL-CLI-USAGE");
}

#[test]
fn test_directory_runs_inline_tests_in_standalone_files() {
    let dir = temp_dir("inline-standalone-dir");
    write_program(&dir, "alpha.sigil", "λmain()=>Int=1\n");
    write_program(
        &dir,
        "beta.sigil",
        concat!(
            "c auditLog=(§topology.logSink(\"auditLog\"):§topology.LogSink)\n\n",
            "c world=(†runtime.withLogSinks(\n",
            "  [†log.captureSink(auditLog)],\n",
            "  †runtime.world(\n",
            "    †clock.systemClock(),\n",
            "    †fs.real(),\n",
            "    [],\n",
            "    †log.capture(),\n",
            "    †process.real(),\n",
            "    †random.seeded(1337),\n",
            "    [],\n",
            "    †timer.virtual()\n",
            "  )\n",
            "):†runtime.World)\n\n",
            "λmain()=>Unit=()\n\n",
            "test \"inline log sink test\" =>!Log {\n",
            "  l _=(§log.write(\n",
            "    \"captured\",\n",
            "    auditLog\n",
            "  ):Unit);\n",
            "  ※check::log.containsAt(\n",
            "    \"captured\",\n",
            "    auditLog\n",
            "  )\n",
            "}\n",
        ),
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("test")
        .arg(&dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc test");
    assert_eq!(json["ok"], true);
    assert_eq!(json["summary"]["files"], 2);
    assert_eq!(json["summary"]["discovered"], 1);
    assert_eq!(json["summary"]["passed"], 1);
}

#[test]
fn test_replay_rejects_env() {
    let dir = temp_dir("replay-env");
    let file = write_program(
        &dir,
        "tests/basic.sigil",
        "λmain()=>Unit=()\n\ntest \"basic\" {\n  true\n}\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("test")
        .arg("--env")
        .arg("local")
        .arg("--replay")
        .arg(dir.join("missing.replay.json"))
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-CLI-USAGE");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--replay"));
}

#[test]
fn test_breakpoint_stop_marks_only_current_test_as_stopped() {
    let dir = temp_dir("break-stop");
    let file = write_program(
        &dir,
        "tests/basic.sigil",
        "λhit(x:Int)=>Int=x+1\n\nλmain()=>Unit=()\n\ntest \"stops here\" {\n  hit(1)=2\n}\n\ntest \"still runs\" {\n  true\n}\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("test")
        .arg("--break-fn")
        .arg("hit")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc test");
    assert_eq!(json["ok"], false);
    assert_eq!(json["summary"]["passed"], 1);
    assert_eq!(json["summary"]["stopped"], 1);

    let results = json["results"].as_array().unwrap();
    let stopped = results
        .iter()
        .find(|result| result["name"] == "stops here")
        .unwrap();
    let passed = results
        .iter()
        .find(|result| result["name"] == "still runs")
        .unwrap();
    assert_eq!(stopped["status"], "stopped");
    assert_eq!(stopped["breakpoints"]["stopped"], true);
    assert_eq!(passed["status"], "pass");
}

#[test]
fn test_trace_is_inline_per_result() {
    let dir = temp_dir("trace-inline");
    let file = write_program(
        &dir,
        "tests/random.sigil",
        r#"λmain()=>Unit=()

test "traces random" =>!Random world {
  c random=(†random.seeded(1337):†random.RandomEntry)
} {
  §random.intBetween(
    1,
    1
  )=1
}
"#,
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("test")
        .arg("--trace")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    let result = &json["results"][0];
    assert_eq!(result["status"], "pass");
    let events = result["trace"]["events"].as_array().unwrap();
    assert!(events
        .iter()
        .any(|event| event["kind"] == "effect_call" && event["effectFamily"] == "random"));
}

#[test]
fn test_error_result_includes_exact_exception_details() {
    let dir = temp_dir("exception");
    let file = write_program(
        &dir,
        "tests/error.sigil",
        "e boom:{explode:λ()=>Int}\n\nλmain()=>Unit=()\n\ntest \"boom\" {\n  boom.explode()=1\n}\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("test")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    let result = &json["results"][0];
    assert_eq!(result["status"], "error");
    assert_eq!(result["exception"]["sigilFrame"]["label"], "boom");
    assert_eq!(
        result["exception"]["sigilExpression"]["kind"],
        "expr_identifier"
    );
}

#[test]
fn test_record_and_replay_preserve_test_local_world_overlays() {
    let dir = temp_dir("replay-world");
    let file = write_program(
        &dir,
        "tests/random.sigil",
        r#"λmain()=>Unit=()

test "seeded replay" =>!Random world {
  c random=(†random.seeded(1337):†random.RandomEntry)
} {
  §random.shuffle([
    1,
    2,
    3,
    4
  ])=[
    3,
    1,
    2,
    4
  ]
}
"#,
    );
    let artifact = dir.join("tests.replay.json");

    let recorded = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("test")
        .arg("--record")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(recorded.status.success());
    assert!(recorded.stderr.is_empty());

    let recorded_json = parse_json(&recorded.stdout);
    assert_eq!(recorded_json["results"][0]["replay"]["mode"], "record");

    let artifact_json = parse_replay_artifact(&artifact);
    assert_eq!(artifact_json["kind"], "sigilTestReplay");
    assert_eq!(
        artifact_json["selectedTestIds"][0],
        format!("{}::seeded replay", file.to_string_lossy())
    );
    let recorded_test = &artifact_json["tests"][0];
    assert_eq!(
        recorded_test["replayArtifact"]["world"]["normalizedWorld"]["random"]["kind"],
        "seeded"
    );

    let replayed = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("test")
        .arg("--replay")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(replayed.status.success());
    assert!(replayed.stderr.is_empty());

    let replayed_json = parse_json(&replayed.stdout);
    assert_eq!(replayed_json["results"][0]["status"], "pass");
    assert_eq!(replayed_json["results"][0]["replay"]["mode"], "replay");
    assert_eq!(replayed_json["results"][0]["replay"]["remainingEvents"], 0);
}
