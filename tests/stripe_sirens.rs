use duck_workshop::{
    models,
    render::{CHEVRON_PRESETS, Chevron, UvOptions},
    sirens::PreviewSettings,
};

#[test]
fn six_presets_set_mirroring_and_preserve_export_properties() {
    assert_eq!(CHEVRON_PRESETS.len(), 6);
    for (i, preset) in CHEVRON_PRESETS.iter().enumerate() {
        let mut c = Chevron {
            width: 256,
            height: 128,
            opacity: 143,
            honeycomb: true,
            ..Default::default()
        };
        c.apply_preset(i);
        assert_eq!(c.mirrored(), preset.mirrored);
        assert_eq!(c.color_a, preset.a);
        assert_eq!(c.color_b, preset.b);
        assert_eq!((c.width, c.height, c.opacity), (256, 128, 143));
        assert!(c.honeycomb);
        assert!(c.image(false, None, &UvOptions::default()).is_ok());
    }
}

#[test]
fn overlays_are_exported_and_keep_opacity_while_seam_is_preview_only() {
    let mut c = Chevron {
        width: 256,
        height: 128,
        opacity: 111,
        ..Default::default()
    };
    let uv = UvOptions::default();
    let base = c.image(false, None, &uv).unwrap();
    c.show_seam = true;
    assert_eq!(c.image(false, None, &uv).unwrap(), base);
    assert_ne!(c.image(true, None, &uv).unwrap(), base);
    c.show_seam = false;
    c.honeycomb = true;
    let honey = c.image(false, None, &uv).unwrap();
    assert_ne!(honey, base);
    assert!(honey.pixels().all(|p| p[3] == 111));
    c.honeycomb = false;
    c.reflective_983 = true;
    let finish = c.image(false, None, &uv).unwrap();
    assert_ne!(finish, base);
    assert_ne!(finish, honey);
    assert!(finish.pixels().all(|p| p[3] == 111));
    c.honeycomb = true;
    assert_ne!(c.image(false, None, &uv).unwrap(), finish);
    c.cell_size = f32::NAN;
    assert!(c.validate().is_err());
}

#[test]
fn independent_mirroring_and_legacy_diagonal_settings_work() {
    let mut c = Chevron {
        width: 256,
        height: 128,
        ..Default::default()
    };
    let uv = UvOptions::default();
    let mirror = c.image(false, None, &uv).unwrap();
    assert_eq!(mirror.get_pixel(40, 35), mirror.get_pixel(216, 35));
    c.mirror_over_seam = Some(false);
    assert_ne!(c.image(false, None, &uv).unwrap(), mirror);
    let mut old = serde_json::to_value(c).unwrap();
    let fields = old.as_object_mut().unwrap();
    for name in [
        "mirror_over_seam",
        "show_seam",
        "honeycomb",
        "reflective_983",
        "cell_size",
    ] {
        fields.remove(name);
    }
    fields.insert("direction".into(), serde_json::json!(2));
    let old: Chevron = serde_json::from_value(old).unwrap();
    assert!(!old.mirrored());
    old.validate().unwrap();
}

#[test]
fn xml_siren_bones_and_preview_settings_survive_save_load() {
    let xml = "<Drawable><Skeleton><Bones><Item><Name>root</Name><ParentIndex value=\"-1\"/><Translation x=\"1\" y=\"2\" z=\"3\"/></Item><Item><Name>siren5</Name><ParentIndex value=\"0\"/><Translation x=\"0.5\" y=\"0\" z=\"1\"/></Item></Bones></Skeleton><DrawableModelsHigh><Item><Geometries><Item><VertexBuffer><Layout><Position/><TexCoord0/></Layout><Data>0 0 0 0 0\n1 0 0 1 0\n0 1 0 0 1</Data></VertexBuffer><IndexBuffer><Data>0 1 2</Data></IndexBuffer></Item></Geometries></Item></DrawableModelsHigh></Drawable>";
    let model = models::parse_xml(xml).unwrap();
    assert_eq!(model.sirens.len(), 1);
    assert_eq!(model.sirens[0].id, 5);
    assert_eq!(model.sirens[0].position, [1.5, 2., 4.]);
    let round: models::Model =
        serde_json::from_slice(&serde_json::to_vec(&model).unwrap()).unwrap();
    assert_eq!(round.sirens[0].position, model.sirens[0].position);
    let mut settings = PreviewSettings::default();
    settings.positions.insert(32, [1., 2., 3.]);
    let round: PreviewSettings =
        serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
    assert_eq!(round.positions[&32], [1., 2., 3.]);
    round.validate().unwrap();
    settings.positions.insert(0, [0.; 3]);
    assert!(settings.validate().is_err());
    let defaults: PreviewSettings = serde_json::from_str("{}").unwrap();
    assert!(defaults.labels && defaults.ground_glow);
}

#[test]
fn multicolor_stripes_repeat_in_order_at_negative_offsets_and_keep_png_alpha() {
    let mut c = Chevron {
        width: 256,
        height: 128,
        stripe: 20.,
        angle: 45.,
        color_a: [220, 0, 0],
        color_b: [255, 255, 255],
        extra_colors: vec![[0, 0, 220]],
        opacity: 83,
        mirror_over_seam: Some(false),
        offset: -400.,
        ..Default::default()
    };
    let uv = UvOptions::default();
    for direction in 0..3 {
        c.direction = direction;
        let rendered = c.image(false, None, &uv).unwrap();
        for x in 0..256 {
            let rad = c.angle.to_radians();
            let q =
                (x as f32 - 128.) * rad.sin() * if direction == 1 { -1. } else { 1. } + c.offset;
            let expected = [c.color_a, c.color_b, c.extra_colors[0]]
                [((q / c.stripe).floor() as i64).rem_euclid(3) as usize];
            assert_eq!(
                rendered.get_pixel(x, 0).0,
                [expected[0], expected[1], expected[2], 83]
            );
        }
        let exported = image::load_from_memory(&c.png(None, &uv).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(exported, rendered);
        assert_eq!(c.image(true, None, &uv).unwrap(), rendered);
    }
    c.opacity = 0;
    c.honeycomb = true;
    c.reflective_983 = true;
    assert!(
        c.image(false, None, &uv)
            .unwrap()
            .pixels()
            .all(|p| p[3] == 0)
    );
    let mut old = serde_json::to_value(&c).unwrap();
    old.as_object_mut().unwrap().remove("extra_colors");
    assert!(
        serde_json::from_value::<Chevron>(old)
            .unwrap()
            .extra_colors
            .is_empty()
    );
    let round: Chevron = serde_json::from_slice(&serde_json::to_vec(&c).unwrap()).unwrap();
    assert_eq!(round.extra_colors, c.extra_colors);
    c.apply_preset(0);
    assert!(c.extra_colors.is_empty());
    c.extra_colors = vec![[0; 3]; 15];
    assert!(c.validate().is_err());
}

#[test]
fn preview_modes_and_opacity_roundtrip_and_legacy_projects_default_to_full_visibility() {
    use duck_workshop::project::Session;
    let mut session = Session {
        uv_whole_vehicle: false,
        ..Default::default()
    };
    session.siren_preview.vehicle_opacity = 25;
    let round: Session = serde_json::from_slice(&serde_json::to_vec(&session).unwrap()).unwrap();
    assert!(!round.uv_whole_vehicle);
    assert_eq!(round.siren_preview.vehicle_opacity, 25);
    round.validate().unwrap();
    let mut old = serde_json::to_value(&session).unwrap();
    old.as_object_mut().unwrap().remove("uv_whole_vehicle");
    old["siren_preview"]
        .as_object_mut()
        .unwrap()
        .remove("vehicle_opacity");
    let old: Session = serde_json::from_value(old).unwrap();
    assert!(old.uv_whole_vehicle);
    assert_eq!(old.siren_preview.vehicle_opacity, 100);
    session.siren_preview.vehicle_opacity = 101;
    assert!(session.validate().is_err());
}
