//! Read-only FiveM vehicle resource discovery and metadata matching.
use crate::{
    Result,
    models::{self, Model},
    patterns::Pattern,
};
use sha2::Digest;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

/// Identify the exact UTF-8 snapshot parsed, even if the file changes afterward.
pub fn metadata_source(path: &Path, source: &str) -> String {
    format!(
        "{} [Workshop {}; SHA256 {:x}]",
        path.display(),
        env!("CARGO_PKG_VERSION"),
        sha2::Sha256::digest(source.as_bytes())
    )
}

#[derive(Clone, Debug)]
pub struct Vehicle {
    pub name: String,
    pub model: PathBuf,
    pub high: Option<PathBuf>,
    pub root: PathBuf,
}
#[derive(Debug)]
pub struct Catalog {
    pub vehicles: Vec<Vehicle>,
    pub warnings: Vec<String>,
}
pub struct Imported {
    pub name: String,
    pub handling: String,
    pub model: Model,
    pub pattern: Option<Pattern>,
    pub metas: [Option<String>; 4],
}
fn text(path: &Path) -> Result<String> {
    String::from_utf8(crate::read_file(path)?).map_err(|e| e.to_string())
}
fn named(path: &Path, name: &str) -> bool {
    path.file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case(name))
}
// Browsers add a numbered suffix when downloading another copy of a metadata file.
// Do not silently prefer one copy: conflicting settings still require a user choice.
fn metadata_named(path: &Path, name: &str) -> bool {
    if named(path, name) {
        return true;
    }
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let Some(stem) = filename.strip_suffix(".meta") else {
        return false;
    };
    let Some(suffix) = stem.strip_prefix(name.trim_end_matches(".meta")) else {
        return false;
    };
    suffix
        .strip_prefix(" (")
        .and_then(|s| s.strip_suffix(')'))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
}
fn files(root: &Path) -> Result<Vec<PathBuf>> {
    fn visit(dir: &Path, depth: usize, count: &mut usize, out: &mut Vec<PathBuf>) -> Result<()> {
        if depth > 12 {
            return Err(
                "Resource folder nesting exceeds 12 levels; choose a smaller folder".into(),
            );
        }
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            *count += 1;
            if *count > 12000 {
                return Err(
                    "Folder has more than 12,000 entries; choose a vehicle resource folder".into(),
                );
            }
            let ty = entry.file_type().map_err(|e| e.to_string())?;
            if ty.is_symlink() {
                continue;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if entry
                    .metadata()
                    .map_err(|e| e.to_string())?
                    .file_attributes()
                    & 0x400
                    != 0
                {
                    continue;
                }
            }
            let p = entry.path();
            if ty.is_dir() {
                if ![".git", "node_modules", "target"]
                    .iter()
                    .any(|n| named(&p, n))
                {
                    visit(&p, depth + 1, count, out)?;
                }
            } else if ty.is_file()
                && p.extension().is_some_and(|e| {
                    ["yft", "ytd", "meta"]
                        .iter()
                        .any(|n| e.eq_ignore_ascii_case(n))
                })
            {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut out = vec![];
    visit(root, 0, &mut 0, &mut out)?;
    out.sort();
    Ok(out)
}
fn resource_root(model: &Path, selected: &Path) -> PathBuf {
    for ancestor in model.ancestors().skip(1) {
        if named(ancestor, "stream") || named(ancestor, "steam") || named(ancestor, "data") {
            return ancestor.parent().unwrap_or(selected).to_owned();
        }
        if ancestor == selected {
            break;
        }
    }
    selected.to_owned()
}
pub fn discover(root: &Path) -> Result<Catalog> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let paths = files(&root)?;
    let mut warnings = vec![];
    let mut declared = BTreeSet::new();
    for path in paths.iter().filter(|p| metadata_named(p, "vehicles.meta")) {
        match text(path).and_then(|s| {
            let doc = crate::xml_document(&s)?;
            Ok(doc
                .descendants()
                .filter(|n| n.has_tag_name("modelName"))
                .filter_map(|n| n.text())
                .map(|n| n.trim().to_ascii_lowercase())
                .collect::<Vec<_>>())
        }) {
            Ok(names) => {
                for name in names {
                    declared.insert((resource_root(path, &root), name));
                }
            }
            Err(e) => warnings.push(format!("{}: {e}", path.display())),
        }
    }
    let mut vehicles = vec![];
    for path in paths
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("yft")))
    {
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if stem.to_ascii_lowercase().ends_with("_hi") {
            continue;
        }
        let high = paths
            .iter()
            .find(|p| p.parent() == path.parent() && named(p, &format!("{stem}_hi.yft")))
            .cloned();
        vehicles.push(Vehicle {
            name: stem,
            model: path.clone(),
            high,
            root: resource_root(path, &root),
        });
    }
    // Put declared vehicles before accessory YFTs, but never hide otherwise usable assets.
    vehicles.sort_by_key(|v| {
        (
            !declared.contains(&(v.root.clone(), v.name.to_ascii_lowercase())),
            v.high.is_none(),
            v.name.to_ascii_lowercase(),
        )
    });
    if vehicles.is_empty() {
        return Err("No base vehicle YFT found in this folder".into());
    }
    Ok(Catalog { vehicles, warnings })
}
fn value(node: roxmltree::Node<'_, '_>, tag: &str) -> String {
    node.children()
        .find(|n| n.has_tag_name(tag))
        .and_then(|n| n.attribute("value").or_else(|| n.text()))
        .unwrap_or("")
        .trim()
        .to_owned()
}
fn safe_txd(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-+".contains(c))
}
/// A directly selected YFT uses the same metadata matching as a folder import.
/// Search only its containing resource, never an unrelated ancestor such as Desktop.
pub fn import_file(path: &Path) -> Result<Imported> {
    let model = path.canonicalize().map_err(|e| e.to_string())?;
    if !model
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("yft"))
    {
        return Err("Choose a YFT vehicle file".into());
    }
    let parent = model.parent().ok_or("Vehicle file has no parent folder")?;
    let stem = model.file_stem().unwrap_or_default().to_string_lossy();
    let name = if stem.to_ascii_lowercase().ends_with("_hi") {
        &stem[..stem.len() - 3]
    } else {
        &stem
    };
    import(
        &Vehicle {
            name: name.to_owned(),
            root: resource_root(&model, parent),
            model,
            high: None,
        },
        false,
    )
}
pub fn import(vehicle: &Vehicle, high: bool) -> Result<Imported> {
    let paths = files(&vehicle.root)?;
    let chosen = if high {
        vehicle.high.as_ref().unwrap_or(&vehicle.model)
    } else {
        &vehicle.model
    };
    // Load geometry first, then prefer explicitly linked texture dictionaries.
    let bytes = crate::read_file(chosen)?;
    let mut model = models::parse_rsc7(&bytes, "yft")?;
    model.native_source = Some(chosen.to_owned());
    model.name = chosen
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into();
    let mut metas: [Option<String>; 4] = Default::default();
    let mut warnings = vec![];
    let mut siren_ids = BTreeSet::new();
    let mut txds = vec![vehicle.name.to_ascii_lowercase()];
    let mut handling = String::new();
    let mut carcols = vec![];
    for path in paths.iter().filter(|p| {
        [
            "vehicles.meta",
            "carvariations.meta",
            "carcols.meta",
            "handling.meta",
        ]
        .iter()
        .any(|n| metadata_named(p, n))
    }) {
        let source = match text(path) {
            Ok(s) => s,
            Err(e) => {
                warnings.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let doc = match crate::xml_document(&source) {
            Ok(d) => d,
            Err(e) => {
                warnings.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        if metadata_named(path, "carcols.meta") {
            carcols.push((path.clone(), source));
            continue;
        }
        if metadata_named(path, "handling.meta") {
            continue;
        }
        for node in doc.descendants().filter(|n| {
            n.children().any(|c| {
                c.has_tag_name("modelName")
                    && c.text()
                        .is_some_and(|v| v.trim().eq_ignore_ascii_case(&vehicle.name))
            })
        }) {
            if metadata_named(path, "vehicles.meta") {
                let txd = value(node, "txdName");
                if safe_txd(&txd) {
                    txds.insert(0, txd.to_ascii_lowercase());
                }
                handling = value(node, "handlingId");
                if crate::meta::entries(&source, 1).is_ok() {
                    metas[1] = Some(source.clone());
                }
                // Resolve shared texture parents, with a bounded cycle-safe walk.
                for _ in 0..32 {
                    let mut added = false;
                    for rel in doc.descendants().filter(|n| n.has_tag_name("Item")) {
                        let child = value(rel, "child").to_ascii_lowercase();
                        let parent = value(rel, "parent").to_ascii_lowercase();
                        if txds.contains(&child) && safe_txd(&parent) && !txds.contains(&parent) {
                            txds.push(parent);
                            added = true;
                        }
                    }
                    if !added {
                        break;
                    }
                }
            } else {
                let id = value(node, "sirenSettings");
                match id.parse::<u32>() {
                    Ok(id) => {
                        siren_ids.insert(id);
                    }
                    Err(_) => warnings.push(format!(
                        "{}: sirenSettings is not a 32-bit unsigned ID",
                        path.display()
                    )),
                };
                if crate::meta::entries(&source, 2).is_ok() {
                    metas[2] = Some(source.clone());
                }
            }
        }
    }
    if !handling.is_empty() {
        for p in paths.iter().filter(|p| metadata_named(p, "handling.meta")) {
            if let Ok(s) = text(p)
                && let Ok(doc) = crate::xml_document(&s)
                && doc.descendants().any(|n| {
                    n.has_tag_name("handlingName")
                        && n.text()
                            .is_some_and(|v| v.trim().eq_ignore_ascii_case(&handling))
                })
                && crate::meta::entries(&s, 0).is_ok()
            {
                metas[0] = Some(s);
                break;
            }
        }
    }
    let mut matches = vec![];
    if siren_ids.len() == 1 {
        let id = *siren_ids.first().unwrap();
        for (path, source) in &carcols {
            let doc = crate::xml_document(source)?;
            if let Some(container) = doc
                .root_element()
                .children()
                .find(|n| n.has_tag_name("Sirens"))
            {
                for (i, item) in container
                    .children()
                    .filter(|n| n.has_tag_name("Item"))
                    .enumerate()
                {
                    if value(item, "id").parse::<u32>().ok() == Some(id) {
                        match Pattern::from_xml(source, i) {
                            Ok(p) => matches.push((p, source.clone(), path)),
                            Err(e) => {
                                warnings.push(format!("{}: {e}", metadata_source(path, source)))
                            }
                        }
                    }
                }
            }
        }
    }
    let pattern = if matches.len() == 1 {
        let (p, s, path) = matches.remove(0);
        warnings.push(format!(
            "Imported siren settings ID {} from {}",
            p.id,
            metadata_source(path, &s)
        ));
        metas[3] = Some(s);
        Some(p)
    } else {
        warnings.push(if matches.len()>1||siren_ids.len()>1 {"Conflicting siren settings: no pattern selected automatically".into()}else{"No valid matching siren pattern. A blank sequence is loaded; import the completed carcols.meta to preview its timing and scale.".into()});
        None
    };
    let mut texture_paths = vec![];
    for txd in txds {
        for name in [format!("{txd}+hi.ytd"), format!("{txd}.ytd")] {
            let found: Vec<_> = paths.iter().filter(|p| named(p, &name)).collect();
            if found.len() == 1 {
                texture_paths.push(found[0].clone());
            } else if found.len() > 1 {
                warnings.push(format!(
                    "More than one {name}; texture dictionary is ambiguous"
                ));
            }
        }
    }
    // Nearby shared dictionaries supplement missing names only; explicit links win.
    texture_paths.extend(
        paths
            .iter()
            .filter(|p| {
                p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ytd"))
                    && p.parent() == vehicle.model.parent()
            })
            .take(32)
            .cloned(),
    );
    models::load_preview_textures(&texture_paths, &mut model);
    model.warnings.extend(warnings);
    model.validate()?;
    Ok(Imported {
        name: vehicle.name.clone(),
        handling,
        model,
        pattern,
        metas,
    })
}
