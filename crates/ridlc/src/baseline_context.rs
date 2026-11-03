//! Portable remote evidence beside a flat set of local IR snapshots.
use ridl_core::MaterializedUnit;
use ridl_ir::v2::{self, Package};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The artifact is evidence, never a compared package snapshot.
pub const CONTEXT_FILE: &str = "context.json";

/// The content identity of a verified remote unit, without its cache path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePin {
    pub url: String,
    pub tree: String,
}

/// Complete remote packages used to resolve one side of a comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteContext {
    pub packages: Vec<Package>,
    /// Pins keyed by the owning unit's canonical identity.
    pub pins: BTreeMap<String, RemotePin>,
}

/// Reached snapshots belonging to one verified remote unit.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteUnitContext {
    pub unit: String,
    pub url: String,
    pub tree: String,
    pub packages: Vec<Package>,
}

/// Versioned evidence bound to every local snapshot by its canonical JSON hash.
#[derive(Debug, Clone, PartialEq)]
pub struct BaselineContext {
    pub version: u32,
    pub local_packages: BTreeMap<String, String>,
    pub remote_units: Vec<RemoteUnitContext>,
}

impl BaselineContext {
    /// The remote packages and their pin evidence, apart from local ownership.
    #[must_use]
    pub fn remote_context(&self) -> RemoteContext {
        RemoteContext {
            packages: self
                .remote_units
                .iter()
                .flat_map(|u| u.packages.clone())
                .collect(),
            pins: self
                .remote_units
                .iter()
                .map(|u| {
                    (
                        u.unit.clone(),
                        RemotePin {
                            url: u.url.clone(),
                            tree: u.tree.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
}

/// Invalid or unreadable context evidence. Absence is represented by `None`.
#[derive(Debug)]
pub struct ContextArtifactError {
    pub path: Option<PathBuf>,
    pub message: String,
}
impl std::fmt::Display for ContextArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(path) = &self.path {
            write!(f, "{}: ", path.display())?;
        }
        write!(f, "invalid baseline context: {}", self.message)
    }
}
impl std::error::Error for ContextArtifactError {}
fn invalid(message: impl Into<String>) -> ContextArtifactError {
    ContextArtifactError {
        path: None,
        message: message.into(),
    }
}
fn with_path(mut error: ContextArtifactError, path: &Path) -> ContextArtifactError {
    error.path = Some(path.to_path_buf());
    error
}
fn hex_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn canonical(package: &Package) -> Result<String, ContextArtifactError> {
    v2::to_json_pretty(package).map_err(|e| invalid(e.to_string()))
}
fn fingerprint(package: &Package) -> Result<String, ContextArtifactError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(canonical(package)?.as_bytes())
    ))
}
fn index<'a>(
    packages: impl IntoIterator<Item = &'a Package>,
) -> Result<BTreeMap<String, &'a Package>, ContextArtifactError> {
    let mut indexed = BTreeMap::new();
    for package in packages {
        if package.name.is_empty() || package.name == "ridl.std" {
            return Err(invalid(format!(
                "invalid package identity `{}`",
                package.name
            )));
        }
        if indexed.insert(package.name.clone(), package).is_some() {
            return Err(invalid(format!(
                "duplicate package identity `{}`",
                package.name
            )));
        }
    }
    Ok(indexed)
}
fn reached(
    own: &BTreeMap<String, &Package>,
    remote: &BTreeMap<String, &Package>,
) -> Result<BTreeSet<String>, ContextArtifactError> {
    let mut pending: Vec<&Package> = own.values().copied().collect();
    let mut visited = BTreeSet::new();
    while let Some(package) = pending.pop() {
        for name in v2::referenced_packages(package) {
            if name == "ridl.std" || own.contains_key(&name) {
                continue;
            }
            let dependency = remote.get(&name).ok_or_else(|| {
                invalid(format!(
                    "missing referenced package `{name}` reached from `{}`",
                    package.name
                ))
            })?;
            if visited.insert(name) {
                pending.push(dependency);
            }
        }
    }
    Ok(visited)
}

/// Captures the full remote package closure of all local declarations and interactions.
/// Materialized units are evidence supplied by the verified workspace loader.
pub fn capture_context(
    own: &[&Package],
    remotes: &[&Package],
    units: &BTreeMap<String, MaterializedUnit>,
) -> Result<BaselineContext, ContextArtifactError> {
    let local = index(own.iter().copied())?;
    let remote = index(remotes.iter().copied())?;
    if let Some(name) = local.keys().find(|name| remote.contains_key(*name)) {
        return Err(invalid(format!("local/remote package collision `{name}`")));
    }
    for package in own {
        if units.contains_key(v2::unit_of(package)) {
            return Err(invalid(format!(
                "local package `{}` belongs to a remote unit",
                package.name
            )));
        }
    }
    let mut grouped: BTreeMap<String, RemoteUnitContext> = BTreeMap::new();
    for name in reached(&local, &remote)? {
        let package = remote[&name];
        let identity = v2::unit_of(package);
        let pin = units.get(identity).ok_or_else(|| {
            invalid(format!(
                "remote package `{name}` has no verified owning unit `{identity}`"
            ))
        })?;
        canonical(package)?;
        grouped
            .entry(identity.to_string())
            .or_insert_with(|| RemoteUnitContext {
                unit: identity.to_string(),
                url: pin.url.clone(),
                tree: pin.tree.clone(),
                packages: Vec::new(),
            })
            .packages
            .push(package.clone());
    }
    let context = BaselineContext {
        version: 1,
        local_packages: local
            .iter()
            .map(|(name, package)| Ok((name.clone(), fingerprint(package)?)))
            .collect::<Result<_, ContextArtifactError>>()?,
        remote_units: grouped.into_values().collect(),
    };
    validate(&context, Some(own))?;
    Ok(context)
}

fn validate(
    context: &BaselineContext,
    own: Option<&[&Package]>,
) -> Result<(), ContextArtifactError> {
    if context.version != 1 {
        return Err(invalid(format!("unsupported version {}", context.version)));
    }
    for (name, hash) in &context.local_packages {
        if name.is_empty() || name == "ridl.std" || !hex_hash(hash) {
            return Err(invalid(format!("invalid local fingerprint for `{name}`")));
        }
    }
    let mut units = BTreeSet::new();
    let mut packages = Vec::new();
    for unit in &context.remote_units {
        if unit.unit.is_empty() || !units.insert(&unit.unit) {
            return Err(invalid(format!(
                "invalid or duplicate remote unit `{}`",
                unit.unit
            )));
        }
        if unit.url.trim().is_empty() || !hex_hash(&unit.tree) || unit.packages.is_empty() {
            return Err(invalid(format!(
                "missing or invalid pin provenance for `{}`",
                unit.unit
            )));
        }
        for package in &unit.packages {
            if v2::unit_of(package) != unit.unit {
                return Err(invalid(format!(
                    "remote package `{}` does not belong to unit `{}`",
                    package.name, unit.unit
                )));
            }
            if context.local_packages.contains_key(&package.name) {
                return Err(invalid(format!(
                    "local/remote package collision `{}`",
                    package.name
                )));
            }
            canonical(package)?;
            packages.push(package);
        }
    }
    let remote = index(packages)?;
    if let Some(own) = own {
        let local = index(own.iter().copied())?;
        let actual = local
            .iter()
            .map(|(name, package)| Ok((name.clone(), fingerprint(package)?)))
            .collect::<Result<BTreeMap<_, _>, ContextArtifactError>>()?;
        if actual != context.local_packages {
            return Err(invalid(
                "local snapshot fingerprints do not match the complete local inventory",
            ));
        }
        if let Some(package) = own
            .iter()
            .find(|p| units.contains(&v2::unit_of(p).to_string()))
        {
            return Err(invalid(format!(
                "local package `{}` belongs to a remote unit",
                package.name
            )));
        }
        let closure = reached(&local, &remote)?;
        if closure.len() != remote.len() {
            return Err(invalid(
                "remote context contains packages outside the reached closure",
            ));
        }
    } else {
        // A write has only fingerprints for the local packages, but every stored
        // remote must still resolve its own whole-package dependencies.
        for package in remote.values() {
            for name in v2::referenced_packages(package) {
                if name != "ridl.std"
                    && !context.local_packages.contains_key(&name)
                    && !remote.contains_key(&name)
                {
                    return Err(invalid(format!(
                        "missing transitive remote package `{name}`"
                    )));
                }
            }
        }
    }
    Ok(())
}

// Raw nested JSON is routed through the IR reader, including its nesting,
// enum and interaction-class guards. Deserializing Package directly bypasses them.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    #[serde(deserialize_with = "unique_fingerprints")]
    local_packages: BTreeMap<String, String>,
    remote_units: Vec<UnitEnvelope>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitEnvelope {
    unit: String,
    url: String,
    tree: String,
    packages: Vec<Box<RawValue>>,
}
fn unique_fingerprints<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    struct Unique;
    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("unique local package fingerprints")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut values = BTreeMap::new();
            while let Some((name, hash)) = map.next_entry::<String, String>()? {
                if values.insert(name.clone(), hash).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate local package identity `{name}`"
                    )));
                }
            }
            Ok(values)
        }
    }
    deserializer.deserialize_map(Unique)
}

/// Reads sibling evidence. Only an absent artifact is a legacy baseline.
pub fn read_context(
    dir: &Path,
    own: &[Package],
) -> Result<Option<BaselineContext>, ContextArtifactError> {
    let path = dir.join(CONTEXT_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // A dangling artifact symlink is malformed evidence, not absence.
            match std::fs::symlink_metadata(&path) {
                Err(m) if m.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                _ => return Err(with_path(invalid(e.to_string()), &path)),
            }
        }
        Err(e) => return Err(with_path(invalid(e.to_string()), &path)),
    };
    let parse = || -> Result<BaselineContext, ContextArtifactError> {
        let envelope: Envelope = serde_json::from_str(&text).map_err(|e| invalid(e.to_string()))?;
        let mut remote_units = Vec::new();
        for unit in envelope.remote_units {
            let packages = unit
                .packages
                .into_iter()
                .map(|raw| v2::from_json(raw.get()).map_err(|e| invalid(e.to_string())))
                .collect::<Result<Vec<_>, _>>()?;
            remote_units.push(RemoteUnitContext {
                unit: unit.unit,
                url: unit.url,
                tree: unit.tree,
                packages,
            });
        }
        let context = BaselineContext {
            version: envelope.version,
            local_packages: envelope.local_packages,
            remote_units,
        };
        validate(&context, Some(&own.iter().collect::<Vec<_>>()))?;
        Ok(context)
    };
    parse().map(Some).map_err(|e| with_path(e, &path))
}

/// Writes stable bytes using the canonical IR writer for every nested package.
pub fn write_context(dir: &Path, context: &BaselineContext) -> Result<(), ContextArtifactError> {
    let path = dir.join(CONTEXT_FILE);
    let write = || -> Result<(), ContextArtifactError> {
        validate(context, None)?;
        let mut remote_units = Vec::new();
        for unit in &context.remote_units {
            let mut packages: Vec<_> = unit.packages.iter().collect();
            packages.sort_by(|a, b| a.name.cmp(&b.name));
            let packages = packages
                .into_iter()
                .map(|p| RawValue::from_string(canonical(p)?).map_err(|e| invalid(e.to_string())))
                .collect::<Result<_, _>>()?;
            remote_units.push(UnitEnvelope {
                unit: unit.unit.clone(),
                url: unit.url.clone(),
                tree: unit.tree.clone(),
                packages,
            });
        }
        remote_units.sort_by(|a, b| a.unit.cmp(&b.unit));
        let envelope = Envelope {
            version: context.version,
            local_packages: context.local_packages.clone(),
            remote_units,
        };
        let mut text =
            serde_json::to_string_pretty(&envelope).map_err(|e| invalid(e.to_string()))?;
        text.push('\n');
        std::fs::write(&path, text).map_err(|e| invalid(e.to_string()))
    };
    write().map_err(|e| with_path(e, &path))
}
