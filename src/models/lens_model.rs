//! Independent emergency-lens colors using native material and texture copies.
//! Original UVs, vertex buffers, specular/dirt samplers and physics are preserved.
//! Legacy PC layouts researched from CodeWalker Drawable.cs, Texture.cs and ResourceFile.cs.
use super::native_edit::{Builder, unpack};
use super::*;
use crate::{lens_colors, patterns::Pattern};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Write},
    path::PathBuf,
};

pub(super) struct NativeMesh {
    pub model: usize,
    pub geometry: usize,
    pub slot: usize,
    pub shader: usize,
    pub group: usize,
}
pub(crate) type Copies = BTreeMap<String, BTreeMap<u8, String>>;
pub(crate) fn copy_name(vehicle: &str, texture: &str, id: u8) -> String {
    format!(
        "dw_{:08x}_{:08x}_s{id:02}",
        shader_hash(&vehicle.to_ascii_lowercase()),
        shader_hash(&texture.to_ascii_lowercase())
    )
}
fn slice(r: &Reader, at: usize, len: usize) -> Result<Vec<u8>> {
    r.data
        .get(at..at.checked_add(len).ok_or("Native buffer overflow")?)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| "Truncated native structure".into())
}
fn put64(data: &mut [u8], at: usize, value: u64) {
    data[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
fn put32(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn clone_shader(r: &Reader, b: &mut Builder, shader: usize, new_name: &str) -> Result<u64> {
    let mut header = slice(r, shader, 48)?;
    let count = r.bytes::<1>(shader + 0x10)?[0] as usize;
    let params = r.req(shader)?;
    let mut numeric_size = 0;
    for i in 0..count {
        numeric_size += r.bytes::<1>(params + i * 16)?[0] as usize * 16;
    }
    let hash_offset = count * 16 + numeric_size;
    let base = 32 + hash_offset + count * 4;
    let size = base + base.next_multiple_of(16) * 4;
    let mut block = slice(r, params, size)?;
    let mut found = false;
    for i in 0..count {
        let ty = r.bytes::<1>(params + i * 16)?[0] as usize;
        if ty != 0 {
            continue;
        }
        let Some(texture) = r.ptr(params + i * 16 + 8)? else {
            continue;
        };
        // RAGE places sampler objects in situ. Each ShaderFX must own its
        // sampler records, including unchanged dirt/specular references. Sharing
        // one record lets a later placement encounter an already fixed name.
        if r.u16(texture + 0x32)? != 2 {
            return Err("Independent lens export requires external sampler references".into());
        }
        let mut texture = slice(r, texture, 80)?;
        if r.u32(params + hash_offset + i * 4)? == shader_hash("diffusesampler") {
            let name = b.string(new_name)?;
            put64(&mut texture, 0x28, name);
            found = true;
        }
        let texture = b.sys(&texture)?;
        put64(&mut block, i * 16 + 8, texture);
    }
    if !found {
        return Err("Lens material has no native diffuse sampler".into());
    }
    let pointer = b.sys(&block)?;
    // Numeric pointers inside the copied parameter block must follow their copy.
    for i in 0..count {
        if r.bytes::<1>(params + i * 16)?[0] == 0 {
            continue;
        }
        let original = r.u64(params + i * 16 + 8)?;
        let original_start = 0x50000000 + params as u64;
        if original >= original_start && original < original_start + size as u64 {
            b.put(
                (pointer - 0x50000000) as usize + i * 16 + 8,
                &(pointer + original - original_start).to_le_bytes(),
            )?;
        }
    }
    put64(&mut header, 0, pointer);
    b.sys(&header)
}
type GeometryEntry = (usize, u64, u16, Vec<u8>);
struct Group {
    original: Vec<u64>,
    added: BTreeMap<(usize, u8), u16>,
}

fn patch_model(bytes: &[u8], copies: &Copies) -> Result<Option<Vec<u8>>> {
    let mut trace = vec![];
    let model = parse_rsc7_traced(bytes, "yft", Some(&mut trace))?;
    let r = unpack(bytes)?;
    let mut b = Builder::new(bytes, &r);
    let mut groups: BTreeMap<usize, Group> = BTreeMap::new();
    let mut models: BTreeMap<usize, Vec<GeometryEntry>> = BTreeMap::new();
    let mut seen = BTreeMap::new();
    let mut changed = false;
    let mut shader_checks = vec![];
    for (mesh, n) in model.meshes.iter().zip(trace) {
        // The same model can occur in multiple fragment references/LODs.
        let signature = (n.group, mesh.siren_ids.clone(), mesh.indices.clone());
        if let Some(old) = seen.insert((n.model, n.slot), signature.clone()) {
            if old != signature {
                return Err(
                    "A shared model has conflicting siren bindings; use a model editor".into(),
                );
            }
            continue;
        }
        let count = r.u16(n.model + 0x10)? as usize;
        let bounds = r.req(n.model + 0x18)?;
        let bound = slice(
            &r,
            bounds + (if count > 1 { n.slot + 1 } else { 0 }) * 32,
            32,
        )?;
        let material = model.material_info.get(&mesh.material);
        let selected = material
            .filter(|m| m.is_light_emissive())
            .and_then(|m| copies.get(&m.diffuse_texture.to_ascii_lowercase()));
        let mut parts: BTreeMap<u8, Vec<u16>> = BTreeMap::new();
        for tri in mesh.indices.chunks_exact(3) {
            let ids = tri
                .iter()
                .map(|i| {
                    mesh.siren_ids
                        .get(*i as usize)
                        .copied()
                        .filter(|id| selected.is_some_and(|s| s.contains_key(id)))
                        .unwrap_or(0)
                })
                .collect::<Vec<_>>();
            if ids.iter().any(|id| *id != ids[0]) {
                return Err("A triangle crosses selected siren bones; edit its bone assignments in a model editor".into());
            }
            parts
                .entry(ids[0])
                .or_default()
                .extend(tri.iter().map(|i| *i as u16));
        }
        let affected = parts.keys().any(|id| *id > 0);
        if !affected {
            models.entry(n.model).or_default().push((
                n.slot,
                r.u64(r.req(n.model + 8)? + n.slot * 8)?,
                n.shader as u16,
                bound,
            ));
            continue;
        }
        changed = true;
        if let std::collections::btree_map::Entry::Vacant(entry) = groups.entry(n.group) {
            let count = r.u16(n.group + 0x18)? as usize;
            let array = r.req(n.group + 0x10)?;
            let original = (0..count)
                .map(|i| r.u64(array + i * 8))
                .collect::<Result<Vec<_>>>()?;
            entry.insert(Group {
                original,
                added: BTreeMap::new(),
            });
        }
        let group = groups.get_mut(&n.group).unwrap();
        let original_shader = r.req(r.req(n.group + 0x10)? + n.shader * 8)?;
        if let Some(dict) = r.ptr(n.group + 8)? {
            let count = r.u16(dict + 0x38)? as usize;
            if count > 4096 {
                return Err("Invalid embedded lens dictionary".into());
            }
            if count > 0 {
                let array = r.req(dict + 0x30)?;
                for i in 0..count {
                    if let Some(t) = r.ptr(array + i * 8)? {
                        let name = r.string(r.req(t + 0x28)?)?.to_ascii_lowercase();
                        if copies.contains_key(&name) {
                            return Err("Embedded lens textures need an external model editor; this development export requires external YTD textures.".into());
                        }
                    }
                }
            }
        }

        // An embedded texture would override the new external YTD reference.
        // The original shader group is left intact; private materials are appended.
        for (id, indices) in &parts {
            let shader = if *id == 0 {
                n.shader as u16
            } else if let Some(index) = group.added.get(&(n.shader, *id)) {
                *index
            } else {
                let index = u16::try_from(group.original.len())
                    .map_err(|_| "Too many native lens materials")?;
                if index >= 4096 {
                    return Err("Model exceeds 4096 native materials".into());
                }
                let pointer = clone_shader(&r, &mut b, original_shader, &selected.unwrap()[id])?;
                shader_checks.push((original_shader, pointer, selected.unwrap()[id].clone()));
                group.original.push(pointer);
                group.added.insert((n.shader, *id), index);
                index
            };
            let geometry = if parts.len() == 1 {
                0x50000000 + n.geometry as u64
            } else {
                let original_ib = r.req(n.geometry + 0x38)?;
                let mut ib = slice(&r, original_ib, 96)?;
                let data: Vec<_> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
                let data = b.sys(&data)?;
                put32(&mut ib, 8, indices.len() as u32);
                put64(&mut ib, 0x10, data);
                let ib = b.sys(&ib)?;
                let mut geometry = slice(&r, n.geometry, 152)?;
                // Buffer records also contain pointers that placement rewrites.
                // Retain the immutable vertex bytes/declaration, own the record.
                for slot in 0..4 {
                    let offset = 0x18 + slot * 8;
                    if let Some(vb) = r.ptr(n.geometry + offset)? {
                        let vb = b.sys(&slice(&r, vb, 128)?)?;
                        put64(&mut geometry, offset, vb);
                    }
                }
                put64(&mut geometry, 0x38, ib);
                put32(&mut geometry, 0x58, indices.len() as u32);
                put32(&mut geometry, 0x5c, indices.len() as u32 / 3);
                b.sys(&geometry)?
            };
            models
                .entry(n.model)
                .or_default()
                .push((n.slot, geometry, shader, bound.clone()));
        }
    }
    if !changed {
        return Ok(None);
    }
    for (address, group) in groups {
        let array: Vec<_> = group
            .original
            .iter()
            .flat_map(|p| p.to_le_bytes())
            .collect();
        let array = b.sys(&array)?;
        b.put(address + 0x10, &array.to_le_bytes())?;
        let count = (group.original.len() as u16).to_le_bytes();
        b.put(address + 0x18, &count)?;
        b.put(address + 0x1a, &count)?;
        b.put(address + 0x30, &4u32.to_le_bytes())?;
    }
    for (address, mut entries) in models {
        entries.sort_by_key(|e| e.0);
        let old_count = r.u16(address + 0x10)? as usize;
        // Empty original geometry slots were omitted by the importer. Retain
        // them and their bounds, rather than silently changing unrelated data.
        let occupied: BTreeSet<_> = entries.iter().map(|e| e.0).collect();
        let old_geoms = r.req(address + 8)?;
        let old_map = r.ptr(address + 0x20)?;
        let old_bounds = r.req(address + 0x18)?;
        for slot in 0..old_count {
            if occupied.contains(&slot) {
                continue;
            }
            let bound = slice(
                &r,
                old_bounds + (if old_count > 1 { slot + 1 } else { 0 }) * 32,
                32,
            )?;
            entries.push((
                slot,
                r.u64(old_geoms + slot * 8)?,
                old_map
                    .map(|p| r.u16(p + slot * 2))
                    .transpose()?
                    .unwrap_or(0),
                bound,
            ));
        }
        entries.sort_by_key(|e| e.0);
        let count = u16::try_from(entries.len()).map_err(|_| "Too many split native geometries")?;
        if count > 4096 {
            return Err("Split model exceeds 4096 geometry slots".into());
        }
        let geoms: Vec<_> = entries.iter().flat_map(|e| e.1.to_le_bytes()).collect();
        let map: Vec<_> = entries.iter().flat_map(|e| e.2.to_le_bytes()).collect();
        let mut bounds = vec![];
        if count > 1 {
            bounds.extend(slice(&r, old_bounds, 32)?);
        }
        for entry in entries {
            bounds.extend(entry.3);
        }
        let geoms = b.sys(&geoms)?;
        let map = b.sys(&map)?;
        let bounds = b.sys(&bounds)?;
        b.put(address + 8, &geoms.to_le_bytes())?;
        b.put(address + 0x18, &bounds.to_le_bytes())?;
        b.put(address + 0x20, &map.to_le_bytes())?;
        for offset in [0x10, 0x12, 0x2e] {
            b.put(address + offset, &count.to_le_bytes())?;
        }
    }
    let mut roots = vec![0];
    for drawable in r.placed_drawables("yft")? {
        if r.u64(drawable.address + 8)? != 0 {
            roots.push(drawable.address);
        }
    }
    roots.sort();
    roots.dedup();
    let output = b.finish(&roots)?;
    validate_lens_fixups(&output)?;
    let readback = unpack(&output)?;
    for (original, copy, name) in shader_checks {
        let copy = (copy - 0x50000000) as usize;
        let old_params = r.req(original)?;
        let new_params = readback.req(copy)?;
        let count = r.bytes::<1>(original + 0x10)?[0] as usize;
        let mut hashes = old_params + count * 16;
        for i in 0..count {
            hashes += r.bytes::<1>(old_params + i * 16)?[0] as usize * 16;
        }
        for i in 0..count {
            let ty = r.bytes::<1>(old_params + i * 16)?[0] as usize;
            if ty == 0 {
                let old_texture = r.ptr(old_params + i * 16 + 8)?;
                let new_texture = readback.ptr(new_params + i * 16 + 8)?;
                let get_name = |reader: &Reader, t: Option<usize>| -> Result<String> {
                    t.map(|t| reader.string(reader.req(t + 0x28)?))
                        .transpose()
                        .map(|s| s.unwrap_or_default())
                };
                let expected = if r.u32(hashes + i * 4)? == shader_hash("diffusesampler") {
                    name.clone()
                } else {
                    get_name(&r, old_texture)?
                };
                if get_name(&readback, new_texture)? != expected {
                    return Err("Native material sampler validation failed".into());
                }
            } else {
                let old = r.req(old_params + i * 16 + 8)?;
                let new = readback.req(new_params + i * 16 + 8)?;
                if slice(&r, old, ty * 16)? != slice(&readback, new, ty * 16)? {
                    return Err("Native material parameter validation failed".into());
                }
            }
        }
    }
    let after = parse_rsc7(&output, "yft")?;
    if after.triangles() != model.triangles() || after.sirens.len() != model.sirens.len() {
        return Err("Native model validation failed after separating lens materials".into());
    }
    Ok(Some(output))
}

/// Validate ownership of mutable native placement records. A preview parser can
/// successfully follow aliases which RAGE's in-place resource loader cannot.
pub fn validate_lens_fixups(bytes: &[u8]) -> Result<()> {
    let mut trace = vec![];
    let model = parse_rsc7_traced(bytes, "yft", Some(&mut trace))?;
    let r = unpack(bytes)?;
    let mut groups = BTreeSet::new();
    let mut geometries = BTreeSet::new();
    let mut buffers = BTreeMap::new();
    let mut samplers = BTreeMap::new();
    let mut private_samplers = BTreeSet::new();
    for (mesh, n) in model.meshes.iter().zip(trace) {
        if geometries.insert(n.geometry) {
            for slot in 0..4 {
                if let Some(vb) = r.ptr(n.geometry + 0x18 + slot * 8)?
                    && let Some(owner) = buffers.insert(vb, n.geometry)
                {
                    return Err(format!(
                        "Native vertex-buffer record is shared by geometries at {owner:#x} and {:#x}; re-export from the original model",
                        n.geometry
                    ));
                }
            }
        }
        let private = model
            .material_info
            .get(&mesh.material)
            .is_some_and(|m| m.diffuse_texture.starts_with("dw_"));
        if private {
            let shader = r.req(r.req(n.group + 0x10)? + n.shader * 8)?;
            private_samplers.insert(shader);
        }
        groups.insert(n.group);
    }
    let mut shaders = BTreeSet::new();
    for group in groups {
        let array = r.req(group + 0x10)?;
        for i in 0..r.u16(group + 0x18)? as usize {
            let Some(shader) = r.ptr(array + i * 8)? else {
                continue;
            };
            if !shaders.insert(shader) {
                continue;
            }
            let params = r.req(shader)?;
            for i in 0..r.bytes::<1>(shader + 0x10)?[0] as usize {
                let at = params + i * 16;
                if r.bytes::<1>(at)?[0] != 0 {
                    continue;
                }
                let Some(texture) = r.ptr(at + 8)? else {
                    continue;
                };
                r.req(texture + 0x28)?;
                if let Some(owner) = samplers.insert(texture, shader)
                    && (private_samplers.contains(&owner) || private_samplers.contains(&shader))
                {
                    return Err("Native sampler record is shared by independent lens materials; re-export from the original model".into());
                }
            }
        }
    }
    Ok(())
}

/// Rebuild a development lens model from its untouched source. Texture names and
/// lens assignments must match exactly; all pixel colors live in the existing YTD.
pub fn repair_model_from_original(
    original: &[u8],
    edited: &[u8],
    vehicle: &str,
) -> Result<Vec<u8>> {
    let old = parse_rsc7(original, "yft")?;
    let current = parse_rsc7(edited, "yft")?;
    let mut copies = Copies::new();
    for mesh in &current.meshes {
        let Some(material) = current.material_info.get(&mesh.material) else {
            continue;
        };
        if !material.diffuse_texture.starts_with("dw_") {
            continue;
        }
        for id in mesh
            .indices
            .iter()
            .filter_map(|i| mesh.siren_ids.get(*i as usize))
            .copied()
            .filter(|id| *id > 0)
        {
            let source = old
                .material_info
                .values()
                .find(|m| {
                    copy_name(vehicle, &m.diffuse_texture.to_ascii_lowercase(), id)
                        == material.diffuse_texture
                })
                .ok_or("Private lens name does not match this original vehicle")?;
            copies
                .entry(source.diffuse_texture.to_ascii_lowercase())
                .or_default()
                .insert(id, material.diffuse_texture.clone());
        }
    }
    if copies.is_empty() {
        return Err("No independent lens materials to repair".into());
    }
    let output = patch_model(original, &copies)?.ok_or("No matching original lens geometry")?;
    let after = parse_rsc7(&output, "yft")?;
    if after.meshes.len() != current.meshes.len()
        || after.meshes.iter().zip(&current.meshes).any(|(a, b)| {
            a.positions != b.positions
                || a.indices != b.indices
                || a.uvs != b.uvs
                || a.siren_ids != b.siren_ids
                || a.vertex_alpha != b.vertex_alpha
                || a.material != b.material
        })
    {
        return Err(
            "Repaired model differs from the current vehicle's geometry or lens assignments".into(),
        );
    }
    Ok(output)
}

fn source_path(model: &Model) -> Result<PathBuf> {
    if let Some(path) = &model.native_source {
        if path.is_file() {
            return Ok(path.clone());
        }
        return Err("The original model moved; reimport the native vehicle folder".into());
    }
    let paths: BTreeSet<_> = model
        .texture_sources
        .values()
        .filter_map(|p| p.parent())
        .map(|p| p.join(&model.name))
        .filter(|p| p.is_file())
        .collect();
    if paths.len() == 1 {
        return Ok(paths.into_iter().next().unwrap());
    }
    Err("Reimport the native vehicle folder to reconnect its model source".into())
}

/// Export private materials/textures for selected sirens. Original native assets
/// are read only; output contains matching base/high models and dictionaries.
pub fn export(model: &Model, pattern: &Pattern) -> Result<Vec<u8>> {
    model.validate()?;
    pattern.validate()?;
    if pattern.recolor_shared_surfaces {
        return Err("Turn off Also recolor shared surfaces for independent lens export; this export preserves those surfaces.".into());
    }
    let colors = lens_colors::colors(pattern);
    if colors.is_empty() {
        return Err("Choose a lens color before exporting".into());
    }
    let source = source_path(model)?;
    let parent = source.parent().ok_or("Native model has no parent folder")?;
    let stem = source
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let stem = stem.strip_suffix("_hi").unwrap_or(&stem);
    let current = parse_rsc7(&crate::read_file(&source)?, "yft")?;
    if current.meshes.len() != model.meshes.len()
        || current.meshes.iter().zip(&model.meshes).any(|(a, b)| {
            a.positions != b.positions
                || a.indices != b.indices
                || a.uvs != b.uvs
                || a.siren_ids != b.siren_ids
                || a.material != b.material
        })
    {
        return Err("The original model differs from this project. Reimport the vehicle before exporting independent lenses.".into());
    }
    let mut variants = vec![];
    for entry in std::fs::read_dir(parent).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        if filename != format!("{stem}.yft") && filename != format!("{stem}_hi.yft") {
            continue;
        }
        let bytes = crate::read_file(&path)?;
        let parsed = parse_rsc7(&bytes, "yft")?;
        variants.push((filename, bytes, parsed));
    }
    let variant_models: Vec<_> = variants.iter().map(|(_, _, model)| model).collect();
    let (colors, unbound) = lens_colors::native_colors(&variant_models, colors);
    let mut copies: Copies = BTreeMap::new();
    let mut found = BTreeSet::new();
    for (_, _, variant) in &variants {
        for mesh in &variant.meshes {
            let Some(material) = variant
                .material_info
                .get(&mesh.material)
                .filter(|m| m.is_light_emissive())
            else {
                continue;
            };
            let texture = material.diffuse_texture.to_ascii_lowercase();
            for index in &mesh.indices {
                let id = mesh.siren_ids.get(*index as usize).copied().unwrap_or(0);
                if colors.contains_key(&id) {
                    if texture.starts_with("dw_") {
                        return Err("These lenses already use Workshop private materials. Export FiveM lens colors + carcols to recolor them, or reimport the original vehicle to separate again.".into());
                    }
                    copies
                        .entry(texture.clone())
                        .or_default()
                        .insert(id, copy_name(stem, &texture, id));
                    found.insert(id);
                }
            }
        }
    }
    let missing: Vec<_> = colors.keys().filter(|id| !found.contains(id)).collect();
    if !missing.is_empty() {
        return Err(format!("No native lens geometry for sirens {missing:?}"));
    }
    let needed = copies.keys().cloned().collect();
    let sources = lens_colors::export_sources(model, &needed)?;
    for name in copies.keys() {
        if !sources.contains_key(name) {
            return Err(format!(
                "Cannot locate native YTD for {name}; reimport the vehicle folder"
            ));
        }
    }
    let mut files = BTreeMap::new();
    let mut model_count = 0;
    for (filename, bytes, _) in variants {
        if let Some(bytes) = patch_model(&bytes, &copies).map_err(|e| format!("{filename}: {e}"))? {
            files.insert(filename, bytes);
            model_count += 1;
        }
    }
    if model_count == 0 && !copies.is_empty() {
        return Err("No native lens materials were separated".into());
    }
    let mut pairs: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    for path in sources.values() {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let stem = stem.strip_suffix("+hi").unwrap_or(&stem);
        let base = path.with_file_name(format!("{stem}.ytd"));
        let high = path.with_file_name(format!("{stem}+hi.ytd"));
        pairs.insert(base, high);
    }
    let mut copied = BTreeSet::new();
    let mut lower_base = false;
    for (base, high) in pairs {
        let base_bytes = crate::read_file(&base)
            .map_err(|e| format!("Native base dictionary {}: {e}", base.display()))?;
        let high_bytes = if high.is_file() {
            Some(crate::read_file(&high)?)
        } else {
            None
        };
        let base_names = texture_dict::source_names(&base_bytes, &needed)?;
        let high_names = high_bytes
            .as_ref()
            .map(|b| texture_dict::source_names(b, &needed))
            .transpose()?
            .unwrap_or_default();
        let names: BTreeSet<_> = base_names.union(&high_names).cloned().collect();
        if names.is_empty() {
            continue;
        }
        let mut base_textures = vec![];
        let mut high_textures = vec![];
        for name in &names {
            let subset = BTreeMap::from([(name.clone(), copies[name].clone())]);
            let base_source = if base_names.contains(name) {
                &base_bytes
            } else {
                high_bytes.as_ref().unwrap()
            };
            let low = texture_dict::private_lenses(base_source, &subset, &colors)?;
            let high_source = if high_names.contains(name) {
                high_bytes.as_ref().unwrap()
            } else {
                base_source
            };
            let full = if std::ptr::eq(base_source, high_source) {
                low.clone()
            } else {
                texture_dict::private_lenses(high_source, &subset, &colors)?
            };
            base_textures.extend(low);
            high_textures.extend(full);
        }
        let mut result = texture_dict::add_private(&base_bytes, &base_textures, true);
        let mut reduced = false;
        while result.as_ref().is_err_and(|e| e.contains("100 MB")) {
            let mut changed = false;
            for texture in &mut base_textures {
                changed |= texture.lower_mip();
            }
            if !changed {
                break;
            }
            reduced = true;
            result = texture_dict::add_private(&base_bytes, &base_textures, true);
        }
        let result = result?;
        let filename = base
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        if files.insert(filename, result).is_some() {
            return Err("Ambiguous native dictionary filenames".into());
        }
        if high_bytes.is_some() || reduced {
            let (target, keep) = match &high_bytes {
                Some(bytes) => (bytes, true),
                None => (&base_bytes, false),
            };
            let output = texture_dict::add_private(target, &high_textures, keep)?;
            let name = high
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase();
            if files.insert(name, output).is_some() {
                return Err("Ambiguous HD dictionary filenames".into());
            }
        }
        lower_base |= reduced;
        copied.extend(names);
    }
    if copied.len() != copies.len() {
        return Err("A native lens dictionary is missing".into());
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in files {
        super::native_edit::validate_streaming_flags(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        )?;
        zip.start_file(format!("stream/{name}"), options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    zip.start_file("data/carcols.meta", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(pattern.to_xml()?.as_bytes())
        .map_err(|e| e.to_string())?;
    zip.start_file("README.txt", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(b"Duck Workshop Early Access - independent lens material patch\n\nInstall all supplied stream YFT and YTD files together. Merge data/carcols.meta into your vehicle resource and keep its sirenSettings ID. Keep your other metadata and fxmanifest. This archive is a patch, not a standalone resource. Test the edited vehicle in FiveM.\n\nSelected sirens use private materials and texture copies. Unselected lenses and static surfaces retain their original materials and pixels. UVs, vertex attributes, bones, physics, dirt/specular samplers and original texture entries are preserved. Affected triangle lists are split by siren ID; no triangles are added or removed. New texture copies retain the original format, mip count and encoded alpha. Each streamed segment is limited to 100 MB. Identical encoded pixel allocations may share storage; each lens keeps its own material and texture name. Original inputs were not modified.\n").map_err(|e|e.to_string())?;
    zip.write_all(lens_colors::unbound_notice(&unbound).as_bytes())
        .map_err(|e| e.to_string())?;
    if lower_base {
        zip.write_all(b"\nThe base dictionary uses existing lower mip levels for private lenses to meet FiveM's 100 MB streaming limit. Full-resolution private lenses are supplied in the matching +hi dictionary. Install BOTH dictionaries. Original textures were not downscaled.\n").map_err(|e|e.to_string())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}
/// Repack a previously exported archive in memory, keeping its model files,
/// metadata and encoded pixel colors. The caller commits the result atomically.
pub fn repair_patch(bytes: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    if source.len() > 256 {
        return Err("Patch has too many files".into());
    }
    for i in 0..source.len() {
        let mut file = source.by_index(i).map_err(|e| e.to_string())?;
        if file.size() > 100 * 1024 * 1024 || file.is_dir() {
            return Err("Unsupported file in lens patch".into());
        }
        let name = file
            .enclosed_name()
            .ok_or("Invalid patch path")?
            .to_string_lossy()
            .replace('\\', "/");
        let mut data = vec![];
        file.read_to_end(&mut data).map_err(|e| e.to_string())?;
        if name.to_ascii_lowercase().ends_with(".ytd") {
            data = texture_dict::compact(&data)?;
        }
        if name.to_ascii_lowercase().ends_with(".ytd")
            || name.to_ascii_lowercase().ends_with(".yft")
        {
            if data.len() < 16 {
                return Err("Truncated native patch file".into());
            }
            super::native_edit::validate_streaming_flags(
                u32::from_le_bytes(data[8..12].try_into().unwrap()),
                u32::from_le_bytes(data[12..16].try_into().unwrap()),
            )?;
        }
        output
            .start_file(name, options)
            .map_err(|e| e.to_string())?;
        output.write_all(&data).map_err(|e| e.to_string())?;
    }
    Ok(output.finish().map_err(|e| e.to_string())?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Explicit local native ownership regression; user-owned assets are not bundled"]
    fn native_placement_rejects_shared_buffer_and_sampler_records() {
        let path =
            std::path::PathBuf::from(std::env::var_os("DUCK_WORKSHOP_TEST_VEHICLE").unwrap());
        let texture = std::env::var("DUCK_WORKSHOP_TEST_TEXTURE")
            .unwrap()
            .to_ascii_lowercase();
        let bytes = crate::read_file(&path).unwrap();
        let model = parse_rsc7(&bytes, "yft").unwrap();
        let ids: BTreeSet<_> = model
            .meshes
            .iter()
            .filter(|m| {
                model
                    .material_info
                    .get(&m.material)
                    .is_some_and(|i| i.diffuse_texture.eq_ignore_ascii_case(&texture))
            })
            .flat_map(|m| {
                m.indices
                    .iter()
                    .filter_map(|i| m.siren_ids.get(*i as usize))
                    .copied()
            })
            .filter(|id| *id > 0)
            .collect();
        assert!(ids.len() >= 2);
        let copies = [(
            texture.clone(),
            ids.into_iter()
                .take(2)
                .map(|id| (id, copy_name("test", &texture, id)))
                .collect(),
        )]
        .into();
        let output = patch_model(&bytes, &copies).unwrap().unwrap();
        validate_lens_fixups(&output).unwrap();
        let encode = |r: Reader| {
            let mut encoder =
                flate2::write::DeflateEncoder::new(vec![], flate2::Compression::fast());
            encoder.write_all(&r.data).unwrap();
            let mut bytes = output[..16].to_vec();
            bytes.extend(encoder.finish().unwrap());
            bytes
        };
        let mut trace = vec![];
        parse_rsc7_traced(&output, "yft", Some(&mut trace)).unwrap();
        let mut r = unpack(&output).unwrap();
        let mut seen = BTreeSet::new();
        let mut vbs = BTreeMap::new();
        let mut tested_buffer = false;
        for n in &trace {
            if !seen.insert(n.geometry) {
                continue;
            }
            let vb = r.req(n.geometry + 0x18).unwrap();
            let data = r.u64(vb + 0x10).unwrap();
            if let Some(previous) = vbs.insert(data, vb) {
                put64(&mut r.data, n.geometry + 0x18, 0x50000000 + previous as u64);
                tested_buffer = true;
                break;
            }
        }
        assert!(tested_buffer, "Fixture needs a split lens geometry");
        assert!(
            validate_lens_fixups(&encode(r))
                .unwrap_err()
                .contains("vertex-buffer record is shared")
        );
        let mut r = unpack(&output).unwrap();
        let mut shaders = BTreeSet::new();
        for n in trace {
            let array = r.req(n.group + 0x10).unwrap();
            for i in 0..r.u16(n.group + 0x18).unwrap() as usize {
                if let Some(s) = r.ptr(array + i * 8).unwrap() {
                    shaders.insert(s);
                }
            }
        }
        let mut samplers = BTreeMap::new();
        let mut tested_sampler = false;
        for s in shaders {
            let params = r.req(s).unwrap();
            let private = (0..r.data[s + 0x10] as usize).any(|i| {
                let at = params + i * 16;
                r.data[at] == 0
                    && r.ptr(at + 8).unwrap().is_some_and(|t| {
                        r.string(r.req(t + 0x28).unwrap())
                            .unwrap()
                            .starts_with("dw_")
                    })
            });
            for i in 0..r.data[s + 0x10] as usize {
                let at = params + i * 16;
                if r.data[at] != 0 {
                    continue;
                }
                let Some(t) = r.ptr(at + 8).unwrap() else {
                    continue;
                };
                let name = r.string(r.req(t + 0x28).unwrap()).unwrap();
                if let Some(previous) = samplers.insert(name, t)
                    && private
                {
                    put64(&mut r.data, at + 8, 0x50000000 + previous as u64);
                    tested_sampler = true;
                    break;
                }
            }
            if tested_sampler {
                break;
            }
        }
        assert!(
            tested_sampler,
            "Fixture needs an unchanged dirt/specular sampler"
        );
        assert!(
            validate_lens_fixups(&encode(r))
                .unwrap_err()
                .contains("sampler record is shared")
        );
    }
    #[test]
    fn private_texture_names_are_stable_and_vehicle_and_siren_specific() {
        assert_eq!(
            copy_name("ELADDERLT", "Whelen_M6_emissives", 7),
            copy_name("eladderlt", "whelen_m6_emissives", 7)
        );
        assert_ne!(copy_name("a", "lens", 7), copy_name("a", "lens", 8));
        assert_ne!(copy_name("a", "lens", 7), copy_name("b", "lens", 7));
    }
}
