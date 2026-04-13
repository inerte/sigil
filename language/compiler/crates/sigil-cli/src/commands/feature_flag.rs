use super::compile_support::collect_sigil_targets;
use super::legacy::CliError;
use crate::project::is_canonical_timestamp_version;
use serde_json::json;
use sigil_ast::Declaration;
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use sigil_validator::print_canonical_type;
use std::fs;
use std::path::Path;
use time::macros::format_description;
use time::{Date, OffsetDateTime, PrimitiveDateTime};

#[derive(Debug)]
struct FeatureFlagRecord {
    age_days: i64,
    created_at: String,
    file: String,
    line: usize,
    name: String,
    type_source: String,
}

pub fn feature_flag_audit_command(
    path: &Path,
    older_than: Option<&str>,
) -> Result<(), CliError> {
    let threshold_days = older_than
        .map(parse_older_than_days)
        .transpose()?;
    let files = collect_sigil_targets("featureFlag audit", path, &[], None)?;
    let mut flags = Vec::new();

    for file in &files {
        flags.extend(feature_flags_in_file(file)?);
    }

    let matched = flags
        .iter()
        .filter(|flag| threshold_days.is_none_or(|days| flag.age_days > days))
        .map(|flag| {
            json!({
                "name": flag.name,
                "type": flag.type_source,
                "createdAt": flag.created_at,
                "ageDays": flag.age_days,
                "file": flag.file,
                "line": flag.line
            })
        })
        .collect::<Vec<_>>();

    println!(
        "{}",
        serde_json::to_string(&json!({
            "formatVersion": 1,
            "command": "sigil featureFlag audit",
            "ok": true,
            "phase": "cli",
            "data": {
                "input": path.to_string_lossy(),
                "summary": {
                    "discoveredFiles": files.len(),
                    "flags": flags.len(),
                    "matched": matched.len(),
                    "olderThanDays": threshold_days
                },
                "flags": matched
            }
        }))
        .unwrap()
    );

    Ok(())
}

fn feature_flags_in_file(file: &Path) -> Result<Vec<FeatureFlagRecord>, CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|error| CliError::Lexer(error.to_string()))?;
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser
        .parse()
        .map_err(|error| CliError::Parser(error.to_string()))?;
    let today = OffsetDateTime::now_utc().date();

    Ok(ast
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::FeatureFlag(feature_flag_decl) => Some(FeatureFlagRecord {
                age_days: feature_flag_age_days(&feature_flag_decl.created_at, today).unwrap_or(0),
                created_at: feature_flag_decl.created_at.clone(),
                file: filename.clone(),
                line: feature_flag_decl.location.start.line,
                name: feature_flag_decl.name.clone(),
                type_source: print_canonical_type(&feature_flag_decl.flag_type),
            }),
            _ => None,
        })
        .collect())
}

fn parse_older_than_days(raw: &str) -> Result<i64, CliError> {
    let number = raw.strip_suffix('d').ok_or_else(|| {
        CliError::Validation("sigil featureFlag audit --older-than expects Nd, for example 180d".to_string())
    })?;
    let days = number.parse::<i64>().map_err(|_| {
        CliError::Validation("sigil featureFlag audit --older-than expects Nd, for example 180d".to_string())
    })?;
    if days <= 0 {
        return Err(CliError::Validation(
            "sigil featureFlag audit --older-than expects a positive day count".to_string(),
        ));
    }
    Ok(days)
}

fn feature_flag_age_days(created_at: &str, today: Date) -> Result<i64, CliError> {
    if !is_canonical_timestamp_version(created_at) {
        return Err(CliError::Validation(format!(
            "invalid feature flag createdAt `{created_at}`"
        )));
    }

    let format = format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]Z");
    let timestamp = PrimitiveDateTime::parse(created_at, format).map_err(|error| {
        CliError::Validation(format!(
            "failed to parse feature flag createdAt `{created_at}`: {error}"
        ))
    })?;

    Ok((today - timestamp.date()).whole_days())
}
