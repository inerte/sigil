use super::legacy::CliError;
use super::shared::{
    extract_error_code, format_validation_errors, output_json_error, phase_for_code,
    project_error_json_details, type_error_json_details, SourcePoint,
};
use crate::module_graph::{
    entry_module_key, load_project_effect_catalog_for, LoadedModule, ModuleGraph, ModuleGraphError,
};
use crate::project::{get_project_config, ProjectConfig};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde_json::json;
use sha2::{Digest, Sha256};
use sigil_ast::{Declaration, LabelRef, Program, Type, TypeDef};
use sigil_codegen::{collect_module_span_map, CodegenOptions, ModuleSpanMap, TypeScriptGenerator};
use sigil_diagnostics::codes;
use sigil_typechecker::types::{
    InferenceType, TConstructor, TFunction, TList, TMap, TRecord, TTuple,
};
use sigil_typechecker::{
    type_check, BindingMeta, BoundaryRule, LabelInfo, TypeCheckOptions, TypeInfo, TypeScheme,
    TypedDeclaration, TypedProgram,
};
use sigil_validator::validate_typed_canonical_form;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct CompileDirectoryIgnore {
    root: PathBuf,
    explicit_paths: Vec<PathBuf>,
    gitignore: Option<Gitignore>,
}

impl CompileDirectoryIgnore {
    fn new(
        root: &Path,
        ignore_paths: &[PathBuf],
        ignore_from: Option<&Path>,
    ) -> Result<Self, CliError> {
        let root = fs::canonicalize(root)?;
        let explicit_paths = ignore_paths
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    Ok(path.to_path_buf())
                } else {
                    Ok(root.join(path))
                }
            })
            .collect::<Result<Vec<_>, std::io::Error>>()?;

        let gitignore = if let Some(ignore_from) = ignore_from {
            let resolved_ignore_from = if ignore_from.is_absolute() {
                ignore_from.to_path_buf()
            } else {
                std::env::current_dir()?.join(ignore_from)
            };
            let mut builder = GitignoreBuilder::new(&root);
            if let Some(error) = builder.add(&resolved_ignore_from) {
                return Err(CliError::Validation(format!(
                    "failed to load ignore rules from '{}': {}",
                    resolved_ignore_from.display(),
                    error
                )));
            }
            Some(builder.build().map_err(|error| {
                CliError::Validation(format!(
                    "failed to parse ignore rules from '{}': {}",
                    resolved_ignore_from.display(),
                    error
                ))
            })?)
        } else {
            None
        };

        Ok(Self {
            root,
            explicit_paths,
            gitignore,
        })
    }

    fn should_ignore(&self, path: &Path, is_dir: bool) -> bool {
        if let Ok(relative) = path.strip_prefix(&self.root) {
            if relative.components().any(|component| {
                matches!(
                    component.as_os_str().to_string_lossy().as_ref(),
                    ".git" | ".local" | ".sigil" | "node_modules" | "target"
                )
            }) {
                return true;
            }
        }

        if self
            .explicit_paths
            .iter()
            .any(|ignore| path.starts_with(ignore))
        {
            return true;
        }

        self.gitignore.as_ref().is_some_and(|gitignore| {
            gitignore
                .matched_path_or_any_parents(path, is_dir)
                .is_ignore()
        })
    }
}

#[derive(Clone)]
pub(super) struct CompileBatchGroup {
    first_index: usize,
    pub files: Vec<PathBuf>,
}

struct CompileEntryOutput {
    input: PathBuf,
    output: PathBuf,
    span_map: PathBuf,
    project_root: Option<PathBuf>,
}

struct CompileBatchOutputs {
    compiled_modules: usize,
    entries: Vec<CompileEntryOutput>,
}

#[derive(Clone)]
pub(super) struct AnalyzedModule {
    pub module_id: String,
    pub file_path: PathBuf,
    pub project: Option<ProjectConfig>,
    pub ast: Program,
    pub typed_program: TypedProgram,
    pub declaration_types: HashMap<String, InferenceType>,
    pub declaration_schemes: HashMap<String, TypeScheme>,
    pub declaration_span_ids: Vec<Option<String>>,
}

pub(super) struct AnalyzedGraphOutputs {
    pub compiled_modules: usize,
    pub modules: HashMap<String, AnalyzedModule>,
    pub coverage_targets: Vec<CoverageTarget>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct CoverageTarget {
    pub id: String,
    pub expected_variants: Vec<String>,
    pub file: String,
    pub function_name: String,
    pub location: SourcePoint,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct CompiledGraphOutputs {
    pub compiled_modules: usize,
    pub entry_output_path: PathBuf,
    pub entry_span_map_path: PathBuf,
    pub module_order: Vec<String>,
    pub module_sources: HashMap<String, PathBuf>,
    pub module_outputs: HashMap<String, PathBuf>,
    pub span_map_outputs: HashMap<String, PathBuf>,
    pub auxiliary_outputs: Vec<PathBuf>,
    pub coverage_targets: Vec<CoverageTarget>,
}

#[derive(Debug, Clone)]
pub(super) struct GeneratedModuleOutput {
    pub output_path: PathBuf,
    pub span_map: ModuleSpanMap,
    pub span_map_path: PathBuf,
    pub ts_code: String,
}

pub(super) struct GeneratedGraphOutputs {
    pub coverage_targets: Vec<CoverageTarget>,
    pub entry_output_path: PathBuf,
    pub entry_span_map_path: PathBuf,
    pub module_outputs: HashMap<String, GeneratedModuleOutput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OutputFlavor {
    TypeScript,
    RuntimeEsm,
}

impl OutputFlavor {
    fn output_extension(self) -> &'static str {
        match self {
            OutputFlavor::TypeScript => "ts",
            OutputFlavor::RuntimeEsm => "mjs",
        }
    }

    fn import_extension(self) -> &'static str {
        match self {
            OutputFlavor::TypeScript => "js",
            OutputFlavor::RuntimeEsm => "mjs",
        }
    }
}

const COMPILE_CACHE_SCHEMA_VERSION: u32 = 1;

static COMPILER_BINARY_HASH: OnceLock<Result<String, String>> = OnceLock::new();

#[derive(Debug, Clone)]
enum CompileCacheOwner {
    LanguageRoot(PathBuf),
    ProjectRoot(PathBuf),
}

impl CompileCacheOwner {
    fn root(&self) -> &Path {
        match self {
            CompileCacheOwner::LanguageRoot(root) | CompileCacheOwner::ProjectRoot(root) => {
                root.as_path()
            }
        }
    }

    fn cache_dir(&self) -> PathBuf {
        self.root()
            .join(".sigil")
            .join("cache")
            .join("compiler")
            .join("compile-v1")
    }

    fn normalized_path(&self, path: &Path) -> String {
        let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        absolute
            .strip_prefix(self.root())
            .unwrap_or(&absolute)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CompileCacheMetadata {
    entry_files: Vec<String>,
    selected_env: Option<String>,
    output_flavor: String,
    trace: bool,
    breakpoints: bool,
    expression_debug: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CompileCacheEntry {
    schema_version: u32,
    cache_key: String,
    metadata: CompileCacheMetadata,
    compiled: CompiledGraphOutputs,
}

#[derive(Debug, Clone)]
struct CompileCacheContext {
    cache_key: String,
    cache_path: PathBuf,
    metadata: CompileCacheMetadata,
}

fn compiler_binary_hash() -> Result<String, CliError> {
    COMPILER_BINARY_HASH
        .get_or_init(|| {
            let executable = std::env::current_exe()
                .map_err(|error| format!("failed to resolve compiler executable path: {error}"))?;
            let bytes = fs::read(&executable).map_err(|error| {
                format!(
                    "failed to read compiler executable '{}': {}",
                    executable.display(),
                    error
                )
            })?;
            Ok(format!("{:x}", Sha256::digest(bytes)))
        })
        .clone()
        .map_err(CliError::Codegen)
}

fn output_flavor_tag(output_flavor: OutputFlavor) -> &'static str {
    match output_flavor {
        OutputFlavor::TypeScript => "ts",
        OutputFlavor::RuntimeEsm => "runtime-esm",
    }
}

fn git_repo_root(path: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(path).ok()?;
    let mut current = if canonical.is_dir() {
        canonical
    } else {
        canonical.parent()?.to_path_buf()
    };

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        let parent = current.parent()?.to_path_buf();
        current = parent;
    }
}

fn compile_cache_owner(entry_files: &[PathBuf]) -> Result<Option<CompileCacheOwner>, CliError> {
    if entry_files.is_empty() {
        return Ok(None);
    }

    let mut project_root = None::<PathBuf>;
    let mut all_project_owned = true;
    for file in entry_files {
        match get_project_config(file)? {
            Some(project) => match project_root.as_ref() {
                Some(root) if root != &project.root => {
                    all_project_owned = false;
                    break;
                }
                Some(_) => {}
                None => project_root = Some(project.root),
            },
            None => {
                all_project_owned = false;
                break;
            }
        }
    }
    if all_project_owned {
        return Ok(project_root.map(CompileCacheOwner::ProjectRoot));
    }

    let mut repo_root = None::<PathBuf>;
    for file in entry_files {
        let Some(root) = git_repo_root(file) else {
            return Ok(None);
        };
        let canonical = fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
        let relative = canonical.strip_prefix(&root).ok();
        let under_language = relative.is_some_and(|relative| {
            relative
                .components()
                .next()
                .is_some_and(|component| component.as_os_str() == "language")
        });
        if !under_language {
            return Ok(None);
        }
        match repo_root.as_ref() {
            Some(existing) if existing != &root => return Ok(None),
            Some(_) => {}
            None => repo_root = Some(root),
        }
    }

    Ok(repo_root.map(CompileCacheOwner::LanguageRoot))
}

fn should_skip_compile_fingerprint_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| matches!(name, ".git" | ".local" | "node_modules" | "target"))
}

fn collect_compile_fingerprint_files(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    include_manifests: bool,
) -> Result<(), CliError> {
    if !dir.exists() {
        return Ok(());
    }

    let mut entries = fs::read_dir(dir)?
        .collect::<Result<Vec<_>, std::io::Error>>()?
        .into_iter()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            if should_skip_compile_fingerprint_dir(&path) {
                continue;
            }
            collect_compile_fingerprint_files(&path, files, include_manifests)?;
        } else if is_sigil_source_file(&path)
            || (include_manifests
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name == "sigil.json"))
        {
            files.push(fs::canonicalize(&path).unwrap_or(path));
        }
    }

    Ok(())
}

fn project_compile_fingerprint_files(project_root: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut files = Vec::new();
    for path in [project_root.join("sigil.json"), project_root.join("sigil.lock")] {
        if path.exists() {
            files.push(fs::canonicalize(&path).unwrap_or(path));
        }
    }
    for dir in ["src", "config", "tests"] {
        collect_compile_fingerprint_files(&project_root.join(dir), &mut files, false)?;
    }
    collect_compile_fingerprint_files(
        &project_root.join(".sigil").join("packages"),
        &mut files,
        true,
    )?;
    files.sort();
    files.dedup();
    Ok(files)
}

fn language_compile_fingerprint_files(repo_root: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut files = Vec::new();
    collect_compile_fingerprint_files(&repo_root.join("language"), &mut files, false)?;
    files.sort();
    files.dedup();
    Ok(files)
}

fn fingerprint_file_set(base: &Path, files: &[PathBuf]) -> Result<String, CliError> {
    let mut hasher = Sha256::new();
    for file in files {
        let canonical = fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
        let relative = canonical
            .strip_prefix(base)
            .unwrap_or(&canonical)
            .to_string_lossy()
            .replace('\\', "/");
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(
            fs::read(&canonical).map_err(|error| {
                CliError::Codegen(format!(
                    "failed to read compile cache input '{}': {}",
                    canonical.display(),
                    error
                ))
            })?,
        );
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn resolve_compile_cache_context(
    entry_files: &[PathBuf],
    output_override: Option<&Path>,
    selected_env: Option<&str>,
    trace: bool,
    breakpoints: bool,
    expression_debug: bool,
    output_flavor: OutputFlavor,
) -> Result<Option<CompileCacheContext>, CliError> {
    if output_override.is_some() {
        return Ok(None);
    }

    let Some(owner) = compile_cache_owner(entry_files)? else {
        return Ok(None);
    };
    let input_fingerprint = match &owner {
        CompileCacheOwner::ProjectRoot(root) => {
            fingerprint_file_set(root, &project_compile_fingerprint_files(root)?)?
        }
        CompileCacheOwner::LanguageRoot(root) => {
            fingerprint_file_set(root, &language_compile_fingerprint_files(root)?)?
        }
    };
    let compiler_hash = compiler_binary_hash()?;
    let mut normalized_entries = entry_files
        .iter()
        .map(|file| owner.normalized_path(file))
        .collect::<Vec<_>>();
    normalized_entries.sort();

    let metadata = CompileCacheMetadata {
        entry_files: normalized_entries.clone(),
        selected_env: selected_env.map(str::to_string),
        output_flavor: output_flavor_tag(output_flavor).to_string(),
        trace,
        breakpoints,
        expression_debug,
    };

    let mut hasher = Sha256::new();
    hasher.update(format!("compile-cache-v{COMPILE_CACHE_SCHEMA_VERSION}").as_bytes());
    hasher.update([0]);
    hasher.update(compiler_hash.as_bytes());
    hasher.update([0]);
    for entry in &normalized_entries {
        hasher.update(entry.as_bytes());
        hasher.update([0]);
    }
    if let Some(env) = selected_env {
        hasher.update(env.as_bytes());
    }
    hasher.update([0]);
    hasher.update(output_flavor_tag(output_flavor).as_bytes());
    hasher.update([0]);
    hasher.update([trace as u8, breakpoints as u8, expression_debug as u8]);
    hasher.update(input_fingerprint.as_bytes());
    let cache_key = format!("{:x}", hasher.finalize());

    Ok(Some(CompileCacheContext {
        cache_path: owner.cache_dir().join(format!("{cache_key}.json")),
        cache_key,
        metadata,
    }))
}

fn compiled_artifacts_exist(compiled: &CompiledGraphOutputs) -> bool {
    compiled.entry_output_path.exists()
        && compiled.entry_span_map_path.exists()
        && compiled.module_outputs.values().all(|path| path.exists())
        && compiled.span_map_outputs.values().all(|path| path.exists())
        && compiled.auxiliary_outputs.iter().all(|path| path.exists())
}

fn load_compile_cache_entry(
    context: &CompileCacheContext,
) -> Result<Option<CompiledGraphOutputs>, CliError> {
    let text = match fs::read_to_string(&context.cache_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };
    let entry = match serde_json::from_str::<CompileCacheEntry>(&text) {
        Ok(entry) => entry,
        Err(_) => return Ok(None),
    };
    if entry.schema_version != COMPILE_CACHE_SCHEMA_VERSION || entry.cache_key != context.cache_key {
        return Ok(None);
    }
    if !compiled_artifacts_exist(&entry.compiled) {
        return Ok(None);
    }
    Ok(Some(entry.compiled))
}

fn store_compile_cache_entry(
    context: &CompileCacheContext,
    compiled: &CompiledGraphOutputs,
) -> Result<(), CliError> {
    let entry = CompileCacheEntry {
        schema_version: COMPILE_CACHE_SCHEMA_VERSION,
        cache_key: context.cache_key.clone(),
        metadata: context.metadata.clone(),
        compiled: compiled.clone(),
    };
    let serialized = serde_json::to_string(&entry)
        .map_err(|error| CliError::Codegen(format!("failed to serialize compile cache entry: {error}")))?;
    write_atomic_file(&context.cache_path, serialized.as_bytes())
}

fn build_module_graph_for_entries(
    entry_files: &[PathBuf],
    selected_env: Option<&str>,
) -> Result<ModuleGraph, ModuleGraphError> {
    if entry_files.len() == 1 {
        ModuleGraph::build_with_env(&entry_files[0], selected_env)
    } else {
        ModuleGraph::build_many_with_env(entry_files, selected_env)
    }
}

pub(super) fn compile_entry_files_with_cache(
    entry_files: &[PathBuf],
    existing_graph: Option<ModuleGraph>,
    output_override: Option<&Path>,
    selected_env: Option<&str>,
    trace: bool,
    breakpoints: bool,
    expression_debug: bool,
    output_flavor: OutputFlavor,
) -> Result<CompiledGraphOutputs, CliError> {
    let cache_context = resolve_compile_cache_context(
        entry_files,
        output_override,
        selected_env,
        trace,
        breakpoints,
        expression_debug,
        output_flavor,
    )?;
    if let Some(context) = cache_context.as_ref() {
        if let Some(compiled) = load_compile_cache_entry(context)? {
            return Ok(compiled);
        }
    }

    let graph = match existing_graph {
        Some(graph) => graph,
        None => build_module_graph_for_entries(entry_files, selected_env)?,
    };
    let compiled = compile_module_graph(
        graph,
        output_override,
        trace,
        breakpoints,
        expression_debug,
        output_flavor,
    )?;
    if let Some(context) = cache_context.as_ref() {
        store_compile_cache_entry(context, &compiled)?;
    }
    Ok(compiled)
}

pub(super) fn project_json(project: Option<&ProjectConfig>) -> Option<serde_json::Value> {
    project.map(|project| {
        serde_json::json!({
            "root": project.root.to_string_lossy(),
            "layout": serde_json::to_value(&project.layout).unwrap_or(serde_json::json!({}))
        })
    })
}

fn is_sigil_source_file(path: &Path) -> bool {
    path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("sigil")
}

fn walk_compile_directory(
    dir: &Path,
    ignore: &CompileDirectoryIgnore,
    files: &mut Vec<PathBuf>,
) -> Result<(), CliError> {
    let mut entries = fs::read_dir(dir)?
        .collect::<Result<Vec<_>, std::io::Error>>()?
        .into_iter()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        let is_dir = path.is_dir();
        if ignore.should_ignore(&path, is_dir) {
            continue;
        }

        if is_dir {
            walk_compile_directory(&path, ignore, files)?;
        } else if is_sigil_source_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

pub(super) fn collect_sigil_targets(
    command_name: &str,
    path: &Path,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<Vec<PathBuf>, CliError> {
    if is_sigil_source_file(path) {
        return Ok(vec![path.to_path_buf()]);
    }

    if path.is_file() {
        return Err(CliError::Validation(format!(
            "{} expects a .sigil file or directory, got '{}'",
            command_name,
            path.display()
        )));
    }

    let ignore = CompileDirectoryIgnore::new(path, ignore_paths, ignore_from)?;
    let mut files = Vec::new();
    walk_compile_directory(&ignore.root, &ignore, &mut files)?;
    files.sort();
    Ok(files)
}

pub(super) fn group_compile_targets(files: &[PathBuf]) -> Result<Vec<CompileBatchGroup>, CliError> {
    let mut project_buckets: HashMap<PathBuf, Vec<(usize, PathBuf, String)>> = HashMap::new();
    let mut standalone_bucket: Vec<(usize, PathBuf, String)> = Vec::new();

    for (index, file) in files.iter().enumerate() {
        let module_key = entry_module_key(file)?;
        if let Some(project) = get_project_config(file)? {
            project_buckets
                .entry(project.root.clone())
                .or_default()
                .push((index, file.clone(), module_key));
        } else {
            standalone_bucket.push((index, file.clone(), module_key));
        }
    }

    let mut groups = Vec::new();

    let mut project_roots = project_buckets.keys().cloned().collect::<Vec<_>>();
    project_roots.sort();
    for root in project_roots {
        let mut bucket = project_buckets.remove(&root).unwrap_or_default();
        bucket.sort_by(|a, b| a.1.cmp(&b.1));
        let mut packed_groups: Vec<(CompileBatchGroup, HashSet<String>)> = Vec::new();
        for (index, file, module_key) in bucket {
            if let Some((group, seen_keys)) = packed_groups
                .iter_mut()
                .find(|(_, seen_keys)| !seen_keys.contains(&module_key))
            {
                group.files.push(file);
                seen_keys.insert(module_key);
            } else {
                let mut seen_keys = HashSet::new();
                seen_keys.insert(module_key);
                packed_groups.push((
                    CompileBatchGroup {
                        first_index: index,
                        files: vec![file],
                    },
                    seen_keys,
                ));
            }
        }
        groups.extend(packed_groups.into_iter().map(|(group, _)| group));
    }

    if !standalone_bucket.is_empty() {
        standalone_bucket.sort_by(|a, b| a.1.cmp(&b.1));
        groups.push(CompileBatchGroup {
            first_index: standalone_bucket
                .iter()
                .map(|(index, _, _)| *index)
                .min()
                .unwrap_or(0),
            files: standalone_bucket
                .into_iter()
                .map(|(_, file, _)| file)
                .collect(),
        });
    }

    groups.sort_by_key(|group| group.first_index);
    for group in &mut groups {
        group.files.sort();
    }
    Ok(groups)
}

fn compile_group(
    group: &CompileBatchGroup,
    selected_env: Option<&str>,
) -> Result<CompileBatchOutputs, CliError> {
    let compiled = compile_entry_files_with_cache(
        &group.files,
        None,
        None,
        selected_env,
        false,
        false,
        false,
        OutputFlavor::TypeScript,
    )?;
    let entries = group.files.iter().map(|input| {
            let module_id = entry_module_key(input)?;
            let output = compiled
                .module_outputs
                .get(&module_id)
                .cloned()
                .ok_or_else(|| {
                    CliError::Codegen(format!(
                        "batch compile did not produce output for '{}'",
                        input.display()
                    ))
                })?;
            let span_map = compiled
                .span_map_outputs
                .get(&module_id)
                .cloned()
                .ok_or_else(|| {
                    CliError::Codegen(format!(
                        "batch compile did not produce span map for '{}'",
                        input.display()
                    ))
                })?;
            Ok(CompileEntryOutput {
                input: input.clone(),
                output,
                span_map,
                project_root: get_project_config(input)?.map(|project| project.root),
            })
        })
        .collect::<Result<Vec<_>, CliError>>()?;

    Ok(CompileBatchOutputs {
        compiled_modules: compiled.compiled_modules,
        entries,
    })
}

fn compile_single_file_command(
    file: &Path,
    output: Option<&Path>,
    show_types: bool,
    selected_env: Option<&str>,
) -> Result<(), CliError> {
    let project_json = project_json(get_project_config(file)?.as_ref());

    let compiled = match compile_entry_files_with_cache(
        &[file.to_path_buf()],
        None,
        output,
        selected_env,
        false,
        false,
        false,
        OutputFlavor::TypeScript,
    ) {
        Ok(compiled) => compiled,
        Err(CliError::ModuleGraph(ModuleGraphError::Validation(errors))) => {
            if let Some(first_error) = errors.first() {
                let error_msg = first_error.to_string();
                let error_code = extract_error_code(&error_msg);

                output_json_error(
                    "sigilc compile",
                    "canonical",
                    &error_code,
                    &error_msg,
                    json!({
                        "file": file.to_string_lossy(),
                        "errors": errors.iter().map(|error| error.to_string()).collect::<Vec<_>>()
                    }),
                );
            }
            return Err(CliError::ModuleGraph(ModuleGraphError::Validation(errors)));
        }
        Err(CliError::ModuleGraph(ModuleGraphError::ProjectConfig(project_error))) => {
            output_json_error(
                "sigilc compile",
                phase_for_code(project_error.code()),
                project_error.code(),
                &project_error.to_string(),
                project_error_json_details(&project_error, "file", file, serde_json::Map::new()),
            );
            return Err(CliError::ModuleGraph(ModuleGraphError::ProjectConfig(project_error)));
        }
        Err(error) => {
            let message = error.to_string();
            let error_code = extract_error_code(&message);
            output_json_error(
                "sigilc compile",
                phase_for_code(&error_code),
                &error_code,
                &message,
                json!({
                    "file": file.to_string_lossy()
                }),
            );
            return Err(error);
        }
    };
    let entry_output = compiled.entry_output_path.clone();

    let all_modules: Vec<serde_json::Value> = compiled
        .module_order
        .iter()
        .map(|module_id| {
            let source_file = compiled
                .module_sources
                .get(module_id)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            let output_file = compiled
                .module_outputs
                .get(module_id)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            let span_map_file = compiled
                .span_map_outputs
                .get(module_id)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();

            serde_json::json!({
                "moduleId": module_id,
                "sourceFile": source_file,
                "outputFile": output_file,
                "spanMapFile": span_map_file
            })
        })
        .collect();

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc compile",
        "ok": true,
        "phase": "codegen",
        "data": {
            "input": file.to_string_lossy(),
            "outputs": {
                "rootTs": entry_output.to_string_lossy(),
                "rootSpanMap": compiled.entry_span_map_path.to_string_lossy(),
                "allModules": all_modules
            },
            "project": project_json,
            "typecheck": {
                "ok": true,
                "inferred": if show_types { vec![] as Vec<serde_json::Value> } else { vec![] }
            }
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    Ok(())
}

fn compile_directory_command(
    path: &Path,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
    selected_env: Option<&str>,
) -> Result<(), CliError> {
    let start_time = Instant::now();
    let files = collect_sigil_targets("compile", path, ignore_paths, ignore_from)?;
    let groups = group_compile_targets(&files)?;
    let group_count = groups.len();
    let file_order = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.clone(), index))
        .collect::<HashMap<_, _>>();

    let mut compiled_file_count = 0usize;
    let mut compiled_module_count = 0usize;
    let mut compiled_entries = Vec::new();

    for group in groups {
        let first_file = group.files.first().cloned();
        let batch = match compile_group(&group, selected_env) {
            Ok(batch) => batch,
            Err(error) => {
                match &error {
                    CliError::Type(type_error) => {
                        let mut details = match type_error_json_details(type_error) {
                            serde_json::Value::Object(map) => map,
                            _ => serde_json::Map::new(),
                        };
                        details.insert(
                            "input".to_string(),
                            json!(path.to_string_lossy().to_string()),
                        );
                        details.insert(
                            "file".to_string(),
                            json!(type_error.source_file.clone().or_else(|| first_file
                                .as_ref()
                                .map(|file| file.to_string_lossy().to_string()))),
                        );
                        details.insert("discovered".to_string(), json!(files.len()));
                        details.insert("compiled".to_string(), json!(compiled_file_count));
                        details.insert(
                            "durationMs".to_string(),
                            json!(start_time.elapsed().as_millis()),
                        );
                        output_json_error(
                            "sigilc compile",
                            "typecheck",
                            &type_error.code,
                            &type_error.message,
                            serde_json::Value::Object(details),
                        );
                    }
                    CliError::ModuleGraph(ModuleGraphError::ProjectConfig(project_error))
                    | CliError::ProjectConfig(project_error) => {
                        output_json_error(
                            "sigilc compile",
                            phase_for_code(project_error.code()),
                            project_error.code(),
                            &project_error.to_string(),
                            project_error_json_details(
                                project_error,
                                "input",
                                path,
                                serde_json::Map::from_iter([
                                    (
                                        "file".to_string(),
                                        json!(first_file
                                            .as_ref()
                                            .map(|file| file.to_string_lossy().to_string())),
                                    ),
                                    ("discovered".to_string(), json!(files.len())),
                                    ("compiled".to_string(), json!(compiled_file_count)),
                                    (
                                        "durationMs".to_string(),
                                        json!(start_time.elapsed().as_millis()),
                                    ),
                                ]),
                            ),
                        );
                    }
                    _ => {
                        let message = error.to_string();
                        let error_code = extract_error_code(&message);
                        output_json_error(
                            "sigilc compile",
                            "codegen",
                            &error_code,
                            &message,
                            json!({
                                "input": path.to_string_lossy(),
                                "file": first_file.map(|file| file.to_string_lossy().to_string()),
                                "discovered": files.len(),
                                "compiled": compiled_file_count,
                                "durationMs": start_time.elapsed().as_millis()
                            }),
                        );
                    }
                }
                return Err(error);
            }
        };

        compiled_module_count += batch.compiled_modules;
        compiled_file_count += batch.entries.len();
        compiled_entries.extend(batch.entries);
    }

    compiled_entries
        .sort_by_key(|entry| file_order.get(&entry.input).copied().unwrap_or(usize::MAX));
    let file_results = compiled_entries
        .into_iter()
        .map(|entry| {
            serde_json::json!({
                "input": entry.input.to_string_lossy(),
                "rootTs": entry.output.to_string_lossy(),
                "rootSpanMap": entry.span_map.to_string_lossy(),
                "projectRoot": entry.project_root.map(|root| root.to_string_lossy().to_string())
            })
        })
        .collect::<Vec<_>>();

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc compile",
        "ok": true,
        "phase": "codegen",
        "data": {
            "input": path.to_string_lossy(),
            "summary": {
                "discovered": files.len(),
                "compiled": compiled_file_count,
                "groups": group_count,
                "modules": compiled_module_count,
                "durationMs": start_time.elapsed().as_millis()
            },
            "files": file_results
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    Ok(())
}

pub(super) fn compile_command(
    path: &Path,
    output: Option<&Path>,
    show_types: bool,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
    selected_env: Option<&str>,
) -> Result<(), CliError> {
    if path.is_dir() {
        if output.is_some() {
            return Err(CliError::Validation(
                "compile -o is only valid when compiling a single file".to_string(),
            ));
        }
        compile_directory_command(path, ignore_paths, ignore_from, selected_env)
    } else {
        compile_single_file_command(path, output, show_types, selected_env)
    }
}

pub(super) fn validate_command(path: &Path, env: &str) -> Result<(), CliError> {
    let project_root = get_project_config(path)?
        .map(|project| project.root)
        .ok_or_else(|| {
            CliError::Validation(format!(
                "{}: no Sigil project found while validating topology",
                codes::topology::MISSING_MODULE
            ))
        })?;

    if !topology_source_path(&project_root).exists() {
        return Err(CliError::Validation(format!(
            "{}: topology-aware projects require src/topology.lib.sigil",
            codes::topology::MISSING_MODULE
        )));
    }

    let _compiled = compile_topology_module(&project_root)?;
    let prelude = build_world_runtime_prelude(&project_root, env, true)?;
    let runner_path = project_root.join(".local/topology.validate.run.mjs");
    if let Some(parent) = runner_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(
        &runner_path,
        format!(
            r#"{prelude}
console.log(JSON.stringify({{
  ok: true,
  environment: {env_json}
}}));
"#,
            prelude = prelude,
            env_json = serde_json::to_string(env).unwrap()
        ),
    )?;

    let abs_runner = fs::canonicalize(&runner_path)?;
    let output = Command::new("node")
        .arg(&abs_runner)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CliError::Runtime(
                    "node not found. Please install Node.js to validate Sigil topology."
                        .to_string(),
                )
            } else {
                CliError::Runtime(format!("Failed to execute topology validation: {}", e))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.trim();
        return Err(CliError::Validation(if message.is_empty() {
            "topology validation failed".to_string()
        } else {
            message.to_string()
        }));
    }

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc validate",
        "ok": true,
        "phase": "topology",
        "data": {
            "environment": env,
            "projectRoot": project_root.to_string_lossy()
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    Ok(())
}

pub(super) fn analyze_module_graph(graph: &ModuleGraph) -> Result<AnalyzedGraphOutputs, CliError> {
    let mut compiled_modules = HashMap::new();
    let mut compiled_schemes = HashMap::new();
    let mut compiled_value_meta = HashMap::new();
    let mut label_registries = HashMap::new();
    let mut compiled_boundary_rules = Vec::new();
    let mut coverage_targets = Vec::new();
    let mut type_registries = HashMap::new();
    let mut analyzed_modules = HashMap::new();

    for module_id in &graph.topo_order {
        let module = &graph.modules[module_id];

        let imported_namespaces = build_imported_namespaces(module, &compiled_modules);
        let imported_type_regs = build_imported_type_registries(module, &type_registries);
        let imported_label_regs = build_imported_label_registries(module, &label_registries);
        let imported_value_schemes = build_imported_value_schemes(module, &compiled_schemes);
        let imported_value_meta = build_imported_value_meta(module, &compiled_value_meta);
        let imported_boundary_rules =
            build_imported_boundary_rules(&module.ast, &compiled_boundary_rules);
        let effect_catalog = load_project_effect_catalog_for(&module.file_path)?;

        let typecheck_result = type_check(
            &module.ast,
            &module.source,
            Some(TypeCheckOptions {
                effect_catalog,
                imported_namespaces: Some(imported_namespaces),
                imported_type_registries: Some(imported_type_regs.clone()),
                imported_label_registries: Some(imported_label_regs),
                imported_value_schemes: Some(imported_value_schemes),
                imported_value_meta: Some(imported_value_meta),
                boundary_rules: Some(imported_boundary_rules),
                module_id: Some(module_id.clone()),
                source_file: Some(module.file_path.to_string_lossy().to_string()),
            }),
        )
        .map_err(CliError::Type)?;

        let extracted_type_registry =
            extract_type_registry(&module.ast, &module.file_path, module_id);

        validate_typed_canonical_form(
            &typecheck_result.typed_program,
            Some(module.file_path.to_string_lossy().as_ref()),
        )
        .map_err(|errors| CliError::Validation(format_validation_errors(&errors)))?;

        coverage_targets.extend(collect_module_coverage_targets(
            module,
            &typecheck_result.typed_program,
            &imported_type_regs,
            &typecheck_result
                .typed_program
                .declarations
                .iter()
                .filter_map(|decl| match decl {
                    TypedDeclaration::Type(type_decl) => Some((
                        type_decl.ast.name.clone(),
                        TypeInfo {
                            type_params: type_decl.ast.type_params.clone(),
                            definition: type_decl.ast.definition.clone(),
                            constraint: type_decl.ast.constraint.clone(),
                            labels: qualify_label_refs(&type_decl.ast.labels, module_id),
                        },
                    )),
                    _ => None,
                })
                .collect(),
        ));

        let collected_span_map = collect_module_span_map(
            module_id,
            &module.file_path.to_string_lossy(),
            "",
            &typecheck_result.typed_program,
        );

        let sigil_typechecker::typed_ir::TypeCheckResult {
            declaration_types,
            declaration_schemes,
            declaration_meta,
            label_registry,
            boundary_rules,
            typed_program,
        } = typecheck_result;

        compiled_schemes.insert(module_id.clone(), declaration_schemes.clone());
        compiled_modules.insert(module_id.clone(), declaration_types.clone());
        compiled_value_meta.insert(module_id.clone(), declaration_meta.clone());
        label_registries.insert(module_id.clone(), label_registry.clone());
        compiled_boundary_rules.extend(boundary_rules.clone());
        type_registries.insert(module_id.clone(), extracted_type_registry);
        analyzed_modules.insert(
            module_id.clone(),
            AnalyzedModule {
                module_id: module_id.clone(),
                file_path: module.file_path.clone(),
                project: module.project.clone(),
                ast: module.ast.clone(),
                typed_program,
                declaration_types,
                declaration_schemes,
                declaration_span_ids: collected_span_map.declaration_span_ids,
            },
        );
    }

    Ok(AnalyzedGraphOutputs {
        compiled_modules: graph.topo_order.len(),
        modules: analyzed_modules,
        coverage_targets,
    })
}

pub(super) fn generate_module_graph_outputs(
    graph: &ModuleGraph,
    output_override: Option<&Path>,
    trace: bool,
    breakpoints: bool,
    expression_debug: bool,
    output_flavor: OutputFlavor,
) -> Result<GeneratedGraphOutputs, CliError> {
    let analyzed = analyze_module_graph(graph)?;
    let entry_module_id = graph
        .topo_order
        .last()
        .ok_or_else(|| CliError::Codegen("codegen requires at least one module".to_string()))?;
    let mut entry_output_path = PathBuf::new();
    let mut entry_span_map_path = PathBuf::new();
    let mut module_outputs = HashMap::new();

    for module_id in &graph.topo_order {
        let module = &graph.modules[module_id];
        let analyzed_module = analyzed.modules.get(module_id).ok_or_else(|| {
            CliError::Codegen(format!(
                "codegen could not resolve analyzed module '{}'",
                module.file_path.display()
            ))
        })?;
        let output_path = if module_id == entry_module_id && output_override.is_some() {
            output_override.unwrap().to_path_buf()
        } else {
            get_module_output_path(module, output_flavor)
        };
        let codegen_options = CodegenOptions {
            module_id: Some(module_id.clone()),
            source_file: Some(module.file_path.to_string_lossy().to_string()),
            output_file: Some(output_path.to_string_lossy().to_string()),
            import_extension: output_flavor.import_extension().to_string(),
            lazy_extern_namespaces: output_flavor == OutputFlavor::RuntimeEsm,
            trace,
            breakpoints,
            expression_debug,
        };
        let mut codegen = TypeScriptGenerator::new(codegen_options);
        let ts_code = codegen
            .generate(&analyzed_module.typed_program)
            .map_err(|e| CliError::Codegen(format!("{}", e)))?;
        let span_map = codegen.generated_span_map().cloned().ok_or_else(|| {
            CliError::Codegen(format!(
                "codegen did not produce a span map for '{}'",
                module.file_path.display()
            ))
        })?;
        let span_map_path = span_map_output_path(&output_path);

        module_outputs.insert(
            module_id.clone(),
            GeneratedModuleOutput {
                output_path: output_path.clone(),
                span_map,
                span_map_path: span_map_path.clone(),
                ts_code,
            },
        );

        if module_id == entry_module_id {
            entry_output_path = output_path;
            entry_span_map_path = span_map_path;
        }
    }

    Ok(GeneratedGraphOutputs {
        coverage_targets: analyzed.coverage_targets,
        entry_output_path,
        entry_span_map_path,
        module_outputs,
    })
}

pub(super) fn compile_module_graph(
    graph: ModuleGraph,
    output_override: Option<&Path>,
    trace: bool,
    breakpoints: bool,
    expression_debug: bool,
    output_flavor: OutputFlavor,
) -> Result<CompiledGraphOutputs, CliError> {
    let generated = generate_module_graph_outputs(
        &graph,
        output_override,
        trace,
        breakpoints,
        expression_debug,
        output_flavor,
    )?;
    let compiled_modules = graph.topo_order.len();
    let module_order = graph.topo_order.clone();
    let module_sources = graph
        .modules
        .iter()
        .map(|(module_id, module)| (module_id.clone(), module.file_path.clone()))
        .collect::<HashMap<_, _>>();
    let mut module_outputs = HashMap::new();
    let mut span_map_outputs = HashMap::new();
    for (module_id, generated_output) in generated.module_outputs {
        if let Some(parent) = generated_output.output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_atomic_file(
            &generated_output.output_path,
            generated_output.ts_code.as_bytes(),
        )?;
        write_span_map_file(&generated_output.span_map_path, &generated_output.span_map)?;
        module_outputs.insert(module_id.clone(), generated_output.output_path);
        span_map_outputs.insert(module_id, generated_output.span_map_path);
    }
    let mut auxiliary_outputs =
        write_public_package_module_aliases(&graph, &module_outputs, output_flavor)?;
    auxiliary_outputs.extend(write_selected_config_aliases(
        &graph,
        &module_outputs,
        output_flavor,
    )?);

    Ok(CompiledGraphOutputs {
        compiled_modules,
        entry_output_path: generated.entry_output_path,
        entry_span_map_path: generated.entry_span_map_path,
        module_order,
        module_sources,
        module_outputs,
        span_map_outputs,
        auxiliary_outputs,
        coverage_targets: generated.coverage_targets,
    })
}

fn write_public_package_module_aliases(
    graph: &ModuleGraph,
    module_outputs: &HashMap<String, PathBuf>,
    output_flavor: OutputFlavor,
) -> Result<Vec<PathBuf>, CliError> {
    let mut alias_outputs = Vec::new();
    for (module_id, module) in &graph.modules {
        let Some(public_module_id) = public_package_module_id(module_id) else {
            continue;
        };
        let Some(output_project) = module.output_project.as_ref() else {
            continue;
        };
        let Some(target_output_path) = module_outputs.get(module_id) else {
            continue;
        };

        let alias_output_path = output_project
            .root
            .join(&output_project.layout.out)
            .join(format!(
                "{}.{}",
                public_module_id.replace("::", "/"),
                output_flavor.output_extension()
            ));
        let alias_parent = alias_output_path
            .parent()
            .ok_or_else(|| CliError::Codegen("package alias output had no parent directory".to_string()))?;
        fs::create_dir_all(alias_parent)?;
        let import_path = relative_import_path(alias_parent, target_output_path);
        let alias_source = format!("export * from '{}';\n", import_path);
        write_atomic_file(&alias_output_path, alias_source.as_bytes())?;
        alias_outputs.push(alias_output_path);
    }

    Ok(alias_outputs)
}

fn write_selected_config_aliases(
    graph: &ModuleGraph,
    module_outputs: &HashMap<String, PathBuf>,
    output_flavor: OutputFlavor,
) -> Result<Vec<PathBuf>, CliError> {
    let mut alias_outputs = Vec::new();
    for (module_id, module) in &graph.modules {
        if !module_id.starts_with("config::") {
            continue;
        }

        let output_project = module
            .output_project
            .as_ref()
            .or(module.project.as_ref())
            .ok_or_else(|| {
                CliError::Codegen(format!(
                    "selected config module '{}' had no owning project",
                    module_id
                ))
            })?;
        let Some(target_output_path) = module_outputs.get(module_id) else {
            continue;
        };

        let alias_output_path = output_project
            .root
            .join(&output_project.layout.out)
            .join(format!("config.{}", output_flavor.output_extension()));
        let alias_parent = alias_output_path.parent().ok_or_else(|| {
            CliError::Codegen("config alias output had no parent directory".to_string())
        })?;
        fs::create_dir_all(alias_parent)?;
        let import_path = relative_import_path(alias_parent, target_output_path);
        let alias_source = format!("export * from '{}';\n", import_path);
        write_atomic_file(&alias_output_path, alias_source.as_bytes())?;
        alias_outputs.push(alias_output_path);
    }

    Ok(alias_outputs)
}

fn span_map_output_path(output_path: &Path) -> PathBuf {
    output_path.with_extension("span.json")
}

fn atomic_write_path(path: &Path) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    path.with_file_name(format!(
        ".{}.{}.{}.tmp",
        file_name,
        std::process::id(),
        unique
    ))
}

fn write_atomic_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = atomic_write_path(path);
    fs::write(&temp_path, bytes)?;

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)?;
    }

    fs::rename(&temp_path, path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        CliError::Io(error)
    })?;
    Ok(())
}

fn relative_import_path(from_dir: &Path, target_file: &Path) -> String {
    let from_components: Vec<_> = from_dir.components().collect();
    let target_components: Vec<_> = target_file.components().collect();
    let common_len = from_components
        .iter()
        .zip(target_components.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let mut relative = PathBuf::new();

    for _ in common_len..from_components.len() {
        relative.push("..");
    }

    for component in &target_components[common_len..] {
        relative.push(component.as_os_str());
    }

    let relative_str = relative.to_string_lossy().replace('\\', "/");
    if relative_str.starts_with("../") {
        relative_str
    } else {
        format!("./{}", relative_str)
    }
}

fn write_span_map_file(path: &Path, span_map: &ModuleSpanMap) -> Result<(), CliError> {
    let serialized = serde_json::to_string(span_map)
        .map_err(|error| CliError::Codegen(format!("failed to serialize span map: {}", error)))?;
    write_atomic_file(path, serialized.as_bytes())?;
    Ok(())
}

pub(super) fn topology_source_path(project_root: &Path) -> PathBuf {
    project_root.join("src/topology.lib.sigil")
}

fn config_source_path(project_root: &Path, env_name: &str) -> PathBuf {
    project_root
        .join("config")
        .join(format!("{}.lib.sigil", env_name))
}

fn compile_topology_module(project_root: &Path) -> Result<CompiledGraphOutputs, CliError> {
    let topology_source = topology_source_path(project_root);
    if !topology_source.exists() {
        return Err(CliError::Validation(format!(
            "{}: topology-aware projects require src/topology.lib.sigil",
            codes::topology::MISSING_MODULE
        )));
    }

    compile_entry_files_with_cache(
        &[topology_source],
        None,
        None,
        None,
        false,
        false,
        false,
        OutputFlavor::RuntimeEsm,
    )
}

fn compile_config_module(
    project_root: &Path,
    env_name: &str,
) -> Result<CompiledGraphOutputs, CliError> {
    let config_source = config_source_path(project_root, env_name);
    if !config_source.exists() {
        return Err(CliError::Validation(format!(
            "{}: topology environment '{}' requires config/{}.lib.sigil",
            codes::topology::MISSING_CONFIG_MODULE,
            env_name,
            env_name
        )));
    }

    compile_entry_files_with_cache(
        &[config_source],
        None,
        None,
        Some(env_name),
        false,
        false,
        false,
        OutputFlavor::RuntimeEsm,
    )
}

pub(super) fn build_world_runtime_prelude(
    project_root: &Path,
    env_name: &str,
    topology_present: bool,
) -> Result<String, CliError> {
    let topology_url = if topology_present {
        let topology_outputs = compile_topology_module(project_root)?;
        let topology_output = topology_outputs.entry_output_path;
        Some(format!(
            "file://{}",
            fs::canonicalize(topology_output)?.display()
        ))
    } else {
        None
    };
    let config_outputs = compile_config_module(project_root, env_name)?;
    let config_output = config_outputs.entry_output_path;
    let config_url = format!("file://{}", fs::canonicalize(config_output)?.display());
    let env_name_json = serde_json::to_string(env_name).unwrap();

    Ok(format!(
        r#"{topology_import}
const __sigil_config_module = await import("{config_url}");
const __sigil_config_exports = Object.fromEntries(
  await Promise.all(
    Object.entries(__sigil_config_module).map(async ([key, value]) => [key, await Promise.resolve(value)])
  )
);
const __sigil_world_env_name = {env_name_json};

function __sigil_runtime_fail(code, message) {{
  const error = new Error(`${{code}}: ${{message}}`);
  error.sigilCode = code;
  throw error;
}}

function __sigil_runtime_collect_topology(moduleExports) {{
  const envs = new Set();
  const http = new Set();
  const tcp = new Set();
  for (const value of Object.values(moduleExports ?? {{}})) {{
    if (value?.__tag === 'Environment') {{
      envs.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'HttpServiceDependency') {{
      http.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'TcpServiceDependency') {{
      tcp.add(String(value.__fields?.[0] ?? ''));
    }}
  }}
  return {{ envs, http, tcp }};
}}

function __sigil_runtime_collect_world_dependency_names(entries, expectedTag) {{
  if (!Array.isArray(entries)) {{
    __sigil_runtime_fail("{binding_kind}", `world ${{
      expectedTag === 'HttpEntry' ? 'http' : 'tcp'
    }} entries must be a list`);
  }}
  const seen = new Set();
  for (const entry of entries) {{
    if (!entry || typeof entry !== 'object' || entry.__tag !== expectedTag) {{
      __sigil_runtime_fail("{binding_kind}", `world entries must be tagged as ${{expectedTag}}`);
    }}
    const dependencyName = String(entry.__fields?.[0]?.dependencyName ?? '');
    if (!dependencyName) {{
      __sigil_runtime_fail("{binding_kind}", 'world entries must include dependencyName');
    }}
    if (seen.has(dependencyName)) {{
      __sigil_runtime_fail("{duplicate_binding}", `duplicate world entry for '${{dependencyName}}'`);
    }}
    seen.add(dependencyName);
  }}
  return seen;
}}

function __sigil_runtime_read_world(configExports) {{
  const world = configExports.world;
  if (!world || typeof world !== 'object') {{
    __sigil_runtime_fail("{invalid_config}", "config module must export a 'world' value");
  }}
  for (const field of ['clock', 'fs', 'http', 'log', 'process', 'random', 'tcp', 'timer']) {{
    if (!(field in world)) {{
      __sigil_runtime_fail("{invalid_config}", `world is missing '${{field}}'`);
    }}
  }}
  return world;
}}

const __sigil_world_value = __sigil_runtime_read_world(__sigil_config_exports);
const __sigil_topology_info = __sigil_runtime_collect_topology(globalThis.__sigil_topology_exports ?? {{}});
if (__sigil_topology_info.envs.size > 0 && !__sigil_topology_info.envs.has(__sigil_world_env_name)) {{
  __sigil_runtime_fail("{env_not_found}", `environment '${{__sigil_world_env_name}}' not declared in src/topology.lib.sigil`);
}}
const __sigil_http_world_names = __sigil_runtime_collect_world_dependency_names(__sigil_world_value.http, 'HttpEntry');
const __sigil_tcp_world_names = __sigil_runtime_collect_world_dependency_names(__sigil_world_value.tcp, 'TcpEntry');
for (const dependencyName of __sigil_topology_info.http) {{
  if (!__sigil_http_world_names.has(dependencyName)) {{
    __sigil_runtime_fail("{missing_binding}", `missing HTTP world entry for '${{dependencyName}}' in environment '${{__sigil_world_env_name}}'`);
  }}
}}
for (const dependencyName of __sigil_topology_info.tcp) {{
  if (!__sigil_tcp_world_names.has(dependencyName)) {{
    __sigil_runtime_fail("{missing_binding}", `missing TCP world entry for '${{dependencyName}}' in environment '${{__sigil_world_env_name}}'`);
  }}
}}
for (const dependencyName of __sigil_http_world_names) {{
  if (__sigil_topology_info.http.size > 0 && !__sigil_topology_info.http.has(dependencyName)) {{
    __sigil_runtime_fail("{invalid_handle}", `HTTP world entry references undeclared dependency '${{dependencyName}}'`);
  }}
}}
for (const dependencyName of __sigil_tcp_world_names) {{
  if (__sigil_topology_info.tcp.size > 0 && !__sigil_topology_info.tcp.has(dependencyName)) {{
    __sigil_runtime_fail("{invalid_handle}", `TCP world entry references undeclared dependency '${{dependencyName}}'`);
  }}
}}
globalThis.__sigil_world_env_name = __sigil_world_env_name;
globalThis.__sigil_world_value = __sigil_world_value;
globalThis.__sigil_world_template_cache = undefined;
globalThis.__sigil_world_current = undefined;
"#,
        topology_import = topology_url.map_or_else(
            || "globalThis.__sigil_topology_exports = null;".to_string(),
            |topology_url| {
                format!(
                    r#"const __sigil_topology_module = await import("{topology_url}");
globalThis.__sigil_topology_exports = Object.fromEntries(
  await Promise.all(
    Object.entries(__sigil_topology_module).map(async ([key, value]) => [key, await Promise.resolve(value)])
  )
);"#
                )
            }
        ),
        config_url = config_url,
        env_name_json = env_name_json,
        invalid_handle = codes::topology::INVALID_HANDLE,
        binding_kind = codes::topology::BINDING_KIND_MISMATCH,
        missing_binding = codes::topology::MISSING_BINDING,
        duplicate_binding = codes::topology::DUPLICATE_BINDING,
        env_not_found = codes::topology::ENV_NOT_FOUND,
        invalid_config = codes::topology::INVALID_CONFIG_MODULE,
    ))
}

fn standalone_world_runtime_setup_source(module_url: &str) -> String {
    let module_url_json = serde_json::to_string(module_url).unwrap();
    format!(
        r#"
async function __sigil_runtime_resolve_exports(moduleExports) {{
  return Object.fromEntries(
    await Promise.all(
      Object.entries(moduleExports ?? {{}}).map(async ([key, value]) => [key, await Promise.resolve(value)])
    )
  );
}}

function __sigil_runtime_collect_local_topology(moduleExports) {{
  const envs = new Set();
  const fsRoots = new Set();
  const http = new Set();
  const logSinks = new Set();
  const processHandles = new Set();
  const tcp = new Set();
  for (const value of Object.values(moduleExports ?? {{}})) {{
    if (value?.__tag === 'Environment') {{
      envs.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'FsRoot') {{
      fsRoots.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'HttpServiceDependency') {{
      http.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'LogSink') {{
      logSinks.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'ProcessHandle') {{
      processHandles.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'TcpServiceDependency') {{
      tcp.add(String(value.__fields?.[0] ?? ''));
    }}
  }}
  return {{ envs, fsRoots, http, logSinks, processHandles, tcp }};
}}

function __sigil_runtime_local_topology_declared(topology) {{
  return topology.envs.size > 0 ||
    topology.fsRoots.size > 0 ||
    topology.http.size > 0 ||
    topology.logSinks.size > 0 ||
    topology.processHandles.size > 0 ||
    topology.tcp.size > 0;
}}

globalThis.__sigil_runtime_apply_program_world = async (moduleExports) => {{
  const resolvedExports = await __sigil_runtime_resolve_exports(moduleExports);
  const topology = __sigil_runtime_collect_local_topology(resolvedExports);
  const hasWorld = !!resolvedExports && Object.prototype.hasOwnProperty.call(resolvedExports, 'world');
  if (!hasWorld && __sigil_runtime_local_topology_declared(topology)) {{
    const error = new Error("{local_world_required}: standalone topology programs must export c world");
    error.sigilCode = "{local_world_required}";
    throw error;
  }}
  globalThis.__sigil_program_exports = resolvedExports ?? null;
  globalThis.__sigil_topology_exports = hasWorld ? (resolvedExports ?? null) : null;
  globalThis.__sigil_world_env_name = null;
  globalThis.__sigil_world_value = hasWorld ? resolvedExports.world : null;
  globalThis.__sigil_world_template_cache = undefined;
  globalThis.__sigil_world_current = undefined;
}};

const __sigil_program_module = await import({module_url_json});
await globalThis.__sigil_runtime_apply_program_world(__sigil_program_module);
"#,
        local_world_required = codes::topology::LOCAL_WORLD_REQUIRED,
        module_url_json = module_url_json
    )
}

fn project_root_and_runtime(path: &Path) -> Result<Option<(PathBuf, bool)>, crate::project::ProjectConfigError> {
    let Some(project) = get_project_config(path)? else {
        return Ok(None);
    };
    let topology_present = topology_source_path(&project.root).exists();
    Ok(Some((project.root, topology_present)))
}

pub(super) fn runner_prelude(
    path: &Path,
    selected_env: Option<&str>,
    entry_output_path: &Path,
) -> Result<Option<String>, CliError> {
    let Some((project_root, topology_present)) = project_root_and_runtime(path)? else {
        let module_url = format!("file://{}", fs::canonicalize(entry_output_path)?.display());
        return Ok(Some(standalone_world_runtime_setup_source(&module_url)));
    };

    if !topology_present {
        return Ok(None);
    }

    let env_name = selected_env.ok_or_else(|| {
        CliError::Validation(format!(
            "{}: runtime-world projects require --env <name>",
            codes::topology::ENV_REQUIRED
        ))
    })?;

    build_world_runtime_prelude(&project_root, env_name, topology_present).map(Some)
}

fn build_imported_namespaces(
    module: &LoadedModule,
    compiled_modules: &HashMap<String, HashMap<String, InferenceType>>,
) -> HashMap<String, InferenceType> {
    let mut imported = HashMap::new();

    for (module_id, types) in compiled_modules {
        let mut fields = HashMap::new();
        for (name, typ) in types {
            fields.insert(
                name.clone(),
                qualify_inference_type_in_context(typ, module_id),
            );
        }

        imported.insert(
            module_id.clone(),
            InferenceType::Record(TRecord {
                fields,
                name: Some(module_id.clone()),
            }),
        );
    }

    for (source_module_id, resolved_module_id) in &module.source_imports {
        if let Some(namespace) = imported.get(resolved_module_id).cloned() {
            let aliased_namespace = if let (Some(public_package_root), Some(internal_package_root)) = (
                public_package_root(source_module_id),
                internal_package_root(resolved_module_id),
            ) {
                rewrite_public_package_inference_type(
                    &namespace,
                    &internal_package_root,
                    &public_package_root,
                )
            } else {
                namespace
            };
            imported.insert(source_module_id.clone(), aliased_namespace);
        }
    }

    imported
}

fn is_core_prelude_name(name: &str) -> bool {
    matches!(
        name,
        "ConcurrentOutcome"
            | "Option"
            | "Result"
            | "Aborted"
            | "Failure"
            | "Success"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
    )
}

fn qualify_inference_type_in_context(typ: &InferenceType, module_id: &str) -> InferenceType {
    match typ {
        InferenceType::Primitive(_) | InferenceType::Var(_) | InferenceType::Any => typ.clone(),
        InferenceType::Function(func) => InferenceType::Function(Box::new(TFunction {
            params: func
                .params
                .iter()
                .map(|param| qualify_inference_type_in_context(param, module_id))
                .collect(),
            return_type: qualify_inference_type_in_context(&func.return_type, module_id),
            effects: func.effects.clone(),
        })),
        InferenceType::List(list) => InferenceType::List(Box::new(TList {
            element_type: qualify_inference_type_in_context(&list.element_type, module_id),
        })),
        InferenceType::Map(map) => InferenceType::Map(Box::new(TMap {
            key_type: qualify_inference_type_in_context(&map.key_type, module_id),
            value_type: qualify_inference_type_in_context(&map.value_type, module_id),
        })),
        InferenceType::Tuple(tuple) => InferenceType::Tuple(TTuple {
            types: tuple
                .types
                .iter()
                .map(|item| qualify_inference_type_in_context(item, module_id))
                .collect(),
        }),
        InferenceType::Record(record) => InferenceType::Record(TRecord {
            fields: record
                .fields
                .iter()
                .map(|(name, field_type)| {
                    (
                        name.clone(),
                        qualify_inference_type_in_context(field_type, module_id),
                    )
                })
                .collect(),
            name: record.name.as_ref().map(|name| {
                if is_core_prelude_name(name) {
                    name.clone()
                } else if name.contains("::") {
                    name.clone()
                } else if name.contains('.') {
                    name.clone()
                } else {
                    format!("{}.{}", module_id, name)
                }
            }),
        }),
        InferenceType::Constructor(constructor) => InferenceType::Constructor(TConstructor {
            name: if is_core_prelude_name(&constructor.name) {
                constructor.name.clone()
            } else if constructor.name.contains("::") || constructor.name.contains('.') {
                constructor.name.clone()
            } else {
                format!("{}.{}", module_id, constructor.name)
            },
            type_args: constructor
                .type_args
                .iter()
                .map(|arg| qualify_inference_type_in_context(arg, module_id))
                .collect(),
        }),
    }
}

fn build_imported_type_registries(
    module: &LoadedModule,
    type_registries: &HashMap<String, HashMap<String, TypeInfo>>,
) -> HashMap<String, HashMap<String, TypeInfo>> {
    let mut imported = type_registries.clone();

    for (source_module_id, resolved_module_id) in &module.source_imports {
        if let Some(registry) = type_registries.get(resolved_module_id) {
            let aliased_registry = if let (Some(public_package_root), Some(internal_package_root)) = (
                public_package_root(source_module_id),
                internal_package_root(resolved_module_id),
            ) {
                registry
                    .iter()
                    .map(|(name, info)| {
                        (
                            name.clone(),
                            rewrite_public_package_type_info(
                                info,
                                &internal_package_root,
                                &public_package_root,
                            ),
                        )
                    })
                    .collect()
            } else {
                registry.clone()
            };
            imported.insert(source_module_id.clone(), aliased_registry);
        }
    }

    imported
}

fn build_imported_label_registries(
    module: &LoadedModule,
    label_registries: &HashMap<String, HashMap<String, LabelInfo>>,
) -> HashMap<String, HashMap<String, LabelInfo>> {
    build_imported_registry_map(module, label_registries)
}

fn build_imported_value_schemes(
    module: &LoadedModule,
    compiled_schemes: &HashMap<String, HashMap<String, TypeScheme>>,
) -> HashMap<String, HashMap<String, TypeScheme>> {
    let mut imported = HashMap::new();

    for (source_module_id, resolved_module_id) in &module.source_imports {
        let Some(schemes) = compiled_schemes.get(resolved_module_id) else {
            continue;
        };
        imported.insert(
            source_module_id.clone(),
            schemes
                .iter()
                .map(|(name, scheme)| {
                    (
                        name.clone(),
                        imported_value_scheme_for_module(
                            source_module_id,
                            resolved_module_id,
                            scheme,
                        ),
                    )
                })
                .collect(),
        );
    }

    imported
}

fn build_imported_value_meta(
    module: &LoadedModule,
    compiled_value_meta: &HashMap<String, HashMap<String, BindingMeta>>,
) -> HashMap<String, HashMap<String, BindingMeta>> {
    build_imported_registry_map(module, compiled_value_meta)
}

fn imported_value_scheme_for_module(
    source_module_id: &str,
    resolved_module_id: &str,
    scheme: &TypeScheme,
) -> TypeScheme {
    let qualification_module_id =
        imported_value_qualification_module_id(source_module_id, resolved_module_id);
    let qualified = qualify_scheme_for_module(&qualification_module_id, scheme);

    let Some(public_package_root) = public_package_root(source_module_id) else {
        return qualified;
    };
    let Some(internal_package_root) = internal_package_root(resolved_module_id) else {
        return qualified;
    };

    TypeScheme {
        quantified_vars: qualified.quantified_vars,
        typ: rewrite_public_package_inference_type(
            &qualified.typ,
            &internal_package_root,
            &public_package_root,
        ),
    }
}

fn imported_value_qualification_module_id(
    source_module_id: &str,
    resolved_module_id: &str,
) -> String {
    if source_module_id.starts_with("package::") {
        source_module_id.to_string()
    } else {
        resolved_module_id.to_string()
    }
}

fn public_package_root(module_id: &str) -> Option<String> {
    let parts = module_id.split("::").collect::<Vec<_>>();
    if parts.len() < 2 || parts[0] != "package" {
        return None;
    }

    Some(format!("{}::{}", parts[0], parts[1]))
}

fn internal_package_root(module_id: &str) -> Option<String> {
    let parts = module_id.split("::").collect::<Vec<_>>();
    if parts.len() < 3 || parts[0] != "package" {
        return None;
    }

    Some(format!("{}::{}::{}", parts[0], parts[1], parts[2]))
}

fn public_package_module_id(module_id: &str) -> Option<String> {
    let parts = module_id.split("::").collect::<Vec<_>>();
    if parts.len() < 3 || parts[0] != "package" {
        return None;
    }

    Some(
        [vec![parts[0].to_string(), parts[1].to_string()], parts[3..].iter().map(|part| (*part).to_string()).collect()]
            .concat()
            .join("::"),
    )
}

fn rewrite_public_package_name(name: &str, internal_root: &str, public_root: &str) -> String {
    name.strip_prefix(internal_root)
        .map(|suffix| format!("{public_root}{suffix}"))
        .unwrap_or_else(|| name.to_string())
}

fn rewrite_public_package_inference_type(
    typ: &sigil_typechecker::InferenceType,
    internal_root: &str,
    public_root: &str,
) -> sigil_typechecker::InferenceType {
    use sigil_typechecker::types::{TConstructor, TFunction, TList, TMap, TRecord, TTuple, TVar};
    use sigil_typechecker::InferenceType;

    match typ {
        InferenceType::Primitive(_) | InferenceType::Any => typ.clone(),
        InferenceType::Var(var) => InferenceType::Var(Box::new(TVar {
            id: var.id,
            name: var.name.clone(),
            instance: var.instance.as_ref().map(|instance| {
                rewrite_public_package_inference_type(instance, internal_root, public_root)
            }),
        })),
        InferenceType::Function(function) => InferenceType::Function(Box::new(TFunction {
            params: function
                .params
                .iter()
                .map(|param| rewrite_public_package_inference_type(param, internal_root, public_root))
                .collect(),
            return_type: rewrite_public_package_inference_type(
                &function.return_type,
                internal_root,
                public_root,
            ),
            effects: function.effects.clone(),
        })),
        InferenceType::List(list) => InferenceType::List(Box::new(TList {
            element_type: rewrite_public_package_inference_type(
                &list.element_type,
                internal_root,
                public_root,
            ),
        })),
        InferenceType::Map(map) => InferenceType::Map(Box::new(TMap {
            key_type: rewrite_public_package_inference_type(&map.key_type, internal_root, public_root),
            value_type: rewrite_public_package_inference_type(
                &map.value_type,
                internal_root,
                public_root,
            ),
        })),
        InferenceType::Tuple(tuple) => InferenceType::Tuple(TTuple {
            types: tuple
                .types
                .iter()
                .map(|item| rewrite_public_package_inference_type(item, internal_root, public_root))
                .collect(),
        }),
        InferenceType::Record(record) => InferenceType::Record(TRecord {
            fields: record
                .fields
                .iter()
                .map(|(name, field_type)| {
                    (
                        name.clone(),
                        rewrite_public_package_inference_type(
                            field_type,
                            internal_root,
                            public_root,
                        ),
                    )
                })
                .collect(),
            name: record
                .name
                .as_ref()
                .map(|name| rewrite_public_package_name(name, internal_root, public_root)),
        }),
        InferenceType::Constructor(constructor) => InferenceType::Constructor(TConstructor {
            name: rewrite_public_package_name(&constructor.name, internal_root, public_root),
            type_args: constructor
                .type_args
                .iter()
                .map(|arg| rewrite_public_package_inference_type(arg, internal_root, public_root))
                .collect(),
        }),
    }
}

fn rewrite_public_package_type_info(
    info: &TypeInfo,
    internal_root: &str,
    public_root: &str,
) -> TypeInfo {
    TypeInfo {
        type_params: info.type_params.clone(),
        definition: rewrite_public_package_type_def(&info.definition, internal_root, public_root),
        constraint: info.constraint.clone(),
        labels: info
            .labels
            .iter()
            .map(|label| rewrite_public_package_name(label, internal_root, public_root))
            .collect(),
    }
}

fn rewrite_public_package_type_def(
    definition: &TypeDef,
    internal_root: &str,
    public_root: &str,
) -> TypeDef {
    match definition {
        TypeDef::Sum(sum) => TypeDef::Sum(sigil_ast::SumType {
            variants: sum
                .variants
                .iter()
                .map(|variant| sigil_ast::Variant {
                    name: variant.name.clone(),
                    types: variant
                        .types
                        .iter()
                        .map(|typ| rewrite_public_package_ast_type(typ, internal_root, public_root))
                        .collect(),
                    location: variant.location,
                })
                .collect(),
            location: sum.location,
        }),
        TypeDef::Product(product) => TypeDef::Product(sigil_ast::ProductType {
            fields: product
                .fields
                .iter()
                .map(|field| sigil_ast::Field {
                    name: field.name.clone(),
                    field_type: rewrite_public_package_ast_type(
                        &field.field_type,
                        internal_root,
                        public_root,
                    ),
                    location: field.location,
                })
                .collect(),
            location: product.location,
        }),
        TypeDef::Alias(alias) => TypeDef::Alias(sigil_ast::TypeAlias {
            aliased_type: rewrite_public_package_ast_type(
                &alias.aliased_type,
                internal_root,
                public_root,
            ),
            location: alias.location,
        }),
    }
}

fn rewrite_public_package_ast_type(typ: &Type, internal_root: &str, public_root: &str) -> Type {
    match typ {
        Type::Primitive(_) | Type::Variable(_) => typ.clone(),
        Type::List(list) => Type::List(Box::new(sigil_ast::ListType {
            element_type: rewrite_public_package_ast_type(
                &list.element_type,
                internal_root,
                public_root,
            ),
            location: list.location,
        })),
        Type::Map(map) => Type::Map(Box::new(sigil_ast::MapType {
            key_type: rewrite_public_package_ast_type(&map.key_type, internal_root, public_root),
            value_type: rewrite_public_package_ast_type(
                &map.value_type,
                internal_root,
                public_root,
            ),
            location: map.location,
        })),
        Type::Function(function) => Type::Function(Box::new(sigil_ast::FunctionType {
            param_types: function
                .param_types
                .iter()
                .map(|param| rewrite_public_package_ast_type(param, internal_root, public_root))
                .collect(),
            effects: function.effects.clone(),
            return_type: rewrite_public_package_ast_type(
                &function.return_type,
                internal_root,
                public_root,
            ),
            location: function.location,
        })),
        Type::Constructor(constructor) => Type::Constructor(sigil_ast::TypeConstructor {
            name: rewrite_public_package_name(&constructor.name, internal_root, public_root),
            type_args: constructor
                .type_args
                .iter()
                .map(|arg| rewrite_public_package_ast_type(arg, internal_root, public_root))
                .collect(),
            location: constructor.location,
        }),
        Type::Tuple(tuple) => Type::Tuple(sigil_ast::TupleType {
            types: tuple
                .types
                .iter()
                .map(|item| rewrite_public_package_ast_type(item, internal_root, public_root))
                .collect(),
            location: tuple.location,
        }),
        Type::Qualified(qualified) => {
            let rewritten_module_id = rewrite_public_package_name(
                &qualified.module_path.join("::"),
                internal_root,
                public_root,
            );
            Type::Qualified(sigil_ast::QualifiedType {
                module_path: rewritten_module_id
                    .split("::")
                    .map(ToString::to_string)
                    .collect(),
                type_name: qualified.type_name.clone(),
                type_args: qualified
                    .type_args
                    .iter()
                    .map(|arg| rewrite_public_package_ast_type(arg, internal_root, public_root))
                    .collect(),
                location: qualified.location,
            })
        }
    }
}

fn build_imported_boundary_rules(
    _ast: &Program,
    compiled_boundary_rules: &[BoundaryRule],
) -> Vec<BoundaryRule> {
    compiled_boundary_rules.to_vec()
}

fn build_imported_registry_map<T: Clone>(
    module: &LoadedModule,
    registries: &HashMap<String, HashMap<String, T>>,
) -> HashMap<String, HashMap<String, T>> {
    let mut imported = registries.clone();

    for (source_module_id, resolved_module_id) in &module.source_imports {
        if let Some(registry) = registries.get(resolved_module_id) {
            imported.insert(source_module_id.clone(), registry.clone());
        }
    }

    imported
}

fn qualify_inference_type_for_module(
    module_id: &str,
    typ: &sigil_typechecker::InferenceType,
) -> sigil_typechecker::InferenceType {
    use sigil_typechecker::types::{TConstructor, TFunction, TList, TRecord, TTuple, TVar};
    use sigil_typechecker::InferenceType;

    match typ {
        InferenceType::Primitive(_) | InferenceType::Any => typ.clone(),
        InferenceType::Var(var) => InferenceType::Var(Box::new(TVar {
            id: var.id,
            name: var.name.clone(),
            instance: var
                .instance
                .as_ref()
                .map(|instance| qualify_inference_type_for_module(module_id, instance)),
        })),
        InferenceType::Function(function) => InferenceType::Function(Box::new(TFunction {
            params: function
                .params
                .iter()
                .map(|param| qualify_inference_type_for_module(module_id, param))
                .collect(),
            return_type: qualify_inference_type_for_module(module_id, &function.return_type),
            effects: function.effects.clone(),
        })),
        InferenceType::List(list) => InferenceType::List(Box::new(TList {
            element_type: qualify_inference_type_for_module(module_id, &list.element_type),
        })),
        InferenceType::Map(map) => InferenceType::Map(Box::new(TMap {
            key_type: qualify_inference_type_for_module(module_id, &map.key_type),
            value_type: qualify_inference_type_for_module(module_id, &map.value_type),
        })),
        InferenceType::Tuple(tuple) => InferenceType::Tuple(TTuple {
            types: tuple
                .types
                .iter()
                .map(|item| qualify_inference_type_for_module(module_id, item))
                .collect(),
        }),
        InferenceType::Record(record) => InferenceType::Record(TRecord {
            fields: record
                .fields
                .iter()
                .map(|(name, field_type)| {
                    (
                        name.clone(),
                        qualify_inference_type_for_module(module_id, field_type),
                    )
                })
                .collect(),
            name: record.name.as_ref().map(|name| {
                if is_core_prelude_name(name) {
                    name.clone()
                } else if name.contains('.') {
                    name.clone()
                } else {
                    format!("{}.{}", module_id, name)
                }
            }),
        }),
        InferenceType::Constructor(constructor) => InferenceType::Constructor(TConstructor {
            name: if is_core_prelude_name(&constructor.name) {
                constructor.name.clone()
            } else if constructor.name.contains('.') {
                constructor.name.clone()
            } else {
                format!("{}.{}", module_id, constructor.name)
            },
            type_args: constructor
                .type_args
                .iter()
                .map(|arg| qualify_inference_type_for_module(module_id, arg))
                .collect(),
        }),
    }
}

fn qualify_scheme_for_module(module_id: &str, scheme: &TypeScheme) -> TypeScheme {
    TypeScheme {
        quantified_vars: scheme.quantified_vars.clone(),
        typ: qualify_inference_type_for_module(module_id, &scheme.typ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_manager::package_install_command;
    use sigil_typechecker::errors::format_type;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn imported_package_value_schemes_use_public_package_type_names() {
        let project_root = repo_root().join("projects/featureFlagStorefront");
        package_install_command(&project_root).unwrap();
        let config_path = project_root.join("config/test.lib.sigil");
        let graph = ModuleGraph::build_with_env(&config_path, Some("test")).unwrap();
        let mut compiled_modules = HashMap::new();
        let mut compiled_schemes = HashMap::new();
        let mut compiled_value_meta = HashMap::new();
        let mut label_registries = HashMap::new();
        let mut compiled_boundary_rules = Vec::new();
        let mut type_registries = HashMap::new();

        for module_id in &graph.topo_order {
            let module = &graph.modules[module_id];
            if module_id == "config::test" {
                let imported_namespaces = build_imported_namespaces(module, &compiled_modules);
                let imported_schemes = build_imported_value_schemes(module, &compiled_schemes);
                let flag_namespace = imported_namespaces
                    .get("package::featureFlagStorefrontFlags::flags")
                    .unwrap();
                let constructor_scheme = imported_schemes
                    .get("package::featureFlagStorefrontFlags::types")
                    .and_then(|schemes| schemes.get("Ocean"))
                    .unwrap_or_else(|| {
                        panic!(
                            "missing package constructor scheme; imported modules: {:?}",
                            imported_schemes.keys().collect::<Vec<_>>()
                        )
                    });

                assert_eq!(
                    format_type(
                        match flag_namespace {
                            InferenceType::Record(record) => {
                                record.fields.get("CheckoutColorChoice").unwrap()
                            }
                            _ => panic!("package flags namespace did not resolve to a record"),
                        }
                    ),
                    "stdlib::featureFlags.Flag[package::featureFlagStorefrontFlags::types.CheckoutColor]"
                );
                assert_eq!(
                    format_type(&constructor_scheme.typ),
                    "() => package::featureFlagStorefrontFlags::types.CheckoutColor"
                );
                return;
            }

            let imported_namespaces = build_imported_namespaces(module, &compiled_modules);
            let imported_type_regs = build_imported_type_registries(module, &type_registries);
            let imported_label_regs = build_imported_label_registries(module, &label_registries);
            let imported_value_schemes = build_imported_value_schemes(module, &compiled_schemes);
            let imported_value_meta = build_imported_value_meta(module, &compiled_value_meta);
            let imported_boundary_rules =
                build_imported_boundary_rules(&module.ast, &compiled_boundary_rules);
            let effect_catalog = load_project_effect_catalog_for(&module.file_path).unwrap();

            let typecheck_result = type_check(
                &module.ast,
                &module.source,
                Some(TypeCheckOptions {
                    effect_catalog,
                    imported_namespaces: Some(imported_namespaces),
                    imported_type_registries: Some(imported_type_regs.clone()),
                    imported_label_registries: Some(imported_label_regs),
                    imported_value_schemes: Some(imported_value_schemes),
                    imported_value_meta: Some(imported_value_meta),
                    boundary_rules: Some(imported_boundary_rules),
                    module_id: Some(module_id.clone()),
                    source_file: Some(module.file_path.to_string_lossy().to_string()),
                }),
            )
            .unwrap();

            let extracted_type_registry =
                extract_type_registry(&module.ast, &module.file_path, module_id);
            compiled_schemes.insert(module_id.clone(), typecheck_result.declaration_schemes);
            compiled_modules.insert(module_id.clone(), typecheck_result.declaration_types);
            compiled_value_meta.insert(module_id.clone(), typecheck_result.declaration_meta);
            label_registries.insert(module_id.clone(), typecheck_result.label_registry);
            compiled_boundary_rules.extend(typecheck_result.boundary_rules);
            type_registries.insert(module_id.clone(), extracted_type_registry);
        }

        panic!("config::test was not part of the featureFlagStorefront module graph");
    }
}

fn qualify_label_ref(label_ref: &LabelRef, module_id: &str) -> String {
    if label_ref.module_path.is_empty() {
        format!("{}.{}", module_id, label_ref.name)
    } else {
        format!("{}.{}", label_ref.module_path.join("::"), label_ref.name)
    }
}

fn qualify_label_refs(label_refs: &[LabelRef], module_id: &str) -> BTreeSet<String> {
    label_refs
        .iter()
        .map(|label_ref| qualify_label_ref(label_ref, module_id))
        .collect()
}

fn qualify_type_in_context(
    ast_type: &Type,
    module_id: &str,
    local_type_registry: &HashMap<String, TypeInfo>,
    type_params: &[String],
) -> Type {
    match ast_type {
        Type::Primitive(_) => ast_type.clone(),
        Type::Qualified(qualified) => Type::Qualified(sigil_ast::QualifiedType {
            module_path: qualified.module_path.clone(),
            type_name: qualified.type_name.clone(),
            type_args: qualified
                .type_args
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect(),
            location: qualified.location,
        }),
        Type::List(list_type) => Type::List(Box::new(sigil_ast::ListType {
            element_type: qualify_type_in_context(
                &list_type.element_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: list_type.location,
        })),
        Type::Map(map_type) => Type::Map(Box::new(sigil_ast::MapType {
            key_type: qualify_type_in_context(
                &map_type.key_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            value_type: qualify_type_in_context(
                &map_type.value_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: map_type.location,
        })),
        Type::Function(func_type) => Type::Function(Box::new(sigil_ast::FunctionType {
            param_types: func_type
                .param_types
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect(),
            effects: func_type.effects.clone(),
            return_type: qualify_type_in_context(
                &func_type.return_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: func_type.location,
        })),
        Type::Tuple(tuple_type) => Type::Tuple(sigil_ast::TupleType {
            types: tuple_type
                .types
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect(),
            location: tuple_type.location,
        }),
        Type::Variable(var_type) => {
            if is_core_prelude_name(&var_type.name)
                || type_params.contains(&var_type.name)
                || !local_type_registry.contains_key(&var_type.name)
            {
                return ast_type.clone();
            }

            Type::Qualified(sigil_ast::QualifiedType {
                module_path: module_id.split("::").map(|s| s.to_string()).collect(),
                type_name: var_type.name.clone(),
                type_args: vec![],
                location: var_type.location,
            })
        }
        Type::Constructor(constructor) => {
            let qualified_args = constructor
                .type_args
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect();

            if local_type_registry.contains_key(&constructor.name)
                && !type_params.contains(&constructor.name)
            {
                Type::Qualified(sigil_ast::QualifiedType {
                    module_path: module_id.split("::").map(|s| s.to_string()).collect(),
                    type_name: constructor.name.clone(),
                    type_args: qualified_args,
                    location: constructor.location,
                })
            } else {
                Type::Constructor(sigil_ast::TypeConstructor {
                    name: constructor.name.clone(),
                    type_args: qualified_args,
                    location: constructor.location,
                })
            }
        }
    }
}

fn qualify_type_def(
    type_def: &TypeDef,
    module_id: &str,
    local_type_registry: &HashMap<String, TypeInfo>,
    type_params: &[String],
) -> TypeDef {
    match type_def {
        TypeDef::Product(product) => TypeDef::Product(sigil_ast::ProductType {
            fields: product
                .fields
                .iter()
                .map(|field| sigil_ast::Field {
                    name: field.name.clone(),
                    field_type: qualify_type_in_context(
                        &field.field_type,
                        module_id,
                        local_type_registry,
                        type_params,
                    ),
                    location: field.location,
                })
                .collect(),
            location: product.location,
        }),
        TypeDef::Alias(alias) => TypeDef::Alias(sigil_ast::TypeAlias {
            aliased_type: qualify_type_in_context(
                &alias.aliased_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: alias.location,
        }),
        TypeDef::Sum(sum) => TypeDef::Sum(sigil_ast::SumType {
            variants: sum
                .variants
                .iter()
                .map(|variant| sigil_ast::Variant {
                    name: variant.name.clone(),
                    types: variant
                        .types
                        .iter()
                        .map(|ty| {
                            qualify_type_in_context(ty, module_id, local_type_registry, type_params)
                        })
                        .collect(),
                    location: variant.location,
                })
                .collect(),
            location: sum.location,
        }),
    }
}

fn extract_type_registry(
    ast: &Program,
    file_path: &Path,
    module_id: &str,
) -> HashMap<String, TypeInfo> {
    let mut registry = HashMap::new();
    let is_lib_file = file_path.to_string_lossy().ends_with(".lib.sigil");

    let mut local_type_registry = HashMap::new();
    for decl in &ast.declarations {
        if let Declaration::Type(type_decl) = decl {
            local_type_registry.insert(
                type_decl.name.clone(),
                TypeInfo {
                    type_params: type_decl.type_params.clone(),
                    definition: type_decl.definition.clone(),
                    constraint: type_decl.constraint.clone(),
                    labels: qualify_label_refs(&type_decl.labels, module_id),
                },
            );
        }
    }

    for decl in &ast.declarations {
        if let Declaration::Type(type_decl) = decl {
            if is_lib_file {
                registry.insert(
                    type_decl.name.clone(),
                    TypeInfo {
                        type_params: type_decl.type_params.clone(),
                        definition: qualify_type_def(
                            &type_decl.definition,
                            module_id,
                            &local_type_registry,
                            &type_decl.type_params,
                        ),
                        constraint: type_decl.constraint.clone(),
                        labels: qualify_label_refs(&type_decl.labels, module_id),
                    },
                );
            }
        }
    }

    registry
}

fn coverage_variant_names_for_type_def(type_def: &TypeDef) -> Vec<String> {
    match type_def {
        TypeDef::Sum(sum) => sum
            .variants
            .iter()
            .map(|variant| variant.name.clone())
            .collect(),
        _ => Vec::new(),
    }
}

fn coverage_expected_variants(
    return_type: &InferenceType,
    local_type_registry: &HashMap<String, TypeInfo>,
    imported_type_regs: &HashMap<String, HashMap<String, TypeInfo>>,
) -> Vec<String> {
    let InferenceType::Constructor(constructor) = return_type else {
        return Vec::new();
    };

    match constructor.name.as_str() {
        "Option" => return vec!["None".to_string(), "Some".to_string()],
        "Result" => return vec!["Err".to_string(), "Ok".to_string()],
        _ => {}
    }

    if let Some(info) = local_type_registry.get(&constructor.name) {
        let variants = coverage_variant_names_for_type_def(&info.definition);
        if !variants.is_empty() {
            return variants;
        }
    }

    let mut imported_matches = imported_type_regs
        .values()
        .filter_map(|registry| registry.get(&constructor.name))
        .map(|info| coverage_variant_names_for_type_def(&info.definition))
        .filter(|variants| !variants.is_empty())
        .collect::<Vec<_>>();

    if imported_matches.len() == 1 {
        return imported_matches.pop().unwrap();
    }

    Vec::new()
}

fn collect_module_coverage_targets(
    module: &LoadedModule,
    typed_program: &TypedProgram,
    imported_type_regs: &HashMap<String, HashMap<String, TypeInfo>>,
    local_type_registry: &HashMap<String, TypeInfo>,
) -> Vec<CoverageTarget> {
    let Some(project) = &module.project else {
        return Vec::new();
    };

    let normalized_path = module.file_path.to_string_lossy().replace('\\', "/");
    let normalized_root = project.root.to_string_lossy().replace('\\', "/");
    if !normalized_path.starts_with(&normalized_root) || !normalized_path.contains("/src/") {
        return Vec::new();
    }
    if normalized_path.contains("/tests/") {
        return Vec::new();
    }

    let is_lib_file = normalized_path.ends_with(".lib.sigil");
    let is_exec_file = normalized_path.ends_with(".sigil") && !is_lib_file;
    if !is_lib_file && !is_exec_file {
        return Vec::new();
    }

    let mut targets = Vec::new();

    for decl in &typed_program.declarations {
        let TypedDeclaration::Function(function) = decl else {
            continue;
        };

        let expected_variants = coverage_expected_variants(
            &function.return_type,
            local_type_registry,
            imported_type_regs,
        );
        let id = format!("{}::{}", module.id, function.name);
        targets.push(CoverageTarget {
            id,
            expected_variants,
            file: normalized_path.clone(),
            function_name: function.name.clone(),
            location: SourcePoint {
                line: function.location.start.line,
                column: function.location.start.column,
            },
        });
    }

    targets
}

fn get_module_output_path(module: &LoadedModule, output_flavor: OutputFlavor) -> PathBuf {
    use std::env;
    use std::fs;

    if let Some(project) = module
        .output_project
        .clone()
        .or_else(|| module.project.clone())
    {
        let path_str = module.id.replace("::", "/");
        return project
            .root
            .join(&project.layout.out)
            .join(format!("{}.{}", path_str, output_flavor.output_extension()));
    }

    let abs_source =
        fs::canonicalize(&module.file_path).unwrap_or_else(|_| module.file_path.clone());
    let mut repo_root = abs_source.parent().unwrap().to_path_buf();

    while !repo_root.join(".git").exists() {
        if let Some(parent) = repo_root.parent() {
            repo_root = parent.to_path_buf();
        } else {
            repo_root = env::current_dir().unwrap();
            break;
        }
    }

    if module.id.contains("::") {
        return repo_root
            .join(".local")
            .join(format!(
                "{}.{}",
                module.id.replace("::", "/"),
                output_flavor.output_extension()
            ));
    }

    let rel_source = abs_source.strip_prefix(&repo_root).unwrap_or(&abs_source);
    let rel_source = rel_source.strip_prefix(".local").unwrap_or(rel_source);

    let mut output = repo_root.join(".local");
    if let Some(parent) = rel_source.parent() {
        output = output.join(parent);
    }
    if let Some(file_name) = rel_source.file_name().and_then(|name| name.to_str()) {
        let stem = file_name
            .strip_suffix(".lib.sigil")
            .or_else(|| file_name.strip_suffix(".sigil"))
            .unwrap_or(file_name);
        output = output.join(format!("{}.{}", stem, output_flavor.output_extension()));
    }

    output
}
