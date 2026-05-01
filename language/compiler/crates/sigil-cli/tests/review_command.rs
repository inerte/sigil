use jsonschema::JSONSchema;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(4)
        .unwrap_or_else(|| {
            panic!(
                "expected CARGO_MANIFEST_DIR to have 4 ancestors, got {}",
                manifest.display()
            )
        })
        .to_path_buf()
}

fn sigil_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sigil"))
}

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = repo_root().join("target").join(format!(
        "sigil-cli-review-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn external_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = env::temp_dir().join(format!(
        "sigil-cli-review-ext-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn parse_json(text: &[u8]) -> Value {
    serde_json::from_slice(text).unwrap_or_else(|error| {
        panic!(
            "failed to parse JSON output: {error}\nstdout bytes: {text:?}\nstdout text:\n{}",
            String::from_utf8_lossy(text)
        )
    })
}

fn cli_schema() -> JSONSchema {
    let schema_path = repo_root().join("language/spec/cli-json.schema.json");
    let schema_text = fs::read_to_string(&schema_path).unwrap_or_else(|error| {
        panic!("failed to read schema `{}`: {error}", schema_path.display())
    });
    let schema_json: Value = serde_json::from_str(&schema_text).unwrap_or_else(|error| {
        panic!(
            "failed to parse schema `{}` as JSON: {error}",
            schema_path.display()
        )
    });
    JSONSchema::compile(&schema_json).unwrap_or_else(|error| {
        panic!(
            "failed to compile schema `{}`: {error}",
            schema_path.display()
        )
    })
}

fn assert_schema_valid(schema: &JSONSchema, instance: &Value) {
    if let Err(errors) = schema.validate(instance) {
        let rendered = errors.map(|error| error.to_string()).collect::<Vec<_>>();
        panic!("schema validation failed:\n{}", rendered.join("\n"));
    }
}

fn run_sigil(dir: &Path, args: &[&str]) -> Output {
    Command::new(sigil_bin())
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run sigil with args {args:?} in `{}`: {error}",
                dir.display()
            )
        })
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected sigil command to succeed\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout_text(output),
        stderr_text(output)
    );
}

fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "expected sigil command to fail\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout_text(output),
        stderr_text(output)
    );
}

fn init_git_repo(dir: &Path) {
    let output = Command::new("git")
        .current_dir(dir)
        .args(["init", "-b", "main"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Sigil Test")
        .env("GIT_AUTHOR_EMAIL", "sigil@example.com")
        .env("GIT_COMMITTER_NAME", "Sigil Test")
        .env("GIT_COMMITTER_EMAIL", "sigil@example.com")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_file(dir: &Path, relative_path: &str, contents: &str) {
    let path = dir.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn write_project(dir: &Path, source: &str, test_source: Option<&str>) {
    write_file(
        dir,
        "sigil.json",
        "{\"name\":\"reviewDemo\",\"version\":\"2026-05-01T00-00-00Z\"}\n",
    );
    write_file(dir, "src/math.lib.sigil", source);
    if let Some(test_source) = test_source {
        write_file(dir, "tests/math.sigil", test_source);
    }
}

fn sample_test_source() -> &'static str {
    "λmain()=>Unit=()\n\ntest \"double\" {\n  •math.double(2)=4\n}\n"
}

#[test]
fn review_json_output_matches_cli_schema_and_reports_contract_changes() {
    let dir = temp_dir("json-schema");
    init_git_repo(&dir);
    write_project(
        &dir,
        "λdouble(x:Int)=>Int=x*2\n",
        Some(sample_test_source()),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_project(
        &dir,
        "λdouble(x:Int)=>Int\nrequires x≥0\n=x*2\n",
        Some(sample_test_source()),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "contract"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        stderr_text(&output)
    );

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigil review");
    assert_eq!(json["phase"], "surface");
    assert_eq!(json["data"]["summary"]["contractChanges"], 1);
    assert_eq!(json["data"]["summary"]["changedCoverageTargets"], 1);
    assert_eq!(json["data"]["changes"][0]["after"]["moduleId"], "src::math");
    assert!(!json["data"]["changes"][0]["after"]["moduleId"]
        .as_str()
        .unwrap()
        .contains(".sigil/review"));
    assert_eq!(json["data"]["testEvidence"]["changedTestFiles"], json!([]));
    assert_schema_valid(&cli_schema(), &json);
}

#[test]
fn review_staged_human_output_mentions_contracts_and_test_evidence() {
    let dir = temp_dir("staged-human");
    init_git_repo(&dir);
    write_project(
        &dir,
        "λdouble(x:Int)=>Int=x*2\n",
        Some(sample_test_source()),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_file(
        &dir,
        "src/math.lib.sigil",
        "λdouble(x:Int)=>Int\nrequires x≥0\n=x*2\n",
    );
    git(&dir, &["add", "src/math.lib.sigil"]);

    let output = run_sigil(&dir, &["review", "--staged"]);

    assert_success(&output);
    let text = stdout_text(&output);
    assert!(text.contains("Contract Changes"));
    assert!(text.contains("`double` in `src/math.lib.sigil`"));
    assert!(text.contains("changed test files: none"));
}

#[test]
fn review_staged_json_output_sets_scope_and_summary() {
    let dir = temp_dir("staged-json");
    init_git_repo(&dir);
    write_project(
        &dir,
        "λdouble(x:Int)=>Int=x*2\n",
        Some(sample_test_source()),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_file(
        &dir,
        "src/math.lib.sigil",
        "λdouble(x:Int)=>Int\nrequires x≥0\n=x*2\n",
    );
    git(&dir, &["add", "src/math.lib.sigil"]);

    let output = run_sigil(&dir, &["review", "--json", "--staged"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["scope"]["mode"], "staged");
    assert_eq!(json["data"]["scope"]["before"], "revision:HEAD");
    assert_eq!(json["data"]["scope"]["after"], "index");
    assert_eq!(json["data"]["summary"]["contractChanges"], 1);
}

#[test]
fn review_llm_output_embeds_grounded_facts() {
    let dir = temp_dir("llm-output");
    init_git_repo(&dir);
    write_project(
        &dir,
        "λdouble(x:Int)=>Int=x*2\n",
        Some(sample_test_source()),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_project(
        &dir,
        "λdouble(x:Int)=>Int\nrequires x≥0\n=x*2\n",
        Some(sample_test_source()),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "contract"]);

    let output = run_sigil(&dir, &["review", "--llm", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    let text = stdout_text(&output);
    assert!(text.contains("Use only the facts below."));
    assert!(text.contains("\"command\": \"sigil review\""));
    assert!(text.contains("\"contractChanges\": 1"));
}

#[test]
fn review_reports_before_snapshot_analysis_fallback_in_json() {
    let dir = temp_dir("parse-fallback");
    init_git_repo(&dir);
    write_project(
        &dir,
        "λvalue()=>Int=•missing.value()\n",
        Some("λmain()=>Unit=()\n\ntest \"placeholder\" {\n  true\n}\n"),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "broken"]);

    write_project(
        &dir,
        "λvalue()=>Int=1\n",
        Some("λmain()=>Unit=()\n\ntest \"placeholder\" {\n  true\n}\n"),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "fixed"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    let issues = json["data"]["issues"].as_array().unwrap();
    assert!(issues.iter().any(|issue| {
        issue["kind"] == "analysis-fallback"
            && issue["severity"] == "warning"
            && issue["message"]
                .as_str()
                .unwrap()
                .contains("before snapshot full analysis failed")
    }));
}

#[test]
fn review_after_snapshot_analysis_failure_exits_nonzero_and_marks_error() {
    let dir = temp_dir("after-fallback");
    init_git_repo(&dir);
    write_project(&dir, "λvalue()=>Int=1\n", None);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_project(&dir, "λvalue()=>Int=•missing.value()\n", None);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "broken"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD~1..HEAD"]);

    assert_failure(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["ok"], false);
    let issues = json["data"]["issues"].as_array().unwrap();
    assert!(issues.iter().any(|issue| {
        issue["kind"] == "analysis-fallback"
            && issue["severity"] == "error"
            && issue["message"]
                .as_str()
                .unwrap()
                .contains("after snapshot full analysis failed")
    }));
}

#[test]
fn review_parse_only_fallback_keeps_matching_generic_signatures_stable() {
    let dir = temp_dir("fallback-signature-spacing");
    init_git_repo(&dir);
    write_project(
        &dir,
        "λpickFirst[A,B](left:A,right:B)=>A=left\n\nλvalue()=>Int=•missing.value()\n",
        None,
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "broken"]);

    write_project(
        &dir,
        "λpickFirst[A,B](left:A,right:B)=>A=left\n\nλvalue()=>Int=1\n",
        None,
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "fixed"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    let issues = json["data"]["issues"].as_array().unwrap();
    assert!(issues.iter().any(|issue| {
        issue["kind"] == "analysis-fallback"
            && issue["message"]
                .as_str()
                .unwrap()
                .contains("before snapshot full analysis failed")
    }));
    let changes = json["data"]["changes"].as_array().unwrap();
    assert!(changes
        .iter()
        .any(|change| { change["declarationName"] == "value" }));
    assert!(!changes
        .iter()
        .any(|change| { change["declarationName"] == "pickFirst" }));
}

#[test]
fn review_reports_added_and_removed_functions() {
    let dir = temp_dir("add-remove");
    init_git_repo(&dir);
    write_project(
        &dir,
        "λdouble(x:Int)=>Int=x*2\n\nλtriple(x:Int)=>Int=x*3\n",
        None,
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_project(
        &dir,
        "λdouble(x:Int)=>Int=x*2\n\nλquadruple(x:Int)=>Int=x*4\n",
        None,
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "rename-surface"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    let changes = json["data"]["changes"].as_array().unwrap();
    assert!(changes
        .iter()
        .any(|change| { change["status"] == "removed" && change["declarationName"] == "triple" }));
    assert!(changes
        .iter()
        .any(|change| { change["status"] == "added" && change["declarationName"] == "quadruple" }));
}

#[test]
fn review_human_output_shows_add_and_remove_markers() {
    let dir = temp_dir("human-markers");
    init_git_repo(&dir);
    write_project(
        &dir,
        "λdouble(x:Int)=>Int=x*2\n\nλtriple(x:Int)=>Int=x*3\n",
        None,
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_project(
        &dir,
        "λdouble(x:Int)=>Int=x*2\n\nλquadruple(x:Int)=>Int=x*4\n",
        None,
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "surface"]);

    let output = run_sigil(&dir, &["review", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    let text = stdout_text(&output);
    assert!(text.contains("Signature Changes"));
    assert!(text.contains("- - function `triple` in `src/math.lib.sigil`"));
    assert!(text.contains("- + function `quadruple` in `src/math.lib.sigil`"));
}

#[test]
fn review_counts_function_effect_changes() {
    let dir = temp_dir("effect-change");
    init_git_repo(&dir);
    write_project(&dir, "λnotify()=>Unit=()\n", None);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_project(
        &dir,
        "e console:{log:λ(String)=>!Log Unit}\n\nλnotify()=>!Log Unit=console.log(\"x\")\n",
        None,
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "effect"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["summary"]["effectChanges"], 1);
    assert!(json["data"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| {
            change["declarationName"] == "notify"
                && change["changeKinds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|kind| kind == "effects")
        }));
}

#[test]
fn review_base_head_mode_sets_scope() {
    let dir = temp_dir("base-head");
    init_git_repo(&dir);
    write_project(&dir, "λdouble(x:Int)=>Int=x*2\n", None);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_project(&dir, "λdouble(x:Int)=>Int\nrequires x≥0\n=x*2\n", None);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "head"]);

    let output = run_sigil(
        &dir,
        &["review", "--json", "--base", "HEAD~1", "--head", "HEAD"],
    );

    assert_success(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["scope"]["mode"], "baseHead");
    assert_eq!(json["data"]["scope"]["before"], "revision:HEAD~1");
    assert_eq!(json["data"]["scope"]["after"], "revision:HEAD");
}

#[test]
fn review_head_without_base_errors_clearly() {
    let dir = temp_dir("head-without-base");
    init_git_repo(&dir);

    let output = run_sigil(&dir, &["review", "--head", "HEAD"]);

    assert_failure(&output);
    assert!(stderr_text(&output).contains("sigil review --head requires --base"));
}

#[test]
fn review_json_no_diff_returns_empty_change_set() {
    let dir = temp_dir("no-diff");
    init_git_repo(&dir);
    write_project(&dir, "λdouble(x:Int)=>Int=x*2\n", None);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD..HEAD"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["summary"]["changedDeclarations"], 0);
    assert_eq!(json["data"]["changes"], json!([]));
}

#[test]
fn review_reports_type_constraint_changes() {
    let dir = temp_dir("type-constraint");
    init_git_repo(&dir);
    write_file(
        &dir,
        "sigil.json",
        "{\"name\":\"reviewDemo\",\"version\":\"2026-05-01T00-00-00Z\"}\n",
    );
    write_file(&dir, "src/types.lib.sigil", "t Age=Int where value≥0\n");
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_file(&dir, "src/types.lib.sigil", "t Age=Int where value>0\n");
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "constraint"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["summary"]["typeChanges"], 1);
    assert!(json["data"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| {
            change["declarationName"] == "Age"
                && change["changeKinds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|kind| kind == "constraint")
        }));
}

#[test]
fn review_reports_decreases_changes() {
    let dir = temp_dir("decreases-change");
    init_git_repo(&dir);
    write_project(
        &dir,
        "total λloop(n:Int)=>Int\nrequires n≥0\ndecreases n\nmatch n=0{\n  true=>0|\n  false=>loop(n-1)\n}\n",
        None,
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_project(
        &dir,
        "total λloop(n:Int)=>Int\nrequires n≥0\ndecreases n+1\nmatch n=0{\n  true=>0|\n  false=>loop(n-1)\n}\n",
        None,
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "decreases"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    assert!(json["data"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| {
            change["declarationName"] == "loop"
                && change["changeKinds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|kind| kind == "decreases")
        }));
}

#[test]
fn review_raw_diff_with_find_renames_handles_renamed_files() {
    let dir = temp_dir("rename-diff");
    init_git_repo(&dir);
    write_project(&dir, "λdouble(x:Int)=>Int=x*2\n", None);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    git(
        &dir,
        &["mv", "src/math.lib.sigil", "src/arithmetic.lib.sigil"],
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "rename"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "-M", "HEAD~1", "HEAD"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["scope"]["mode"], "raw");
    assert_eq!(json["data"]["summary"]["changedDeclarations"], 0);
    assert_eq!(json["data"]["changes"], json!([]));
}

#[test]
fn review_test_evidence_warning_is_non_blocking() {
    let dir = temp_dir("test-evidence-warning");
    init_git_repo(&dir);
    write_project(
        &dir,
        "λdouble(x:Int)=>Int=x*2\n",
        Some(sample_test_source()),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_project(
        &dir,
        "λdouble(x:Int)=>Int\nrequires x≥0\n=x*2\n",
        Some(sample_test_source()),
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "contract"]);

    let output = run_sigil(&dir, &["review", "--json", "--", "HEAD~1..HEAD"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["summary"]["changedCoverageTargets"], 1);
    assert_eq!(json["data"]["summary"]["changedTestFiles"], 0);
    let issues = json["data"]["issues"].as_array().unwrap();
    assert!(issues.iter().any(|issue| {
        issue["kind"] == "test-evidence"
            && issue["severity"] == "warning"
            && issue["message"]
                .as_str()
                .unwrap()
                .contains("changed coverage targets detected (1)")
    }));
}

#[test]
fn review_ignores_internal_review_artifacts() {
    let dir = temp_dir("internal-artifacts");
    init_git_repo(&dir);
    write_project(&dir, "λdouble(x:Int)=>Int=x*2\n", None);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "base"]);

    write_file(&dir, ".sigil/review/leaked.sigil", "λleak()=>Int=1\n");
    git(&dir, &["add", ".sigil/review/leaked.sigil"]);

    let output = run_sigil(&dir, &["review", "--json", "--staged"]);

    assert_success(&output);
    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["summary"]["changedDeclarations"], 0);
    assert_eq!(json["data"]["changes"], json!([]));
}

#[test]
fn review_non_git_repo_errors_clearly() {
    let dir = external_temp_dir("non-git");
    let output = run_sigil(&dir, &["review"]);

    assert_failure(&output);
    assert!(stderr_text(&output).contains("sigil review requires a git repository"));
}
