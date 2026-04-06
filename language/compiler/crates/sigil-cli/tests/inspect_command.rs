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
        "sigil-cli-inspect-{label}-{}-{unique}",
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

#[test]
fn inspect_types_reports_top_level_types_and_spans() {
    let dir = temp_dir("types-single");
    let file = write_program(
        &dir,
        "generic.lib.sigil",
        "c answer=(41:Int)\n\nλidentity[T](x:T)=>T=x\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("types")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect types");
    assert_eq!(json["ok"], true);
    assert_eq!(json["phase"], "typecheck");
    assert_eq!(json["data"]["summary"]["functions"], 1);
    assert_eq!(json["data"]["summary"]["consts"], 1);

    let declarations = json["data"]["declarations"].as_array().unwrap();
    assert_eq!(declarations.len(), 2);
    let identity = declarations
        .iter()
        .find(|declaration| declaration["name"] == "identity")
        .unwrap();
    let answer = declarations
        .iter()
        .find(|declaration| declaration["name"] == "answer")
        .unwrap();
    assert_eq!(identity["type"], "∀T. (T) => T");
    assert!(!identity["spanId"].as_str().unwrap().is_empty());
    assert_eq!(identity["location"]["start"]["line"], 3);
    assert_eq!(answer["type"], "Int");
}

#[test]
fn inspect_types_reports_named_type_inventory_and_constraints() {
    let dir = temp_dir("types-named");
    let file = write_program(
        &dir,
        "types.lib.sigil",
        concat!(
            "t Age=Int\n\n",
            "t BirthYear=Int where value>1800 and value<10000\n\n",
            "t User={birthYear:BirthYear,name:String}\n\n",
            "t DateRange={end:Int,start:Int} where value.end≥value.start\n\n",
            "t Result=Ok(Int)|Err(String)\n",
        ),
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("types")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    let module_id = json["data"]["moduleId"].as_str().unwrap();
    assert_eq!(json["data"]["summary"]["types"], 5);
    assert_eq!(json["data"]["summary"]["aliases"], 2);
    assert_eq!(json["data"]["summary"]["products"], 2);
    assert_eq!(json["data"]["summary"]["sums"], 1);
    assert_eq!(json["data"]["summary"]["constrainedTypes"], 2);

    let types = json["data"]["types"].as_array().unwrap();
    assert_eq!(types.len(), 5);

    let age = types.iter().find(|entry| entry["name"] == "Age").unwrap();
    assert_eq!(age["typeId"], format!("{module_id}.Age"));
    assert_eq!(age["kind"], "alias");
    assert_eq!(age["constrained"], false);
    assert_eq!(age["equalityMode"], "structural");
    assert_eq!(age["definitionAst"]["kind"], "alias");

    let birth_year = types
        .iter()
        .find(|entry| entry["name"] == "BirthYear")
        .unwrap();
    assert_eq!(birth_year["typeId"], format!("{module_id}.BirthYear"));
    assert_eq!(birth_year["kind"], "alias");
    assert_eq!(birth_year["constrained"], true);
    assert_eq!(birth_year["equalityMode"], "refinement");
    assert_eq!(birth_year["definitionSource"], "Int");
    assert_eq!(birth_year["constraintSource"], "value>1800 and value<10000");
    assert_eq!(birth_year["constraintAst"]["kind"], "binary");
    assert_eq!(birth_year["constraintAst"]["operator"], "and");
    assert_eq!(birth_year["location"]["start"]["line"], 3);

    let user = types.iter().find(|entry| entry["name"] == "User").unwrap();
    assert_eq!(user["kind"], "product");
    assert_eq!(user["constrained"], false);
    assert_eq!(user["equalityMode"], "structural");
    assert_eq!(user["definitionAst"]["kind"], "product");

    let date_range = types
        .iter()
        .find(|entry| entry["name"] == "DateRange")
        .unwrap();
    assert_eq!(date_range["kind"], "product");
    assert_eq!(date_range["constrained"], true);
    assert_eq!(date_range["equalityMode"], "refinement");
    assert_eq!(date_range["constraintAst"]["kind"], "binary");
    assert_eq!(date_range["constraintAst"]["operator"], "≥");

    let result = types
        .iter()
        .find(|entry| entry["name"] == "Result")
        .unwrap();
    assert_eq!(result["kind"], "sum");
    assert_eq!(result["constrained"], false);
    assert_eq!(result["equalityMode"], "nominal");
    assert_eq!(result["definitionAst"]["kind"], "sum");
}

#[test]
fn inspect_types_directory_reports_requested_modules_only() {
    let dir = temp_dir("types-directory");
    write_program(
        &dir,
        "sigil.json",
        "{\"name\":\"inspectTypes\",\"version\":\"2026-04-05T14-58-24Z\"}\n",
    );
    let helper = write_program(&dir, "src/helper.lib.sigil", "λdouble(x:Int)=>Int=x*2\n");
    let main = write_program(&dir, "src/main.sigil", "λmain()=>Int=•helper.double(21)\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("types")
        .arg(&dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect types");
    assert_eq!(json["data"]["summary"]["discovered"], 2);
    assert_eq!(json["data"]["summary"]["inspected"], 2);

    let files = json["data"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    let main_result = files
        .iter()
        .find(|entry| entry["input"] == main.to_string_lossy().to_string())
        .unwrap();
    let helper_result = files
        .iter()
        .find(|entry| entry["input"] == helper.to_string_lossy().to_string())
        .unwrap();

    assert_eq!(main_result["moduleId"], "src::main");
    assert_eq!(main_result["declarations"].as_array().unwrap().len(), 1);
    assert_eq!(main_result["declarations"][0]["name"], "main");
    assert_eq!(main_result["declarations"][0]["type"], "() => Int");
    assert_eq!(helper_result["moduleId"], "src::helper");
    assert_eq!(helper_result["declarations"].as_array().unwrap().len(), 1);
    assert_eq!(helper_result["declarations"][0]["name"], "double");
}

#[test]
fn inspect_types_emits_json_error_on_type_failure() {
    let dir = temp_dir("types-error");
    let file = write_program(&dir, "broken.sigil", "λmain()=>Int=\"oops\"\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("types")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect types");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "typecheck");
}

#[test]
fn inspect_proof_reports_constraints_contracts_and_branch_sites() {
    let dir = temp_dir("proof-single");
    let file = write_program(
        &dir,
        "proof.lib.sigil",
        concat!(
            "t BirthYear=Int where value>1800\n\n",
            "λnormalize(raw:Int)=>Int\n",
            "requires raw>0\n",
            "ensures result>1800\n",
            "match raw{\n",
            "  value when value>1800=>value|\n",
            "  _=>1900\n",
            "}\n",
        ),
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("proof")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect proof");
    assert_eq!(json["ok"], true);
    assert_eq!(json["phase"], "proof");
    assert_eq!(json["data"]["summary"]["typeConstraints"], 1);
    assert_eq!(json["data"]["summary"]["requires"], 1);
    assert_eq!(json["data"]["summary"]["ensures"], 1);
    assert_eq!(json["data"]["summary"]["matchArms"], 2);
    assert_eq!(json["data"]["summary"]["ifConditions"], 0);

    let sites = json["data"]["sites"].as_array().unwrap();
    assert!(sites.iter().any(|site| site["kind"] == "typeConstraint"));
    assert!(sites.iter().any(|site| site["kind"] == "requires"));
    assert!(sites.iter().any(|site| site["kind"] == "ensures"));
    assert!(sites.iter().any(|site| {
        site["kind"] == "matchArm"
            && site["patternSource"] == "value"
            && site["predicateSource"] == "value>1800"
    }));
}

#[test]
fn inspect_validate_returns_canonical_source_for_noncanonical_input() {
    let dir = temp_dir("validate-single");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("validate")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect validate");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["alreadyCanonical"], false);
    assert_eq!(json["data"]["validation"]["ok"], false);
    assert_eq!(json["data"]["canonicalSource"], "λmain()=>Int=1\n");
    assert!(!json["data"]["validation"]["errors"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn inspect_validate_directory_reports_per_file_status() {
    let dir = temp_dir("validate-directory");
    let canonical = write_program(&dir, "ok.sigil", "λmain()=>Int=1\n");
    let noncanonical = write_program(&dir, "no_newline.sigil", "λmain()=>Int=2");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("validate")
        .arg(&dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect validate");
    assert_eq!(json["data"]["summary"]["discovered"], 2);
    assert_eq!(json["data"]["summary"]["inspected"], 2);

    let files = json["data"]["files"].as_array().unwrap();
    let canonical_result = files
        .iter()
        .find(|entry| entry["input"] == canonical.to_string_lossy().to_string())
        .unwrap();
    let noncanonical_result = files
        .iter()
        .find(|entry| entry["input"] == noncanonical.to_string_lossy().to_string())
        .unwrap();

    assert_eq!(canonical_result["alreadyCanonical"], true);
    assert_eq!(canonical_result["validation"]["ok"], true);
    assert_eq!(noncanonical_result["alreadyCanonical"], false);
    assert_eq!(noncanonical_result["validation"]["ok"], false);
    assert_eq!(noncanonical_result["canonicalSource"], "λmain()=>Int=2\n");
}

#[test]
fn inspect_codegen_returns_inline_ts_and_module_inventory_without_writing_files() {
    let dir = temp_dir("codegen-single");
    write_program(
        &dir,
        "sigil.json",
        "{\"name\":\"inspectCodegen\",\"version\":\"2026-04-05T14-58-24Z\"}\n",
    );
    write_program(&dir, "src/helper.lib.sigil", "λdouble(x:Int)=>Int=x*2\n");
    let main = write_program(&dir, "src/main.sigil", "λmain()=>Int=•helper.double(21)\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("codegen")
        .arg(&main)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect codegen");
    assert_eq!(json["ok"], true);
    assert_eq!(json["phase"], "codegen");
    assert_eq!(json["data"]["moduleId"], "src::main");
    assert_eq!(
        json["data"]["sourceFile"],
        main.to_string_lossy().to_string()
    );
    assert!(json["data"]["summary"]["modules"].as_u64().unwrap() >= 2);

    let codegen = &json["data"]["codegen"];
    let source = codegen["source"].as_str().unwrap();
    assert!(source.contains("export function main"));
    assert_eq!(
        codegen["lineCount"].as_u64().unwrap() as usize,
        source.lines().count()
    );
    assert_eq!(codegen["spanMapSummary"]["formatVersion"], 1);
    assert!(codegen["spanMapSummary"]["spans"].as_u64().unwrap() >= 1);
    assert!(
        codegen["spanMapSummary"]["generatedRanges"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(
        codegen["spanMapSummary"]["topLevelAnchors"]
            .as_u64()
            .unwrap()
            >= 1
    );

    let output_file = PathBuf::from(codegen["outputFile"].as_str().unwrap());
    let span_map_file = PathBuf::from(codegen["spanMapFile"].as_str().unwrap());
    assert!(!output_file.exists());
    assert!(!span_map_file.exists());

    let modules = json["data"]["modules"].as_array().unwrap();
    assert!(modules.len() >= 2);
    assert!(modules.iter().any(|entry| entry["moduleId"] == "src::main"
        && entry["sourceFile"] == main.to_string_lossy().to_string()));
    assert!(modules
        .iter()
        .any(|entry| entry["moduleId"] == "src::helper"));
}

#[test]
fn inspect_codegen_directory_batches_requested_files_and_respects_ignore_rules() {
    let dir = temp_dir("codegen-directory");
    let alpha = write_program(&dir, "alpha.sigil", "λmain()=>Int=1\n");
    let beta = write_program(&dir, "beta.sigil", "λmain()=>Int=2\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("codegen")
        .arg(&dir)
        .arg("--ignore")
        .arg(&beta)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect codegen");
    assert_eq!(json["data"]["summary"]["discovered"], 1);
    assert_eq!(json["data"]["summary"]["inspected"], 1);
    assert_eq!(json["data"]["summary"]["groups"], 1);

    let files = json["data"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["input"], alpha.to_string_lossy().to_string());
    assert!(files[0]["codegen"]["source"]
        .as_str()
        .unwrap()
        .contains("export function main"));
}

#[test]
fn inspect_codegen_emits_json_error_on_pipeline_failure() {
    let dir = temp_dir("codegen-error");
    let file = write_program(&dir, "broken.sigil", "λmain()=>Int=\"oops\"\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("codegen")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect codegen");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "typecheck");
}

#[test]
fn inspect_world_reports_normalized_runtime_world_for_topology_project() {
    let dir = temp_dir("world-topology");
    write_program(
        &dir,
        "sigil.json",
        "{\"name\":\"inspectWorld\",\"version\":\"2026-04-05T14-58-24Z\"}\n",
    );
    write_program(
        &dir,
        "src/topology.lib.sigil",
        "c local=(§topology.environment(\"local\"):§topology.Environment)\n\nc mailerApi=(§topology.httpService(\"mailerApi\"):§topology.HttpServiceDependency)\n",
    );
    write_program(
        &dir,
        "config/local.lib.sigil",
        "c world=(†runtime.world(†clock.systemClock(),†fs.real(),[†http.proxy(\"http://127.0.0.1:45110\",•topology.mailerApi)],†log.capture(),†process.real(),†random.seeded(1337),[],†timer.virtual()):†runtime.World)\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("world")
        .arg(&dir)
        .arg("--env")
        .arg("local")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect world");
    assert_eq!(json["ok"], true);
    assert_eq!(json["phase"], "topology");
    assert_eq!(json["data"]["environment"], "local");
    assert!(json["data"].get("sources").is_none());
    assert_eq!(json["data"]["topology"]["present"], true);
    assert_eq!(json["data"]["topology"]["declaredEnvs"][0], "local");
    assert_eq!(json["data"]["topology"]["httpDependencies"][0], "mailerApi");
    assert_eq!(json["data"]["summary"]["logKind"], "capture");
    assert_eq!(json["data"]["summary"]["randomKind"], "seeded");
    assert_eq!(json["data"]["summary"]["timerKind"], "virtual");
    assert_eq!(json["data"]["summary"]["httpBindings"], 1);
    assert_eq!(
        json["data"]["normalizedWorld"]["http"]["mailerApi"]["kind"],
        "proxy"
    );
    assert_eq!(
        json["data"]["normalizedWorld"]["http"]["mailerApi"]["baseUrl"],
        "http://127.0.0.1:45110"
    );
}

#[test]
fn inspect_world_supports_config_only_projects_without_topology() {
    let dir = temp_dir("world-config-only");
    write_program(
        &dir,
        "sigil.json",
        "{\"name\":\"inspectWorld\",\"version\":\"2026-04-05T14-58-24Z\"}\n",
    );
    write_program(
        &dir,
        "config/local.lib.sigil",
        "c world=(†runtime.world(†clock.systemClock(),†fs.real(),[],†log.stdout(),†process.real(),†random.seeded(7),[],†timer.real()):†runtime.World)\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("world")
        .arg(&dir)
        .arg("--env")
        .arg("local")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect world");
    assert_eq!(json["ok"], true);
    assert!(json["data"].get("sources").is_none());
    assert_eq!(json["data"]["topology"]["present"], false);
    assert_eq!(
        json["data"]["topology"]["declaredEnvs"],
        serde_json::json!([])
    );
    assert_eq!(json["data"]["summary"]["httpBindings"], 0);
    assert_eq!(json["data"]["summary"]["tcpBindings"], 0);
    assert_eq!(json["data"]["normalizedWorld"]["random"]["kind"], "seeded");
    assert_eq!(json["data"]["normalizedWorld"]["timer"]["kind"], "real");
}

#[test]
fn inspect_world_emits_json_error_when_env_is_undeclared() {
    let dir = temp_dir("world-env-error");
    write_program(
        &dir,
        "sigil.json",
        "{\"name\":\"inspectWorld\",\"version\":\"2026-04-05T14-58-24Z\"}\n",
    );
    write_program(
        &dir,
        "src/topology.lib.sigil",
        "c local=(§topology.environment(\"local\"):§topology.Environment)\n",
    );
    write_program(
        &dir,
        "config/prod.lib.sigil",
        "c world=(†runtime.world(†clock.systemClock(),†fs.real(),[],†log.stdout(),†process.real(),†random.real(),[],†timer.real()):†runtime.World)\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("world")
        .arg(&dir)
        .arg("--env")
        .arg("prod")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect world");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "topology");
    assert_eq!(json["error"]["code"], "SIGIL-TOPO-ENV-NOT-FOUND");
}

#[test]
fn inspect_world_emits_json_error_when_config_module_is_missing() {
    let dir = temp_dir("world-missing-config");
    write_program(
        &dir,
        "sigil.json",
        "{\"name\":\"inspectWorld\",\"version\":\"2026-04-05T14-58-24Z\"}\n",
    );
    write_program(
        &dir,
        "src/topology.lib.sigil",
        "c local=(§topology.environment(\"local\"):§topology.Environment)\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("world")
        .arg(&dir)
        .arg("--env")
        .arg("local")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect world");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "topology");
    assert_eq!(json["error"]["code"], "SIGIL-TOPO-MISSING-CONFIG-MODULE");
}
