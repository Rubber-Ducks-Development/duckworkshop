use duck_workshop::{models, patterns::Pattern, sirens};

#[test]
fn xml_lenses_use_bone_palettes_and_do_not_bind_the_housing() {
    let xml = r#"<Drawable><ShaderGroup><Shaders><Item><Name>vehicle_lightsemissive</Name><Parameters><Item name="DiffuseSampler"><Name>leds</Name></Item></Parameters></Item><Item><Name>vehicle_mesh</Name></Item></Shaders></ShaderGroup><Skeleton><Bones><Item><Name>root</Name><ParentIndex value="-1"/></Item><Item><Name>siren2</Name><ParentIndex value="0"/></Item><Item><Name>lens_child</Name><ParentIndex value="1"/></Item></Bones></Skeleton><DrawableModelsHigh><Item><BoneIndex value="1"/><Geometries><Item><ShaderIndex value="0"/><BoneIDs>2 0</BoneIDs><VertexBuffer><Layout><Position/><BlendWeights/><BlendIndices/><TexCoord0/></Layout><Data>0 0 0 0 255 0 0 0 0 0 0 0 0
1 0 0 0 255 0 0 0 0 0 0 1 0
0 1 0 0 255 0 0 0 0 0 0 0 1</Data></VertexBuffer><IndexBuffer><Data>0 1 2</Data></IndexBuffer></Item></Geometries></Item></DrawableModelsHigh></Drawable>"#;
    let m = models::parse_xml(xml).unwrap();
    assert_eq!(m.meshes[0].siren_ids, [2, 2, 2]);
    let alpha = models::parse_xml(&xml.replacen(
        "<Name>vehicle_lightsemissive</Name>",
        "<Name>vehicle_lightsemissive</Name><RenderBucket value=\"1\"/>",
        1,
    ))
    .unwrap();
    assert_eq!(
        alpha.material_info[&alpha.meshes[0].material].render_bucket,
        1
    );
    let round: models::Model =
        serde_json::from_slice(&serde_json::to_vec(&alpha).unwrap()).unwrap();
    assert_eq!(
        round.material_info[&round.meshes[0].material].render_bucket,
        1
    );
    assert_eq!(
        m.material_info[&m.meshes[0].material].diffuse_texture,
        "leds"
    );
    let housing =
        models::parse_xml(&xml.replace("<ShaderIndex value=\"0\"/>", "<ShaderIndex value=\"1\"/>"))
            .unwrap();
    assert!(housing.meshes[0].siren_ids.is_empty());
    let invalid = xml.replacen(
        "0 0 0 0 255 0 0 0 0 0 0 0 0",
        "0 0 0 0 255 0 0 0 -1 0 0 0 0",
        1,
    );
    assert!(models::parse_xml(&invalid).is_err());
}

#[test]
fn lens_scaling_matches_imported_carcols_and_default_exports() {
    let mut p = Pattern::default();
    assert_eq!(sirens::surface_scales(&p), [100.; 32]);
    p.source_index = 1;
    p.source_xml=Some("<CVehicleModelInfoVarGlobal><Sirens><Item/><Item><sirens><Item><scale value=\"true\"/><scaleFactor value=\"10\"/></Item><Item><scale value=\"false\"/><scaleFactor value=\"40\"/></Item><Item><scaleFactor value=\"NaN\"/></Item></sirens></Item></Sirens></CVehicleModelInfoVarGlobal>".into());
    let scales = sirens::surface_scales(&p);
    assert_eq!(&scales[..4], &[10., 1., 100., 100.]);
    let markers = sirens::markers(None, &p, &Default::default(), 0);
    assert_eq!(markers[0].surface_scale, 10.);
    assert_eq!(markers[1].surface_scale, 1.);
    let mut model =
        models::parse_obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nf 1/1 2/2 3/3")
            .unwrap();
    model.meshes[0].siren_ids = vec![33; 3];
    assert!(model.validate().is_err());
    model.meshes[0].siren_ids = vec![1];
    assert!(model.validate().is_err());
}

#[test]
fn real_lenses_do_not_use_export_defaults_when_vehicle_settings_are_missing() {
    let mut model =
        models::parse_obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nf 1/1 2/2 3/3")
            .unwrap();
    model.meshes[0].siren_ids = vec![1; 3];
    model.sirens.push(sirens::SirenAnchor {
        id: 1,
        position: [0.; 3],
    });
    let mut pattern = Pattern::default();
    assert!(sirens::missing_surface_scales(&model, &pattern));
    assert_eq!(
        sirens::markers(Some(&model), &pattern, &Default::default(), 0)[0].surface_scale,
        1.
    );
    assert_eq!(sirens::surface_scales(&pattern)[0], 100.); // New exports retain their documented default.
    for scale in [10., 100.] {
        pattern.source_xml = Some(format!(
            "<CVehicleModelInfoVarGlobal><Sirens><Item><sirens><Item><scale value=\"true\"/><scaleFactor value=\"{scale}\"/></Item></sirens></Item></Sirens></CVehicleModelInfoVarGlobal>"
        ));
        assert!(!sirens::missing_surface_scales(&model, &pattern));
        assert_eq!(
            sirens::markers(Some(&model), &pattern, &Default::default(), 0)[0].surface_scale,
            scale
        );
    }
    // An incomplete or invalid source must not silently enlarge other real lenses.
    model.meshes[0].siren_ids = vec![2; 3];
    model.sirens[0].id = 2;
    assert!(sirens::missing_surface_scales(&model, &pattern));
    assert_eq!(
        sirens::markers(Some(&model), &pattern, &Default::default(), 0)[0].surface_scale,
        1.
    );
    pattern.source_xml = Some("invalid XML".into());
    assert!(sirens::missing_surface_scales(&model, &pattern));
}
#[test]
fn imported_vehicles_hide_missing_siren_slots_but_keep_schematic_and_manual_positions() {
    use duck_workshop::{
        models::Model,
        patterns::Channel,
        sirens::{PreviewSettings, SirenAnchor},
    };
    let p = Pattern {
        channels: (1..=32)
            .map(|i| Channel::new(i.to_string(), [255, 0, 0]))
            .collect(),
        ..Default::default()
    };
    let model = Model {
        sirens: vec![
            SirenAnchor {
                id: 1,
                position: [1., 2., 3.],
            },
            SirenAnchor {
                id: 20,
                position: [4., 5., 6.],
            },
        ],
        ..Default::default()
    };
    let mut settings = PreviewSettings::default();
    let m = sirens::markers(Some(&model), &p, &settings, 0);
    assert_eq!(m.iter().map(|m| m.id).collect::<Vec<_>>(), [1, 20]);
    assert_eq!(m[1].channel, 19);
    assert_eq!(m[1].position, [4., 5., 6.]);
    assert_eq!(sirens::markers(None, &p, &settings, 0).len(), 32);
    settings.positions.insert(32, [7., 8., 9.]);
    assert_eq!(
        sirens::markers(Some(&model), &p, &settings, 0)
            .last()
            .unwrap()
            .id,
        32
    );
}
