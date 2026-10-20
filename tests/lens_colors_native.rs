use base64::Engine;
use duck_workshop::{lens_colors, models, vehicle_resource};
use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
    path::PathBuf,
};

/// Uses user-owned assets only when explicitly requested; none are bundled in the repository.
#[test]
#[ignore]
fn native_vehicle_lens_patch_round_trip() {
    let path = PathBuf::from(
        std::env::var_os("DUCK_WORKSHOP_TEST_VEHICLE")
            .expect("Set DUCK_WORKSHOP_TEST_VEHICLE to a native YFT"),
    );
    let imported = vehicle_resource::import_file(&path).unwrap();
    let model = imported.model;
    let mut pattern = imported.pattern.expect("Matching carcols settings");
    pattern.recolor_shared_surfaces =
        std::env::var("DUCK_WORKSHOP_TEST_SHARED").as_deref() == Ok("1");
    println!(
        "Imported {}: {} sirens, {} source textures",
        model.name,
        pattern.siren_count(),
        model.texture_sources.len()
    );
    let sources: Vec<_> = model
        .texture_sources
        .values()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|p| (p.clone(), std::fs::read(p).unwrap()))
        .collect();
    let mut id = std::env::var("DUCK_WORKSHOP_TEST_SIREN")
        .unwrap_or("1".into())
        .parse::<u8>()
        .unwrap();
    if let Ok(texture) = std::env::var("DUCK_WORKSHOP_TEST_TEXTURE") {
        id = model
            .meshes
            .iter()
            .filter(|m| {
                model
                    .material_info
                    .get(&m.material)
                    .is_some_and(|m| m.diffuse_texture.eq_ignore_ascii_case(&texture))
            })
            .flat_map(|m| &m.siren_ids)
            .copied()
            .find(|id| *id > 0)
            .expect("Requested lens texture has no siren binding");
        assert!(
            model
                .texture_sources
                .contains_key(&texture.to_ascii_lowercase()),
            "Lens provenance was lost to preview limits"
        );
        assert!(
            model
                .preview_textures
                .contains_key(&texture.to_ascii_lowercase()),
            "Emergency lens preview was displaced by body textures"
        );
        println!("Testing {texture}, siren {id}");
    }
    let linked = lens_colors::linked(&model, id).unwrap();
    println!("Linked to siren {id}: {linked:?}");
    for (i, c) in pattern.channels.iter_mut().enumerate() {
        if linked.contains(&((i + 1) as u8)) {
            c.set_color(0, [0, 255, 0]);
        }
    }
    let mut export_model = model.clone();
    if let Ok(texture) = std::env::var("DUCK_WORKSHOP_TEST_RECOVER_SOURCE") {
        export_model.texture_sources.remove(&texture);
    }
    if pattern.recolor_shared_surfaces {
        let mut protected = pattern.clone();
        protected.recolor_shared_surfaces = false;
        assert!(
            lens_colors::export(&export_model, &protected)
                .unwrap_err()
                .contains("unassigned surface")
        );
    }
    let bytes = lens_colors::export(&export_model, &pattern).unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    let mut xml = String::new();
    zip.by_name("data/carcols.meta")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    let exported = duck_workshop::patterns::Pattern::from_xml(&xml, pattern.source_index).unwrap();
    for id in &linked {
        assert_eq!(exported.sirens()[*id as usize - 1].color, [0, 255, 0]);
    }
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        if file.name().ends_with(".ytd") {
            let path = temp
                .path()
                .join(std::path::Path::new(file.name()).file_name().unwrap());
            let mut out = Vec::new();
            file.read_to_end(&mut out).unwrap();
            std::fs::write(&path, out).unwrap();
            paths.push(path);
        }
    }
    assert!(!paths.is_empty());
    // A patch may contain only the base YTD, whereas the imported preview used
    // higher-resolution body textures from +hi. Compare each patched dictionary
    // with its own original instead of comparing different source variants.
    let original_paths: Vec<_> = paths
        .iter()
        .map(|p| {
            sources
                .iter()
                .find(|(source, _)| {
                    source
                        .file_name()
                        .unwrap()
                        .eq_ignore_ascii_case(p.file_name().unwrap())
                })
                .unwrap()
                .0
                .clone()
        })
        .collect();
    let mut before_model = model.clone();
    before_model.preview_textures.clear();
    before_model.texture_sources.clear();
    models::load_preview_textures(&original_paths, &mut before_model);
    let mut after = model.clone();
    after.preview_textures.clear();
    after.texture_sources.clear();
    models::load_preview_textures(&paths, &mut after);
    let mut changed = 0;
    for (name, texture) in &after.preview_textures {
        let Some(before_texture) = before_model.preview_textures.get(name) else {
            continue;
        };
        if before_texture != texture {
            let decode = |data: &str| {
                duck_workshop::textures::decode(
                    &base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .unwrap(),
                )
                .unwrap()
            };
            let before = decode(before_texture);
            let after = decode(texture);
            assert_eq!(before.dimensions(), after.dimensions());
            assert!(
                before
                    .pixels()
                    .zip(after.pixels())
                    .all(|(a, b)| a[3] == b[3]),
                "Lens alpha changed"
            );
            assert!(
                after.pixels().any(|p| p[1] > 200 && p[0] < 20 && p[2] < 20),
                "No recolored green pixels"
            );
            changed += 1;
            println!("Changed {name}");
        }
    }
    assert!(changed > 0);
    for (path, bytes) in sources {
        assert_eq!(
            std::fs::read(path).unwrap(),
            bytes,
            "Original dictionary changed"
        );
    }
    if let Some(output) = std::env::var_os("DUCK_WORKSHOP_TEST_OUTPUT") {
        std::fs::create_dir_all(&output).unwrap();
        let output = PathBuf::from(output);
        let stem = path.file_stem().unwrap().to_string_lossy();
        std::fs::write(output.join(format!("{stem}-lens-colors-green.zip")), bytes).unwrap();
        let session = duck_workshop::project::Session {
            pattern,
            ..Default::default()
        };
        let project = duck_workshop::project::Project {
            format: "duck-workshop".into(),
            version: 1,
            session,
            model: Some(model),
            tool: 1,
            dark_mode: true,
            comparison: Default::default(),
            textures: vec![],
        };
        let path = output.join(format!("{stem}-lens-colors.duckworkshop"));
        std::fs::write(&path, project.to_json().unwrap()).unwrap();
        let reopened = duck_workshop::project::Project::load(&path).unwrap();
        assert_eq!(
            lens_colors::colors(&reopened.session.pattern),
            lens_colors::colors(&project.session.pattern)
        );
    }
}
