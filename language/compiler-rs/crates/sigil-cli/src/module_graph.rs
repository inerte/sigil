//! Module graph construction and dependency resolution
//!
//! Handles building a dependency graph of Sigil modules for multi-module compilation

use crate::project::{get_project_config, ProjectConfig};
use sigil_ast::{Declaration, Program};
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use sigil_validator::{validate_canonical_form, validate_surface_form};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModuleGraphError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lexer error: {0}")]
    Lexer(String),

    #[error("Parser error: {0}")]
    Parser(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Import cycle detected: {0:?}")]
    ImportCycle(Vec<String>),

    #[error("Import not found: {module_id} (expected at {expected_path})")]
    ImportNotFound {
        module_id: String,
        expected_path: String,
    },
}

pub struct ModuleGraph {
    pub modules: HashMap<String, LoadedModule>,
    pub topo_order: Vec<String>,
}

pub struct LoadedModule {
    pub id: String,
    pub file_path: PathBuf,
    pub source: String,
    pub ast: Program,
    pub project: Option<ProjectConfig>,
}

impl ModuleGraph {
    pub fn build(entry_file: &Path) -> Result<Self, ModuleGraphError> {
        let mut builder = ModuleGraphBuilder::new();
        builder.visit(entry_file, None, None)?;
        Ok(ModuleGraph {
            modules: builder.modules,
            topo_order: builder.topo_order,
        })
    }
}

struct ModuleGraphBuilder {
    modules: HashMap<String, LoadedModule>,
    topo_order: Vec<String>,
    visiting: HashSet<String>,
    visit_stack: Vec<String>,
}

impl ModuleGraphBuilder {
    fn new() -> Self {
        Self {
            modules: HashMap::new(),
            topo_order: Vec::new(),
            visiting: HashSet::new(),
            visit_stack: Vec::new(),
        }
    }

    fn visit(
        &mut self,
        file_path: &Path,
        logical_id: Option<String>,
        inherited_project: Option<ProjectConfig>,
    ) -> Result<(), ModuleGraphError> {
        let abs_file = fs::canonicalize(file_path)?;

        // Determine project
        let project = get_project_config(&abs_file).or(inherited_project);

        // Compute logical ID
        let computed_id = logical_id.or_else(|| file_path_to_module_id(&abs_file, &project));
        let module_key = computed_id.unwrap_or_else(|| abs_file.to_string_lossy().to_string());

        // Check if already visited
        if self.modules.contains_key(&module_key) {
            return Ok(());
        }

        // Check for cycles
        if self.visiting.contains(&module_key) {
            let start_idx = self.visit_stack.iter().position(|k| k == &module_key);
            let cycle = if let Some(idx) = start_idx {
                let mut c = self.visit_stack[idx..].to_vec();
                c.push(module_key.clone());
                c
            } else {
                vec![module_key.clone(), module_key.clone()]
            };
            return Err(ModuleGraphError::ImportCycle(cycle));
        }

        self.visiting.insert(module_key.clone());
        self.visit_stack.push(module_key.clone());

        // Load and parse the module
        let source = fs::read_to_string(&abs_file)?;

        // Tokenize
        let mut lexer = Lexer::new(&source);
        let tokens = lexer
            .tokenize()
            .map_err(|e| ModuleGraphError::Lexer(format!("{:?}", e)))?;

        // Parse
        let filename = abs_file.to_string_lossy().to_string();
        let mut parser = Parser::new(tokens, &filename);
        let ast = parser
            .parse()
            .map_err(|e| ModuleGraphError::Parser(format!("{:?}", e)))?;

        // Validate
        validate_surface_form(&ast)
            .map_err(|e| ModuleGraphError::Validation(format!("{} errors", e.len())))?;
        validate_canonical_form(&ast)
            .map_err(|e| ModuleGraphError::Validation(format!("{} errors", e.len())))?;

        // Process imports
        for decl in &ast.declarations {
            if let Declaration::Import(import_decl) = decl {
                let module_id = import_decl.module_path.join("⋅");

                // Only process Sigil imports (stdlib⋅ or src⋅)
                if !is_sigil_import_path(&module_id) {
                    continue;
                }

                // Resolve import to file path
                let resolved =
                    resolve_sigil_import(&abs_file, project.as_ref(), &module_id)?;

                if !resolved.file_path.exists() {
                    return Err(ModuleGraphError::ImportNotFound {
                        module_id: module_id.clone(),
                        expected_path: resolved.file_path.to_string_lossy().to_string(),
                    });
                }

                // Recursively visit
                self.visit(&resolved.file_path, Some(resolved.module_id), resolved.project)?;
            }
        }

        // Done visiting this module
        self.visiting.remove(&module_key);
        self.visit_stack.pop();

        // Add to graph
        self.modules.insert(
            module_key.clone(),
            LoadedModule {
                id: module_key.clone(),
                file_path: abs_file.clone(),
                source,
                ast,
                project,
            },
        );
        self.topo_order.push(module_key);

        Ok(())
    }
}

fn is_sigil_import_path(module_path: &str) -> bool {
    module_path.starts_with("stdlib⋅") || module_path.starts_with("src⋅")
}

fn file_path_to_module_id(abs_path: &Path, project: &Option<ProjectConfig>) -> Option<String> {
    let path_str = abs_path.to_string_lossy();

    // Check if it's a stdlib module
    if path_str.contains("/stdlib/") {
        if let Some(relative) = path_str.split("/stdlib/").nth(1) {
            if let Some(without_ext) = relative.strip_suffix(".sigil") {
                return Some(format!("stdlib⋅{}", without_ext.replace('/', "⋅")));
            }
        }
    }

    // Check if it's a project src module
    if let Some(ref proj) = project {
        let proj_root = proj.root.to_string_lossy();
        if path_str.starts_with(proj_root.as_ref()) {
            if let Some(relative) = path_str.strip_prefix(proj_root.as_ref()) {
                let relative = relative.trim_start_matches('/');
                if let Some(without_ext) = relative.strip_suffix(".sigil") {
                    return Some(without_ext.replace('/', "⋅"));
                }
            }
        }
    }

    None
}

struct ResolvedImport {
    module_id: String,
    file_path: PathBuf,
    project: Option<ProjectConfig>,
}

fn resolve_sigil_import(
    importer_file: &Path,
    importer_project: Option<&ProjectConfig>,
    module_id: &str,
) -> Result<ResolvedImport, ModuleGraphError> {
    // Convert module ID (with ⋅ separators) to file path
    let file_path_str = module_id.replace('⋅', "/");

    if module_id.starts_with("src⋅") {
        // Project import
        let project = importer_project.ok_or_else(|| ModuleGraphError::ImportNotFound {
            module_id: module_id.to_string(),
            expected_path: "project not found".to_string(),
        })?;

        let file_path = project.root.join(format!("{}.sigil", file_path_str));

        Ok(ResolvedImport {
            module_id: module_id.to_string(),
            file_path,
            project: Some(project.clone()),
        })
    } else if module_id.starts_with("stdlib⋅") {
        // Stdlib import - find language root
        let language_root = find_language_root(importer_file)?;
        let file_path = language_root.join(format!("{}.sigil", file_path_str));

        Ok(ResolvedImport {
            module_id: module_id.to_string(),
            file_path,
            project: importer_project.cloned(),
        })
    } else {
        Err(ModuleGraphError::ImportNotFound {
            module_id: module_id.to_string(),
            expected_path: "unknown import type".to_string(),
        })
    }
}

fn find_language_root(start_path: &Path) -> Result<PathBuf, ModuleGraphError> {
    let mut current = start_path.to_path_buf();

    // Walk up until we find a directory containing stdlib/
    loop {
        if current.is_file() {
            current = current.parent().unwrap().to_path_buf();
        }

        let stdlib_dir = current.join("stdlib");
        if stdlib_dir.exists() && stdlib_dir.is_dir() {
            return Ok(current);
        }

        let language_dir = current.join("language");
        if language_dir.exists() {
            let lang_stdlib = language_dir.join("stdlib");
            if lang_stdlib.exists() && lang_stdlib.is_dir() {
                return Ok(language_dir);
            }
        }

        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    Err(ModuleGraphError::ImportNotFound {
        module_id: "stdlib".to_string(),
        expected_path: "language root not found".to_string(),
    })
}
