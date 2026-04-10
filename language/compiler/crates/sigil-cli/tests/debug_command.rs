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
        "sigil-cli-debug-{label}-{}-{unique}",
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

fn watch_entry<'a>(json: &'a Value, selector: &str) -> &'a Value {
    json["data"]["snapshot"]["watches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["selector"] == selector)
        .unwrap()
}

fn line_break_selector(file: &Path, line: usize) -> String {
    format!("{}:{}", file.to_string_lossy(), line)
}

#[test]
fn debug_run_start_pauses_at_main_entry() {
    let dir = temp_dir("run-start");
    let file = write_program(
        &dir,
        "main.sigil",
        "λhelper(x:Int)=>Int=x+1\n\nλmain()=>Int=helper(1)\n",
    );
    let artifact = dir.join("run.replay.json");

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

    let started = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("start")
        .arg("--replay")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();

    assert!(started.status.success());
    assert!(started.stderr.is_empty());
    let json = parse_json(&started.stdout);
    assert_eq!(json["command"], "sigilc debug run");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["snapshot"]["eventKind"], "function_enter");
    assert_eq!(json["data"]["snapshot"]["declarationLabel"], "main");
    assert_eq!(json["data"]["snapshot"]["pauseReason"], "start");
    let session_file = PathBuf::from(json["data"]["session"]["file"].as_str().unwrap());
    assert!(session_file.exists());
}

#[test]
fn debug_run_step_over_and_continue_complete_the_session() {
    let dir = temp_dir("run-step");
    let file = write_program(
        &dir,
        "main.sigil",
        "λhelper(x:Int)=>Int=x+1\n\nλmain()=>Int=helper(1)\n",
    );
    let artifact = dir.join("run.replay.json");

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

    let started = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("start")
        .arg("--replay")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();
    let started_json = parse_json(&started.stdout);
    let session = started_json["data"]["session"]["file"].as_str().unwrap();

    let step_into = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("step-into")
        .arg(session)
        .output()
        .unwrap();
    assert!(step_into.status.success());
    let step_into_json = parse_json(&step_into.stdout);
    assert_eq!(
        step_into_json["data"]["snapshot"]["eventKind"],
        "expr_enter"
    );

    let step_over = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("step-over")
        .arg(session)
        .output()
        .unwrap();
    assert!(step_over.status.success());
    let step_over_json = parse_json(&step_over.stdout);
    assert_eq!(
        step_over_json["data"]["snapshot"]["eventKind"],
        "expr_return"
    );
    assert_eq!(
        step_over_json["data"]["snapshot"]["lastCompleted"]["kind"],
        "expr_return"
    );

    let continued = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("continue")
        .arg(session)
        .output()
        .unwrap();
    assert!(continued.status.success());
    let continued_json = parse_json(&continued.stdout);
    assert_eq!(continued_json["data"]["session"]["state"], "completed");
    assert_eq!(
        continued_json["data"]["snapshot"]["eventKind"],
        "program_exit"
    );
    assert_eq!(continued_json["data"]["snapshot"]["stdoutSoFar"], "2\n");
}

#[test]
fn debug_run_rejects_advancing_completed_session() {
    let dir = temp_dir("run-completed");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1\n");
    let artifact = dir.join("run.replay.json");

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

    let started = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("start")
        .arg("--replay")
        .arg(&artifact)
        .arg(&file)
        .output()
        .unwrap();
    let session = parse_json(&started.stdout)["data"]["session"]["file"]
        .as_str()
        .unwrap()
        .to_string();

    let continued = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("continue")
        .arg(&session)
        .output()
        .unwrap();
    assert!(continued.status.success());

    let invalid = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("step-into")
        .arg(&session)
        .output()
        .unwrap();

    assert!(!invalid.status.success());
    assert!(invalid.stderr.is_empty());
    let json = parse_json(&invalid.stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "SIGIL-CLI-UNEXPECTED");
}

#[test]
fn debug_test_start_and_step_over_use_exact_test_ids() {
    let dir = temp_dir("test-step");
    let file = write_program(
        &dir,
        "tests/basic.sigil",
        "λhelper(x:Int)=>Int=x+1\n\nλmain()=>Unit=()\n\ntest \"demo\" {\n  helper(1)=2\n}\n",
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
    let recorded_json = parse_json(&recorded.stdout);
    let test_id = recorded_json["results"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let started = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("test")
        .arg("start")
        .arg("--replay")
        .arg(&artifact)
        .arg("--test")
        .arg(&test_id)
        .arg(&file)
        .output()
        .unwrap();

    assert!(started.status.success());
    let started_json = parse_json(&started.stdout);
    assert_eq!(started_json["command"], "sigilc debug test");
    assert_eq!(started_json["data"]["snapshot"]["eventKind"], "test_enter");
    assert_eq!(started_json["data"]["snapshot"]["testId"], test_id);
    let session = started_json["data"]["session"]["file"].as_str().unwrap();

    let step_over = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("test")
        .arg("step-over")
        .arg(session)
        .output()
        .unwrap();

    assert!(step_over.status.success());
    let step_over_json = parse_json(&step_over.stdout);
    assert_eq!(
        step_over_json["data"]["snapshot"]["eventKind"],
        "test_return"
    );
    assert_eq!(step_over_json["data"]["snapshot"]["testStatus"], "pass");
    assert_eq!(
        step_over_json["data"]["snapshot"]["lastCompleted"]["kind"],
        "test_return"
    );
}

#[test]
fn debug_test_continue_completes_one_test_session() {
    let dir = temp_dir("test-continue");
    let file = write_program(
        &dir,
        "tests/basic.sigil",
        "λmain()=>Unit=()\n\ntest \"pass\" {\n  true\n}\n\ntest \"also pass\" {\n  true\n}\n",
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
    let recorded_json = parse_json(&recorded.stdout);
    let test_id = recorded_json["results"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let started = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("test")
        .arg("start")
        .arg("--replay")
        .arg(&artifact)
        .arg("--test")
        .arg(&test_id)
        .arg(&file)
        .output()
        .unwrap();
    let session = parse_json(&started.stdout)["data"]["session"]["file"]
        .as_str()
        .unwrap()
        .to_string();

    let continued = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("test")
        .arg("continue")
        .arg(&session)
        .output()
        .unwrap();

    assert!(continued.status.success());
    let json = parse_json(&continued.stdout);
    assert_eq!(json["data"]["session"]["state"], "completed");
    assert_eq!(json["data"]["snapshot"]["eventKind"], "test_exit");
    assert_eq!(json["data"]["snapshot"]["testId"], test_id);
    assert_eq!(json["data"]["snapshot"]["testStatus"], "pass");
}

#[test]
fn debug_run_watches_follow_scope_and_record_fields() {
    let dir = temp_dir("run-watch");
    let file = write_program(
        &dir,
        "main.sigil",
        r#"t User={
  name:String,
  score:Int
}

t UserId=Int where value≥0

λhelper(user:User,userId:UserId)=>Int={
  l current=(userId:UserId);
  user.score+(match current=(0:UserId){
    true=>0|
    false=>current-current
  })
}

λmain()=>Int=helper(
  {
    name:"Ada",
    score:1
  },
  (1:UserId)
)
"#,
    );
    let artifact = dir.join("run.replay.json");

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

    let started = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("start")
        .arg("--replay")
        .arg(&artifact)
        .arg("--watch")
        .arg("userId")
        .arg("--watch")
        .arg("current")
        .arg("--watch")
        .arg("user.score")
        .arg("--watch")
        .arg("user.name.first")
        .arg("--break")
        .arg(line_break_selector(&file, 10))
        .arg(&file)
        .output()
        .unwrap();

    assert!(started.status.success());
    let started_json = parse_json(&started.stdout);
    assert_eq!(started_json["data"]["session"]["watches"][0], "userId");
    assert_eq!(
        watch_entry(&started_json, "userId")["status"],
        "not_in_scope"
    );
    assert_eq!(
        watch_entry(&started_json, "current")["status"],
        "not_in_scope"
    );
    assert_eq!(
        watch_entry(&started_json, "user.score")["status"],
        "not_in_scope"
    );
    assert_eq!(
        watch_entry(&started_json, "user.name.first")["status"],
        "not_in_scope"
    );
    let session = started_json["data"]["session"]["file"].as_str().unwrap();

    let continued = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("continue")
        .arg(session)
        .output()
        .unwrap();

    assert!(continued.status.success());
    let continued_json = parse_json(&continued.stdout);
    assert_eq!(
        continued_json["data"]["snapshot"]["pauseReason"],
        "breakpoint"
    );
    assert_eq!(watch_entry(&continued_json, "userId")["status"], "ok");
    assert!(watch_entry(&continued_json, "userId")["value"]["typeId"]
        .as_str()
        .unwrap()
        .ends_with(".UserId"));
    assert_eq!(watch_entry(&continued_json, "current")["status"], "ok");
    assert!(watch_entry(&continued_json, "current")["value"]["typeId"]
        .as_str()
        .unwrap()
        .ends_with(".UserId"));
    assert_eq!(watch_entry(&continued_json, "user.score")["status"], "ok");
    assert_eq!(
        watch_entry(&continued_json, "user.score")["value"]["kind"],
        "int"
    );
    assert_eq!(
        watch_entry(&continued_json, "user.score")["value"]["value"],
        1
    );
    assert_eq!(
        watch_entry(&continued_json, "user.name.first")["status"],
        "path_missing"
    );
}

#[test]
fn debug_run_rejects_invalid_watch_selector() {
    let dir = temp_dir("run-watch-invalid");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1\n");
    let artifact = dir.join("run.replay.json");

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

    let started = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("run")
        .arg("start")
        .arg("--replay")
        .arg(&artifact)
        .arg("--watch")
        .arg("user..score")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!started.status.success());
    assert!(started.stderr.is_empty());
    let json = parse_json(&started.stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "SIGIL-CLI-USAGE");
}

#[test]
fn debug_test_watches_resolve_at_breakpoint_scope() {
    let dir = temp_dir("test-watch");
    let file = write_program(
        &dir,
        "tests/basic.sigil",
        r#"t User={
  name:String,
  score:Int
}

t UserId=Int where value≥0

λhelper(user:User,userId:UserId)=>Int={
  l current=(userId:UserId);
  user.score+(match current=(0:UserId){
    true=>0|
    false=>current-current
  })
}

λmain()=>Unit=()

test "demo" {
  helper(
    {
      name:"Ada",
      score:2
    },
    (2:UserId)
  )=2
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
    let recorded_json = parse_json(&recorded.stdout);
    let test_id = recorded_json["results"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let started = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("test")
        .arg("start")
        .arg("--replay")
        .arg(&artifact)
        .arg("--test")
        .arg(&test_id)
        .arg("--watch")
        .arg("userId")
        .arg("--watch")
        .arg("current")
        .arg("--watch")
        .arg("user.score")
        .arg("--break")
        .arg(line_break_selector(&file, 10))
        .arg(&file)
        .output()
        .unwrap();

    assert!(started.status.success());
    let started_json = parse_json(&started.stdout);
    assert_eq!(
        watch_entry(&started_json, "userId")["status"],
        "not_in_scope"
    );
    assert_eq!(
        watch_entry(&started_json, "current")["status"],
        "not_in_scope"
    );
    assert_eq!(
        watch_entry(&started_json, "user.score")["status"],
        "not_in_scope"
    );
    let session = started_json["data"]["session"]["file"].as_str().unwrap();

    let continued = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("debug")
        .arg("test")
        .arg("continue")
        .arg(session)
        .output()
        .unwrap();

    assert!(continued.status.success());
    let continued_json = parse_json(&continued.stdout);
    assert_eq!(
        continued_json["data"]["snapshot"]["pauseReason"],
        "breakpoint"
    );
    assert_eq!(watch_entry(&continued_json, "userId")["status"], "ok");
    assert!(watch_entry(&continued_json, "userId")["value"]["typeId"]
        .as_str()
        .unwrap()
        .ends_with(".UserId"));
    assert_eq!(watch_entry(&continued_json, "current")["status"], "ok");
    assert!(watch_entry(&continued_json, "current")["value"]["typeId"]
        .as_str()
        .unwrap()
        .ends_with(".UserId"));
    assert_eq!(watch_entry(&continued_json, "user.score")["status"], "ok");
    assert_eq!(
        watch_entry(&continued_json, "user.score")["value"]["kind"],
        "int"
    );
    assert_eq!(
        watch_entry(&continued_json, "user.score")["value"]["value"],
        2
    );
}
