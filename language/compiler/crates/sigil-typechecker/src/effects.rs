use crate::environment::TypeEnvironment;
use crate::errors::TypeError;
use crate::typed_ir::PurityClass;
use crate::types::EffectSet;
use sigil_ast::{Declaration, EffectDecl, Program};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub const PRIMITIVE_EFFECTS: [&str; 15] = [
    "Clock",
    "Fs",
    "FsWatch",
    "Http",
    "Log",
    "Process",
    "Pty",
    "Random",
    "Sql",
    "Stream",
    "Task",
    "Tcp",
    "Terminal",
    "Timer",
    "WebSocket",
];

#[cfg(test)]
mod tests {
    use super::EffectCatalog;

    #[test]
    fn stream_is_a_primitive_effect() {
        assert!(EffectCatalog::is_primitive("Stream"));
    }

    #[test]
    fn fswatch_is_a_primitive_effect() {
        assert!(EffectCatalog::is_primitive("FsWatch"));
    }

    #[test]
    fn pty_is_a_primitive_effect() {
        assert!(EffectCatalog::is_primitive("Pty"));
    }

    #[test]
    fn websocket_is_a_primitive_effect() {
        assert!(EffectCatalog::is_primitive("WebSocket"));
    }

    #[test]
    fn sql_is_a_primitive_effect() {
        assert!(EffectCatalog::is_primitive("Sql"));
    }

    #[test]
    fn task_is_a_primitive_effect() {
        assert!(EffectCatalog::is_primitive("Task"));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectAlias {
    pub expanded: BTreeSet<String>,
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectCatalog {
    aliases: BTreeMap<String, EffectAlias>,
}

impl EffectCatalog {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn aliases(&self) -> &BTreeMap<String, EffectAlias> {
        &self.aliases
    }

    pub fn is_primitive(name: &str) -> bool {
        PRIMITIVE_EFFECTS.contains(&name)
    }

    pub fn contains_name(&self, name: &str) -> bool {
        Self::is_primitive(name) || self.aliases.contains_key(name)
    }

    pub fn from_program(program: &Program) -> Result<Self, String> {
        let mut raw: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for decl in &program.declarations {
            if let Declaration::Effect(EffectDecl { effects, name, .. }) = decl {
                if raw.insert(name.clone(), effects.clone()).is_some() {
                    return Err(format!("Duplicate effect declaration '{}'", name));
                }
            }
        }

        build_effect_catalog(raw)
    }

    pub fn merged_with(&self, other: &Self) -> Result<Self, String> {
        if self.aliases.is_empty() {
            return Ok(other.clone());
        }
        if other.aliases.is_empty() {
            return Ok(self.clone());
        }

        let mut raw: BTreeMap<String, Vec<String>> = self
            .aliases
            .iter()
            .map(|(name, alias)| (name.clone(), alias.members.clone()))
            .collect();

        for (name, alias) in &other.aliases {
            match raw.get(name) {
                Some(existing) if existing == &alias.members => {}
                Some(_) => return Err(format!("Duplicate effect declaration '{}'", name)),
                None => {
                    raw.insert(name.clone(), alias.members.clone());
                }
            }
        }

        build_effect_catalog(raw)
    }

    pub fn expand_effect_names(&self, names: &[String]) -> Result<BTreeSet<String>, String> {
        let mut expanded = BTreeSet::new();
        for name in names {
            if Self::is_primitive(name) {
                expanded.insert(name.clone());
            } else if let Some(alias) = self.aliases.get(name) {
                expanded.extend(alias.expanded.iter().cloned());
            } else {
                return Err(format!("Unknown effect '{}'", name));
            }
        }
        Ok(expanded)
    }

    pub fn canonicalize_names(
        &self,
        names: &[String],
        exclude_alias: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let expanded = self.expand_effect_names(names)?;
        Ok(self.canonical_cover_from_expanded(&expanded, exclude_alias))
    }

    pub fn canonical_cover_from_expanded(
        &self,
        expanded: &BTreeSet<String>,
        exclude_alias: Option<&str>,
    ) -> Vec<String> {
        if expanded.is_empty() {
            return Vec::new();
        }

        let candidates: Vec<(String, BTreeSet<String>)> = self
            .aliases
            .iter()
            .filter(|(name, alias)| {
                Some(name.as_str()) != exclude_alias && alias.expanded.is_subset(expanded)
            })
            .map(|(name, alias)| (name.clone(), alias.expanded.clone()))
            .collect();

        let candidate_count = candidates.len();
        let mut best_score: Option<(usize, usize, Vec<String>)> = None;

        for mask in 0..(1usize << candidate_count) {
            let mut union = BTreeSet::new();
            let mut chosen_aliases = Vec::new();

            for (index, (name, alias_expanded)) in candidates.iter().enumerate() {
                if (mask & (1 << index)) != 0 {
                    union.extend(alias_expanded.iter().cloned());
                    chosen_aliases.push(name.clone());
                }
            }

            if !union.is_subset(expanded) {
                continue;
            }

            let mut surface = chosen_aliases;
            for primitive in expanded.difference(&union) {
                surface.push(primitive.clone());
            }
            surface.sort();

            let primitive_count = surface
                .iter()
                .filter(|name| Self::is_primitive(name))
                .count();
            let score = (surface.len(), primitive_count, surface.clone());
            if best_score.as_ref().is_none_or(|current| score < *current) {
                best_score = Some(score);
            }
        }

        best_score
            .map(|(_, _, names)| names)
            .unwrap_or_else(|| expanded.iter().cloned().collect())
    }
}

fn build_effect_catalog(raw: BTreeMap<String, Vec<String>>) -> Result<EffectCatalog, String> {
    let mut aliases = BTreeMap::new();
    let mut resolving = HashSet::new();
    let mut resolved = HashMap::new();

    for name in raw.keys() {
        let expanded = expand_alias(name, &raw, &mut resolving, &mut resolved)?;
        aliases.insert(
            name.clone(),
            EffectAlias {
                expanded,
                members: raw.get(name).cloned().unwrap_or_default(),
            },
        );
    }

    for (name, alias) in &aliases {
        if alias.expanded.len() < 2 {
            return Err(format!(
                "Effect '{}' must expand to at least two primitive effects",
                name
            ));
        }
    }

    let mut seen_expansions: HashMap<BTreeSet<String>, String> = HashMap::new();
    for (name, alias) in &aliases {
        if let Some(existing) = seen_expansions.insert(alias.expanded.clone(), name.clone()) {
            return Err(format!(
                "Effect '{}' duplicates the expanded primitive set of '{}'",
                name, existing
            ));
        }
    }

    Ok(EffectCatalog { aliases })
}

fn expand_alias(
    name: &str,
    raw: &BTreeMap<String, Vec<String>>,
    resolving: &mut HashSet<String>,
    resolved: &mut HashMap<String, BTreeSet<String>>,
) -> Result<BTreeSet<String>, String> {
    if let Some(existing) = resolved.get(name) {
        return Ok(existing.clone());
    }

    if !resolving.insert(name.to_string()) {
        return Err(format!("Effect '{}' is part of a cycle", name));
    }

    let mut expanded = BTreeSet::new();
    let members = raw
        .get(name)
        .ok_or_else(|| format!("Unknown effect '{}'", name))?;

    for member in members {
        if EffectCatalog::is_primitive(member) {
            expanded.insert(member.clone());
        } else if raw.contains_key(member) {
            expanded.extend(expand_alias(member, raw, resolving, resolved)?);
        } else {
            return Err(format!(
                "Effect '{}' references unknown effect '{}'",
                name, member
            ));
        }
    }

    resolving.remove(name);
    resolved.insert(name.to_string(), expanded.clone());
    Ok(expanded)
}

// ============================================================================
// Effect utilities used by the typechecker
// ============================================================================

pub(crate) fn effects_option_to_set(effects: &Option<EffectSet>) -> EffectSet {
    effects.clone().unwrap_or_default()
}

pub(crate) fn resolve_effect_names(
    env: &TypeEnvironment,
    effects: &[String],
    location: sigil_ast::SourceLocation,
    context: &str,
) -> Result<EffectSet, TypeError> {
    env.effect_catalog()
        .expand_effect_names(effects)
        .map(|expanded| expanded.into_iter().collect())
        .map_err(|message| TypeError::new(format!("{}: {}", context, message), Some(location)))
}

pub(crate) fn declared_effects_match_actual(
    env: &TypeEnvironment,
    declared_surface_effects: &[String],
    actual_effects: &EffectSet,
    location: sigil_ast::SourceLocation,
    context: &str,
) -> Result<(), TypeError> {
    let declared_effects = resolve_effect_names(env, declared_surface_effects, location, context)?;
    let mut missing: Vec<String> = actual_effects
        .difference(&declared_effects)
        .cloned()
        .collect();
    missing.sort();
    if !missing.is_empty() {
        return Err(TypeError::new(
            format!(
                "{} is missing declared effects: {}",
                context,
                missing
                    .into_iter()
                    .map(|effect| format!("!{}", effect))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Some(location),
        ));
    }

    let mut unused: Vec<String> = declared_effects
        .difference(actual_effects)
        .cloned()
        .collect();
    unused.sort();
    if unused.is_empty() {
        return Ok(());
    }

    Err(TypeError::new(
        format!(
            "{} has unused declared effects: {}",
            context,
            unused
                .into_iter()
                .map(|effect| format!("!{}", effect))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        Some(location),
    ))
}

pub(crate) fn merge_effects(values: impl IntoIterator<Item = EffectSet>) -> EffectSet {
    let mut merged = HashSet::new();
    for value in values {
        merged.extend(value);
    }
    merged
}

pub(crate) fn purity_from_effects(effects: &EffectSet) -> PurityClass {
    if effects.is_empty() {
        PurityClass::Pure
    } else {
        PurityClass::Effectful
    }
}
