//! Module graph construction and dependency resolution
//!
//! Handles building a dependency graph of Sigil modules for multi-module compilation

use crate::project::{
    get_project_config, package_version_fragment, ProjectConfig, ProjectConfigError,
};
use sigil_ast::{
    ConcurrentStep, Declaration, Expr, Pattern, Program, RecordPatternField, Type, TypeDef,
};
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use sigil_typechecker::EffectCatalog;
use sigil_validator::{validate_canonical_form_with_options, ValidationError, ValidationOptions};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

fn format_validation_errors(errors: &[ValidationError]) -> String {
    if errors.is_empty() {
        "validation errors".to_string()
    } else {
        errors
            .iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[derive(Debug, Error)]
pub enum ModuleGraphError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lexer error: {0}")]
    Lexer(String),

    #[error("Parser error: {0}")]
    Parser(String),

    #[error("Validation error: {}", format_validation_errors(.0))]
    Validation(Vec<ValidationError>),

    #[error("Module cycle detected: {0:?}")]
    ImportCycle(Vec<String>),

    #[error("Module not found: {module_id} (expected at {expected_path})")]
    ImportNotFound {
        module_id: String,
        expected_path: String,
    },

    #[error(transparent)]
    ProjectConfig(#[from] ProjectConfigError),
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
    pub output_project: Option<ProjectConfig>,
    pub source_imports: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInstance {
    pub name: String,
    pub version: String,
}

impl ModuleGraph {
    pub fn build(entry_file: &Path) -> Result<Self, ModuleGraphError> {
        let mut builder = ModuleGraphBuilder::new();
        builder.visit(entry_file, None, None, None, None)?;
        Ok(ModuleGraph {
            modules: builder.modules,
            topo_order: builder.topo_order,
        })
    }

    pub fn build_many(entry_files: &[PathBuf]) -> Result<Self, ModuleGraphError> {
        let mut builder = ModuleGraphBuilder::new();
        let mut sorted_entries = entry_files.to_vec();
        sorted_entries.sort();
        for entry_file in sorted_entries {
            builder.visit(&entry_file, None, None, None, None)?;
        }
        Ok(ModuleGraph {
            modules: builder.modules,
            topo_order: builder.topo_order,
        })
    }
}

pub fn entry_module_key(file_path: &Path) -> Result<String, ModuleGraphError> {
    let abs_file = fs::canonicalize(file_path)?;
    let project = get_project_config(&abs_file)?;
    Ok(file_path_to_module_id(&abs_file, &project)
        .unwrap_or_else(|| abs_file.to_string_lossy().to_string()))
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
        inherited_output_project: Option<ProjectConfig>,
        inherited_package_instance: Option<PackageInstance>,
    ) -> Result<(), ModuleGraphError> {
        let abs_file = fs::canonicalize(file_path)?;

        // Determine project
        let project = get_project_config(&abs_file)?.or(inherited_project);
        let output_project = inherited_output_project.or_else(|| project.clone());

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
            .map_err(|e| ModuleGraphError::Lexer(format!("{}", e)))?;

        // Parse
        let filename = abs_file.to_string_lossy().to_string();
        let mut parser = Parser::new(tokens, &filename);
        let ast = parser
            .parse()
            .map_err(|e| ModuleGraphError::Parser(format!("{}", e)))?;

        let effect_catalog = load_project_effect_catalog(project.as_ref())?;

        // Validate
        validate_canonical_form_with_options(
            &ast,
            Some(&filename),
            Some(&source),
            ValidationOptions { effect_catalog },
        )
        .map_err(|e| ModuleGraphError::Validation(e))?;

        let mut source_imports = HashMap::new();

        // Process implicit core prelude first for non-core modules.
        if module_key != "core::prelude" {
            let resolved = resolve_sigil_import(
                &abs_file,
                &module_key,
                project.as_ref(),
                output_project.as_ref(),
                inherited_package_instance.as_ref(),
                "core::prelude",
            )?;
            if resolved.file_path.exists() {
                source_imports.insert("core::prelude".to_string(), resolved.module_id.clone());
                self.visit(
                    &resolved.file_path,
                    Some(resolved.module_id),
                    resolved.project,
                    resolved.output_project,
                    resolved.package_instance,
                )?;
            }
        }

        // Process referenced Sigil modules
        for module_id in collect_referenced_module_ids(&ast) {
            if module_id == "core::prelude" {
                continue;
            }

            let resolved = resolve_sigil_import(
                &abs_file,
                &module_key,
                project.as_ref(),
                output_project.as_ref(),
                inherited_package_instance.as_ref(),
                &module_id,
            )?;

            if !resolved.file_path.exists() {
                return Err(ModuleGraphError::ImportNotFound {
                    module_id: module_id.clone(),
                    expected_path: resolved.file_path.to_string_lossy().to_string(),
                });
            }

            source_imports.insert(module_id.clone(), resolved.module_id.clone());
            self.visit(
                &resolved.file_path,
                Some(resolved.module_id),
                resolved.project,
                resolved.output_project,
                resolved.package_instance,
            )?;
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
                output_project,
                source_imports,
            },
        );
        self.topo_order.push(module_key);

        Ok(())
    }
}

pub fn load_project_effect_catalog_for(
    file_path: &Path,
) -> Result<Option<EffectCatalog>, ModuleGraphError> {
    let project = get_project_config(file_path)?;
    load_project_effect_catalog(project.as_ref())
}

fn is_sigil_import_path(module_path: &str) -> bool {
    module_path.starts_with("core::")
        || module_path.starts_with("stdlib::")
        || module_path.starts_with("world::")
        || module_path.starts_with("test::")
        || module_path.starts_with("src::")
        || module_path.starts_with("config::")
        || module_path.starts_with("package::")
}

pub fn collect_referenced_module_ids(program: &Program) -> HashSet<String> {
    let mut modules = HashSet::new();
    for declaration in &program.declarations {
        collect_declaration_modules(declaration, &mut modules);
    }
    modules.retain(|module_id| is_sigil_import_path(module_id));
    modules
}

fn collect_declaration_modules(declaration: &Declaration, modules: &mut HashSet<String>) {
    match declaration {
        Declaration::Function(function) => {
            for param in &function.params {
                if let Some(type_annotation) = &param.type_annotation {
                    collect_type_modules(type_annotation, modules);
                }
            }
            if let Some(return_type) = &function.return_type {
                collect_type_modules(return_type, modules);
            }
            collect_expr_modules(&function.body, modules);
        }
        Declaration::Type(type_decl) => match &type_decl.definition {
            TypeDef::Sum(sum) => {
                for variant in &sum.variants {
                    for typ in &variant.types {
                        collect_type_modules(typ, modules);
                    }
                }
            }
            TypeDef::Product(product) => {
                for field in &product.fields {
                    collect_type_modules(&field.field_type, modules);
                }
            }
            TypeDef::Alias(alias) => collect_type_modules(&alias.aliased_type, modules),
        },
        Declaration::Effect(_) => {}
        Declaration::Const(const_decl) => {
            if let Some(type_annotation) = &const_decl.type_annotation {
                collect_type_modules(type_annotation, modules);
            }
            collect_expr_modules(&const_decl.value, modules);
        }
        Declaration::Test(test_decl) => {
            for binding in &test_decl.world_bindings {
                if let Some(type_annotation) = &binding.type_annotation {
                    collect_type_modules(type_annotation, modules);
                }
                collect_expr_modules(&binding.value, modules);
            }
            collect_expr_modules(&test_decl.body, modules);
        }
        Declaration::Extern(_) => {}
    }
}

fn collect_expr_modules(expr: &Expr, modules: &mut HashSet<String>) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) => {}
        Expr::Lambda(lambda) => {
            for param in &lambda.params {
                if let Some(type_annotation) = &param.type_annotation {
                    collect_type_modules(type_annotation, modules);
                }
            }
            collect_type_modules(&lambda.return_type, modules);
            collect_expr_modules(&lambda.body, modules);
        }
        Expr::Application(application) => {
            collect_expr_modules(&application.func, modules);
            for arg in &application.args {
                collect_expr_modules(arg, modules);
            }
        }
        Expr::Binary(binary) => {
            collect_expr_modules(&binary.left, modules);
            collect_expr_modules(&binary.right, modules);
        }
        Expr::Unary(unary) => collect_expr_modules(&unary.operand, modules),
        Expr::Match(match_expr) => {
            collect_expr_modules(&match_expr.scrutinee, modules);
            for arm in &match_expr.arms {
                collect_pattern_modules(&arm.pattern, modules);
                if let Some(guard) = &arm.guard {
                    collect_expr_modules(guard, modules);
                }
                collect_expr_modules(&arm.body, modules);
            }
        }
        Expr::Let(let_expr) => {
            collect_pattern_modules(&let_expr.pattern, modules);
            collect_expr_modules(&let_expr.value, modules);
            collect_expr_modules(&let_expr.body, modules);
        }
        Expr::If(if_expr) => {
            collect_expr_modules(&if_expr.condition, modules);
            collect_expr_modules(&if_expr.then_branch, modules);
            if let Some(else_branch) = &if_expr.else_branch {
                collect_expr_modules(else_branch, modules);
            }
        }
        Expr::List(list) => {
            for element in &list.elements {
                collect_expr_modules(element, modules);
            }
        }
        Expr::Record(record) => {
            for field in &record.fields {
                collect_expr_modules(&field.value, modules);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                collect_expr_modules(&entry.key, modules);
                collect_expr_modules(&entry.value, modules);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_expr_modules(element, modules);
            }
        }
        Expr::FieldAccess(access) => collect_expr_modules(&access.object, modules),
        Expr::Index(index) => {
            collect_expr_modules(&index.object, modules);
            collect_expr_modules(&index.index, modules);
        }
        Expr::Pipeline(pipeline) => {
            collect_expr_modules(&pipeline.left, modules);
            collect_expr_modules(&pipeline.right, modules);
        }
        Expr::Map(map) => {
            collect_expr_modules(&map.list, modules);
            collect_expr_modules(&map.func, modules);
        }
        Expr::Filter(filter) => {
            collect_expr_modules(&filter.list, modules);
            collect_expr_modules(&filter.predicate, modules);
        }
        Expr::Fold(fold) => {
            collect_expr_modules(&fold.list, modules);
            collect_expr_modules(&fold.func, modules);
            collect_expr_modules(&fold.init, modules);
        }
        Expr::Concurrent(concurrent) => {
            collect_expr_modules(&concurrent.width, modules);
            if let Some(policy) = &concurrent.policy {
                collect_expr_modules(&Expr::Record(policy.clone()), modules);
            }
            for step in &concurrent.steps {
                match step {
                    ConcurrentStep::Spawn(spawn) => collect_expr_modules(&spawn.expr, modules),
                    ConcurrentStep::SpawnEach(spawn_each) => {
                        collect_expr_modules(&spawn_each.list, modules);
                        collect_expr_modules(&spawn_each.func, modules);
                    }
                }
            }
        }
        Expr::MemberAccess(member_access) => {
            modules.insert(member_access.namespace.join("::"));
        }
        Expr::TypeAscription(ascription) => {
            collect_expr_modules(&ascription.expr, modules);
            collect_type_modules(&ascription.ascribed_type, modules);
        }
    }
}

fn collect_pattern_modules(pattern: &Pattern, modules: &mut HashSet<String>) {
    match pattern {
        Pattern::Literal(_) | Pattern::Identifier(_) | Pattern::Wildcard(_) => {}
        Pattern::Constructor(constructor) => {
            if !constructor.module_path.is_empty() {
                modules.insert(constructor.module_path.join("::"));
            }
            for nested in &constructor.patterns {
                collect_pattern_modules(nested, modules);
            }
        }
        Pattern::List(list) => {
            for nested in &list.patterns {
                collect_pattern_modules(nested, modules);
            }
        }
        Pattern::Record(record) => {
            for RecordPatternField { pattern, .. } in &record.fields {
                if let Some(nested) = pattern {
                    collect_pattern_modules(nested, modules);
                }
            }
        }
        Pattern::Tuple(tuple) => {
            for nested in &tuple.patterns {
                collect_pattern_modules(nested, modules);
            }
        }
    }
}

fn collect_type_modules(typ: &Type, modules: &mut HashSet<String>) {
    match typ {
        Type::Primitive(_) | Type::Variable(_) => {}
        Type::List(list) => collect_type_modules(&list.element_type, modules),
        Type::Map(map) => {
            collect_type_modules(&map.key_type, modules);
            collect_type_modules(&map.value_type, modules);
        }
        Type::Function(function) => {
            for param in &function.param_types {
                collect_type_modules(param, modules);
            }
            collect_type_modules(&function.return_type, modules);
        }
        Type::Constructor(constructor) => {
            for arg in &constructor.type_args {
                collect_type_modules(arg, modules);
            }
        }
        Type::Tuple(tuple) => {
            for nested in &tuple.types {
                collect_type_modules(nested, modules);
            }
        }
        Type::Qualified(qualified) => {
            modules.insert(qualified.module_path.join("::"));
            for arg in &qualified.type_args {
                collect_type_modules(arg, modules);
            }
        }
    }
}

fn file_path_to_module_id(abs_path: &Path, project: &Option<ProjectConfig>) -> Option<String> {
    let path_str = abs_path.to_string_lossy();

    // Check if it's a core module
    if path_str.contains("/core/") {
        if let Some(relative) = path_str.split("/core/").nth(1) {
            if let Some(without_ext) = strip_sigil_ext(relative) {
                return Some(format!("core::{}", without_ext.replace('/', "::")));
            }
        }
    }

    // Check if it's a stdlib module
    if path_str.contains("/stdlib/") {
        if let Some(relative) = path_str.split("/stdlib/").nth(1) {
            if let Some(without_ext) = strip_sigil_ext(relative) {
                return Some(format!("stdlib::{}", without_ext.replace('/', "::")));
            }
        }
    }

    // Check if it's a world module
    if path_str.contains("/world/") {
        if let Some(relative) = path_str.split("/world/").nth(1) {
            if let Some(without_ext) = strip_sigil_ext(relative) {
                return Some(format!("world::{}", without_ext.replace('/', "::")));
            }
        }
    }

    // Check if it's a test module
    if path_str.contains("/test/") {
        if let Some(relative) = path_str.split("/test/").nth(1) {
            if let Some(without_ext) = strip_sigil_ext(relative) {
                return Some(format!("test::{}", without_ext.replace('/', "::")));
            }
        }
    }

    // Check if it's a project src module
    if let Some(ref proj) = project {
        let proj_root = proj.root.to_string_lossy();
        if path_str.starts_with(proj_root.as_ref()) {
            if let Some(relative) = path_str.strip_prefix(proj_root.as_ref()) {
                let relative = relative.trim_start_matches('/');
                if let Some(without_ext) = strip_sigil_ext(relative) {
                    return Some(without_ext.replace('/', "::"));
                }
            }
        }
    }

    None
}

fn strip_sigil_ext(relative: &str) -> Option<&str> {
    if let Some(without_ext) = relative.strip_suffix(".lib.sigil") {
        Some(without_ext)
    } else {
        relative.strip_suffix(".sigil")
    }
}

struct ResolvedImport {
    module_id: String,
    file_path: PathBuf,
    project: Option<ProjectConfig>,
    output_project: Option<ProjectConfig>,
    package_instance: Option<PackageInstance>,
}

fn resolve_import_path(base_path: &Path, file_path_str: &str) -> Result<PathBuf, ModuleGraphError> {
    let lib_path = base_path.join(format!("{}.lib.sigil", file_path_str));

    if lib_path.exists() {
        Ok(lib_path)
    } else {
        Err(ModuleGraphError::ImportNotFound {
            module_id: file_path_str.to_string(),
            expected_path: format!(
                "Expected: {:?} (libraries must use .lib.sigil extension)",
                lib_path
            ),
        })
    }
}

fn load_project_effect_catalog(
    project: Option<&ProjectConfig>,
) -> Result<Option<EffectCatalog>, ModuleGraphError> {
    let Some(project) = project else {
        return Ok(None);
    };

    let effects_path = project.root.join("src/effects.lib.sigil");
    if !effects_path.exists() {
        return Ok(None);
    }

    let source = fs::read_to_string(&effects_path)?;
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| ModuleGraphError::Lexer(format!("{}", e)))?;
    let filename = effects_path.to_string_lossy().to_string();
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser
        .parse()
        .map_err(|e| ModuleGraphError::Parser(format!("{}", e)))?;
    let effect_catalog =
        EffectCatalog::from_program(&ast).map_err(|message| ModuleGraphError::Parser(message))?;

    validate_canonical_form_with_options(
        &ast,
        Some(&filename),
        Some(&source),
        ValidationOptions {
            effect_catalog: Some(effect_catalog.clone()),
        },
    )
    .map_err(ModuleGraphError::Validation)?;

    Ok(Some(effect_catalog))
}

fn package_internal_root_segments(instance: &PackageInstance) -> Result<Vec<String>, ModuleGraphError> {
    let version_fragment = package_version_fragment(&instance.version).ok_or_else(|| {
        ModuleGraphError::ProjectConfig(ProjectConfigError::Invalid {
            path: PathBuf::from("sigil.json"),
            message: format!(
                "package version `{}` must use canonical UTC timestamp format",
                instance.version
            ),
        })
    })?;

    Ok(vec![
        "package".to_string(),
        instance.name.clone(),
        version_fragment,
    ])
}

fn internal_package_module_id(
    instance: &PackageInstance,
    source_subpath: &[&str],
) -> Result<String, ModuleGraphError> {
    let mut segments = package_internal_root_segments(instance)?;
    segments.extend(source_subpath.iter().map(|segment| (*segment).to_string()));
    Ok(segments.join("::"))
}

fn internal_project_module_id(
    instance: &PackageInstance,
    source_module_id: &str,
) -> Result<String, ModuleGraphError> {
    let parts = source_module_id.split("::").collect::<Vec<_>>();
    let (root, rest) = parts.split_first().ok_or_else(|| ModuleGraphError::ImportNotFound {
        module_id: source_module_id.to_string(),
        expected_path: "empty module id".to_string(),
    })?;

    let internal_root = if *root == "config" {
        "packageConfig"
    } else {
        "package"
    };

    let version_fragment = package_version_fragment(&instance.version).ok_or_else(|| {
        ModuleGraphError::ImportNotFound {
            module_id: source_module_id.to_string(),
            expected_path: format!(
                "invalid package version `{}` for dependency `{}`",
                instance.version, instance.name
            ),
        }
    })?;

    let mut segments = vec![
        internal_root.to_string(),
        instance.name.clone(),
        version_fragment,
    ];

    if *root == "src" && rest.len() == 1 && rest[0] == "package" {
        return Ok(segments.join("::"));
    }

    if *root == "src" && !rest.is_empty() && rest[0] == "package" {
        segments.extend(rest[1..].iter().map(|segment| (*segment).to_string()));
        return Ok(segments.join("::"));
    }

    segments.extend(rest.iter().map(|segment| (*segment).to_string()));
    Ok(segments.join("::"))
}

fn resolve_package_import(
    importer_project: &ProjectConfig,
    importer_output_project: Option<&ProjectConfig>,
    module_id: &str,
) -> Result<ResolvedImport, ModuleGraphError> {
    let parts = module_id.split("::").collect::<Vec<_>>();
    if parts.len() < 2 {
        return Err(ModuleGraphError::ImportNotFound {
            module_id: module_id.to_string(),
            expected_path: "package imports must name a direct dependency".to_string(),
        });
    }

    let dependency_name = parts[1];
    let dependency_version = importer_project
        .dependencies
        .get(dependency_name)
        .ok_or_else(|| ModuleGraphError::ImportNotFound {
            module_id: module_id.to_string(),
            expected_path: format!(
                "direct dependency `{dependency_name}` is not declared in sigil.json"
            ),
        })?;

    let dependency_root = importer_project
        .package_store_root()
        .join(dependency_name)
        .join(dependency_version);

    let dependency_project =
        get_project_config(&dependency_root)?.ok_or_else(|| ModuleGraphError::ImportNotFound {
            module_id: module_id.to_string(),
            expected_path: dependency_root.join("sigil.json").to_string_lossy().to_string(),
        })?;

    if dependency_project.name != dependency_name {
        return Err(ModuleGraphError::ImportNotFound {
            module_id: module_id.to_string(),
            expected_path: format!(
                "installed package at {} declares name `{}` instead of `{}`",
                dependency_root.display(),
                dependency_project.name,
                dependency_name
            ),
        });
    }

    if dependency_project.version != *dependency_version {
        return Err(ModuleGraphError::ImportNotFound {
            module_id: module_id.to_string(),
            expected_path: format!(
                "installed package at {} declares version `{}` instead of `{}`",
                dependency_root.display(),
                dependency_project.version,
                dependency_version
            ),
        });
    }

    let package_instance = PackageInstance {
        name: dependency_name.to_string(),
        version: dependency_version.clone(),
    };

    let subpath = &parts[2..];
    let file_path = if subpath.is_empty() {
        dependency_root.join("src/package.lib.sigil")
    } else {
        dependency_root
            .join("src")
            .join(subpath.join("/"))
            .with_extension("lib.sigil")
    };

    Ok(ResolvedImport {
        module_id: internal_package_module_id(&package_instance, subpath)?,
        file_path,
        project: Some(dependency_project),
        output_project: importer_output_project.cloned(),
        package_instance: Some(package_instance),
    })
}

fn resolve_sigil_import(
    importer_file: &Path,
    importer_module_id: &str,
    importer_project: Option<&ProjectConfig>,
    importer_output_project: Option<&ProjectConfig>,
    importer_package_instance: Option<&PackageInstance>,
    module_id: &str,
) -> Result<ResolvedImport, ModuleGraphError> {
    // Convert module ID (with :: separators) to file path
    let file_path_str = module_id.replace("::", "/");

    if module_id.starts_with("src::") || module_id.starts_with("config::") {
        // Project module reference
        let project = importer_project.ok_or_else(|| ModuleGraphError::ImportNotFound {
            module_id: module_id.to_string(),
            expected_path: "project not found".to_string(),
        })?;

        let file_path = resolve_import_path(&project.root, &file_path_str)?;
        let resolved_module_id = if let Some(package_instance) = importer_package_instance {
            internal_project_module_id(package_instance, module_id)?
        } else {
            module_id.to_string()
        };

        Ok(ResolvedImport {
            module_id: resolved_module_id,
            file_path,
            project: Some(project.clone()),
            output_project: importer_output_project.cloned(),
            package_instance: importer_package_instance.cloned(),
        })
    } else if module_id.starts_with("package::") {
        let project = importer_project.ok_or_else(|| ModuleGraphError::ImportNotFound {
            module_id: module_id.to_string(),
            expected_path: "project not found".to_string(),
        })?;

        resolve_package_import(project, importer_output_project, module_id)
    } else if module_id.starts_with("stdlib::")
        || module_id.starts_with("core::")
        || module_id.starts_with("world::")
        || module_id.starts_with("test::")
    {
        // Language module reference - find language root
        let language_root = find_language_root(importer_file)?;
        let file_path = resolve_import_path(&language_root, &file_path_str)?;

        Ok(ResolvedImport {
            module_id: module_id.to_string(),
            file_path,
            project: importer_project.cloned(),
            output_project: importer_output_project.cloned(),
            package_instance: importer_package_instance.cloned(),
        })
    } else {
        Err(ModuleGraphError::ImportNotFound {
            module_id: importer_module_id.to_string(),
            expected_path: "unknown module root".to_string(),
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

        let is_generated_local_dir = current
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == ".local");

        if is_generated_local_dir {
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
                continue;
            }
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
