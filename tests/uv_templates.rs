use duck_workshop::{
    models::{MaterialInfo, Mesh, Model},
    render::{self, UvOptions},
};

fn quad(material: &str) -> Mesh {
    Mesh {
        vertex_alpha: vec![],
        siren_ids: vec![],
        name: material.into(),
        material: material.into(),
        lod: "High".into(),
        positions: vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
        uvs: vec![
            vec![[0.1, 0.1], [0.3, 0.1], [0.3, 0.3], [0.1, 0.3]],
            vec![[0.6, 0.6], [0.9, 0.6], [0.9, 0.9], [0.6, 0.9]],
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        enabled: true,
    }
}

fn vehicle() -> Model {
    let mut model = Model {
        meshes: vec![quad("body"), quad("interior")],
        ..Default::default()
    };
    // A larger unrelated material must not outrank an identified livery.
    model.meshes[1].indices = vec![0, 1, 2, 0, 2, 3, 0, 1, 3];
    model.material_info.insert(
        "body".into(),
        MaterialInfo {
            render_bucket: 0,
            shader: "vehicle_paint3".into(),
            diffuse_texture: "base".into(),
            secondary_texture: "car_sign_1".into(),
        },
    );
    model
}

#[test]
fn livery_selection_uses_its_material_channel_and_lod() {
    let mut model = vehicle();
    let mut low = quad("body");
    low.lod = "Low".into();
    model.meshes.push(low);
    let original = model.meshes[0].uvs.clone();
    let mut opt = UvOptions {
        channel: 7,
        lod: "All".into(),
        ..Default::default()
    };
    render::select_template(&model, &mut opt);
    assert_eq!(opt.material.as_deref(), Some("body"));
    assert_eq!(opt.channel, 1);
    assert_eq!(opt.lod, "High");
    assert!(!opt.boundaries);
    assert!(opt.surface_colors);
    assert_eq!(opt.size, 4096);
    assert_eq!(opt.background, [0, 0, 0, 255]);
    assert_eq!(render::selected(&model, &opt).count(), 1);
    assert_eq!(render::uv_channels(&model, &opt), [0, 1]);
    assert!(
        render::uv_edges(&model, &opt)
            .iter()
            .all(|(a, b)| a[0] >= 0.6 && b[0] >= 0.6)
    );
    assert_eq!(
        model.meshes[0].uvs, original,
        "selection must never repack source UVs"
    );
    render::select_material(&model, &mut opt, Some("interior".into()));
    assert_eq!(opt.channel, 0);
    render::select_material(&model, &mut opt, None);
    assert_eq!(render::selected(&model, &opt).count(), 2);
}

#[test]
fn automatic_selection_uses_existing_mapped_channels_only() {
    let mut model = vehicle();
    model.meshes[0].uvs[1].clear();
    let mut opt = UvOptions::default();
    render::select_template(&model, &mut opt);
    assert_eq!(opt.material.as_deref(), Some("interior"));
    assert_eq!(opt.channel, 0);
    model.meshes[1].enabled = false;
    render::select_template(&model, &mut opt);
    assert_eq!(opt.material.as_deref(), Some("body"));
    assert_eq!(opt.channel, 0);
    model.meshes[0].uvs.clear();
    let mut low = quad("low");
    low.lod = "Low".into();
    low.uvs[0].clear();
    model.meshes.push(low);
    render::select_template(&model, &mut opt);
    assert_eq!(opt.lod, "Low");
    assert_eq!(opt.channel, 1);
}

#[test]
fn overlapping_shells_keep_their_outlines_and_hide_internal_diagonals() {
    let mut model = Model {
        meshes: vec![quad("body"), quad("body")],
        ..Default::default()
    };
    let opt = UvOptions {
        boundaries: true,
        ..Default::default()
    };
    assert_eq!(
        render::uv_edges(&model, &opt).len(),
        4,
        "duplicate meshes must not erase borders"
    );
    let mut other = quad("body");
    for p in &mut other.positions {
        p[2] = 2.;
    }
    let mesh = &mut model.meshes[0];
    mesh.positions.extend(other.positions);
    mesh.uvs[0].extend(&other.uvs[0]);
    mesh.uvs[1].extend(&other.uvs[1]);
    mesh.indices.extend(other.indices.iter().map(|i| i + 4));
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    assert_eq!(
        render::uv_edges(&model, &opt).len(),
        4,
        "mirrored UVs and duplicate triangles keep their boundary"
    );
    assert_eq!(
        render::uv_edges(
            &model,
            &UvOptions {
                boundaries: false,
                ..opt
            }
        )
        .len(),
        5
    );
}

#[test]
fn png_and_svg_export_the_same_isolated_livery_as_the_canvas() {
    let model = vehicle();
    let mut opt = UvOptions {
        size: 256,
        ..Default::default()
    };
    render::select_template(&model, &mut opt);
    render::set_template_style(&mut opt, false);
    opt.size = 256;
    let png = image::load_from_memory(&render::uv_png(&model, &opt).unwrap())
        .unwrap()
        .to_rgba8();
    assert!(
        png.enumerate_pixels()
            .filter(|(_, _, p)| p[3] > 0)
            .all(|(x, y, _)| x >= 152 && y <= 104)
    );
    assert!(png.pixels().any(|p| p[3] > 0));
    let svg = render::uv_svg(&model, &opt).unwrap();
    assert_eq!(svg.matches('M').count(), 4);
    assert!(svg.contains("M153.60001") || svg.contains("L153.60001"));
}

#[test]
fn old_sessions_and_models_still_load_and_new_selection_roundtrips() {
    let mut old = serde_json::to_value(UvOptions::default()).unwrap();
    old.as_object_mut().unwrap().remove("material");
    old.as_object_mut().unwrap().remove("surface_colors");
    let old_options: UvOptions = serde_json::from_value(old.clone()).unwrap();
    assert!(!old_options.surface_colors);
    assert!(
        serde_json::from_value::<UvOptions>(old)
            .unwrap()
            .material
            .is_none()
    );
    let model = vehicle();
    let mut opt = UvOptions::default();
    render::select_template(&model, &mut opt);
    let round: Model = serde_json::from_slice(&serde_json::to_vec(&model).unwrap()).unwrap();
    assert!(round.material_info["body"].is_livery());
    let round: UvOptions = serde_json::from_slice(&serde_json::to_vec(&opt).unwrap()).unwrap();
    assert_eq!(round.material.as_deref(), Some("body"));
    assert_eq!(round.channel, 1);
    assert!(round.surface_colors);
    let mut old = serde_json::to_value(model).unwrap();
    old.as_object_mut().unwrap().remove("material_info");
    assert!(
        serde_json::from_value::<Model>(old)
            .unwrap()
            .material_info
            .is_empty()
    );
}

#[test]
fn colored_templates_fill_the_selected_surface_and_leave_the_background_black() {
    let model = vehicle();
    let mut opt = UvOptions::default();
    render::select_template(&model, &mut opt);
    opt.size = 128;
    opt.boundaries = true;
    let original = model.meshes[0].uvs.clone();
    let image = image::load_from_memory(&render::uv_png(&model, &opt).unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(image.get_pixel(5, 5).0, [0, 0, 0, 255]);
    assert_eq!(image.get_pixel(96, 32).0, [159, 159, 255, 255]);
    assert_eq!(model.meshes[0].uvs, original);
    assert_eq!(render::uv_faces(&model, &opt).len(), 2);
    assert!(image.pixels().all(|p| p[3] == 255));
    // A plane facing +X is pink rather than the roof's purple.
    let mut side = model.clone();
    for p in &mut side.meshes[0].positions {
        *p = [0., p[0], p[1]];
    }
    let image = image::load_from_memory(&render::uv_png(&side, &opt).unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(image.get_pixel(96, 32).0, [255, 159, 159, 255]);
}

#[test]
fn curved_surfaces_have_interpolated_colors_and_flipped_v_uses_the_same_layout() {
    let mut model = vehicle();
    model.meshes[0].positions[2][2] = 1.;
    let mut opt = UvOptions::default();
    render::select_template(&model, &mut opt);
    opt.size = 128;
    let faces = render::uv_faces(&model, &opt);
    assert_ne!(faces[0].colors[0], faces[0].colors[1]);
    let image = image::load_from_memory(&render::uv_png(&model, &opt).unwrap())
        .unwrap()
        .to_rgba8();
    let colors: std::collections::BTreeSet<_> = image
        .pixels()
        .filter(|p| p[0] > 60 && p[1] > 60 && p[2] > 60)
        .map(|p| p.0)
        .collect();
    assert!(
        colors.len() > 30,
        "curved panels need smooth shading, not one flat island color"
    );
    opt.flip_v = false;
    let flipped = image::load_from_memory(&render::uv_png(&model, &opt).unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(flipped.get_pixel(96, 32).0, [0, 0, 0, 255]);
    assert_ne!(flipped.get_pixel(96, 96).0, [0, 0, 0, 255]);
}

#[test]
fn colored_svg_preserves_vector_lines_and_embeds_the_same_surface_colors() {
    use base64::Engine;
    let model = vehicle();
    let mut opt = UvOptions::default();
    render::select_template(&model, &mut opt);
    opt.size = 128;
    let svg = render::uv_svg(&model, &opt).unwrap();
    let doc = duck_workshop::xml_document(&svg).unwrap();
    let href = doc
        .descendants()
        .find(|n| n.has_tag_name("image"))
        .unwrap()
        .attribute("href")
        .unwrap();
    let png = base64::engine::general_purpose::STANDARD
        .decode(href.strip_prefix("data:image/png;base64,").unwrap())
        .unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (128, 128));
    assert_eq!(image.get_pixel(96, 32).0, [159, 159, 255, 255]);
    assert_eq!(
        image.get_pixel(5, 5).0,
        [0, 0, 0, 0],
        "background is emitted once in the SVG"
    );
    assert!(
        doc.descendants()
            .any(|n| n.has_tag_name("path") && n.attribute("stroke").is_some())
    );
}

#[test]
fn colored_raster_clips_outside_uvs_and_accepts_both_triangle_windings() {
    let mut model = vehicle();
    let mut opt = UvOptions::default();
    render::select_template(&model, &mut opt);
    opt.size = 64;
    model.meshes[0].uvs[1] = vec![[-1., -1.], [2., -1.], [2., 2.], [-1., 2.]];
    opt.boundaries = true;
    let image = image::load_from_memory(&render::uv_png(&model, &opt).unwrap())
        .unwrap()
        .to_rgba8();
    assert!(image.pixels().all(|p| p.0 == [159, 159, 255, 255]));
    // Reverse UV winding while retaining the surface's 3D normal.
    model.meshes[0].uvs[1].reverse();
    let image = image::load_from_memory(&render::uv_png(&model, &opt).unwrap())
        .unwrap()
        .to_rgba8();
    assert!(image.pixels().all(|p| p.0 == [159, 159, 255, 255]));
}
