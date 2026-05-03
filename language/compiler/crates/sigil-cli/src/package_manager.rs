use crate::commands::CliError;
use crate::hash::encode_lower_hex;
use crate::module_graph::collect_referenced_module_ids;
use crate::project::{
    get_project_config, get_project_config_at_root, is_lower_camel_name,
    sigil_name_to_npm_package_name, sigil_version_to_npm_version, write_project_manifest,
    ProjectConfig, ProjectManifest,
};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const LOCKFILE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageLockfile {
    format_version: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    root_dependencies: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    packages: BTreeMap<String, LockedPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LockedPackage {
    name: String,
    sigil_version: String,
    npm_package: String,
    npm_version: String,
    integrity: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct NpmPackEntry {
    filename: String,
    #[serde(default)]
    integrity: Option<String>,
}

struct InstalledPackage {
    project: ProjectConfig,
    npm_package: String,
    npm_version: String,
    integrity: String,
}

struct ProjectStateBackup {
    manifest_text: String,
    lockfile_text: Option<String>,
    package_store_backup: Option<PathBuf>,
}

pub fn package_add_command(path: &Path, name: &str) -> Result<(), CliError> {
    if !is_lower_camel_name(name) {
        return Err(CliError::Validation(
            "package names must use lowerCamel with ASCII letters and digits only".to_string(),
        ));
    }

    let project = require_project(path)?;
    let mut manifest = project.manifest();
    let latest_version = latest_registry_version(name)?;
    manifest
        .dependencies
        .insert(name.to_string(), latest_version.clone());

    apply_manifest_with_install(&project.root, &manifest)?;
    print_package_success(
        "sigil package add",
        json!({
            "project": project.name,
            "dependency": name,
            "version": latest_version
        }),
    );
    Ok(())
}

pub fn package_install_command(path: &Path) -> Result<(), CliError> {
    let project = require_project(path)?;
    install_manifest_dependencies(&project, true)?;
    print_package_success(
        "sigil package install",
        json!({
            "project": project.name,
            "dependencies": project.dependencies
        }),
    );
    Ok(())
}

pub fn package_update_command(
    path: &Path,
    dependency_name: Option<&str>,
    keep_failing: bool,
) -> Result<(), CliError> {
    let project = require_project(path)?;
    let mut manifest = project.manifest();
    let target_names = if let Some(name) = dependency_name {
        if !manifest.dependencies.contains_key(name) {
            return Err(CliError::Validation(format!(
                "direct dependency `{name}` is not declared in sigil.json"
            )));
        }
        vec![name.to_string()]
    } else {
        manifest.dependencies.keys().cloned().collect::<Vec<_>>()
    };

    let mut updated = Vec::new();
    for name in target_names {
        let current_version = manifest
            .dependencies
            .get(&name)
            .cloned()
            .ok_or_else(|| CliError::Validation(format!("missing dependency `{name}`")))?;
        let latest_version = latest_registry_version(&name)?;
        if latest_version > current_version {
            manifest
                .dependencies
                .insert(name.clone(), latest_version.clone());
            updated.push(json!({
                "dependency": name,
                "from": current_version,
                "to": latest_version
            }));
        }
    }

    if updated.is_empty() {
        print_package_success(
            "sigil package update",
            json!({
                "project": project.name,
                "updated": []
            }),
        );
        return Ok(());
    }

    let backup = backup_project_state(&project.root)?;
    if let Err(error) = write_project_manifest(&project.root, &manifest) {
        restore_project_state(&project.root, backup)?;
        return Err(CliError::ProjectConfig(error));
    }

    if let Err(error) = install_manifest_dependencies(&require_project(&project.root)?, false) {
        restore_project_state(&project.root, backup)?;
        return Err(error);
    }

    match run_project_tests(&project.root)? {
        TestRunResult::Passed => {
            discard_backup(backup)?;
            print_package_success(
                "sigil package update",
                json!({
                    "project": project.name,
                    "updated": updated,
                    "tests": {
                        "ok": true
                    }
                }),
            );
            Ok(())
        }
        TestRunResult::Failed { output } => {
            if keep_failing {
                discard_backup(backup)?;
                Err(CliError::Validation(format!(
                    "dependency update kept failing versions after test failure:\n{}",
                    output.trim()
                )))
            } else {
                restore_project_state(&project.root, backup)?;
                Err(CliError::Validation(format!(
                    "dependency update failed project tests and was rolled back:\n{}",
                    output.trim()
                )))
            }
        }
    }
}

pub fn package_remove_command(path: &Path, name: &str) -> Result<(), CliError> {
    let project = require_project(path)?;
    let mut manifest = project.manifest();
    if manifest.dependencies.remove(name).is_none() {
        return Err(CliError::Validation(format!(
            "direct dependency `{name}` is not declared in sigil.json"
        )));
    }

    apply_manifest_with_install(&project.root, &manifest)?;
    print_package_success(
        "sigil package remove",
        json!({
            "project": project.name,
            "dependency": name
        }),
    );
    Ok(())
}

pub fn package_list_command(path: &Path) -> Result<(), CliError> {
    let project = require_project(path)?;
    let lockfile = load_lockfile(&project.root)?;
    let dependencies = project
        .dependencies
        .iter()
        .map(|(name, version)| {
            let lock_key = package_key(name, version);
            let installed = lockfile
                .as_ref()
                .is_some_and(|lock| lock.packages.contains_key(&lock_key));
            json!({
                "dependency": name,
                "version": version,
                "installed": installed
            })
        })
        .collect::<Vec<_>>();

    print_package_success(
        "sigil package list",
        json!({
            "project": project.name,
            "dependencies": dependencies
        }),
    );
    Ok(())
}

pub fn package_why_command(path: &Path, name: &str) -> Result<(), CliError> {
    let project = require_project(path)?;
    let lockfile = load_lockfile(&project.root)?.ok_or_else(|| {
        CliError::Validation("sigil.lock not found; run `sigil package install` first".to_string())
    })?;

    let mut paths = Vec::new();
    for (dependency_name, version) in &lockfile.root_dependencies {
        let mut current_path = vec![format!("{dependency_name}@{version}")];
        if dependency_name == name {
            paths.push(current_path.clone());
        }
        collect_dependency_paths(
            &lockfile,
            dependency_name,
            version,
            name,
            &mut current_path,
            &mut paths,
        );
    }

    if paths.is_empty() {
        return Err(CliError::Validation(format!(
            "package `{name}` is not present in sigil.lock"
        )));
    }

    print_package_success(
        "sigil package why",
        json!({
            "project": project.name,
            "package": name,
            "paths": paths
        }),
    );
    Ok(())
}

pub fn package_publish_command(path: &Path) -> Result<(), CliError> {
    let project = require_project(path)?;
    if !project.is_publishable_package() {
        return Err(CliError::Validation(
            "sigil package publish requires `publish` in sigil.json".to_string(),
        ));
    }

    let npm_name = npm_package_name(&project.name)?;
    let npm_version = npm_transport_version(&project.version)?;
    validate_publishable_package(&project, "sigil package publish")?;
    let publish_dir =
        prepare_publish_dir(&project, &npm_name, &npm_version, "sigil-package-publish")?;

    let output = Command::new("npm")
        .current_dir(&publish_dir)
        .arg("publish")
        .output()
        .map_err(|error| CliError::Runtime(format!("failed to run npm publish: {error}")))?;

    if !output.status.success() {
        return Err(CliError::Runtime(format!(
            "npm publish failed: {}",
            combined_output(&output.stdout, &output.stderr)
        )));
    }

    print_package_success(
        "sigil package publish",
        json!({
            "project": project.name,
            "version": project.version,
            "npmPackage": npm_name,
            "npmVersion": npm_version
        }),
    );
    Ok(())
}

pub fn package_validate_command(path: &Path) -> Result<(), CliError> {
    let project = require_project(path)?;
    if !project.is_publishable_package() {
        return Err(CliError::Validation(
            "sigil package validate requires `publish` in sigil.json".to_string(),
        ));
    }

    let npm_name = npm_package_name(&project.name)?;
    let npm_version = npm_transport_version(&project.version)?;
    validate_publishable_package(&project, "sigil package validate")?;

    let publish_dir =
        prepare_publish_dir(&project, &npm_name, &npm_version, "sigil-package-validate")?;
    let tarball_path = pack_publish_dir(&publish_dir)?;
    let unpack_root = project_temp_dir(&project.root, "package-validate-unpack")?;
    unpack_package_archive(&tarball_path, &unpack_root)?;
    let installed_root = unpack_root.join("package");
    validate_staged_package(&installed_root)?;

    print_package_success(
        "sigil package validate",
        json!({
            "project": project.name,
            "version": project.version,
            "npmPackage": npm_name,
            "npmVersion": npm_version
        }),
    );
    Ok(())
}

fn validate_public_package_modules(project: &ProjectConfig) -> Result<(), CliError> {
    let src_dir = project.root.join("src");
    let public_files = collect_sigil_library_files(&src_dir)?;
    for file in public_files {
        let source = fs::read_to_string(&file)?;
        let mut lexer = Lexer::new(&source);
        let tokens = lexer
            .tokenize()
            .map_err(|error| CliError::Lexer(error.to_string()))?;
        let mut parser = Parser::new(tokens, file.to_string_lossy().to_string());
        let ast = parser
            .parse()
            .map_err(|error| CliError::Parser(error.to_string()))?;
        let package_refs = collect_referenced_module_ids(&ast)
            .into_iter()
            .filter(|module_id| module_id.starts_with("package::"))
            .collect::<Vec<_>>();
        if !package_refs.is_empty() {
            return Err(CliError::Validation(format!(
                "public package modules may not reference third-party packages in v1: {} ({})",
                file.display(),
                package_refs.join(", ")
            )));
        }
    }
    Ok(())
}

fn validate_publishable_package(
    project: &ProjectConfig,
    command_name: &str,
) -> Result<(), CliError> {
    validate_public_package_modules(project)?;

    match run_project_tests(&project.root)? {
        TestRunResult::Passed => Ok(()),
        TestRunResult::Failed { output } => Err(CliError::Validation(format!(
            "{command_name} requires passing project tests:\n{}",
            output.trim()
        ))),
    }
}

fn collect_sigil_library_files(root: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_sigil_library_files(&path)?);
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".lib.sigil"))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn collect_dependency_paths(
    lockfile: &PackageLockfile,
    dependency_name: &str,
    dependency_version: &str,
    target_name: &str,
    current_path: &mut Vec<String>,
    paths: &mut Vec<Vec<String>>,
) {
    let lock_key = package_key(dependency_name, dependency_version);
    let Some(package) = lockfile.packages.get(&lock_key) else {
        return;
    };

    for (child_name, child_version) in &package.dependencies {
        current_path.push(format!("{child_name}@{child_version}"));
        if child_name == target_name {
            paths.push(current_path.clone());
        }
        collect_dependency_paths(
            lockfile,
            child_name,
            child_version,
            target_name,
            current_path,
            paths,
        );
        current_path.pop();
    }
}

fn require_project(path: &Path) -> Result<ProjectConfig, CliError> {
    get_project_config(path)?
        .ok_or_else(|| CliError::Validation("sigil.json not found".to_string()))
}

fn npm_package_name(name: &str) -> Result<String, CliError> {
    sigil_name_to_npm_package_name(name).ok_or_else(|| {
        CliError::Validation(format!(
            "package name `{name}` must use lowerCamel with ASCII letters and digits only"
        ))
    })
}

fn npm_transport_version(version: &str) -> Result<String, CliError> {
    sigil_version_to_npm_version(version).ok_or_else(|| {
        CliError::Validation(format!(
            "package version `{version}` must use canonical UTC timestamp format"
        ))
    })
}

fn prepare_publish_dir(
    project: &ProjectConfig,
    npm_name: &str,
    npm_version: &str,
    label: &str,
) -> Result<PathBuf, CliError> {
    let publish_dir = temp_dir(label)?;
    copy_publish_inputs(&project.root, &publish_dir)?;
    write_package_transport_manifest(&publish_dir, npm_name, npm_version)?;
    Ok(publish_dir)
}

fn write_package_transport_manifest(
    publish_dir: &Path,
    npm_name: &str,
    npm_version: &str,
) -> Result<(), CliError> {
    let package_json = json!({
        "name": npm_name,
        "version": npm_version,
        "private": false
    });
    fs::write(
        publish_dir.join("package.json"),
        format!("{}\n", serde_json::to_string_pretty(&package_json).unwrap()),
    )?;
    Ok(())
}

fn pack_publish_dir(publish_dir: &Path) -> Result<PathBuf, CliError> {
    let output = Command::new("npm")
        .current_dir(publish_dir)
        .args(["pack", "--json"])
        .output()
        .map_err(|error| CliError::Runtime(format!("failed to run npm pack: {error}")))?;
    if !output.status.success() {
        return Err(CliError::Runtime(format!(
            "npm pack failed: {}",
            combined_output(&output.stdout, &output.stderr)
        )));
    }

    let entries: Vec<NpmPackEntry> = serde_json::from_slice(&output.stdout).map_err(|error| {
        CliError::Validation(format!("npm pack returned invalid JSON: {error}"))
    })?;
    let entry = entries.first().ok_or_else(|| {
        CliError::Validation("npm pack returned no archive information".to_string())
    })?;
    Ok(publish_dir.join(&entry.filename))
}

fn unpack_package_archive(tarball_path: &Path, unpack_root: &Path) -> Result<(), CliError> {
    let tar_output = Command::new("tar")
        .arg("-xzf")
        .arg(tarball_path)
        .arg("-C")
        .arg(unpack_root)
        .output()
        .map_err(|error| CliError::Runtime(format!("failed to extract npm package: {error}")))?;
    if !tar_output.status.success() {
        return Err(CliError::Runtime(format!(
            "failed to extract npm package: {}",
            combined_output(&tar_output.stdout, &tar_output.stderr)
        )));
    }
    Ok(())
}

fn validate_staged_package(root: &Path) -> Result<(), CliError> {
    let project = get_project_config_at_root(root)?.ok_or_else(|| {
        CliError::Validation(format!(
            "packaged project is missing {}",
            root.join("sigil.json").display()
        ))
    })?;
    validate_public_package_modules(&project)?;
    for file in collect_sigil_library_files(&root.join("src"))? {
        let relative = file.strip_prefix(root).map_err(|error| {
            CliError::Validation(format!("invalid packaged module path: {error}"))
        })?;
        run_compile_in_project(root, relative.to_string_lossy().as_ref())?;
    }
    Ok(())
}

fn run_compile_in_project(root: &Path, target: &str) -> Result<(), CliError> {
    let output = Command::new(std::env::current_exe()?)
        .current_dir(root)
        .args(["compile", target])
        .output()
        .map_err(|error| {
            CliError::Runtime(format!("failed to compile packaged project: {error}"))
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(CliError::Validation(format!(
            "packaged project compile failed:\n{}",
            combined_output(&output.stdout, &output.stderr).trim()
        )))
    }
}

fn package_key(name: &str, version: &str) -> String {
    format!("{name}@{version}")
}

fn print_package_success(command: &str, data: Value) {
    println!(
        "{}",
        serde_json::to_string(&json!({
            "formatVersion": 1,
            "command": command,
            "ok": true,
            "phase": "package",
            "data": data
        }))
        .unwrap()
    );
}

fn lockfile_path(root: &Path) -> PathBuf {
    root.join("sigil.lock")
}

fn load_lockfile(root: &Path) -> Result<Option<PackageLockfile>, CliError> {
    let path = lockfile_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)?;
    let lockfile = serde_json::from_str(&text)
        .map_err(|error| CliError::Validation(format!("invalid sigil.lock: {error}")))?;
    Ok(Some(lockfile))
}

fn write_lockfile(root: &Path, lockfile: &PackageLockfile) -> Result<(), CliError> {
    fs::write(
        lockfile_path(root),
        format!("{}\n", serde_json::to_string_pretty(lockfile).unwrap()),
    )?;
    Ok(())
}

fn apply_manifest_with_install(root: &Path, manifest: &ProjectManifest) -> Result<(), CliError> {
    let backup = backup_project_state(root)?;
    if let Err(error) = write_project_manifest(root, manifest) {
        restore_project_state(root, backup)?;
        return Err(CliError::ProjectConfig(error));
    }

    let project = match require_project(root) {
        Ok(project) => project,
        Err(error) => {
            restore_project_state(root, backup)?;
            return Err(error);
        }
    };

    match install_manifest_dependencies(&project, false) {
        Ok(()) => {
            discard_backup(backup)?;
            Ok(())
        }
        Err(error) => {
            restore_project_state(root, backup)?;
            Err(error)
        }
    }
}

fn install_manifest_dependencies(
    project: &ProjectConfig,
    use_backup: bool,
) -> Result<(), CliError> {
    let backup = if use_backup {
        Some(backup_project_state(&project.root)?)
    } else {
        None
    };

    let result = install_manifest_dependencies_inner(project);
    match (result, backup) {
        (Ok(()), Some(backup)) => {
            discard_backup(backup)?;
            Ok(())
        }
        (Ok(()), None) => Ok(()),
        (Err(error), Some(backup)) => {
            restore_project_state(&project.root, backup)?;
            Err(error)
        }
        (Err(error), None) => Err(error),
    }
}

fn install_manifest_dependencies_inner(project: &ProjectConfig) -> Result<(), CliError> {
    let package_store_root = project.package_store_root();
    if package_store_root.exists() {
        fs::remove_dir_all(&package_store_root)?;
    }
    fs::create_dir_all(&package_store_root)?;

    let mut lockfile = PackageLockfile {
        format_version: LOCKFILE_FORMAT_VERSION,
        root_dependencies: project.dependencies.clone(),
        packages: BTreeMap::new(),
    };

    let mut stack = Vec::new();
    for (dependency_name, dependency_version) in &project.dependencies {
        install_dependency_tree(
            &project.root,
            dependency_name,
            dependency_version,
            &mut lockfile,
            &mut stack,
        )?;
    }

    write_lockfile(&project.root, &lockfile)?;
    Ok(())
}

fn install_dependency_tree(
    parent_root: &Path,
    dependency_name: &str,
    dependency_version: &str,
    lockfile: &mut PackageLockfile,
    stack: &mut Vec<String>,
) -> Result<(), CliError> {
    let cycle_key = package_key(dependency_name, dependency_version);
    if stack.contains(&cycle_key) {
        return Err(CliError::Validation(format!(
            "package dependency cycle detected: {} -> {}",
            stack.join(" -> "),
            cycle_key
        )));
    }

    let install_root = parent_root
        .join(".sigil")
        .join("packages")
        .join(dependency_name)
        .join(dependency_version);
    if install_root.exists() {
        fs::remove_dir_all(&install_root)?;
    }
    fs::create_dir_all(install_root.parent().unwrap())?;

    let installed_package = fetch_and_unpack_package(
        parent_root,
        dependency_name,
        dependency_version,
        &install_root,
    )?;
    stack.push(cycle_key.clone());
    for (child_name, child_version) in &installed_package.project.dependencies {
        install_dependency_tree(&install_root, child_name, child_version, lockfile, stack)?;
    }
    stack.pop();

    lockfile.packages.entry(cycle_key).or_insert(LockedPackage {
        name: installed_package.project.name.clone(),
        sigil_version: installed_package.project.version.clone(),
        npm_package: installed_package.npm_package,
        npm_version: installed_package.npm_version,
        integrity: installed_package.integrity,
        dependencies: installed_package.project.dependencies.clone(),
    });
    Ok(())
}

fn fetch_and_unpack_package(
    parent_root: &Path,
    dependency_name: &str,
    dependency_version: &str,
    install_root: &Path,
) -> Result<InstalledPackage, CliError> {
    let npm_name = npm_package_name(dependency_name)?;
    let npm_version = npm_transport_version(dependency_version)?;

    if let Some(local_project) =
        find_local_publishable_package(parent_root, dependency_name, dependency_version)?
    {
        let publish_dir = prepare_publish_dir(
            &local_project,
            &npm_name,
            &npm_version,
            "sigil-package-local-pack",
        )?;
        let tarball_path = pack_publish_dir(&publish_dir)?;
        let integrity = sha256_hex(&tarball_path)?;
        let project = install_archive_into_root(
            &tarball_path,
            install_root,
            dependency_name,
            dependency_version,
        )?;

        return Ok(InstalledPackage {
            project,
            npm_package: npm_name,
            npm_version,
            integrity,
        });
    }

    let pack_spec = format!("{npm_name}@{npm_version}");
    let pack_dir = temp_dir("sigil-package-pack")?;

    let output = Command::new("npm")
        .current_dir(&pack_dir)
        .args(["pack", pack_spec.as_str(), "--json"])
        .output()
        .map_err(|error| CliError::Runtime(format!("failed to run npm pack: {error}")))?;
    if !output.status.success() {
        return Err(CliError::Runtime(format!(
            "npm pack failed: {}",
            combined_output(&output.stdout, &output.stderr)
        )));
    }

    let entries: Vec<NpmPackEntry> = serde_json::from_slice(&output.stdout).map_err(|error| {
        CliError::Validation(format!("npm pack returned invalid JSON: {error}"))
    })?;
    let entry = entries.first().ok_or_else(|| {
        CliError::Validation("npm pack returned no archive information".to_string())
    })?;

    let tarball_path = pack_dir.join(&entry.filename);
    let project = install_archive_into_root(
        &tarball_path,
        install_root,
        dependency_name,
        dependency_version,
    )?;

    let integrity = match &entry.integrity {
        Some(integrity) => integrity.clone(),
        None => sha256_hex(&tarball_path)?,
    };

    Ok(InstalledPackage {
        project,
        npm_package: npm_name,
        npm_version,
        integrity,
    })
}

fn install_archive_into_root(
    tarball_path: &Path,
    install_root: &Path,
    dependency_name: &str,
    dependency_version: &str,
) -> Result<ProjectConfig, CliError> {
    let unpack_dir = temp_dir("sigil-package-unpack")?;
    unpack_package_archive(tarball_path, &unpack_dir)?;

    let extracted_root = unpack_dir.join("package");
    copy_dir_recursive(&extracted_root, install_root)?;

    let project = get_project_config_at_root(install_root)?.ok_or_else(|| {
        CliError::Validation(format!(
            "installed package is missing {}",
            install_root.join("sigil.json").display()
        ))
    })?;
    if project.name != dependency_name {
        return Err(CliError::Validation(format!(
            "installed package `{}` declared name `{}` instead of `{}`",
            install_root.display(),
            project.name,
            dependency_name
        )));
    }
    if project.version != dependency_version {
        return Err(CliError::Validation(format!(
            "installed package `{}` declared version `{}` instead of `{}`",
            install_root.display(),
            project.version,
            dependency_version
        )));
    }
    Ok(project)
}

fn find_local_publishable_package(
    start_root: &Path,
    dependency_name: &str,
    dependency_version: &str,
) -> Result<Option<ProjectConfig>, CliError> {
    let Some(workspace_root) = find_git_workspace_root(start_root) else {
        return Ok(None);
    };

    let mut matches = Vec::new();
    let mut walker = WalkBuilder::new(&workspace_root);
    walker.hidden(false);
    walker.git_ignore(false);
    walker.git_exclude(false);
    walker.git_global(false);
    let workspace_root_for_filter = workspace_root.clone();
    walker.filter_entry(move |entry| {
        !is_nested_git_workspace(entry.path(), &workspace_root_for_filter)
    });
    let walker = walker.build();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                return Err(CliError::Runtime(format!(
                    "failed to scan local workspace packages: {error}"
                )));
            }
        };
        if entry
            .file_type()
            .is_none_or(|file_type| !file_type.is_file())
        {
            continue;
        }
        if entry.file_name() != "sigil.json" {
            continue;
        }

        let Some(project_root) = entry.path().parent() else {
            continue;
        };
        if should_skip_local_package_candidate(project_root) {
            continue;
        }

        let Some(project) = get_project_config(project_root)? else {
            continue;
        };
        if !project.is_publishable_package() {
            continue;
        }
        if project.name == dependency_name && project.version == dependency_version {
            matches.push(project);
        }
    }

    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.pop()),
        _ => Err(CliError::Validation(format!(
            "multiple local publishable packages match `{dependency_name}@{dependency_version}`: {}",
            matches
                .iter()
                .map(|project| project.root.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn should_skip_local_package_candidate(path: &Path) -> bool {
    path.components().any(|component| {
        let segment = component.as_os_str().to_string_lossy();
        matches!(
            segment.as_ref(),
            ".git" | ".local" | ".sigil" | "node_modules" | "target"
        )
    })
}

fn is_nested_git_workspace(path: &Path, workspace_root: &Path) -> bool {
    path != workspace_root && path.is_dir() && path.join(".git").exists()
}

fn find_git_workspace_root(start_root: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(start_root).ok()?;
    canonical
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf)
}

fn latest_registry_version(dependency_name: &str) -> Result<String, CliError> {
    let npm_name = npm_package_name(dependency_name)?;
    let output = Command::new("npm")
        .args(["view", npm_name.as_str(), "versions", "--json"])
        .output()
        .map_err(|error| CliError::Runtime(format!("failed to run npm view: {error}")))?;

    if !output.status.success() {
        return Err(CliError::Runtime(format!(
            "npm view failed: {}",
            combined_output(&output.stdout, &output.stderr)
        )));
    }

    let versions_value: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        CliError::Validation(format!("npm view returned invalid JSON: {error}"))
    })?;
    let versions = if let Some(version) = versions_value.as_str() {
        vec![version.to_string()]
    } else {
        versions_value
            .as_array()
            .ok_or_else(|| {
                CliError::Validation("npm view returned an unexpected versions payload".to_string())
            })?
            .iter()
            .filter_map(|value| value.as_str().map(ToString::to_string))
            .collect::<Vec<_>>()
    };

    let mut sigil_versions = versions
        .into_iter()
        .filter_map(|version| crate::project::npm_version_to_sigil_version(&version))
        .collect::<Vec<_>>();
    sigil_versions.sort();
    sigil_versions.pop().ok_or_else(|| {
        CliError::Validation(format!(
            "npm registry did not return any canonical Sigil versions for `{dependency_name}`"
        ))
    })
}

fn temp_dir(label: &str) -> Result<PathBuf, CliError> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sigil-{label}-{unique}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn project_temp_dir(root: &Path, label: &str) -> Result<PathBuf, CliError> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = root.join(".sigil").join(format!("sigil-{label}-{unique}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn sha256_hex(path: &Path) -> Result<String, CliError> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{}", encode_lower_hex(hasher.finalize())))
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), CliError> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = to.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &dest_path)?;
        }
    }
    Ok(())
}

fn copy_publish_inputs(project_root: &Path, publish_dir: &Path) -> Result<(), CliError> {
    for file_name in ["sigil.json", "README.md"] {
        let source = project_root.join(file_name);
        if source.exists() {
            fs::copy(&source, publish_dir.join(file_name))?;
        }
    }

    for dir_name in ["src", "tests", "config"] {
        let source = project_root.join(dir_name);
        if source.exists() {
            copy_dir_recursive(&source, &publish_dir.join(dir_name))?;
        }
    }
    Ok(())
}

fn backup_project_state(root: &Path) -> Result<ProjectStateBackup, CliError> {
    let sigil_dir = root.join(".sigil");
    fs::create_dir_all(&sigil_dir)?;
    let manifest_text = fs::read_to_string(root.join("sigil.json"))?;
    let lockfile_text = fs::read_to_string(lockfile_path(root)).ok();

    let package_store = sigil_dir.join("packages");
    let package_store_backup = if package_store.exists() {
        let backup = sigil_dir.join(format!(
            "packages.backup.{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::rename(&package_store, &backup)?;
        Some(backup)
    } else {
        None
    };

    Ok(ProjectStateBackup {
        manifest_text,
        lockfile_text,
        package_store_backup,
    })
}

fn restore_project_state(root: &Path, backup: ProjectStateBackup) -> Result<(), CliError> {
    fs::write(root.join("sigil.json"), backup.manifest_text)?;
    match backup.lockfile_text {
        Some(lockfile_text) => fs::write(lockfile_path(root), lockfile_text)?,
        None => {
            let path = lockfile_path(root);
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
    }

    let package_store = root.join(".sigil").join("packages");
    if package_store.exists() {
        fs::remove_dir_all(&package_store)?;
    }
    if let Some(package_store_backup) = backup.package_store_backup {
        if let Some(parent) = package_store.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(package_store_backup, package_store)?;
    }
    Ok(())
}

fn discard_backup(backup: ProjectStateBackup) -> Result<(), CliError> {
    if let Some(package_store_backup) = backup.package_store_backup {
        if package_store_backup.exists() {
            fs::remove_dir_all(package_store_backup)?;
        }
    }
    Ok(())
}

enum TestRunResult {
    Passed,
    Failed { output: String },
}

fn run_project_tests(root: &Path) -> Result<TestRunResult, CliError> {
    let topology_present = root.join("src/topology.lib.sigil").exists();
    let output = if topology_present {
        Command::new(std::env::current_exe()?)
            .current_dir(root)
            .args(["test", "tests", "--env", "test"])
            .output()
            .map_err(|error| CliError::Runtime(format!("failed to run project tests: {error}")))?
    } else {
        Command::new(std::env::current_exe()?)
            .current_dir(root)
            .args(["test", "tests"])
            .output()
            .map_err(|error| CliError::Runtime(format!("failed to run project tests: {error}")))?
    };

    if output.status.success() {
        Ok(TestRunResult::Passed)
    } else {
        Ok(TestRunResult::Failed {
            output: combined_output(&output.stdout, &output.stderr),
        })
    }
}

fn combined_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout_text = String::from_utf8_lossy(stdout);
    let stderr_text = String::from_utf8_lossy(stderr);
    match (stdout_text.trim(), stderr_text.trim()) {
        ("", "") => String::new(),
        ("", stderr_text) => stderr_text.to_string(),
        (stdout_text, "") => stdout_text.to_string(),
        (stdout_text, stderr_text) => format!("{stdout_text}\n{stderr_text}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{package_key, PackageLockfile};
    use crate::project::{npm_version_to_sigil_version, sigil_version_to_npm_version};

    #[test]
    fn lockfile_uses_canonical_package_keys() {
        assert_eq!(
            package_key("router", "2026-04-05T14-58-24Z"),
            "router@2026-04-05T14-58-24Z"
        );
    }

    #[test]
    fn lockfile_round_trips_transport_versions() {
        let npm_version = sigil_version_to_npm_version("2026-04-05T14-58-24Z").unwrap();
        assert_eq!(
            npm_version_to_sigil_version(&npm_version).unwrap(),
            "2026-04-05T14-58-24Z"
        );
    }

    #[test]
    fn empty_lockfile_serializes_with_format_version() {
        let lockfile = PackageLockfile {
            format_version: 1,
            root_dependencies: Default::default(),
            packages: Default::default(),
        };

        let text = serde_json::to_string(&lockfile).unwrap();
        assert!(text.contains("\"formatVersion\":1"));
    }
}
