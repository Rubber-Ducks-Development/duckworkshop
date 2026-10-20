//! Visible lens colors: native UV coverage, shared-pixel checks and a FiveM patch ZIP.
use crate::{
    Result,
    models::{Model, texture_dict},
    patterns::Pattern,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::RgbaImage;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Write},
    path::PathBuf,
};

pub fn colors(pattern: &Pattern) -> BTreeMap<u8, [u8; 3]> {
    pattern
        .sirens()
        .iter()
        .enumerate()
        .filter(|(_, s)| s.recolor_lens)
        .map(|(i, s)| ((i + 1) as u8, s.color))
        .collect()
}

/// Match the preview shader: replace hue, retaining brightness and alpha.
pub fn tint(pixel: &mut [u8; 4], color: [u8; 3]) {
    let value = *pixel[..3].iter().max().unwrap() as u16;
    for c in 0..3 {
        pixel[c] = ((value * color[c] as u16 + 127) / 255) as u8;
    }
}

/// Whether a siren exists on the imported model, even if its lens is not editable.
pub fn has_siren_binding(model: &Model, id: u8) -> bool {
    model.sirens.iter().any(|s| s.id == id)
        || model.meshes.iter().any(|mesh| {
            model
                .material_info
                .get(&mesh.material)
                .is_some_and(|m| m.is_light_emissive())
                && mesh
                    .indices
                    .iter()
                    .any(|i| mesh.siren_ids.get(*i as usize) == Some(&id))
        })
}

// carcols often contains unused slots. Keep their settings, but only require
// native lens edits for IDs that actually have bindings in a model variant.
pub(crate) fn native_colors(
    models: &[&Model],
    mut colors: BTreeMap<u8, [u8; 3]>,
) -> (BTreeMap<u8, [u8; 3]>, Vec<u8>) {
    let unbound: Vec<_> = colors
        .keys()
        .copied()
        .filter(|id| !models.iter().any(|model| has_siren_binding(model, *id)))
        .collect();
    for id in &unbound {
        colors.remove(id);
    }
    (colors, unbound)
}

pub(crate) fn unbound_notice(ids: &[u8]) -> String {
    if ids.is_empty() {
        return String::new();
    }
    format!(
        "\nSirens {ids:?} have no siren bones or bound lens geometry in the supplied model. Their colors and sequences are retained in carcols.meta only; no visible lenses were created or recolored for those IDs. To make these sirens visible, add correctly bound geometry in a model editor.\n"
    )
}

fn coverage(model: &Model, texture: &str, w: u32, h: u32) -> Result<Vec<u64>> {
    if w == 0 || h == 0 || w.max(h) > 4096 {
        return Err("Unsupported lens texture size".into());
    }
    let mut owners = vec![0u64; (w * h) as usize];
    // Include every LOD and disabled mesh: export changes the shared native texture.
    for mesh in &model.meshes {
        let Some(material) = model.material_info.get(&mesh.material) else {
            continue;
        };
        if !material.diffuse_texture.eq_ignore_ascii_case(texture)
            && !material.secondary_texture.eq_ignore_ascii_case(texture)
        {
            continue;
        }
        let channel = usize::from(
            material.is_livery() && material.secondary_texture.eq_ignore_ascii_case(texture),
        );
        let Some(uv) = mesh.uvs.get(channel).or_else(|| mesh.uvs.first()) else {
            continue;
        };
        if uv.is_empty() {
            continue;
        }
        for tri in mesh.indices.chunks_exact(3) {
            let ids = tri.map_indices(|i| mesh.siren_ids.get(i).copied().unwrap_or(0));
            let id = if material.is_light_emissive()
                && ids[0] == ids[1]
                && ids[1] == ids[2]
                && (1..=32).contains(&ids[0])
            {
                ids[0]
            } else {
                0
            };
            let p = tri.map_indices(|i| uv[i]);
            raster(p, w, h, |x, y| owners[(y * w + x) as usize] |= 1u64 << id)?;
        }
    }
    Ok(owners)
}

trait Triangle {
    fn map_indices<T>(&self, f: impl Fn(usize) -> T) -> [T; 3];
}
impl Triangle for [u32] {
    fn map_indices<T>(&self, f: impl Fn(usize) -> T) -> [T; 3] {
        [
            f(self[0] as usize),
            f(self[1] as usize),
            f(self[2] as usize),
        ]
    }
}

fn raster(p: [[f32; 2]; 3], w: u32, h: u32, mut put: impl FnMut(u32, u32)) -> Result<()> {
    if p.iter().flatten().any(|v| !v.is_finite() || v.abs() > 64.) {
        return Err("Lens UVs exceed the supported wrapping range".into());
    }
    let min = [
        p.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min),
        p.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min),
    ];
    let max = [
        p.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max),
        p.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max),
    ];
    if max[0] - min[0] > 4. || max[1] - min[1] > 4. {
        return Err("Lens UVs repeat more than four times; edit this texture externally".into());
    }
    let edge = |a: [f32; 2], b: [f32; 2], q: [f32; 2]| {
        (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0])
    };
    if edge(p[0], p[1], p[2]).abs() < 1e-12 {
        return Ok(());
    }
    for ty in min[1].floor() as i32..=max[1].floor() as i32 {
        for tx in min[0].floor() as i32..=max[0].floor() as i32 {
            let x0 = ((min[0] - tx as f32) * w as f32).floor().max(0.) as u32;
            let y0 = ((min[1] - ty as f32) * h as f32).floor().max(0.) as u32;
            let x1 = (((max[0] - tx as f32) * w as f32).ceil().max(0.) as u32).min(w);
            let y1 = (((max[1] - ty as f32) * h as f32).ceil().max(0.) as u32).min(h);
            for y in y0..y1 {
                for x in x0..x1 {
                    let q = [
                        (x as f32 + 0.5) / w as f32 + tx as f32,
                        (y as f32 + 0.5) / h as f32 + ty as f32,
                    ];
                    let e = [
                        edge(p[0], p[1], q),
                        edge(p[1], p[2], q),
                        edge(p[2], p[0], q),
                    ];
                    if e.iter().all(|e| *e >= -1e-8) || e.iter().all(|e| *e <= 1e-8) {
                        put(x, y);
                    }
                }
            }
        }
    }
    Ok(())
}

fn edit(
    model: &Model,
    name: &str,
    mut image: RgbaImage,
    colors: &BTreeMap<u8, [u8; 3]>,
    found: &mut BTreeSet<u8>,
    allow_shared: bool,
) -> Result<(RgbaImage, Vec<bool>)> {
    let owners = coverage(model, name, image.width(), image.height())?;
    let mut mask = vec![false; owners.len()];
    let enabled = colors.keys().fold(0u64, |b, id| b | 1u64 << id);
    for (i, (pixel, owner)) in image.pixels_mut().zip(owners).enumerate() {
        if owner & enabled == 0 {
            continue;
        }
        let Some((_, &color)) = colors.iter().find(|(id, _)| owner & (1u64 << **id) != 0) else {
            continue;
        };
        // Transparent pixels do not affect visible lenses and need no hue changes.
        if pixel[3] == 0 || pixel.0[..3] == [0, 0, 0] {
            continue;
        }
        let mut linked = Vec::new();
        for id in 1..=32u8 {
            if owner & (1u64 << id) != 0 {
                linked.push(id);
            }
        }
        if (owner & 1 != 0 && !allow_shared)
            || linked.iter().any(|id| colors.get(id) != Some(&color))
        {
            return Err(format!(
                "Texture {name} shares pixels between sirens {linked:?}{}. Use Apply color to linked lenses for matching colors. If the unassigned surfaces should change too, enable Also recolor shared surfaces; To keep different lens colors, turn that option off and export FiveM independent lens colors + model instead.",
                if owner & 1 != 0 {
                    " and an unassigned surface"
                } else {
                    ""
                }
            ));
        }
        found.extend(linked);
        tint(&mut pixel.0, color);
        mask[i] = true;
    }
    Ok((image, mask))
}

/// UV-linked IDs for the inspector's explicit "Apply to linked lenses" action.
pub fn linked(model: &Model, id: u8) -> Result<BTreeSet<u8>> {
    model.validate()?;
    if !(1..=32).contains(&id) {
        return Err("Invalid siren ID".into());
    }
    let mut ids = BTreeSet::from([id]);
    let mut ownership = Vec::new();
    for (name, encoded) in &model.preview_textures {
        if !model.meshes.iter().any(|m| {
            m.siren_ids.iter().any(|id| (1..=32).contains(id))
                && model.material_info.get(&m.material).is_some_and(|m| {
                    m.is_light_emissive() && m.diffuse_texture.eq_ignore_ascii_case(name)
                })
        }) {
            continue;
        }
        let image = crate::textures::decode(&STANDARD.decode(encoded).map_err(|e| e.to_string())?)?;
        let mut owners = coverage(model, name, image.width(), image.height())?;
        for (owner, pixel) in owners.iter_mut().zip(image.pixels()) {
            if pixel[3] == 0 || pixel.0[..3] == [0, 0, 0] {
                *owner = 0;
            }
        }
        ownership.push(owners);
    }
    loop {
        let before = ids.len();
        let bits = ids.iter().fold(0u64, |b, id| b | 1u64 << id);
        for owner in ownership.iter().flatten().filter(|o| **o & bits != 0) {
            ids.extend((1..=32u8).filter(|id| owner & (1u64 << id) != 0));
        }
        if before == ids.len() {
            return Ok(ids);
        }
    }
}

/// Bake the same shared UV pixels used by export, including static surfaces.
pub fn shared_preview(
    model: &Model,
    name: &str,
    image: &RgbaImage,
    pattern: &Pattern,
) -> Result<Option<RgbaImage>> {
    if !pattern.recolor_shared_surfaces {
        return Ok(None);
    }
    let colors = colors(pattern);
    if !model.meshes.iter().any(|m| {
        m.siren_ids.iter().any(|id| colors.contains_key(id))
            && model.material_info.get(&m.material).is_some_and(|m| {
                m.is_light_emissive() && m.diffuse_texture.eq_ignore_ascii_case(name)
            })
    }) {
        return Ok(None);
    }
    let (edited, mask) = edit(
        model,
        name,
        image.clone(),
        &colors,
        &mut BTreeSet::new(),
        true,
    )?;
    Ok(mask.iter().any(|b| *b).then_some(edited))
}

/// Build completely in memory before the caller writes a new archive. Originals are read only.
pub fn export(model: &Model, pattern: &Pattern) -> Result<Vec<u8>> {
    model.validate()?;
    pattern.validate()?;
    let colors = colors(pattern);
    if colors.is_empty() {
        return Err("Choose a light color or enable Recolor vehicle lens first".into());
    }
    let (colors, unbound) = native_colors(&[model], colors);
    let mut needed = BTreeSet::new();
    for mesh in &model.meshes {
        if !mesh.siren_ids.iter().any(|id| colors.contains_key(id)) {
            continue;
        }
        let material = model
            .material_info
            .get(&mesh.material)
            .ok_or("Lens material information is missing; reimport the native YFT")?;
        if !material.is_light_emissive() {
            continue;
        }
        let name = material.diffuse_texture.to_ascii_lowercase();
        needed.insert(name);
    }
    let sources = export_sources(model, &needed)?;
    let mut paths: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for name in needed {
        let path=sources.get(&name).ok_or_else(||format!("Cannot locate the native YTD containing {name}. Reimport the vehicle folder to reconnect its source files; embedded or external image textures require a separate editor."))?;
        paths.entry(path.clone()).or_default().insert(name);
    }
    // +hi dictionaries override the normal dictionary in game. Patch both variants.
    for (path, names) in paths.clone() {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let other = if let Some(stem) = stem.strip_suffix("+hi") {
            path.with_file_name(format!("{stem}.ytd"))
        } else {
            path.with_file_name(format!("{stem}+hi.ytd"))
        };
        if other.is_file() {
            paths.entry(other).or_default().extend(names);
        }
    }
    let mut files = BTreeMap::new();
    let mut found = BTreeSet::new();
    for (path, names) in paths {
        let bytes = crate::read_file(&path)?;
        let textures = texture_dict::read_for_export(&bytes, &names)?;
        let mut edits = BTreeMap::new();
        for (name, encoded) in textures {
            let image =
                crate::textures::decode(&STANDARD.decode(encoded).map_err(|e| e.to_string())?)?;
            let edited = edit(
                model,
                &name,
                image,
                &colors,
                &mut found,
                pattern.recolor_shared_surfaces,
            )?;
            if edited.1.iter().any(|b| *b) {
                edits.insert(name, edited);
            }
        }
        if edits.is_empty() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid dictionary filename")?
            .to_owned();
        if files.contains_key(&name.to_ascii_lowercase()) {
            return Err(
                "Two source dictionaries have the same filename; export them separately".into(),
            );
        }
        files.insert(
            name.to_ascii_lowercase(),
            texture_dict::recolor(&bytes, &edits)?,
        );
    }
    let missing: Vec<_> = colors.keys().filter(|id| !found.contains(id)).collect();
    if !missing.is_empty() {
        return Err(format!(
            "No editable native lens pixels found for sirens {missing:?}. Import the vehicle's native YFT and YTD files and check its siren assignments."
        ));
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in files {
        crate::models::native_edit::validate_streaming_flags(
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
    zip.write_all(b"Duck Workshop Early Access - vehicle lens color patch\n\nBack up your vehicle resource. Replace the corresponding YTD dictionaries with stream/ contents. Replace the matching carcols.meta settings with data/carcols.meta (merge into your existing metadata if needed). Keep your existing YFT, vehicles.meta, carvariations.meta and fxmanifest. Ensure sirenSettings references the exported ID. Restart the resource and test in FiveM.\n\nThis archive is a patch for your existing vehicle, not a standalone resource. Lens hue is baked into textures; flashing remains controlled by carcols. Alpha, resource layout and texture compression formats are retained. BC texture blocks may introduce small color quantization differences. Shared texture areas must use matching colors. Original files were not changed.\n").map_err(|e|e.to_string())?;
    zip.write_all(unbound_notice(&unbound).as_bytes())
        .map_err(|e| e.to_string())?;
    if pattern.recolor_shared_surfaces {
        zip.write_all(b"\nShared-surface recoloring was enabled. Static or unassigned geometry using the affected texture pixels was recolored too.\n").map_err(|e|e.to_string())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}

pub(crate) fn export_sources(
    model: &Model,
    needed: &BTreeSet<String>,
) -> Result<BTreeMap<String, PathBuf>> {
    let mut sources: BTreeMap<_, _> = model
        .texture_sources
        .iter()
        .filter(|(name, path)| needed.contains(*name) && path.is_file())
        .map(|(n, p)| (n.clone(), p.clone()))
        .collect();
    let mut missing: BTreeSet<_> = needed
        .iter()
        .filter(|n| !sources.contains_key(*n))
        .cloned()
        .collect();
    if missing.is_empty() {
        return Ok(sources);
    }
    // Older development projects only recorded sources for images that fitted
    // in the preview cache. Recover missing names from the dictionaries already
    // associated with this vehicle, without replacing the edited pattern.
    let mut candidates: Vec<_> = model
        .texture_sources
        .values()
        .filter(|p| p.is_file())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    candidates.sort_by_key(|p| {
        !p.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with("+hi")
    });
    for path in candidates {
        let names = texture_dict::source_names(&crate::read_file(&path)?, &missing)?;
        for name in names {
            missing.remove(&name);
            sources.insert(name, path.clone());
        }
        if missing.is_empty() {
            break;
        }
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MaterialInfo, Mesh};
    fn model(ids: &[u8]) -> Model {
        Model {
            meshes: ids
                .iter()
                .map(|id| Mesh {
                    name: format!("lens{id}"),
                    material: "lens".into(),
                    lod: "High".into(),
                    enabled: true,
                    positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
                    indices: vec![0, 1, 2],
                    uvs: vec![vec![[0., 0.], [1., 0.], [0., 1.]]],
                    siren_ids: vec![*id; 3],
                    vertex_alpha: vec![],
                })
                .collect(),
            material_info: BTreeMap::from([(
                "lens".into(),
                MaterialInfo {
                    shader: "vehicle_lightsemissive".into(),
                    diffuse_texture: "lens".into(),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        }
    }
    #[test]
    fn unused_metadata_slots_do_not_require_lenses_but_real_bindings_stay_strict() {
        let mut base = model(&[1]);
        // A real bone without usable lens geometry must still fail export checks.
        base.sirens.push(crate::sirens::SirenAnchor::new(2, [0.; 3]));
        // Unreferenced vertices are not evidence of a visible lens binding.
        base.meshes[0].siren_ids.push(21);
        let high = model(&[3]);
        let selected = BTreeMap::from([
            (1, [0, 255, 0]),
            (2, [0, 0, 255]),
            (3, [255, 0, 0]),
            (21, [255, 255, 0]),
            (26, [0, 255, 255]),
        ]);
        let (native, unbound) = native_colors(&[&base, &high], selected.clone());
        assert_eq!(native.keys().copied().collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(unbound, [21, 26]);
        assert!(unbound_notice(&unbound).contains("carcols.meta only"));
        assert!(unbound_notice(&[]).is_empty());
        let (native, unbound) = native_colors(&[&model(&[])], selected);
        assert!(native.is_empty());
        assert_eq!(unbound, [1, 2, 3, 21, 26]);
    }
    #[test]
    fn recoloring_preserves_value_alpha_and_unselected_pixels() {
        let mut pixel = [192, 24, 10, 83];
        tint(&mut pixel, [0, 128, 255]);
        assert_eq!(pixel, [0, 96, 192, 83]);
        let original = RgbaImage::from_pixel(4, 4, image::Rgba([192, 24, 10, 83]));
        let (edited, mask) = edit(
            &model(&[1]),
            "lens",
            original.clone(),
            &BTreeMap::from([(1, [0, 0, 255])]),
            &mut BTreeSet::new(),
            false,
        )
        .unwrap();
        assert_eq!(edited.get_pixel(0, 0).0, [0, 0, 192, 83]);
        assert_eq!(edited.get_pixel(3, 3), original.get_pixel(3, 3));
        assert!(!mask[15]);
    }
    #[test]
    fn shared_uvs_require_identical_explicit_colors_and_protect_unassigned_geometry() {
        let image = RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        let mut colors = BTreeMap::from([(1, [0, 0, 255])]);
        assert!(
            edit(
                &model(&[1, 2]),
                "lens",
                image.clone(),
                &colors,
                &mut BTreeSet::new(),
                false,
            )
            .unwrap_err()
            .contains("sirens [1, 2]")
        );
        colors.insert(2, [0, 0, 255]);
        assert!(
            edit(
                &model(&[1, 2]),
                "lens",
                image.clone(),
                &colors,
                &mut BTreeSet::new(),
                false,
            )
            .is_ok()
        );
        colors.insert(2, [0, 255, 0]);
        assert!(
            edit(
                &model(&[1, 2]),
                "lens",
                image.clone(),
                &colors,
                &mut BTreeSet::new(),
                false,
            )
            .is_err()
        );
        assert!(
            edit(
                &model(&[1, 0]),
                "lens",
                image,
                &colors,
                &mut BTreeSet::new(),
                false,
            )
            .unwrap_err()
            .contains("unassigned surface")
        );
    }
    #[test]
    fn wrapped_uvs_keep_native_orientation_and_invalid_coordinates_are_rejected() {
        let mut pixels = BTreeSet::new();
        raster([[1., 0.], [2., 0.], [1., 1.]], 4, 4, |x, y| {
            pixels.insert((x, y));
        })
        .unwrap();
        assert!(pixels.contains(&(0, 0)));
        assert!(!pixels.contains(&(3, 3)));
        assert!(raster([[f32::NAN, 0.], [1., 0.], [0., 1.]], 4, 4, |_, _| {}).is_err());
    }
    #[test]
    fn shared_surface_opt_in_matches_preview_and_keeps_siren_color_conflicts_protected() {
        let m = model(&[1, 0]);
        let image = RgbaImage::from_pixel(4, 4, image::Rgba([192, 24, 10, 83]));
        let mut p = Pattern::default();
        p.channels[0].set_color(0, [0, 128, 255]);
        assert!(shared_preview(&m, "lens", &image, &p).unwrap().is_none());
        p.recolor_shared_surfaces = true;
        let preview = shared_preview(&m, "lens", &image, &p).unwrap().unwrap();
        let exported = edit(
            &m,
            "lens",
            image.clone(),
            &colors(&p),
            &mut BTreeSet::new(),
            true,
        )
        .unwrap()
        .0;
        assert_eq!(preview, exported);
        assert_eq!(preview.get_pixel(0, 0).0, [0, 96, 192, 83]);
        assert!(shared_preview(&model(&[1, 2, 0]), "lens", &image, &p).is_err());
        let restored: Pattern = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        assert!(restored.recolor_shared_surfaces);
    }
    #[test]
    fn old_patterns_keep_texture_colors_and_new_edits_follow_undo_and_layer_order() {
        let mut p = Pattern::default();
        let old = serde_json::to_value(&p).unwrap();
        assert!(colors(&p).is_empty());
        let before = p.clone();
        p.channels[0].set_color(0, [1, 2, 3]);
        p.channels[0].add_color([4, 5, 6]);
        assert_eq!(colors(&p)[&1], [1, 2, 3]);
        assert_eq!(colors(&p)[&2], [4, 5, 6]);
        let saved = serde_json::to_vec(&p).unwrap();
        let loaded: Pattern = serde_json::from_slice(&saved).unwrap();
        assert_eq!(colors(&p), colors(&loaded));
        p.channels[0].remove_color(0);
        assert_eq!(colors(&p)[&1], [4, 5, 6]);
        p = before;
        assert!(colors(&p).is_empty());
        let mut legacy = old;
        for c in legacy["channels"].as_array_mut().unwrap() {
            c.as_object_mut().unwrap().remove("recolor_lens");
        }
        assert!(colors(&serde_json::from_value(legacy).unwrap()).is_empty());
    }
}
