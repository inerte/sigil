use super::compile_support::analyze_module_graph;
use super::legacy::CliError;
use super::shared::output_json_value;
use crate::module_graph::{ModuleGraph, ModuleGraphError};
use serde::Serialize;
use serde_json::json;
use sigil_ast::{
    Declaration, ExternDecl, FeatureFlagDecl, FunctionDecl, Param, Program, TestDecl, TransformDecl,
};
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use sigil_typechecker::typed_ir::{TypedDeclaration, TypedFunctionDecl, TypedTestDecl};
use sigil_typechecker::types::{InferenceType, TypeScheme};
use sigil_validator::{
    print_canonical_expr, print_canonical_program, print_canonical_type,
    print_canonical_type_definition,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const REVIEW_COMMAND: &str = "sigil review";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum AnalysisMode {
    Typed,
    ParseOnly,
}

#[derive(Clone, Debug)]
enum SnapshotSource {
    Revision(String),
    Index,
    Worktree,
}

impl SnapshotSource {
    fn label(&self) -> String {
        match self {
            Self::Revision(rev) => format!("revision:{rev}"),
            Self::Index => "index".to_string(),
            Self::Worktree => "worktree".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct ReviewSelection {
    mode: String,
    before: SnapshotSource,
    after: SnapshotSource,
    git_diff_args: Vec<String>,
}

#[derive(Clone, Debug)]
struct DiffEntry {
    status: char,
    before_path: Option<String>,
    after_path: Option<String>,
}

impl DiffEntry {
    fn supported(&self) -> bool {
        self.before_path
            .as_deref()
            .is_some_and(is_supported_review_path)
            || self
                .after_path
                .as_deref()
                .is_some_and(is_supported_review_path)
    }
}

#[derive(Clone, Debug)]
struct SnapshotAnalysis {
    coverage_targets: BTreeMap<(String, String), ReviewCoverageTarget>,
    files: BTreeMap<String, ReviewFileSnapshot>,
    issues: Vec<ReviewIssue>,
}

#[derive(Clone, Debug)]
struct ReviewFileSnapshot {
    analysis_mode: AnalysisMode,
    declarations: BTreeMap<String, ReviewDeclaration>,
    module_id: Option<String>,
    path: String,
}

#[derive(Clone, Debug)]
struct ReviewDeclaration {
    facts: DeclarationFacts,
    key: String,
    kind: String,
    line: usize,
    name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeclarationFacts {
    surface: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    effects: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decreases: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ensures: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    definition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    constraint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_expr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    module_path: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    members: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewChangeSide {
    analysis_mode: AnalysisMode,
    facts: DeclarationFacts,
    line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    module_id: Option<String>,
    path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewChange {
    after: Option<ReviewChangeSide>,
    before: Option<ReviewChangeSide>,
    change_kinds: Vec<String>,
    declaration_key: String,
    declaration_kind: String,
    declaration_name: String,
    status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewIssue {
    kind: String,
    message: String,
    phase: String,
    severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewCoverageTarget {
    file: String,
    function_name: String,
    id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewTestEvidence {
    changed_coverage_targets: Vec<ReviewCoverageTarget>,
    changed_test_declarations: Vec<String>,
    changed_test_files: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewSummary {
    changed_coverage_targets: usize,
    changed_declarations: usize,
    changed_files: usize,
    changed_test_files: usize,
    compile_issues: usize,
    contract_changes: usize,
    effect_changes: usize,
    implementation_changes: usize,
    signature_changes: usize,
    trust_surface_changes: usize,
    type_changes: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewScope {
    after: String,
    before: String,
    git_args: Vec<String>,
    mode: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewData {
    changes: Vec<ReviewChange>,
    issues: Vec<ReviewIssue>,
    scope: ReviewScope,
    summary: ReviewSummary,
    test_evidence: ReviewTestEvidence,
}

#[derive(Debug)]
struct SnapshotDir {
    path: PathBuf,
}

impl SnapshotDir {
    fn create(repo_root: &Path, source: &SnapshotSource, label: &str) -> Result<Self, CliError> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = repo_root
            .join(".sigil")
            .join("review")
            .join(format!("{label}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&dir)?;
        let snapshot = Self { path: dir };
        materialize_snapshot(repo_root, source, snapshot.path())?;
        Ok(snapshot)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SnapshotDir {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!(
                "warning: failed to clean up review snapshot `{}`: {}",
                self.path.display(),
                error
            );
        }
    }
}

pub fn review_command(
    json_output: bool,
    llm_output: bool,
    staged: bool,
    base: Option<&str>,
    head: Option<&str>,
    path_filters: &[PathBuf],
    git_diff_args: &[String],
) -> Result<(), CliError> {
    let cwd = std::env::current_dir()?;
    let repo_root = git_repo_root(&cwd)?;
    let selection = resolve_selection(&repo_root, staged, base, head, path_filters, git_diff_args)?;
    let diff_entries = git_diff_entries(&repo_root, &selection.git_diff_args)?;
    let supported_entries = diff_entries
        .into_iter()
        .filter(DiffEntry::supported)
        .collect::<Vec<_>>();

    if supported_entries.is_empty() {
        let data = ReviewData {
            changes: Vec::new(),
            issues: Vec::new(),
            scope: ReviewScope {
                after: selection.after.label(),
                before: selection.before.label(),
                git_args: selection.git_diff_args.clone(),
                mode: selection.mode.clone(),
            },
            summary: ReviewSummary {
                changed_coverage_targets: 0,
                changed_declarations: 0,
                changed_files: 0,
                changed_test_files: 0,
                compile_issues: 0,
                contract_changes: 0,
                effect_changes: 0,
                implementation_changes: 0,
                signature_changes: 0,
                trust_surface_changes: 0,
                type_changes: 0,
            },
            test_evidence: ReviewTestEvidence {
                changed_coverage_targets: Vec::new(),
                changed_test_declarations: Vec::new(),
                changed_test_files: Vec::new(),
            },
        };
        emit_review(&data, json_output, llm_output)?;
        return Ok(());
    }

    let before_paths = supported_entries
        .iter()
        .filter_map(|entry| entry.before_path.clone())
        .collect::<BTreeSet<_>>();
    let after_paths = supported_entries
        .iter()
        .filter_map(|entry| entry.after_path.clone())
        .collect::<BTreeSet<_>>();

    let before_dir = SnapshotDir::create(&repo_root, &selection.before, "before")?;
    let after_dir = SnapshotDir::create(&repo_root, &selection.after, "after")?;
    let before_analysis = analyze_snapshot(SnapshotSide::Before, before_dir.path(), &before_paths)?;
    let after_analysis = analyze_snapshot(SnapshotSide::After, after_dir.path(), &after_paths)?;

    let mut changes = Vec::new();
    for entry in &supported_entries {
        changes.extend(build_changes_for_entry(
            entry,
            &before_analysis,
            &after_analysis,
        ));
    }
    changes.sort_by(|left, right| {
        change_path(left)
            .cmp(&change_path(right))
            .then(left.declaration_kind.cmp(&right.declaration_kind))
            .then(left.declaration_name.cmp(&right.declaration_name))
            .then(left.declaration_key.cmp(&right.declaration_key))
    });

    let mut issues = before_analysis.issues;
    issues.extend(after_analysis.issues);

    let mut changed_test_files = supported_entries
        .iter()
        .filter_map(|entry| entry.after_path.as_ref().or(entry.before_path.as_ref()))
        .filter(|path| is_test_file(path))
        .cloned()
        .collect::<Vec<_>>();
    changed_test_files.sort();
    changed_test_files.dedup();

    let changed_test_declarations = changes
        .iter()
        .filter(|change| change.declaration_kind == "test")
        .map(|change| format!("{} in {}", change.declaration_name, change_path(change)))
        .collect::<Vec<_>>();

    let mut changed_coverage_targets = changes
        .iter()
        .filter(|change| change.status != "removed")
        .filter_map(|change| {
            let after = change.after.as_ref()?;
            let function_name = if change.declaration_kind == "function"
                || change.declaration_kind == "transform"
            {
                Some(change.declaration_name.clone())
            } else {
                None
            }?;
            after_analysis
                .coverage_targets
                .get(&(after.path.clone(), function_name))
                .cloned()
        })
        .collect::<Vec<_>>();
    changed_coverage_targets.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.function_name.cmp(&right.function_name))
            .then(left.id.cmp(&right.id))
    });
    changed_coverage_targets.dedup_by(|left, right| left.id == right.id);

    if !changed_coverage_targets.is_empty() && changed_test_files.is_empty() {
        issues.push(ReviewIssue {
            kind: "test-evidence".to_string(),
            message: format!(
                "changed coverage targets detected ({}), but no test files changed in this diff",
                changed_coverage_targets.len()
            ),
            phase: "review".to_string(),
            severity: "warning".to_string(),
            path: None,
        });
    }

    let summary = build_summary(
        &changes,
        &issues,
        &changed_coverage_targets,
        &changed_test_files,
    );
    let ok = issues.iter().all(|issue| issue.severity != "error");
    let data = ReviewData {
        changes,
        issues,
        scope: ReviewScope {
            after: selection.after.label(),
            before: selection.before.label(),
            git_args: selection.git_diff_args,
            mode: selection.mode,
        },
        summary,
        test_evidence: ReviewTestEvidence {
            changed_coverage_targets,
            changed_test_declarations,
            changed_test_files,
        },
    };

    emit_review(&data, json_output, llm_output)?;
    if !ok {
        return Err(CliError::Reported(1));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum SnapshotSide {
    Before,
    After,
}

impl SnapshotSide {
    fn label(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }

    fn failure_severity(self) -> &'static str {
        match self {
            Self::Before => "warning",
            Self::After => "error",
        }
    }
}

fn resolve_selection(
    repo_root: &Path,
    staged: bool,
    base: Option<&str>,
    head: Option<&str>,
    path_filters: &[PathBuf],
    git_diff_args: &[String],
) -> Result<ReviewSelection, CliError> {
    if !git_diff_args.is_empty() {
        if staged || base.is_some() || head.is_some() || !path_filters.is_empty() {
            return Err(CliError::Validation(
                "sigil review raw git diff passthrough cannot be combined with --staged, --base, --head, or --path".to_string(),
            ));
        }
        let parsed = parse_raw_diff_args(repo_root, git_diff_args)?;
        return Ok(ReviewSelection {
            mode: "raw".to_string(),
            before: parsed.before,
            after: parsed.after,
            git_diff_args: git_diff_args.to_vec(),
        });
    }

    if head.is_some() && base.is_none() {
        return Err(CliError::Validation(
            "sigil review --head requires --base".to_string(),
        ));
    }
    if staged && base.is_some() {
        return Err(CliError::Validation(
            "sigil review --staged cannot be combined with --base/--head".to_string(),
        ));
    }

    if staged {
        let mut git_args = vec!["--cached".to_string()];
        append_path_filters(&mut git_args, path_filters);
        return Ok(ReviewSelection {
            mode: "staged".to_string(),
            before: SnapshotSource::Revision("HEAD".to_string()),
            after: SnapshotSource::Index,
            git_diff_args: git_args,
        });
    }

    if let Some(base_rev) = base {
        let head_rev = head.unwrap_or("HEAD");
        let mut git_args = vec![base_rev.to_string(), head_rev.to_string()];
        append_path_filters(&mut git_args, path_filters);
        return Ok(ReviewSelection {
            mode: "baseHead".to_string(),
            before: SnapshotSource::Revision(base_rev.to_string()),
            after: SnapshotSource::Revision(head_rev.to_string()),
            git_diff_args: git_args,
        });
    }

    let mut git_args = Vec::new();
    append_path_filters(&mut git_args, path_filters);
    Ok(ReviewSelection {
        mode: "worktree".to_string(),
        before: SnapshotSource::Index,
        after: SnapshotSource::Worktree,
        git_diff_args: git_args,
    })
}

fn append_path_filters(args: &mut Vec<String>, path_filters: &[PathBuf]) {
    if path_filters.is_empty() {
        return;
    }
    args.push("--".to_string());
    args.extend(
        path_filters
            .iter()
            .map(|path| path.to_string_lossy().replace('\\', "/")),
    );
}

struct ParsedRawDiffArgs {
    before: SnapshotSource,
    after: SnapshotSource,
}

fn parse_raw_diff_args(
    repo_root: &Path,
    raw_args: &[String],
) -> Result<ParsedRawDiffArgs, CliError> {
    let split_index = raw_args.iter().position(|arg| arg == "--");
    let pre_args = split_index
        .map(|index| &raw_args[..index])
        .unwrap_or(raw_args);
    let cached = pre_args
        .iter()
        .any(|arg| arg == "--cached" || arg == "--staged");
    let revs = rev_tokens(repo_root, pre_args)?;

    if revs.len() > 2 {
        return Err(CliError::Validation(
            "sigil review supports git diff scopes with at most two revision endpoints".to_string(),
        ));
    }

    if revs.is_empty() {
        return Ok(ParsedRawDiffArgs {
            before: if cached {
                SnapshotSource::Revision("HEAD".to_string())
            } else {
                SnapshotSource::Index
            },
            after: if cached {
                SnapshotSource::Index
            } else {
                SnapshotSource::Worktree
            },
        });
    }

    if revs.len() == 1 {
        let token = &revs[0];
        if let Some((left, right)) = token.split_once("...") {
            let merge_base = git_text(repo_root, &["merge-base", left, right])?;
            return Ok(ParsedRawDiffArgs {
                before: SnapshotSource::Revision(merge_base.trim().to_string()),
                after: SnapshotSource::Revision(right.to_string()),
            });
        }
        if let Some((left, right)) = token.split_once("..") {
            return Ok(ParsedRawDiffArgs {
                before: SnapshotSource::Revision(left.to_string()),
                after: SnapshotSource::Revision(right.to_string()),
            });
        }
        return Ok(ParsedRawDiffArgs {
            before: SnapshotSource::Revision(token.to_string()),
            after: if cached {
                SnapshotSource::Index
            } else {
                SnapshotSource::Worktree
            },
        });
    }

    Ok(ParsedRawDiffArgs {
        before: SnapshotSource::Revision(revs[0].clone()),
        after: SnapshotSource::Revision(revs[1].clone()),
    })
}

fn rev_tokens(repo_root: &Path, pre_args: &[String]) -> Result<Vec<String>, CliError> {
    let mut revs = Vec::new();
    for token in pre_args {
        if token.starts_with('-') {
            continue;
        }
        if token.contains("...") || token.contains("..") {
            revs.push(token.clone());
            continue;
        }
        if resolves_revision(repo_root, token)? {
            revs.push(token.clone());
        }
    }
    Ok(revs)
}

fn resolves_revision(repo_root: &Path, token: &str) -> Result<bool, CliError> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["rev-parse", "--verify", &format!("{token}^{{object}}")])
        .output()?;
    Ok(output.status.success())
}

fn git_repo_root(cwd: &Path) -> Result<PathBuf, CliError> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        return Err(CliError::Validation(
            "sigil review requires a git repository".to_string(),
        ));
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

fn git_diff_entries(
    repo_root: &Path,
    git_diff_args: &[String],
) -> Result<Vec<DiffEntry>, CliError> {
    let (pre_args, post_args) = split_git_diff_args(git_diff_args);
    let mut command = Command::new("git");
    command.current_dir(repo_root);
    command.arg("diff");
    command.arg("--no-ext-diff");
    command.arg("--no-textconv");
    command.args(pre_args);
    command.arg("--name-status");
    command.arg("-z");
    if let Some(pathspecs) = post_args {
        command.arg("--");
        command.args(pathspecs);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(CliError::Runtime(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_name_status_z(&output.stdout)
}

fn split_git_diff_args(raw_args: &[String]) -> (&[String], Option<&[String]>) {
    if let Some(index) = raw_args.iter().position(|arg| arg == "--") {
        (&raw_args[..index], Some(&raw_args[(index + 1)..]))
    } else {
        (raw_args, None)
    }
}

fn parse_name_status_z(bytes: &[u8]) -> Result<Vec<DiffEntry>, CliError> {
    let fields = split_nul_fields(bytes);
    let mut index = 0;
    let mut entries = Vec::new();
    while index < fields.len() {
        let status = String::from_utf8(fields[index].to_vec()).map_err(|error| {
            CliError::Runtime(format!("git diff returned invalid UTF-8 status: {error}"))
        })?;
        index += 1;
        let code = status
            .chars()
            .next()
            .ok_or_else(|| CliError::Runtime("git diff returned an empty status".to_string()))?;
        match code {
            'R' => {
                let before = bytes_field(&fields, index)?;
                let after = bytes_field(&fields, index + 1)?;
                index += 2;
                entries.push(DiffEntry {
                    status: code,
                    before_path: Some(before),
                    after_path: Some(after),
                });
            }
            'C' => {
                let _before = bytes_field(&fields, index)?;
                let after = bytes_field(&fields, index + 1)?;
                index += 2;
                entries.push(DiffEntry {
                    status: code,
                    before_path: None,
                    after_path: Some(after),
                });
            }
            _ => {
                let path = bytes_field(&fields, index)?;
                index += 1;
                entries.push(match code {
                    'A' => DiffEntry {
                        status: code,
                        before_path: None,
                        after_path: Some(path),
                    },
                    'D' => DiffEntry {
                        status: code,
                        before_path: Some(path),
                        after_path: None,
                    },
                    _ => DiffEntry {
                        status: code,
                        before_path: Some(path.clone()),
                        after_path: Some(path),
                    },
                });
            }
        }
    }
    Ok(entries)
}

fn split_nul_fields(bytes: &[u8]) -> Vec<&[u8]> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect()
}

fn bytes_field(fields: &[&[u8]], index: usize) -> Result<String, CliError> {
    let field = fields
        .get(index)
        .ok_or_else(|| CliError::Runtime("git diff returned a truncated record".to_string()))?;
    String::from_utf8(field.to_vec()).map_err(|error| {
        CliError::Runtime(format!("git diff returned invalid UTF-8 path: {error}"))
    })
}

fn materialize_snapshot(
    repo_root: &Path,
    source: &SnapshotSource,
    target_root: &Path,
) -> Result<(), CliError> {
    let paths = snapshot_paths(repo_root, source)?;
    for path in paths {
        let bytes = snapshot_file_bytes(repo_root, source, &path)?;
        let target = target_root.join(&path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, bytes)?;
    }
    Ok(())
}

fn snapshot_paths(repo_root: &Path, source: &SnapshotSource) -> Result<Vec<String>, CliError> {
    let mut paths = match source {
        SnapshotSource::Revision(rev) => split_nul_or_lines(&git_bytes(
            repo_root,
            &["ls-tree", "-r", "-z", "--name-only", rev],
        )?),
        SnapshotSource::Index | SnapshotSource::Worktree => {
            split_nul_or_lines(&git_bytes(repo_root, &["ls-files", "-z"])?)
        }
    };
    paths.retain(|path| is_snapshot_support_path(path));
    if matches!(source, SnapshotSource::Worktree) {
        paths.retain(|path| repo_root.join(path).is_file());
    }
    paths.sort();
    Ok(paths)
}

fn snapshot_file_bytes(
    repo_root: &Path,
    source: &SnapshotSource,
    path: &str,
) -> Result<Vec<u8>, CliError> {
    match source {
        SnapshotSource::Revision(rev) => git_bytes(repo_root, &["show", &format!("{rev}:{path}")]),
        SnapshotSource::Index => git_bytes(repo_root, &["show", &format!(":{path}")]),
        SnapshotSource::Worktree => fs::read(repo_root.join(path)).map_err(CliError::from),
    }
}

fn git_bytes(repo_root: &Path, args: &[&str]) -> Result<Vec<u8>, CliError> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(CliError::Runtime(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn git_text(repo_root: &Path, args: &[&str]) -> Result<String, CliError> {
    String::from_utf8(git_bytes(repo_root, args)?)
        .map_err(|error| CliError::Runtime(format!("git output was not valid UTF-8: {error}")))
}

fn split_nul_or_lines(bytes: &[u8]) -> Vec<String> {
    if bytes.contains(&0) {
        return bytes
            .split(|byte| *byte == 0)
            .filter(|field| !field.is_empty())
            .map(|field| String::from_utf8_lossy(field).to_string())
            .collect();
    }
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn analyze_snapshot(
    side: SnapshotSide,
    snapshot_root: &Path,
    selected_paths: &BTreeSet<String>,
) -> Result<SnapshotAnalysis, CliError> {
    let existing_files = selected_paths
        .iter()
        .map(|path| (path.clone(), snapshot_root.join(path)))
        .filter(|(_, path)| path.is_file())
        .collect::<Vec<_>>();

    if existing_files.is_empty() {
        return Ok(SnapshotAnalysis {
            coverage_targets: BTreeMap::new(),
            files: BTreeMap::new(),
            issues: Vec::new(),
        });
    }

    let entry_files = existing_files
        .iter()
        .map(|(_, path)| path.clone())
        .collect::<Vec<_>>();

    let mut issues = Vec::new();
    let mut files = BTreeMap::new();
    let mut coverage_targets = BTreeMap::new();

    match ModuleGraph::build_many_with_env(&entry_files, None)
        .map_err(CliError::from)
        .and_then(|graph| analyze_module_graph(&graph).map(|analyzed| (graph, analyzed)))
    {
        Ok((_, analyzed)) => {
            let module_by_path = analyzed
                .modules
                .values()
                .map(|module| (module.file_path.clone(), module))
                .collect::<HashMap<_, _>>();

            for target in analyzed.coverage_targets {
                if let Ok(relative) = Path::new(&target.file).strip_prefix(snapshot_root) {
                    coverage_targets.insert(
                        (
                            relative.to_string_lossy().replace('\\', "/"),
                            target.function_name.clone(),
                        ),
                        ReviewCoverageTarget {
                            file: relative.to_string_lossy().replace('\\', "/"),
                            function_name: target.function_name,
                            id: target.id,
                        },
                    );
                }
            }

            for (relative_path, file_path) in existing_files {
                let file = if let Some(module) = module_by_path.get(&file_path) {
                    typed_file_snapshot(module, &relative_path)
                } else {
                    match parse_file_snapshot(&file_path, &relative_path) {
                        Ok(snapshot) => snapshot,
                        Err(parse_error) => {
                            issues.push(review_issue_for_parse_failure(
                                side,
                                &relative_path,
                                &parse_error,
                            ));
                            continue;
                        }
                    }
                };
                files.insert(relative_path, file);
            }
        }
        Err(error) => {
            issues.push(review_issue_for_analysis_failure(side, &error));
            for (relative_path, file_path) in existing_files {
                match parse_file_snapshot(&file_path, &relative_path) {
                    Ok(snapshot) => {
                        files.insert(relative_path, snapshot);
                    }
                    Err(parse_error) => issues.push(review_issue_for_parse_failure(
                        side,
                        &relative_path,
                        &parse_error,
                    )),
                }
            }
        }
    }

    Ok(SnapshotAnalysis {
        coverage_targets,
        files,
        issues,
    })
}

fn typed_file_snapshot(
    module: &super::compile_support::AnalyzedModule,
    relative_path: &str,
) -> ReviewFileSnapshot {
    let typed_functions = module
        .typed_program
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            TypedDeclaration::Function(function) => Some((function.name.clone(), function)),
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    let typed_tests = module
        .typed_program
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            TypedDeclaration::Test(test) => Some((test.description.clone(), test)),
            _ => None,
        })
        .collect::<HashMap<_, _>>();

    let declarations = module
        .ast
        .declarations
        .iter()
        .map(|decl| {
            let snapshot = declaration_snapshot(
                &module.ast,
                decl,
                Some(&module.declaration_schemes),
                Some(&typed_functions),
                Some(&typed_tests),
            );
            (snapshot.key.clone(), snapshot)
        })
        .collect();

    ReviewFileSnapshot {
        analysis_mode: AnalysisMode::Typed,
        declarations,
        module_id: Some(review_module_id(relative_path)),
        path: relative_path.to_string(),
    }
}

fn parse_file_snapshot(
    file_path: &Path,
    relative_path: &str,
) -> Result<ReviewFileSnapshot, CliError> {
    let source = fs::read_to_string(file_path)?;
    let filename = file_path.to_string_lossy().to_string();
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|error| CliError::Lexer(error.to_string()))?;
    let mut parser = Parser::new(tokens, &filename);
    let program = parser
        .parse()
        .map_err(|error| CliError::Parser(error.to_string()))?;
    let declarations = program
        .declarations
        .iter()
        .map(|decl| {
            let snapshot = declaration_snapshot(&program, decl, None, None, None);
            (snapshot.key.clone(), snapshot)
        })
        .collect();
    Ok(ReviewFileSnapshot {
        analysis_mode: AnalysisMode::ParseOnly,
        declarations,
        module_id: Some(review_module_id(relative_path)),
        path: relative_path.to_string(),
    })
}

fn declaration_snapshot(
    program: &Program,
    declaration: &Declaration,
    schemes: Option<&HashMap<String, TypeScheme>>,
    typed_functions: Option<&HashMap<String, &TypedFunctionDecl>>,
    typed_tests: Option<&HashMap<String, &TypedTestDecl>>,
) -> ReviewDeclaration {
    match declaration {
        Declaration::Function(function) => function_declaration_snapshot(
            "function",
            function,
            program,
            false,
            schemes,
            typed_functions,
        ),
        Declaration::Transform(TransformDecl { function }) => function_declaration_snapshot(
            "transform",
            function,
            program,
            true,
            schemes,
            typed_functions,
        ),
        Declaration::Type(type_decl) => ReviewDeclaration {
            facts: DeclarationFacts {
                constraint: type_decl.constraint.as_ref().map(print_canonical_expr),
                definition: Some(print_canonical_type_definition(&type_decl.definition)),
                surface: render_declaration_surface(program, declaration),
                ..DeclarationFacts::default()
            },
            key: format!("type:{}", type_decl.name),
            kind: "type".to_string(),
            line: type_decl.location.start.line,
            name: type_decl.name.clone(),
        },
        Declaration::Extern(extern_decl) => ReviewDeclaration {
            facts: DeclarationFacts {
                members: extern_members(extern_decl),
                module_path: Some(extern_decl.module_path.join("::")),
                surface: render_declaration_surface(program, declaration),
                ..DeclarationFacts::default()
            },
            key: format!(
                "extern:{}",
                if extern_decl.module_path.is_empty() {
                    format!("line{}", extern_decl.location.start.line)
                } else {
                    extern_decl.module_path.join("::")
                }
            ),
            kind: "extern".to_string(),
            line: extern_decl.location.start.line,
            name: if extern_decl.module_path.is_empty() {
                "<extern>".to_string()
            } else {
                extern_decl.module_path.join("::")
            },
        },
        Declaration::Effect(effect_decl) => ReviewDeclaration {
            facts: DeclarationFacts {
                effects: sorted_effects(effect_decl.effects.iter().cloned()),
                surface: render_declaration_surface(program, declaration),
                ..DeclarationFacts::default()
            },
            key: format!("effect:{}", effect_decl.name),
            kind: "effect".to_string(),
            line: effect_decl.location.start.line,
            name: effect_decl.name.clone(),
        },
        Declaration::FeatureFlag(feature_flag_decl) => {
            feature_flag_snapshot(program, declaration, feature_flag_decl)
        }
        Declaration::Const(const_decl) => ReviewDeclaration {
            facts: DeclarationFacts {
                signature: const_decl
                    .type_annotation
                    .as_ref()
                    .map(print_canonical_type)
                    .or_else(|| {
                        schemes
                            .and_then(|values| values.get(&const_decl.name).map(format_type_scheme))
                    }),
                surface: render_declaration_surface(program, declaration),
                ..DeclarationFacts::default()
            },
            key: format!("const:{}", const_decl.name),
            kind: "const".to_string(),
            line: const_decl.location.start.line,
            name: const_decl.name.clone(),
        },
        Declaration::Test(test_decl) => test_snapshot(program, declaration, test_decl, typed_tests),
        Declaration::Protocol(protocol_decl) => ReviewDeclaration {
            facts: DeclarationFacts {
                surface: render_declaration_surface(program, declaration),
                ..DeclarationFacts::default()
            },
            key: format!("protocol:{}", protocol_decl.name),
            kind: "protocol".to_string(),
            line: protocol_decl.location.start.line,
            name: protocol_decl.name.clone(),
        },
        Declaration::Label(label_decl) => ReviewDeclaration {
            facts: DeclarationFacts {
                surface: render_declaration_surface(program, declaration),
                ..DeclarationFacts::default()
            },
            key: format!("label:{}", label_decl.name),
            kind: "label".to_string(),
            line: label_decl.location.start.line,
            name: label_decl.name.clone(),
        },
        Declaration::Rule(rule_decl) => ReviewDeclaration {
            facts: DeclarationFacts {
                surface: render_declaration_surface(program, declaration),
                ..DeclarationFacts::default()
            },
            key: format!(
                "rule:{}:{}",
                rule_decl.boundary.module_path.join("::"),
                rule_decl.boundary.member
            ),
            kind: "rule".to_string(),
            line: rule_decl.location.start.line,
            name: format!(
                "{}.{}",
                rule_decl.boundary.module_path.join("::"),
                rule_decl.boundary.member
            ),
        },
    }
}

fn function_declaration_snapshot(
    kind: &str,
    function: &FunctionDecl,
    program: &Program,
    is_transform: bool,
    _schemes: Option<&HashMap<String, TypeScheme>>,
    typed_functions: Option<&HashMap<String, &TypedFunctionDecl>>,
) -> ReviewDeclaration {
    let typed_effects = typed_functions
        .and_then(|values| values.get(&function.name).copied())
        .map(|decl| typed_effects_for_function(decl));
    let declaration = if is_transform {
        Declaration::Transform(TransformDecl {
            function: function.clone(),
        })
    } else {
        Declaration::Function(function.clone())
    };
    ReviewDeclaration {
        facts: DeclarationFacts {
            decreases: function.decreases.as_ref().map(print_canonical_expr),
            effects: typed_effects
                .unwrap_or_else(|| sorted_effects(function.effects.iter().cloned())),
            ensures: function.ensures.as_ref().map(print_canonical_expr),
            mode: Some(function.mode.keyword().to_string()),
            requires: function.requires.as_ref().map(print_canonical_expr),
            // Review diffs compare the declared function surface. Using the
            // source-level signature here keeps typed and parse-only snapshots
            // stable when one side falls back.
            signature: Some(render_ast_function_signature(function)),
            surface: render_declaration_surface(program, &declaration),
            ..DeclarationFacts::default()
        },
        key: format!("{kind}:{}", function.name),
        kind: kind.to_string(),
        line: function.location.start.line,
        name: function.name.clone(),
    }
}

fn feature_flag_snapshot(
    program: &Program,
    declaration: &Declaration,
    feature_flag_decl: &FeatureFlagDecl,
) -> ReviewDeclaration {
    ReviewDeclaration {
        facts: DeclarationFacts {
            created_at: Some(feature_flag_decl.created_at.clone()),
            default_expr: Some(print_canonical_expr(&feature_flag_decl.default)),
            definition: Some(print_canonical_type(&feature_flag_decl.flag_type)),
            surface: render_declaration_surface(program, declaration),
            ..DeclarationFacts::default()
        },
        key: format!("featureFlag:{}", feature_flag_decl.name),
        kind: "featureFlag".to_string(),
        line: feature_flag_decl.location.start.line,
        name: feature_flag_decl.name.clone(),
    }
}

fn test_snapshot(
    program: &Program,
    declaration: &Declaration,
    test_decl: &TestDecl,
    typed_tests: Option<&HashMap<String, &TypedTestDecl>>,
) -> ReviewDeclaration {
    let effects = typed_tests
        .and_then(|values| values.get(&test_decl.description).copied())
        .map(typed_effects_for_test)
        .unwrap_or_else(|| sorted_effects(test_decl.effects.iter().cloned()));
    ReviewDeclaration {
        facts: DeclarationFacts {
            effects,
            surface: render_declaration_surface(program, declaration),
            ..DeclarationFacts::default()
        },
        key: format!("test:{}", test_decl.description),
        kind: "test".to_string(),
        line: test_decl.location.start.line,
        name: test_decl.description.clone(),
    }
}

fn render_declaration_surface(program: &Program, declaration: &Declaration) -> String {
    normalize_surface(print_canonical_program(&Program::new(
        vec![declaration.clone()],
        declaration_location(declaration),
        program.default_function_mode,
    )))
}

fn declaration_location(declaration: &Declaration) -> sigil_ast::SourceLocation {
    match declaration {
        Declaration::Function(value) => value.location,
        Declaration::Transform(value) => value.function.location,
        Declaration::Type(value) => value.location,
        Declaration::Protocol(value) => value.location,
        Declaration::Label(value) => value.location,
        Declaration::Rule(value) => value.location,
        Declaration::Effect(value) => value.location,
        Declaration::FeatureFlag(value) => value.location,
        Declaration::Const(value) => value.location,
        Declaration::Test(value) => value.location,
        Declaration::Extern(value) => value.location,
    }
}

fn normalize_surface(surface: String) -> String {
    surface.trim_end_matches('\n').to_string()
}

fn extern_members(extern_decl: &ExternDecl) -> Vec<String> {
    let mut members = extern_decl
        .members
        .as_ref()
        .map(|items| {
            items
                .iter()
                .map(|member| {
                    format!(
                        "{}:{}",
                        member.name,
                        print_canonical_type(&member.member_type)
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    members.sort();
    members
}

fn render_ast_function_signature(function: &FunctionDecl) -> String {
    let type_params = if function.type_params.is_empty() {
        String::new()
    } else {
        format!("[{}]", function.type_params.join(","))
    };
    let params = function
        .params
        .iter()
        .map(render_param_signature)
        .collect::<Vec<_>>()
        .join(",");
    let return_type = function
        .return_type
        .as_ref()
        .map(print_canonical_type)
        .unwrap_or_else(|| "Any".to_string());
    format!(
        "{}{}({})=>{}",
        function.name, type_params, params, return_type
    )
}

fn render_param_signature(param: &Param) -> String {
    let prefix = if param.is_mutable { "mut " } else { "" };
    let type_source = param
        .type_annotation
        .as_ref()
        .map(print_canonical_type)
        .unwrap_or_else(|| "Any".to_string());
    format!("{prefix}{}:{}", param.name, type_source)
}

fn typed_effects_for_function(function: &TypedFunctionDecl) -> Vec<String> {
    sorted_effects(
        function
            .effects
            .as_ref()
            .into_iter()
            .flat_map(|effects| effects.iter().cloned()),
    )
}

fn typed_effects_for_test(test: &TypedTestDecl) -> Vec<String> {
    sorted_effects(
        test.effects
            .as_ref()
            .into_iter()
            .flat_map(|effects| effects.iter().cloned()),
    )
}

fn sorted_effects(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut effects = values.into_iter().collect::<Vec<_>>();
    effects.sort();
    effects.dedup();
    effects
}

fn build_changes_for_entry(
    entry: &DiffEntry,
    before: &SnapshotAnalysis,
    after: &SnapshotAnalysis,
) -> Vec<ReviewChange> {
    match (&entry.before_path, &entry.after_path) {
        (Some(before_path), Some(after_path)) if entry.status == 'R' => {
            diff_file_pair(before.files.get(before_path), after.files.get(after_path))
        }
        (Some(path), Some(_)) => diff_file_pair(before.files.get(path), after.files.get(path)),
        (Some(path), None) => diff_file_pair(before.files.get(path), None),
        (None, Some(path)) => diff_file_pair(None, after.files.get(path)),
        (None, None) => Vec::new(),
    }
}

fn diff_file_pair(
    before_file: Option<&ReviewFileSnapshot>,
    after_file: Option<&ReviewFileSnapshot>,
) -> Vec<ReviewChange> {
    let mut keys = BTreeSet::new();
    if let Some(file) = before_file {
        keys.extend(file.declarations.keys().cloned());
    }
    if let Some(file) = after_file {
        keys.extend(file.declarations.keys().cloned());
    }

    let mut changes = Vec::new();
    for key in keys {
        let before_decl = before_file.and_then(|file| file.declarations.get(&key));
        let after_decl = after_file.and_then(|file| file.declarations.get(&key));
        if let Some(change) = diff_declaration(before_file, before_decl, after_file, after_decl) {
            changes.push(change);
        }
    }
    changes
}

fn diff_declaration(
    before_file: Option<&ReviewFileSnapshot>,
    before_decl: Option<&ReviewDeclaration>,
    after_file: Option<&ReviewFileSnapshot>,
    after_decl: Option<&ReviewDeclaration>,
) -> Option<ReviewChange> {
    match (before_file, before_decl, after_file, after_decl) {
        (_, None, _, None) => None,
        (Some(before_file), Some(before_decl), _, None) => Some(ReviewChange {
            after: None,
            before: Some(change_side(before_file, before_decl)),
            change_kinds: vec!["removed".to_string()],
            declaration_key: before_decl.key.clone(),
            declaration_kind: before_decl.kind.clone(),
            declaration_name: before_decl.name.clone(),
            status: "removed".to_string(),
        }),
        (_, None, Some(after_file), Some(after_decl)) => Some(ReviewChange {
            after: Some(change_side(after_file, after_decl)),
            before: None,
            change_kinds: vec!["added".to_string()],
            declaration_key: after_decl.key.clone(),
            declaration_kind: after_decl.kind.clone(),
            declaration_name: after_decl.name.clone(),
            status: "added".to_string(),
        }),
        (Some(before_file), Some(before_decl), Some(after_file), Some(after_decl)) => {
            let change_kinds =
                compute_change_kinds(&before_decl.kind, &before_decl.facts, &after_decl.facts);
            if change_kinds.is_empty() {
                return None;
            }
            Some(ReviewChange {
                after: Some(change_side(after_file, after_decl)),
                before: Some(change_side(before_file, before_decl)),
                change_kinds,
                declaration_key: after_decl.key.clone(),
                declaration_kind: after_decl.kind.clone(),
                declaration_name: after_decl.name.clone(),
                status: "modified".to_string(),
            })
        }
        _ => None,
    }
}

fn change_side(file: &ReviewFileSnapshot, declaration: &ReviewDeclaration) -> ReviewChangeSide {
    ReviewChangeSide {
        analysis_mode: file.analysis_mode,
        facts: declaration.facts.clone(),
        line: declaration.line,
        module_id: file.module_id.clone(),
        path: file.path.clone(),
    }
}

fn compute_change_kinds(
    kind: &str,
    before: &DeclarationFacts,
    after: &DeclarationFacts,
) -> Vec<String> {
    let mut changes = Vec::new();
    match kind {
        "function" | "transform" => {
            if before.signature != after.signature {
                changes.push("signature".to_string());
            }
            if before.mode != after.mode {
                changes.push("mode".to_string());
            }
            if before.effects != after.effects {
                changes.push("effects".to_string());
            }
            if before.requires != after.requires {
                changes.push("requires".to_string());
            }
            if before.decreases != after.decreases {
                changes.push("decreases".to_string());
            }
            if before.ensures != after.ensures {
                changes.push("ensures".to_string());
            }
            if changes.is_empty() && before.surface != after.surface {
                changes.push("implementation".to_string());
            }
        }
        "type" => {
            if before.definition != after.definition {
                changes.push("definition".to_string());
            }
            if before.constraint != after.constraint {
                changes.push("constraint".to_string());
            }
            if changes.is_empty() && before.surface != after.surface {
                changes.push("declaration".to_string());
            }
        }
        "extern" => {
            if before.module_path != after.module_path || before.members != after.members {
                changes.push("trustSurface".to_string());
            }
        }
        "effect" => {
            if before.effects != after.effects {
                changes.push("effectAlias".to_string());
            }
        }
        "featureFlag" => {
            if before.definition != after.definition
                || before.created_at != after.created_at
                || before.default_expr != after.default_expr
            {
                changes.push("featureFlag".to_string());
            }
        }
        "const" => {
            if before.signature != after.signature {
                changes.push("signature".to_string());
            }
            if changes.is_empty() && before.surface != after.surface {
                changes.push("implementation".to_string());
            }
        }
        "test" => {
            if before.effects != after.effects {
                changes.push("test".to_string());
            }
            if changes.is_empty() && before.surface != after.surface {
                changes.push("implementation".to_string());
            }
        }
        _ => {
            if before.surface != after.surface {
                changes.push("declaration".to_string());
            }
        }
    }
    changes
}

fn review_issue_for_analysis_failure(side: SnapshotSide, error: &CliError) -> ReviewIssue {
    ReviewIssue {
        kind: "analysis-fallback".to_string(),
        message: format!(
            "{} snapshot full analysis failed; review fell back to parse-only facts: {}",
            side.label(),
            error
        ),
        phase: error_phase(error).to_string(),
        severity: side.failure_severity().to_string(),
        path: None,
    }
}

fn review_issue_for_parse_failure(side: SnapshotSide, path: &str, error: &CliError) -> ReviewIssue {
    ReviewIssue {
        kind: "parse-failure".to_string(),
        message: format!(
            "{} snapshot could not parse `{}`: {}",
            side.label(),
            path,
            error
        ),
        phase: error_phase(error).to_string(),
        severity: side.failure_severity().to_string(),
        path: Some(path.to_string()),
    }
}

fn error_phase(error: &CliError) -> &'static str {
    match error {
        CliError::Lexer(_) => "lexer",
        CliError::Parser(_) => "parser",
        CliError::Validation(_) => "canonical",
        CliError::Type(_) => "typecheck",
        CliError::ModuleGraph(module_error) => match module_error {
            ModuleGraphError::Lexer(_) => "lexer",
            ModuleGraphError::Parser(_) => "parser",
            ModuleGraphError::Validation(_) => "canonical",
            ModuleGraphError::ImportCycle(_)
            | ModuleGraphError::ImportNotFound { .. }
            | ModuleGraphError::SelectedConfigEnvRequired
            | ModuleGraphError::SelectedConfigModuleNotFound { .. } => "cli",
            ModuleGraphError::ProjectConfig(_) => "cli",
            ModuleGraphError::Io(_) => "io",
        },
        CliError::Io(_) => "io",
        CliError::Codegen(_) => "codegen",
        CliError::Runtime(_) => "runtime",
        CliError::ProjectConfig(_) => "cli",
        CliError::Breakpoint { .. } => "runtime",
        CliError::Reported(_) => "internal",
    }
}

fn build_summary(
    changes: &[ReviewChange],
    issues: &[ReviewIssue],
    changed_coverage_targets: &[ReviewCoverageTarget],
    changed_test_files: &[String],
) -> ReviewSummary {
    ReviewSummary {
        changed_coverage_targets: changed_coverage_targets.len(),
        changed_declarations: changes.len(),
        changed_files: changes
            .iter()
            .map(change_path)
            .collect::<BTreeSet<_>>()
            .len(),
        changed_test_files: changed_test_files.len(),
        compile_issues: issues
            .iter()
            .filter(|issue| issue.severity == "error")
            .count(),
        contract_changes: changes
            .iter()
            .filter(|change| {
                has_change_kind(change, "requires") || has_change_kind(change, "ensures")
            })
            .count(),
        effect_changes: changes
            .iter()
            .filter(|change| {
                has_change_kind(change, "effects")
                    || has_change_kind(change, "effectAlias")
                    || (change.status != "modified"
                        && (change.declaration_kind == "effect"
                            || ((change.declaration_kind == "function"
                                || change.declaration_kind == "transform")
                                && change_has_effect_surface(change))))
            })
            .count(),
        implementation_changes: changes
            .iter()
            .filter(|change| has_change_kind(change, "implementation"))
            .count(),
        signature_changes: changes
            .iter()
            .filter(|change| {
                has_change_kind(change, "signature")
                    || has_change_kind(change, "mode")
                    || (change.status != "modified"
                        && matches!(
                            change.declaration_kind.as_str(),
                            "function" | "transform" | "const"
                        ))
            })
            .count(),
        trust_surface_changes: changes
            .iter()
            .filter(|change| {
                change.declaration_kind == "extern" || has_change_kind(change, "trustSurface")
            })
            .count(),
        type_changes: changes
            .iter()
            .filter(|change| {
                change.declaration_kind == "type"
                    || has_change_kind(change, "definition")
                    || has_change_kind(change, "constraint")
            })
            .count(),
    }
}

fn has_change_kind(change: &ReviewChange, kind: &str) -> bool {
    change.change_kinds.iter().any(|value| value == kind)
}

fn change_path(change: &ReviewChange) -> String {
    change
        .after
        .as_ref()
        .map(|side| side.path.clone())
        .or_else(|| change.before.as_ref().map(|side| side.path.clone()))
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn emit_review(data: &ReviewData, json_output: bool, llm_output: bool) -> Result<(), CliError> {
    let ok = data.issues.iter().all(|issue| issue.severity != "error");
    if json_output {
        output_json_value(
            &json!({
                "formatVersion": 1,
                "command": REVIEW_COMMAND,
                "ok": ok,
                "phase": "surface",
                "data": data
            }),
            false,
        );
        return Ok(());
    }

    if llm_output {
        println!("{}", render_llm_review(data, ok)?);
        return Ok(());
    }

    println!("{}", render_human_review(data, ok));
    Ok(())
}

fn render_human_review(data: &ReviewData, ok: bool) -> String {
    let mut out = String::new();
    out.push_str("## Sigil Review\n\n");
    out.push_str("Summary\n");
    out.push_str(&format!(
        "- changed declarations: {}\n- signature changes: {}\n- contract changes: {}\n- effect changes: {}\n- type/refinement changes: {}\n- trust surface changes: {}\n- changed test files: {}\n",
        data.summary.changed_declarations,
        data.summary.signature_changes,
        data.summary.contract_changes,
        data.summary.effect_changes,
        data.summary.type_changes,
        data.summary.trust_surface_changes,
        data.summary.changed_test_files,
    ));

    if data.changes.is_empty() {
        out.push_str("\nNo semantic declaration changes detected in the selected Sigil files.\n");
    } else {
        render_section(
            &mut out,
            "Signature Changes",
            data.changes.iter().filter(|change| {
                has_change_kind(change, "signature")
                    || has_change_kind(change, "mode")
                    || (change.status != "modified"
                        && matches!(
                            change.declaration_kind.as_str(),
                            "function" | "transform" | "const"
                        ))
            }),
        );
        render_section(
            &mut out,
            "Type And Refinement Changes",
            data.changes.iter().filter(|change| {
                change.declaration_kind == "type"
                    || has_change_kind(change, "definition")
                    || has_change_kind(change, "constraint")
            }),
        );
        render_section(
            &mut out,
            "Effect Changes",
            data.changes.iter().filter(|change| {
                has_change_kind(change, "effects")
                    || has_change_kind(change, "effectAlias")
                    || (change.status != "modified"
                        && matches!(
                            change.declaration_kind.as_str(),
                            "effect" | "function" | "transform"
                        ))
            }),
        );
        render_section(
            &mut out,
            "Contract Changes",
            data.changes.iter().filter(|change| {
                has_change_kind(change, "requires") || has_change_kind(change, "ensures")
            }),
        );
        render_section(
            &mut out,
            "Termination Changes",
            data.changes
                .iter()
                .filter(|change| has_change_kind(change, "decreases")),
        );
        render_section(
            &mut out,
            "Trust Surface Changes",
            data.changes.iter().filter(|change| {
                change.declaration_kind == "extern" || has_change_kind(change, "trustSurface")
            }),
        );
        render_section(
            &mut out,
            "Implementation Changes",
            data.changes
                .iter()
                .filter(|change| has_change_kind(change, "implementation")),
        );
    }

    out.push_str("\nTest Evidence\n");
    if data.test_evidence.changed_test_files.is_empty() {
        out.push_str("- changed test files: none\n");
    } else {
        out.push_str("- changed test files:\n");
        for file in &data.test_evidence.changed_test_files {
            out.push_str(&format!("  - `{file}`\n"));
        }
    }
    if data.test_evidence.changed_coverage_targets.is_empty() {
        out.push_str("- changed coverage targets: none\n");
    } else {
        out.push_str("- changed coverage targets:\n");
        for target in &data.test_evidence.changed_coverage_targets {
            out.push_str(&format!(
                "  - `{}` in `{}` ({})\n",
                target.function_name, target.file, target.id
            ));
        }
    }
    if !data.test_evidence.changed_test_declarations.is_empty() {
        out.push_str("- changed test declarations:\n");
        for test in &data.test_evidence.changed_test_declarations {
            out.push_str(&format!("  - `{test}`\n"));
        }
    }

    if !data.issues.is_empty() {
        out.push_str("\nCompiler Issues\n");
        for issue in &data.issues {
            if let Some(path) = &issue.path {
                out.push_str(&format!(
                    "- {} [{}] `{}`: {}\n",
                    issue.severity, issue.phase, path, issue.message
                ));
            } else {
                out.push_str(&format!(
                    "- {} [{}]: {}\n",
                    issue.severity, issue.phase, issue.message
                ));
            }
        }
    }

    if !ok {
        out.push_str("\nReview detected blocking issues.\n");
    }
    out
}

fn render_section<'a>(
    out: &mut String,
    title: &str,
    changes: impl Iterator<Item = &'a ReviewChange>,
) {
    let items = changes.collect::<Vec<_>>();
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("\n{title}\n"));
    for change in items {
        let marker = match change.status.as_str() {
            "added" => "+",
            "removed" => "-",
            _ => "~",
        };
        out.push_str(&format!(
            "- {} {} `{}` in `{}`\n",
            marker,
            change.declaration_kind,
            change.declaration_name,
            change_path(change)
        ));
        if let (Some(before), Some(after)) = (&change.before, &change.after) {
            if before.facts.signature != after.facts.signature {
                out.push_str(&format!(
                    "  - signature: `{}` -> `{}`\n",
                    before.facts.signature.as_deref().unwrap_or("<none>"),
                    after.facts.signature.as_deref().unwrap_or("<none>")
                ));
            }
            if before.facts.mode != after.facts.mode {
                out.push_str(&format!(
                    "  - mode: `{}` -> `{}`\n",
                    before.facts.mode.as_deref().unwrap_or("<none>"),
                    after.facts.mode.as_deref().unwrap_or("<none>")
                ));
            }
            if before.facts.effects != after.facts.effects {
                out.push_str(&format!(
                    "  - effects: `{}` -> `{}`\n",
                    render_effects(&before.facts.effects),
                    render_effects(&after.facts.effects)
                ));
            }
            if before.facts.requires != after.facts.requires {
                out.push_str(&format!(
                    "  - requires: `{}` -> `{}`\n",
                    before.facts.requires.as_deref().unwrap_or("<none>"),
                    after.facts.requires.as_deref().unwrap_or("<none>")
                ));
            }
            if before.facts.decreases != after.facts.decreases {
                out.push_str(&format!(
                    "  - decreases: `{}` -> `{}`\n",
                    before.facts.decreases.as_deref().unwrap_or("<none>"),
                    after.facts.decreases.as_deref().unwrap_or("<none>")
                ));
            }
            if before.facts.ensures != after.facts.ensures {
                out.push_str(&format!(
                    "  - ensures: `{}` -> `{}`\n",
                    before.facts.ensures.as_deref().unwrap_or("<none>"),
                    after.facts.ensures.as_deref().unwrap_or("<none>")
                ));
            }
            if before.facts.definition != after.facts.definition {
                out.push_str(&format!(
                    "  - definition: `{}` -> `{}`\n",
                    before.facts.definition.as_deref().unwrap_or("<none>"),
                    after.facts.definition.as_deref().unwrap_or("<none>")
                ));
            }
            if before.facts.constraint != after.facts.constraint {
                out.push_str(&format!(
                    "  - constraint: `{}` -> `{}`\n",
                    before.facts.constraint.as_deref().unwrap_or("<none>"),
                    after.facts.constraint.as_deref().unwrap_or("<none>")
                ));
            }
            if before.analysis_mode != after.analysis_mode {
                out.push_str(&format!(
                    "  - analysis mode: `{:?}` -> `{:?}`\n",
                    before.analysis_mode, after.analysis_mode
                ));
            } else if after.analysis_mode == AnalysisMode::ParseOnly {
                out.push_str("  - analysis mode: `parseOnly`\n");
            }
            if change.change_kinds.len() == 1 && change.change_kinds[0] == "implementation" {
                out.push_str("  - implementation changed without a surface-level signature/contract/effect delta\n");
            }
        } else if let Some(after) = &change.after {
            if let Some(signature) = &after.facts.signature {
                out.push_str(&format!("  - signature: `{signature}`\n"));
            }
            if !after.facts.effects.is_empty() {
                out.push_str(&format!(
                    "  - effects: `{}`\n",
                    render_effects(&after.facts.effects)
                ));
            }
            if after.analysis_mode == AnalysisMode::ParseOnly {
                out.push_str("  - analysis mode: `parseOnly`\n");
            }
        } else if let Some(before) = &change.before {
            if let Some(signature) = &before.facts.signature {
                out.push_str(&format!("  - signature: `{signature}`\n"));
            }
            if !before.facts.effects.is_empty() {
                out.push_str(&format!(
                    "  - effects: `{}`\n",
                    render_effects(&before.facts.effects)
                ));
            }
            if before.analysis_mode == AnalysisMode::ParseOnly {
                out.push_str("  - analysis mode: `parseOnly`\n");
            }
        }
    }
}

fn render_effects(effects: &[String]) -> String {
    if effects.is_empty() {
        return "<none>".to_string();
    }
    effects
        .iter()
        .map(|effect| format!("!{effect}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_llm_review(data: &ReviewData, ok: bool) -> Result<String, CliError> {
    let facts = serde_json::to_string_pretty(&json!({
        "formatVersion": 1,
        "command": REVIEW_COMMAND,
        "ok": ok,
        "phase": "surface",
        "data": data
    }))
    .map_err(|error| CliError::Runtime(format!("failed to serialize review facts: {error}")))?;
    Ok(format!(
        "You are reviewing a Sigil semantic diff.\n\nUse only the facts below.\nDo not infer behavior that is not explicitly listed.\nIf analysisMode is `parseOnly`, call out that limitation.\nIf any issue has severity `error`, list it first.\n\nFacts:\n{facts}"
    ))
}

fn format_type_scheme(scheme: &TypeScheme) -> String {
    let type_text = sigil_typechecker::format_type(&scheme.typ);
    if scheme.quantified_vars.is_empty() {
        return type_text;
    }
    let mut names = HashMap::new();
    collect_quantified_var_names(&scheme.typ, &scheme.quantified_vars, &mut names);
    let mut quantified = scheme
        .quantified_vars
        .iter()
        .map(|id| {
            (
                names.get(id).cloned().unwrap_or_else(|| format!("α{id}")),
                *id,
            )
        })
        .collect::<Vec<_>>();
    quantified.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    format!(
        "∀{}. {}",
        quantified
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>()
            .join(", "),
        type_text
    )
}

fn collect_quantified_var_names(
    typ: &InferenceType,
    quantified_vars: &HashSet<u32>,
    names: &mut HashMap<u32, String>,
) {
    match typ {
        InferenceType::Primitive(_) | InferenceType::Any => {}
        InferenceType::Var(var) => {
            if quantified_vars.contains(&var.id) {
                names
                    .entry(var.id)
                    .or_insert_with(|| var.name.clone().unwrap_or_else(|| format!("α{}", var.id)));
            }
            if let Some(instance) = &var.instance {
                collect_quantified_var_names(instance, quantified_vars, names);
            }
        }
        InferenceType::Function(function) => {
            for param in &function.params {
                collect_quantified_var_names(param, quantified_vars, names);
            }
            collect_quantified_var_names(&function.return_type, quantified_vars, names);
        }
        InferenceType::List(list) => {
            collect_quantified_var_names(&list.element_type, quantified_vars, names);
        }
        InferenceType::Map(map) => {
            collect_quantified_var_names(&map.key_type, quantified_vars, names);
            collect_quantified_var_names(&map.value_type, quantified_vars, names);
        }
        InferenceType::Tuple(tuple) => {
            for item in &tuple.types {
                collect_quantified_var_names(item, quantified_vars, names);
            }
        }
        InferenceType::Record(record) => {
            for field_type in record.fields.values() {
                collect_quantified_var_names(field_type, quantified_vars, names);
            }
        }
        InferenceType::Constructor(constructor) => {
            for argument in &constructor.type_args {
                collect_quantified_var_names(argument, quantified_vars, names);
            }
        }
        InferenceType::Owned(inner) => {
            collect_quantified_var_names(inner, quantified_vars, names);
        }
        InferenceType::Borrowed(borrowed) => {
            collect_quantified_var_names(&borrowed.resource_type, quantified_vars, names);
        }
    }
}

fn is_snapshot_support_path(path: &str) -> bool {
    !is_internal_review_artifact(path)
        && (path.ends_with(".sigil")
            || path.ends_with(".lib.sigil")
            || path.ends_with("/sigil.json")
            || path == "sigil.json")
}

fn is_supported_review_path(path: &str) -> bool {
    !is_internal_review_artifact(path) && (path.ends_with(".sigil") || path.ends_with(".lib.sigil"))
}

fn is_test_file(path: &str) -> bool {
    path.split('/').any(|segment| segment == "tests") && path.ends_with(".sigil")
}

fn review_module_id(relative_path: &str) -> String {
    relative_path
        .trim_end_matches(".lib.sigil")
        .trim_end_matches(".sigil")
        .replace('/', "::")
}

fn change_has_effect_surface(change: &ReviewChange) -> bool {
    change
        .after
        .as_ref()
        .or(change.before.as_ref())
        .is_some_and(|side| !side.facts.effects.is_empty())
}

fn is_internal_review_artifact(path: &str) -> bool {
    path == ".sigil/review"
        || path.starts_with(".sigil/review/")
        || path.contains("/.sigil/review/")
}
