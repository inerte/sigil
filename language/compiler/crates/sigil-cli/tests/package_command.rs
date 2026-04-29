use serde_json::Value;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(path: PathBuf) -> Self {
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl AsRef<Path> for TestDir {
    fn as_ref(&self) -> &Path {
        self.path.as_path()
    }
}

impl Deref for TestDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path.as_path()
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

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
        "sigil-cli-package-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn external_temp_dir(label: &str) -> TestDir {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let parent = repo_root().join("tmp").join("sigil-cli-tests");
    fs::create_dir_all(&parent).unwrap();
    let dir = parent.join(format!(
        "package-external-{label}-{}-{unique}",
        std::process::id()
    ));
    TestDir::new(dir)
}

fn write_file(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn parse_json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

fn npm_env(command: &mut Command, fake_npm_dir: &Path, registry_dir: &Path) {
    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{current_path}", fake_npm_dir.display());
    command.env("PATH", new_path);
    command.env("SIGIL_FAKE_NPM_REGISTRY", registry_dir);
}

fn write_fake_npm(fake_npm_dir: &Path) {
    let script = r#"#!/bin/sh
set -eu

registry="${SIGIL_FAKE_NPM_REGISTRY:?missing registry}"
cmd="$1"
shift

case "$cmd" in
  view)
    pkg="$1"
    shift
    if [ "$1" != "versions" ] || [ "$2" != "--json" ]; then
      echo "unsupported npm view invocation" >&2
      exit 1
    fi
    cat "$registry/$pkg/versions.json"
    ;;
  pack)
    if [ "$#" -eq 1 ] && [ "$1" = "--json" ]; then
      pkg_json="$PWD/package.json"
      if [ ! -f "$pkg_json" ]; then
        echo "missing package.json for local npm pack" >&2
        exit 1
      fi
      name="$(sed -n 's/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$pkg_json" | head -n 1)"
      ver="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$pkg_json" | head -n 1)"
      if [ -z "$name" ] || [ -z "$ver" ]; then
        echo "invalid package.json for local npm pack" >&2
        exit 1
      fi
      tar_name="${name}-${ver}.tgz"
      stage="$(mktemp -d "${TMPDIR:-/tmp}/sigil-fake-npm-pack-XXXXXX")"
      mkdir -p "$stage/package"
      cp -R "$PWD"/. "$stage/package/"
      tar -czf "$PWD/$tar_name" -C "$stage" package
      rm -rf "$stage"
      printf '[{"filename":"%s","integrity":"sha512-fake"}]\n' "$tar_name"
      exit 0
    fi
    spec="$1"
    shift
    if [ "$1" != "--json" ]; then
      echo "unsupported npm pack invocation" >&2
      exit 1
    fi
    pkg="${spec%@*}"
    ver="${spec##*@}"
    src="$registry/$pkg/$ver.tgz"
    dest="$PWD/$(basename "$src")"
    cp "$src" "$dest"
    printf '[{"filename":"%s","integrity":"sha512-fake"}]\n' "$(basename "$src")"
    ;;
  publish)
    mkdir -p "$registry/published"
    cp "$PWD/package.json" "$registry/published/package.json"
    if [ -f "$PWD/sigil.json" ]; then
      cp "$PWD/sigil.json" "$registry/published/sigil.json"
    fi
    printf '{"ok":true}\n'
    ;;
  *)
    echo "unsupported npm command: $cmd" >&2
    exit 1
    ;;
esac
"#;
    let path = fake_npm_dir.join("npm");
    fs::create_dir_all(fake_npm_dir).unwrap();
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
    }
}

fn sigil_to_npm_version(version: &str) -> String {
    format!(
        "{}{}.{}.0",
        &version[0..4],
        version[5..7].to_string() + &version[8..10],
        version[11..13].to_string() + &version[14..16].to_string() + &version[17..19]
    )
}

fn sigil_to_npm_name(name: &str) -> String {
    let mut npm_name = String::new();
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            npm_name.push('-');
            npm_name.push(ch.to_ascii_lowercase());
        } else {
            npm_name.push(ch);
        }
    }
    npm_name
}

fn create_registry_package(
    registry_dir: &Path,
    name: &str,
    version: &str,
    dependencies: &[(&str, &str)],
    files: &[(&str, &str)],
) {
    let npm_name = sigil_to_npm_name(name);
    let npm_version = sigil_to_npm_version(version);
    let pkg_dir = registry_dir.join(&npm_name);
    fs::create_dir_all(&pkg_dir).unwrap();

    let versions_path = pkg_dir.join("versions.json");
    let mut versions = if versions_path.exists() {
        serde_json::from_str::<Vec<String>>(&fs::read_to_string(&versions_path).unwrap()).unwrap()
    } else {
        Vec::new()
    };
    if !versions.contains(&npm_version) {
        versions.push(npm_version.clone());
    }
    versions.sort();
    fs::write(
        &versions_path,
        format!("{}\n", serde_json::to_string_pretty(&versions).unwrap()),
    )
    .unwrap();

    let staging_root = temp_dir("registry-package");
    let package_root = staging_root.join("package");
    fs::create_dir_all(&package_root).unwrap();

    let manifest = serde_json::json!({
        "name": name,
        "version": version,
        "dependencies": dependencies.iter().map(|(dep_name, dep_version)| ((*dep_name).to_string(), Value::String((*dep_version).to_string()))).collect::<serde_json::Map<String, Value>>(),
        "publish": {}
    });
    fs::write(
        package_root.join("sigil.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .unwrap();

    for (relative, contents) in files {
        write_file(&package_root, relative, contents);
    }

    let tarball_path = pkg_dir.join(format!("{npm_version}.tgz"));
    let output = Command::new("tar")
        .arg("-czf")
        .arg(&tarball_path)
        .arg("-C")
        .arg(&staging_root)
        .arg("package")
        .output()
        .unwrap();
    assert!(output.status.success(), "tar failed: {:?}", output);
}

fn write_consumer_project(root: &Path, main_source: &str, test_source: Option<&str>) {
    write_file(
        root,
        "sigil.json",
        "{\n  \"name\": \"consumerApp\",\n  \"version\": \"2026-04-05T14-58-24Z\"\n}\n",
    );
    write_file(root, "src/main.sigil", main_source);
    if let Some(test_source) = test_source {
        write_file(root, "tests/main.sigil", test_source);
    }
}

#[test]
fn package_commands_add_list_why_remove_and_block_transitive_imports() {
    let registry_dir = temp_dir("registry");
    let fake_npm_dir = temp_dir("fake-npm");
    write_fake_npm(&fake_npm_dir);

    create_registry_package(
        &registry_dir,
        "helper",
        "2026-04-05T14-57-00Z",
        &[],
        &[("src/package.lib.sigil", "λdouble(value:Int)=>Int=value*2\n")],
    );
    create_registry_package(
        &registry_dir,
        "router",
        "2026-04-05T14-58-24Z",
        &[("helper", "2026-04-05T14-57-00Z")],
        &[(
            "src/package.lib.sigil",
            "λdouble(value:Int)=>Int=☴helper.double(value)\n",
        )],
    );

    let workspace_root = external_temp_dir("consumer-workspace");
    fs::create_dir_all(workspace_root.join(".git")).unwrap();
    let consumer_dir = workspace_root.join("projects").join("consumer");
    write_consumer_project(&consumer_dir, "λmain()=>Int=☴router.double(21)\n", None);
    let transitive_probe = consumer_dir.join("src/transitive.sigil");
    write_file(
        &consumer_dir,
        "src/transitive.sigil",
        "λmain()=>Int=☴helper.double(21)\n",
    );

    let mut add = Command::new(sigil_bin());
    add.current_dir(&consumer_dir)
        .args(["package", "add", "router"]);
    npm_env(&mut add, &fake_npm_dir, &registry_dir);
    let add_output = add.output().unwrap();
    assert!(add_output.status.success(), "{:?}", add_output);

    let add_json = parse_json(&add_output.stdout);
    assert_eq!(add_json["ok"], true);
    assert_eq!(add_json["data"]["dependency"], "router");
    assert!(consumer_dir
        .join(".sigil/packages/router/2026-04-05T14-58-24Z/src/package.lib.sigil")
        .exists());

    let mut list = Command::new(sigil_bin());
    list.current_dir(&consumer_dir).args(["package", "list"]);
    npm_env(&mut list, &fake_npm_dir, &registry_dir);
    let list_output = list.output().unwrap();
    assert!(list_output.status.success(), "{:?}", list_output);
    let list_json = parse_json(&list_output.stdout);
    assert_eq!(list_json["data"]["dependencies"][0]["dependency"], "router");
    assert_eq!(list_json["data"]["dependencies"][0]["installed"], true);

    let mut why = Command::new(sigil_bin());
    why.current_dir(&consumer_dir)
        .args(["package", "why", "helper"]);
    npm_env(&mut why, &fake_npm_dir, &registry_dir);
    let why_output = why.output().unwrap();
    assert!(why_output.status.success(), "{:?}", why_output);
    let why_json = parse_json(&why_output.stdout);
    assert_eq!(
        why_json["data"]["paths"][0],
        serde_json::json!(["router@2026-04-05T14-58-24Z", "helper@2026-04-05T14-57-00Z"])
    );

    let mut compile_direct = Command::new(sigil_bin());
    compile_direct
        .current_dir(&consumer_dir)
        .args(["compile", "src/main.sigil"]);
    npm_env(&mut compile_direct, &fake_npm_dir, &registry_dir);
    let compile_direct_output = compile_direct.output().unwrap();
    assert!(
        compile_direct_output.status.success(),
        "{:?}",
        compile_direct_output
    );

    let mut compile_transitive = Command::new(sigil_bin());
    compile_transitive
        .current_dir(&consumer_dir)
        .arg("compile")
        .arg(&transitive_probe);
    npm_env(&mut compile_transitive, &fake_npm_dir, &registry_dir);
    let compile_transitive_output = compile_transitive.output().unwrap();
    assert!(!compile_transitive_output.status.success());
    let compile_transitive_stderr = String::from_utf8_lossy(&compile_transitive_output.stderr);
    assert!(compile_transitive_stderr.contains("Module not found: package::helper"));
    assert!(compile_transitive_stderr.contains("direct dependency `helper` is not declared"));

    let mut remove = Command::new(sigil_bin());
    remove
        .current_dir(&consumer_dir)
        .args(["package", "remove", "router"]);
    npm_env(&mut remove, &fake_npm_dir, &registry_dir);
    let remove_output = remove.output().unwrap();
    assert!(remove_output.status.success(), "{:?}", remove_output);

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(consumer_dir.join("sigil.json")).unwrap())
            .unwrap();
    assert!(manifest.get("dependencies").is_none());
}

#[test]
fn package_add_ignores_publishable_packages_inside_nested_git_workspaces() {
    let registry_dir = temp_dir("nested-workspace-registry");
    let fake_npm_dir = temp_dir("nested-workspace-fake-npm");
    write_fake_npm(&fake_npm_dir);

    create_registry_package(
        &registry_dir,
        "router",
        "2026-04-05T14-58-24Z",
        &[],
        &[("src/package.lib.sigil", "λdouble(value:Int)=>Int=value*2\n")],
    );

    let nested_workspace = external_temp_dir("nested-workspace");
    fs::create_dir_all(&nested_workspace).unwrap();
    fs::create_dir_all(nested_workspace.join(".git")).unwrap();
    let nested_router_dir = nested_workspace.join("projects").join("router");
    write_file(
        &nested_router_dir,
        "sigil.json",
        "{\n  \"name\": \"router\",\n  \"version\": \"2026-04-05T14-58-24Z\",\n  \"publish\": {}\n}\n",
    );
    write_file(
        &nested_router_dir,
        "src/package.lib.sigil",
        "λdouble(value:Int)=>Int=999\n",
    );

    let consumer_dir = temp_dir("nested-workspace-consumer");
    write_consumer_project(&consumer_dir, "λmain()=>Int=☴router.double(21)\n", None);

    let mut add = Command::new(sigil_bin());
    add.current_dir(&consumer_dir)
        .args(["package", "add", "router"]);
    npm_env(&mut add, &fake_npm_dir, &registry_dir);
    let add_output = add.output().unwrap();
    assert!(add_output.status.success(), "{:?}", add_output);
}

#[test]
fn package_update_rolls_back_when_tests_fail() {
    let registry_dir = temp_dir("update-registry");
    let fake_npm_dir = temp_dir("update-fake-npm");
    write_fake_npm(&fake_npm_dir);

    create_registry_package(
        &registry_dir,
        "router",
        "2026-04-05T14-00-00Z",
        &[],
        &[("src/package.lib.sigil", "λvalue()=>Int=1\n")],
    );
    create_registry_package(
        &registry_dir,
        "router",
        "2026-04-05T15-00-00Z",
        &[],
        &[("src/package.lib.sigil", "λvalue()=>Int=2\n")],
    );

    let workspace_root = external_temp_dir("update-workspace");
    fs::create_dir_all(workspace_root.join(".git")).unwrap();
    let consumer_dir = workspace_root.join("projects").join("consumer");
    write_file(
        &consumer_dir,
        "sigil.json",
        "{\n  \"name\": \"consumerApp\",\n  \"version\": \"2026-04-05T14-58-24Z\",\n  \"dependencies\": {\n    \"router\": \"2026-04-05T14-00-00Z\"\n  }\n}\n",
    );
    write_file(
        &consumer_dir,
        "src/main.sigil",
        "λmain()=>Int=☴router.value()\n",
    );
    write_file(
        &consumer_dir,
        "tests/main.sigil",
        "test \"router value\" {\n  ☴router.value()=1\n}\n",
    );

    let mut install = Command::new(sigil_bin());
    install
        .current_dir(&consumer_dir)
        .args(["package", "install"]);
    npm_env(&mut install, &fake_npm_dir, &registry_dir);
    let install_output = install.output().unwrap();
    assert!(install_output.status.success(), "{:?}", install_output);

    let mut update = Command::new(sigil_bin());
    update
        .current_dir(&consumer_dir)
        .args(["package", "update", "router"]);
    npm_env(&mut update, &fake_npm_dir, &registry_dir);
    let update_output = update.output().unwrap();
    assert!(!update_output.status.success());

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(consumer_dir.join("sigil.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["dependencies"]["router"], "2026-04-05T14-00-00Z");

    let lockfile: Value =
        serde_json::from_str(&fs::read_to_string(consumer_dir.join("sigil.lock")).unwrap())
            .unwrap();
    assert!(lockfile["packages"]
        .as_object()
        .unwrap()
        .contains_key("router@2026-04-05T14-00-00Z"));
    assert!(!lockfile["packages"]
        .as_object()
        .unwrap()
        .contains_key("router@2026-04-05T15-00-00Z"));
}

#[test]
fn package_publish_uses_derived_npm_identity() {
    let registry_dir = temp_dir("publish-registry");
    let fake_npm_dir = temp_dir("publish-fake-npm");
    write_fake_npm(&fake_npm_dir);

    let package_dir = temp_dir("publish-package");
    write_file(
        &package_dir,
        "sigil.json",
        "{\n  \"name\": \"router\",\n  \"version\": \"2026-04-05T14-58-24Z\",\n  \"publish\": {}\n}\n",
    );
    write_file(
        &package_dir,
        "src/package.lib.sigil",
        "λdouble(value:Int)=>Int=value*2\n",
    );
    write_file(
        &package_dir,
        "tests/main.sigil",
        "λmain()=>Unit=()\n\ntest \"double\" {\n  •package.double(2)=4\n}\n",
    );

    let mut publish = Command::new(sigil_bin());
    publish
        .current_dir(&package_dir)
        .args(["package", "publish"]);
    npm_env(&mut publish, &fake_npm_dir, &registry_dir);
    let publish_output = publish.output().unwrap();
    assert!(publish_output.status.success(), "{:?}", publish_output);

    let published_package_json: Value = serde_json::from_str(
        &fs::read_to_string(registry_dir.join("published/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(published_package_json["name"], "router");
    assert_eq!(published_package_json["version"], "20260405.145824.0");
}

#[test]
fn package_validate_smoke_tests_local_publishability() {
    let fake_npm_dir = temp_dir("validate-fake-npm");
    let registry_dir = temp_dir("validate-registry");
    write_fake_npm(&fake_npm_dir);

    let package_dir = temp_dir("validate-package");
    write_file(
        &package_dir,
        "sigil.json",
        "{\n  \"name\": \"router\",\n  \"version\": \"2026-04-05T14-58-24Z\",\n  \"publish\": {}\n}\n",
    );
    write_file(
        &package_dir,
        "src/package.lib.sigil",
        "λdouble(value:Int)=>Int=value*2\n",
    );
    write_file(
        &package_dir,
        "tests/main.sigil",
        "λmain()=>Unit=()\n\ntest \"double\" {\n  •package.double(2)=4\n}\n",
    );

    let mut validate = Command::new(sigil_bin());
    validate
        .current_dir(&package_dir)
        .args(["package", "validate"]);
    npm_env(&mut validate, &fake_npm_dir, &registry_dir);
    let validate_output = validate.output().unwrap();
    assert!(validate_output.status.success(), "{:?}", validate_output);

    let validate_json = parse_json(&validate_output.stdout);
    assert_eq!(validate_json["command"], "sigil package validate");
    assert_eq!(validate_json["ok"], true);
    assert_eq!(validate_json["phase"], "package");
    assert_eq!(validate_json["data"]["project"], "router");
    assert_eq!(validate_json["data"]["npmPackage"], "router");
    assert_eq!(validate_json["data"]["npmVersion"], "20260405.145824.0");
}

#[test]
fn package_install_uses_local_workspace_publishable_package_before_registry() {
    let registry_dir = temp_dir("local-workspace-registry");
    let fake_npm_dir = temp_dir("local-workspace-fake-npm");
    write_fake_npm(&fake_npm_dir);

    let workspace_root = external_temp_dir("local-workspace-root");
    fs::create_dir_all(workspace_root.join(".git")).unwrap();

    let package_dir = workspace_root.join("projects").join("router");
    write_file(
        &package_dir,
        "sigil.json",
        "{\n  \"name\": \"router\",\n  \"version\": \"2026-04-05T14-58-24Z\",\n  \"publish\": {}\n}\n",
    );
    write_file(
        &package_dir,
        "src/package.lib.sigil",
        "λdouble(value:Int)=>Int=value*2\n",
    );

    let consumer_dir = workspace_root.join("projects").join("consumer");
    write_file(
        &consumer_dir,
        "sigil.json",
        "{\n  \"name\": \"consumerApp\",\n  \"version\": \"2026-04-05T15-00-00Z\",\n  \"dependencies\": {\n    \"router\": \"2026-04-05T14-58-24Z\"\n  }\n}\n",
    );
    write_file(
        &consumer_dir,
        "src/main.sigil",
        "λmain()=>Int=☴router.double(21)\n",
    );

    let mut install = Command::new(sigil_bin());
    install
        .current_dir(&consumer_dir)
        .args(["package", "install"]);
    npm_env(&mut install, &fake_npm_dir, &registry_dir);
    let install_output = install.output().unwrap();
    assert!(install_output.status.success(), "{:?}", install_output);

    assert!(consumer_dir
        .join(".sigil/packages/router/2026-04-05T14-58-24Z/src/package.lib.sigil")
        .exists());

    let mut compile = Command::new(sigil_bin());
    compile
        .current_dir(&consumer_dir)
        .args(["compile", "src/main.sigil"]);
    npm_env(&mut compile, &fake_npm_dir, &registry_dir);
    let compile_output = compile.output().unwrap();
    assert!(compile_output.status.success(), "{:?}", compile_output);
}
