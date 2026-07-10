use super::compile_support::{analyze_module_graph, collect_sigil_targets, AnalyzedModule};
use super::legacy::CliError;
use super::shared::{extract_error_code, output_json_value, phase_for_code};
use crate::module_graph::ModuleGraph;
use serde_json::{json, Value};
use sigil_ast::{Declaration, RuleAction, SourceLocation};
use sigil_typechecker::typed_ir::{
    MethodSelector, TypedConcurrentStep, TypedDeclaration, TypedExpr, TypedExprKind,
};
use sigil_validator::print_canonical_type;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const COMMAND: &str = "sigil inspect trust";

pub(super) fn inspect_trust_command(
    path: &Path,
    selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    let files = collect_sigil_targets("inspect trust", path, ignore_paths, ignore_from)?;
    let mut diagnostics = Vec::new();
    let mut results_by_path = BTreeMap::new();
    let mut analyzed_files = 0usize;

    for file in &files {
        let graph = match ModuleGraph::build_many_with_env(std::slice::from_ref(file), selected_env)
        {
            Ok(graph) => graph,
            Err(error) => {
                diagnostics.push(error_diagnostic(
                    &error.to_string(),
                    std::slice::from_ref(file),
                ));
                continue;
            }
        };
        match analyze_module_graph(&graph) {
            Ok(analyzed) => {
                analyzed_files += 1;
                for module in analyzed.modules.values() {
                    let result = module_trust_json(module);
                    results_by_path.insert(
                        result["path"].as_str().unwrap_or_default().to_string(),
                        result,
                    );
                }
            }
            Err(error) => diagnostics.push(error_diagnostic(
                &error.to_string(),
                std::slice::from_ref(file),
            )),
        }
    }

    let results = results_by_path.into_values().collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| {
        left["details"]["files"]
            .to_string()
            .cmp(&right["details"]["files"].to_string())
            .then(left["code"].as_str().cmp(&right["code"].as_str()))
    });

    let assumptions = results
        .iter()
        .map(|file| file["assumptions"].as_array().map_or(0, Vec::len))
        .sum::<usize>();
    let controls = results
        .iter()
        .map(|file| file["controls"].as_array().map_or(0, Vec::len))
        .sum::<usize>();
    let runtime = results
        .iter()
        .map(|file| file["runtime"].as_array().map_or(0, Vec::len))
        .sum::<usize>();
    let project = results
        .iter()
        .filter_map(|file| file.get("project"))
        .find(|project| !project.is_null())
        .cloned();
    let dependencies = project
        .as_ref()
        .and_then(|project| project.get("root"))
        .and_then(Value::as_str)
        .map(project_dependencies)
        .unwrap_or_default();
    let ok = diagnostics.is_empty();
    let status = if ok {
        "complete"
    } else if results.is_empty() {
        "failed"
    } else {
        "partial"
    };

    output_json_value(
        &json!({
            "formatVersion": 1,
            "command": COMMAND,
            "ok": ok,
            "phase": diagnostics.first().and_then(|d| d["phase"].as_str()).unwrap_or("typecheck"),
            "analysis": {"status": status, "level": if ok {"typed"} else if results.is_empty() {"parsed"} else {"mixed"}},
            "data": {
                "input": normalize_path(path),
                "project": project,
                "dependencies": dependencies,
                "summary": {
                    "discoveredFiles": files.len(),
                    "analyzedFiles": analyzed_files,
                    "assumptions": assumptions,
                    "controls": controls,
                    "runtimeEntries": runtime
                },
                "files": results
            },
            "diagnostics": diagnostics
        }),
        false,
    );

    if ok {
        Ok(())
    } else {
        Err(CliError::Reported(1))
    }
}

fn module_trust_json(module: &AnalyzedModule) -> Value {
    let path = normalize_path(&module.file_path);
    let mut assumptions = Vec::new();
    let mut controls = Vec::new();
    let mut runtime = Vec::new();
    let mut extern_uses: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    for declaration in &module.typed_program.declarations {
        match declaration {
            TypedDeclaration::Function(function) => {
                collect_extern_calls(&function.body, &path, &mut extern_uses);
                if let Some(effects) = &function.effects {
                    runtime.push(runtime_effects(&function.name, effects));
                }
            }
            TypedDeclaration::Const(constant) => {
                collect_extern_calls(&constant.value, &path, &mut extern_uses)
            }
            TypedDeclaration::Test(test) => {
                collect_extern_calls(&test.body, &path, &mut extern_uses);
                if let Some(effects) = &test.effects {
                    runtime.push(runtime_effects(&test.description, effects));
                }
            }
            TypedDeclaration::JsonCodec(codec) => {
                let validates_refinements = codec
                    .named_types
                    .iter()
                    .any(|named| named.constraint.is_some());
                controls.push(json!({
                    "kind": "derivedJsonCodec",
                    "evidenceLevel": "typed",
                    "target": codec.target_name,
                    "typeId": codec.target_type_id,
                    "validation": if validates_refinements {"refinement"} else {"shapeOnly"},
                    "location": source_span(&path, codec.location)
                }));
            }
            TypedDeclaration::Extern(_) | TypedDeclaration::Type(_) => {}
        }
    }

    for declaration in &module.ast.declarations {
        match declaration {
            Declaration::Extern(extern_decl) => {
                let namespace = extern_decl.module_path.join("::");
                let members = extern_decl
                    .members
                    .as_ref()
                    .map(|members| {
                        members
                            .iter()
                            .map(|member| {
                                json!({
                                    "name": member.name,
                                    "kind": format!("{:?}", member.kind).to_lowercase(),
                                    "type": print_canonical_type(&member.member_type)
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let uses = extern_uses
                    .iter()
                    .filter(|(key, _)| key.starts_with(&format!("{namespace}.")))
                    .flat_map(|(_, uses)| uses.clone())
                    .collect::<Vec<_>>();
                assumptions.push(json!({
                    "kind": "extern",
                    "evidenceLevel": "typed",
                    "trustMode": if extern_decl.members.is_some() {"typed"} else {"untyped"},
                    "namespace": namespace,
                    "members": members,
                    "uses": uses,
                    "location": source_span(&path, extern_decl.location)
                }));
            }
            Declaration::Protocol(protocol) => {
                for transition in &protocol.transitions {
                    for via in &transition.via {
                        assumptions.push(json!({
                            "kind": "protocolStateAxiom",
                            "evidenceLevel": "typed",
                            "protocol": protocol.name,
                            "from": transition.from,
                            "to": transition.to,
                            "via": via,
                            "location": source_span(&path, transition.location)
                        }));
                    }
                }
            }
            Declaration::Rule(rule) => {
                let action = match &rule.action {
                    RuleAction::Allow { .. } => json!({"kind": "allow"}),
                    RuleAction::Block { .. } => json!({"kind": "block"}),
                    RuleAction::Through { transform, .. } => json!({
                        "kind": "through",
                        "transform": format!("{}.{}", transform.module_path.join("::"), transform.member)
                    }),
                };
                controls.push(json!({
                    "kind": "boundaryPolicy",
                    "evidenceLevel": "typed",
                    "boundary": format!("{}.{}", rule.boundary.module_path.join("::"), rule.boundary.member),
                    "labels": rule.labels.iter().map(|label| label.name.clone()).collect::<Vec<_>>(),
                    "action": action,
                    "location": source_span(&path, rule.location)
                }));
            }
            Declaration::Const(constant)
                if path.ends_with("/src/topology.lib.sigil")
                    || path.ends_with("src/topology.lib.sigil") =>
            {
                controls.push(json!({
                    "kind": "topologyBoundary",
                    "evidenceLevel": "typed",
                    "name": constant.name,
                    "type": constant.type_annotation.as_ref().map(print_canonical_type),
                    "location": source_span(&path, constant.location)
                }));
            }
            _ => {}
        }
    }

    assumptions.sort_by_key(|item| item.to_string());
    controls.sort_by_key(|item| item.to_string());
    runtime.sort_by_key(|item| item.to_string());

    json!({
        "path": path,
        "moduleId": module.module_id,
        "analysis": {"status": "complete", "level": "typed"},
        "project": module.project.as_ref().map(|project| json!({
            "name": project.name,
            "root": normalize_path(&project.root),
            "version": project.version
        })),
        "assumptions": assumptions,
        "controls": controls,
        "runtime": runtime
    })
}

fn runtime_effects(name: &str, effects: &std::collections::HashSet<String>) -> Value {
    let mut effects = effects.iter().cloned().collect::<Vec<_>>();
    effects.sort();
    json!({
        "kind": "effects",
        "evidenceLevel": "typed",
        "declaration": name,
        "effects": effects
    })
}

fn collect_extern_calls(expr: &TypedExpr, path: &str, uses: &mut BTreeMap<String, Vec<Value>>) {
    match &expr.kind {
        TypedExprKind::ExternCall(call) => {
            uses.entry(format!("{}.{}", call.namespace.join("::"), call.member))
                .or_default()
                .push(json!({
                    "member": call.member,
                    "subscription": call.subscription,
                    "location": source_span(path, expr.location)
                }));
            for arg in &call.args {
                collect_extern_calls(arg, path, uses);
            }
        }
        TypedExprKind::Lambda(value) => collect_extern_calls(&value.body, path, uses),
        TypedExprKind::Call(value) => {
            collect_extern_calls(&value.func, path, uses);
            walk_exprs(&value.args, path, uses);
        }
        TypedExprKind::ConstructorCall(value) => walk_exprs(&value.args, path, uses),
        TypedExprKind::MethodCall(value) => {
            collect_extern_calls(&value.receiver, path, uses);
            if let MethodSelector::Index(index) = &value.selector {
                collect_extern_calls(index, path, uses);
            }
            walk_exprs(&value.args, path, uses);
        }
        TypedExprKind::Binary(value) => {
            collect_extern_calls(&value.left, path, uses);
            collect_extern_calls(&value.right, path, uses);
        }
        TypedExprKind::Unary(value) => collect_extern_calls(&value.operand, path, uses),
        TypedExprKind::Match(value) => {
            collect_extern_calls(&value.scrutinee, path, uses);
            for arm in &value.arms {
                if let Some(guard) = &arm.guard {
                    collect_extern_calls(guard, path, uses);
                }
                collect_extern_calls(&arm.body, path, uses);
            }
        }
        TypedExprKind::Let(value) => {
            collect_extern_calls(&value.value, path, uses);
            collect_extern_calls(&value.body, path, uses);
        }
        TypedExprKind::Using(value) => {
            collect_extern_calls(&value.value, path, uses);
            collect_extern_calls(&value.body, path, uses);
        }
        TypedExprKind::If(value) => {
            collect_extern_calls(&value.condition, path, uses);
            collect_extern_calls(&value.then_branch, path, uses);
            if let Some(otherwise) = &value.else_branch {
                collect_extern_calls(otherwise, path, uses);
            }
        }
        TypedExprKind::List(value) => walk_exprs(&value.elements, path, uses),
        TypedExprKind::Tuple(value) => walk_exprs(&value.elements, path, uses),
        TypedExprKind::Record(value) => {
            for field in &value.fields {
                collect_extern_calls(&field.value, path, uses);
            }
        }
        TypedExprKind::MapLiteral(value) => {
            for entry in &value.entries {
                collect_extern_calls(&entry.key, path, uses);
                collect_extern_calls(&entry.value, path, uses);
            }
        }
        TypedExprKind::FieldAccess(value) => collect_extern_calls(&value.object, path, uses),
        TypedExprKind::Index(value) => {
            collect_extern_calls(&value.object, path, uses);
            collect_extern_calls(&value.index, path, uses);
        }
        TypedExprKind::Map(value) => {
            collect_extern_calls(&value.list, path, uses);
            collect_extern_calls(&value.func, path, uses);
        }
        TypedExprKind::Filter(value) => {
            collect_extern_calls(&value.list, path, uses);
            collect_extern_calls(&value.predicate, path, uses);
        }
        TypedExprKind::Fold(value) => {
            collect_extern_calls(&value.list, path, uses);
            collect_extern_calls(&value.func, path, uses);
            collect_extern_calls(&value.init, path, uses);
        }
        TypedExprKind::Concurrent(value) => {
            collect_extern_calls(&value.config.width, path, uses);
            for optional in [
                &value.config.jitter_ms,
                &value.config.stop_on,
                &value.config.window_ms,
            ]
            .into_iter()
            .flatten()
            {
                collect_extern_calls(optional, path, uses);
            }
            for step in &value.steps {
                match step {
                    TypedConcurrentStep::Spawn(spawn) => {
                        collect_extern_calls(&spawn.expr, path, uses)
                    }
                    TypedConcurrentStep::SpawnEach(spawn) => {
                        collect_extern_calls(&spawn.list, path, uses);
                        collect_extern_calls(&spawn.func, path, uses);
                    }
                }
            }
        }
        TypedExprKind::Pipeline(value) => {
            collect_extern_calls(&value.left, path, uses);
            collect_extern_calls(&value.right, path, uses);
        }
        TypedExprKind::Literal(_)
        | TypedExprKind::Identifier(_)
        | TypedExprKind::NamespaceMember { .. } => {}
    }
}

fn walk_exprs(expressions: &[TypedExpr], path: &str, uses: &mut BTreeMap<String, Vec<Value>>) {
    for expression in expressions {
        collect_extern_calls(expression, path, uses);
    }
}

fn source_span(file: &str, location: SourceLocation) -> Value {
    json!({
        "file": file,
        "start": {
            "line": location.start.line,
            "column": location.start.column,
            "offset": location.start.offset
        },
        "end": {
            "line": location.end.line,
            "column": location.end.column,
            "offset": location.end.offset
        }
    })
}

fn project_dependencies(root: &str) -> Vec<Value> {
    let root = Path::new(root);
    let manifest = fs::read_to_string(root.join("sigil.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let direct = manifest
        .as_ref()
        .and_then(|manifest| manifest["dependencies"].as_object())
        .cloned()
        .unwrap_or_default();
    let lock = fs::read_to_string(root.join("sigil.lock"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let mut entries = BTreeMap::new();
    for (name, version) in direct {
        entries.insert(
            format!("{name}@{}", version.as_str().unwrap_or_default()),
            json!({"name": name, "version": version, "direct": true}),
        );
    }
    if let Some(packages) = lock.as_ref().and_then(|lock| lock["packages"].as_object()) {
        for key in packages.keys() {
            entries.entry(key.clone()).or_insert_with(|| {
                let (name, version) = key.rsplit_once('@').unwrap_or((key, ""));
                json!({"name": name, "version": version, "direct": false})
            });
        }
    }
    entries.into_values().collect()
}

fn error_diagnostic(message: &str, files: &[PathBuf]) -> Value {
    let code = extract_error_code(message);
    let code = if code.starts_with("SIGIL-") {
        code
    } else {
        "SIGIL-CLI-UNEXPECTED".to_string()
    };
    json!({
        "code": code,
        "phase": phase_for_code(&code),
        "severity": "error",
        "message": message,
        "details": {"files": files.iter().map(|file| normalize_path(file)).collect::<Vec<_>>()}
    })
}

fn normalize_path(path: &Path) -> String {
    let display = std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf());
    display.to_string_lossy().replace('\\', "/")
}
