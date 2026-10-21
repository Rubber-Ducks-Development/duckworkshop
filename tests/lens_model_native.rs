use base64::{Engine, engine::general_purpose::STANDARD};
use duck_workshop::{models, vehicle_resource};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
fn inputs(dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let p = entry.path();
        if entry.file_type().unwrap().is_dir() {
            inputs(&p, out);
        } else if p.extension().is_some_and(|e| {
            ["yft", "ydr", "ytd"]
                .iter()
                .any(|s| e.eq_ignore_ascii_case(s))
        }) {
            out.insert(
                p,
                Sha256::digest(std::fs::read(entry.path()).unwrap()).to_vec(),
            );
        }
    }
}
fn decode(data: &str) -> image::RgbaImage {
    duck_workshop::textures::decode(&STANDARD.decode(data).unwrap()).unwrap()
}
fn original_texture(name: &str, copies: &BTreeMap<String, String>) -> String {
    copies
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}
fn triangles(model: &models::Model, copies: &BTreeMap<String, String>) -> BTreeMap<Vec<u8>, usize> {
    let mut out = BTreeMap::new();
    for mesh in &model.meshes {
        let material = model.material_info.get(&mesh.material);
        for tri in mesh.indices.as_chunks::<3>().0 {
            let mut key = Vec::new();
            key.extend(mesh.lod.as_bytes());
            key.push(0);
            if let Some(m) = material {
                key.extend(m.shader.as_bytes());
                key.push(0);
                key.extend(
                    original_texture(&m.diffuse_texture.to_ascii_lowercase(), copies).as_bytes(),
                );
            }
            key.push(0);
            for i in tri {
                let i = *i as usize;
                for v in mesh.positions[i] {
                    key.extend(v.to_bits().to_le_bytes());
                }
                for uv in &mesh.uvs {
                    if let Some(uv) = uv.get(i) {
                        for v in uv {
                            key.extend(v.to_bits().to_le_bytes());
                        }
                    }
                }
                key.push(mesh.siren_ids.get(i).copied().unwrap_or(0));
                key.push(mesh.vertex_alpha.get(i).copied().unwrap_or(255));
            }
            *out.entry(key).or_insert(0) += 1;
        }
    }
    out
}
#[test]
#[ignore = "Uses user-owned assets when explicitly requested"]
fn native_independent_lenses_preserve_other_lenses_and_model_data() {
    let path = PathBuf::from(
        std::env::var_os("DUCK_WORKSHOP_TEST_VEHICLE").expect("Set native vehicle YFT"),
    );
    let vehicle = vehicle_resource::import_file(&path).unwrap();
    let mut model = vehicle.model;
    assert!(model.native_source.is_some());
    model.native_source = None; // Older saved projects recover source from known YTDs.
    let mut pattern = if let Some(path) = std::env::var_os("DUCK_WORKSHOP_TEST_CARCOLS") {
        let xml = std::fs::read_to_string(path).unwrap();
        duck_workshop::patterns::Pattern::from_xml(&xml, 0).unwrap()
    } else {
        vehicle
            .pattern
            .expect("Matching metadata is ambiguous; set DUCK_WORKSHOP_TEST_CARCOLS explicitly")
    };
    let texture =
        std::env::var("DUCK_WORKSHOP_TEST_TEXTURE").unwrap_or("whelen_m6_emissives".into());
    let mut ids: Vec<_> = model
        .meshes
        .iter()
        .filter(|mesh| {
            model
                .material_info
                .get(&mesh.material)
                .is_some_and(|m| m.diffuse_texture.eq_ignore_ascii_case(&texture))
        })
        .flat_map(|m| m.siren_ids.iter().copied())
        .filter(|id| *id > 0)
        .collect();
    ids.sort();
    ids.dedup();
    assert!(ids.len() > 1);
    let chosen = [ids[0], ids[1]];
    let mut colors = BTreeMap::from([(chosen[0], [0, 255, 0]), (chosen[1], [0, 0, 255])]);
    // Reproduce cambo1 exports with real lens edits plus unused metadata slots.
    if std::env::var("DUCK_WORKSHOP_TEST_UNBOUND_SIRENS").as_deref() == Ok("1") {
        for id in 21..=26 {
            assert!(!duck_workshop::lens_colors::has_siren_binding(&model, id));
            colors.insert(id, [255, 255, 0]);
        }
    }
    let mut id = 1;
    for channel in &mut pattern.channels {
        for layer in 0..channel.color_count() {
            channel.set_recolor(layer, false);
            if let Some(color) = colors.get(&id) {
                channel.set_color(layer, *color);
            }
            id += 1;
        }
    }
    pattern.recolor_shared_surfaces = false;
    let root = path.parent().unwrap();
    let mut originals = BTreeMap::new();
    inputs(root, &mut originals);
    println!(
        "Exporting {} with independent sirens {chosen:?}; checking {} original inputs",
        model.name,
        originals.len()
    );
    let bytes = models::lens_model::export(&model, &pattern).unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    if colors.contains_key(&21) {
        let mut note = String::new();
        zip.by_name("README.txt")
            .unwrap()
            .read_to_string(&mut note)
            .unwrap();
        assert!(note.contains("[21, 22, 23, 24, 25, 26]"));
        assert!(note.contains("carcols.meta only"));
    }
    let temp = tempfile::tempdir().unwrap();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        if file.name().ends_with(".ytd") || file.name().ends_with(".yft") {
            let mut header = [0u8; 16];
            file.read_exact(&mut header).unwrap();
            for offset in [8, 12] {
                let size = models::resource_size(u32::from_le_bytes(
                    header[offset..offset + 4].try_into().unwrap(),
                ));
                assert!(
                    size <= 100_000_000,
                    "FiveM streaming chunk too large: {} / {} bytes",
                    file.name(),
                    size
                );
            }
            println!("FiveM size check {} passed", file.name());
            drop(file);
            file = zip.by_index(i).unwrap();
        }

        let out = temp.path().join(file.enclosed_name().unwrap());
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        let mut data = vec![];
        file.read_to_end(&mut data).unwrap();
        std::fs::write(out, data).unwrap();
    }
    let mut outputs = BTreeMap::new();
    inputs(&temp.path().join("stream"), &mut outputs);
    let source_names: Vec<_> = model
        .material_info
        .values()
        .map(|m| m.diffuse_texture.to_ascii_lowercase())
        .collect();
    let mut copies = BTreeMap::new();
    // Resolve private names through unchanged UV/position geometry, without
    // duplicating the exporter's naming algorithm in this regression.
    let mut model_count = 0;
    let mut changed_triangles = 0;
    let mut unselected_triangles = 0;
    for out in outputs
        .keys()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("yft")))
    {
        let before = models::load(&root.join(out.file_name().unwrap())).unwrap();
        let after = models::load(out).unwrap();
        for mesh in &after.meshes {
            let Some(material) = after.material_info.get(&mesh.material) else {
                continue;
            };
            let name = material.diffuse_texture.to_ascii_lowercase();
            if source_names.contains(&name) {
                unselected_triangles += mesh.indices.len() / 3;
                continue;
            }
            let original = before
                .meshes
                .iter()
                .find(|m| {
                    m.positions == mesh.positions
                        && m.uvs == mesh.uvs
                        && m.vertex_alpha == mesh.vertex_alpha
                        && m.siren_ids == mesh.siren_ids
                        && m.lod == mesh.lod
                })
                .expect("Vertex attributes changed during split");
            copies.insert(
                name,
                before.material_info[&original.material]
                    .diffuse_texture
                    .to_ascii_lowercase(),
            );
            for tri in mesh.indices.as_chunks::<3>().0 {
                assert!(colors.contains_key(&mesh.siren_ids[tri[0] as usize]));
                changed_triangles += 1;
            }
        }
        assert_eq!(
            triangles(&before, &BTreeMap::new()),
            triangles(&after, &copies),
            "Triangle topology, UVs or vertex attributes changed"
        );
        assert_eq!(
            serde_json::to_string(&before.sirens).unwrap(),
            serde_json::to_string(&after.sirens).unwrap()
        );
        model_count += 1;
        println!(
            "Validated native model {}: {} -> {} geometry slots, {} triangles preserved",
            out.file_name().unwrap().to_string_lossy(),
            before.meshes.len(),
            after.meshes.len(),
            before.triangles()
        );
    }
    assert!(model_count > 0 && changed_triangles > 0 && unselected_triangles > 0);
    let mut checked_copies = 0;
    let mut checked_originals = 0;
    for dictionary in outputs
        .keys()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ytd")))
    {
        let original = root.join(dictionary.file_name().unwrap());
        let mut fixture = model.clone();
        fixture.preview_textures.clear();
        fixture.texture_sources.clear();
        for (copy, source) in &copies {
            fixture.material_info.insert(
                copy.clone(),
                models::MaterialInfo {
                    shader: "vehicle_lightsemissive".into(),
                    diffuse_texture: copy.clone(),
                    ..Default::default()
                },
            );
            fixture.material_info.insert(
                source.clone(),
                models::MaterialInfo {
                    shader: "vehicle_lightsemissive".into(),
                    diffuse_texture: source.clone(),
                    ..Default::default()
                },
            );
        }
        let mut before = fixture.clone();
        models::load_preview_textures(std::slice::from_ref(&original), &mut before);
        models::load_preview_textures(std::slice::from_ref(dictionary), &mut fixture);
        for (name, image) in &before.preview_textures {
            if let Some(after) = fixture.preview_textures.get(name) {
                assert_eq!(image, after, "Original texture {name} changed");
                checked_originals += 1;
            }
        }
        for (copy, source) in &copies {
            let Some(image) = fixture.preview_textures.get(copy) else {
                continue;
            };
            if !before.preview_textures.contains_key(source) {
                let base_dictionary = root.join(path.file_stem().unwrap()).with_extension("ytd");
                models::load_preview_textures(&[base_dictionary], &mut before);
            }
            let base = decode(&before.preview_textures[source]);
            let image = decode(image);
            // Base dictionaries may use lower native mips; full-resolution
            // copies are validated through the matching +hi dictionary.
            if image.dimensions() != base.dimensions() {
                assert!(image.width() < base.width() && image.height() < base.height());
                continue;
            }
            assert_eq!(base.dimensions(), image.dimensions());
            assert!(
                base.pixels().zip(image.pixels()).all(|(a, b)| a[3] == b[3]),
                "Private texture alpha changed"
            );
            let id = copy.rsplit_once("_s").unwrap().1.parse::<u8>().unwrap();
            let color = colors[&id];
            assert!(
                image
                    .pixels()
                    .any(|p| p.0[..3].iter().max().copied().unwrap() > 20)
            );
            for pixel in image.pixels() {
                for c in 0..3 {
                    if color[c] == 0 && pixel[3] > 0 {
                        assert!(pixel[c] < 10, "Independent lens hue mismatch");
                    }
                }
            }
            checked_copies += 1;
        }
    }
    assert!(checked_copies >= 2 && checked_originals > 0);
    let xml = std::fs::read_to_string(temp.path().join("data/carcols.meta")).unwrap();
    let exported = duck_workshop::patterns::Pattern::from_xml(&xml, pattern.source_index).unwrap();
    for (id, color) in colors {
        assert_eq!(exported.sirens()[id as usize - 1].color, color);
    }
    for (p, hash) in originals {
        assert_eq!(Sha256::digest(std::fs::read(p).unwrap()).to_vec(), hash);
    }
    println!(
        "Passed: {model_count} models; {changed_triangles} privately colored triangles; {unselected_triangles} original-material triangles; {checked_copies} private textures and {checked_originals} unchanged originals"
    );
}

#[test]
#[ignore = "Uses the user-owned cambo1 assets when explicitly requested"]
fn cambo1_unused_sirens_export_metadata_without_rewriting_assets() {
    let path = PathBuf::from(
        std::env::var_os("DUCK_WORKSHOP_TEST_VEHICLE").expect("Set cambo1 native YFT"),
    );
    let model = models::load(&path).unwrap();
    let xml = std::fs::read_to_string(
        path.parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("carcols.meta"),
    )
    .unwrap();
    let mut pattern = duck_workshop::patterns::Pattern::from_xml(&xml, 0).unwrap();
    let expected: Vec<_> = (21..=26).collect();
    let before = std::fs::read(&path).unwrap();
    for (index, channel) in pattern.channels.iter_mut().enumerate() {
        if expected.contains(&((index + 1) as u8)) {
            assert!(!duck_workshop::lens_colors::has_siren_binding(
                &model,
                (index + 1) as u8
            ));
            channel.set_color(0, [0, 255, 0]);
            channel.bits = [true; 32];
        }
    }
    for export in [
        duck_workshop::lens_colors::export,
        models::lens_model::export,
    ] {
        let bytes = export(&model, &pattern).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(
            archive.len(),
            2,
            "Only metadata and an explanation should be emitted"
        );
        let mut note = String::new();
        archive
            .by_name("README.txt")
            .unwrap()
            .read_to_string(&mut note)
            .unwrap();
        assert!(note.contains("[21, 22, 23, 24, 25, 26]"));
        assert!(note.contains("carcols.meta only"));
        let mut xml = String::new();
        archive
            .by_name("data/carcols.meta")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert_eq!(
            xml,
            pattern.to_xml().unwrap(),
            "Preserve every metadata setting"
        );
        let roundtrip = duck_workshop::patterns::Pattern::from_xml(&xml, 0).unwrap();
        for id in &expected {
            assert_eq!(roundtrip.sirens()[*id as usize - 1].color, [0, 255, 0]);
            assert_eq!(roundtrip.sirens()[*id as usize - 1].bits, [true; 32]);
        }
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
