use sigil_ast::{Declaration, EffectDecl, Program};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub const PRIMITIVE_EFFECTS: [&str; 7] = ["Clock", "Fs", "Http", "Log", "Process", "Tcp", "Timer"];

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

        Ok(Self { aliases })
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
