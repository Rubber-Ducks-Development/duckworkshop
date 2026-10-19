use duck_workshop::{
    diff, meta, models,
    patterns::{self, Pattern},
    render,
};
use std::io::Write;

#[test]
fn obj_recovers_negative_indices_materials_and_ngons() {
    let obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nusemtl livery\nf -4/-4 -3/-3 -2/-2 -1/-1";
    let m = models::parse_obj(obj).unwrap();
    assert_eq!(m.triangles(), 2);
    assert_eq!(m.meshes[0].material, "livery");
    assert_eq!(m.meshes[0].uvs[0][5], [0., 1.]);
}
#[test]
fn obj_rejects_partial_uvs_and_invalid_indices() {
    assert!(models::parse_obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nf 1/1 2 3").is_err());
    assert!(models::parse_obj("v 0 0 0\nf 1/1 2/1 3/1").is_err());
}
#[test]
fn xml_recovers_two_uv_channels_and_lod() {
    let xml = "<Drawable><DrawableModelsMedium><Item><Geometries><Item><ShaderIndex value=\"3\"/><VertexBuffer><Layout><Position/><Normal/><TexCoord0/><TexCoord1/></Layout><Data>0 0 0 0 0 1 0 0 0.1 0.2\n1 0 0 0 0 1 1 0 0.9 0.2\n0 1 0 0 0 1 0 1 0.1 0.8</Data></VertexBuffer><IndexBuffer><Data>0 1 2</Data></IndexBuffer></Item></Geometries></Item></DrawableModelsMedium></Drawable>";
    let m = models::parse_xml(xml).unwrap();
    assert_eq!(m.meshes[0].lod, "Med");
    assert_eq!(m.meshes[0].uvs[1][2], [0.1, 0.8]);
}
fn rsc_fixture(ext: &str) -> Vec<u8> {
    let mut b = vec![0u8; 8192 + 512];
    let ptr = |b: &mut Vec<u8>, o: usize, p: u64| b[o..o + 8].copy_from_slice(&p.to_le_bytes());
    let u16w = |b: &mut Vec<u8>, o: usize, v: u16| b[o..o + 2].copy_from_slice(&v.to_le_bytes());
    let u32w = |b: &mut Vec<u8>, o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    let d = if ext == "yft" {
        ptr(&mut b, 0x30, 0x50000140);
        0x140
    } else {
        0
    };
    ptr(&mut b, d + 0x50, 0x50000200);
    ptr(&mut b, 0x200, 0x50000220);
    u16w(&mut b, 0x208, 1);
    u16w(&mut b, 0x20a, 1);
    ptr(&mut b, 0x220, 0x50000240);
    ptr(&mut b, 0x248, 0x50000280);
    u16w(&mut b, 0x250, 1);
    ptr(&mut b, 0x260, 0x500002a0);
    u16w(&mut b, 0x2a0, 7);
    ptr(&mut b, 0x280, 0x50000300);
    ptr(&mut b, 0x318, 0x50000400);
    ptr(&mut b, 0x338, 0x50000500);
    u16w(&mut b, 0x408, 20);
    ptr(&mut b, 0x410, 0x60000000);
    u32w(&mut b, 0x418, 3);
    ptr(&mut b, 0x430, 0x50000600);
    u32w(&mut b, 0x508, 3);
    ptr(&mut b, 0x510, 0x60000080);
    u32w(&mut b, 0x600, (1 << 0) | (1 << 6));
    u16w(&mut b, 0x604, 20);
    ptr(&mut b, 0x608, 6 | (5 << 24));
    for (i, vertex) in [
        [0f32, 0., 0., 0., 0.],
        [1., 0., 0., 1., 0.],
        [0., 1., 0., 0., 1.],
    ]
    .iter()
    .enumerate()
    {
        for (j, v) in vertex.iter().enumerate() {
            b[8192 + i * 20 + j * 4..8192 + i * 20 + j * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    for i in 0..3 {
        u16w(&mut b, 8192 + 0x80 + i * 2, i as u16);
    }
    let mut out = b"RSC7".to_vec();
    out.extend_from_slice(&(if ext == "yft" { 162u32 } else { 165 }).to_le_bytes());
    out.extend_from_slice(&(1u32 << 17).to_le_bytes());
    out.extend_from_slice(&(1u32 << 27).to_le_bytes());
    let mut encoder = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::default());
    encoder.write_all(&b).unwrap();
    out.extend(encoder.finish().unwrap());
    out
}
#[test]
fn native_yft_and_ydr_extract_indices_shader_and_uvs() {
    for ext in ["yft", "ydr"] {
        let m = models::parse_rsc7(&rsc_fixture(ext), ext).unwrap();
        assert_eq!(m.triangles(), 1);
        assert_eq!(m.meshes[0].material, "Shader 7");
        assert_eq!(m.meshes[0].uvs[0][1], [1., 0.]);
        assert_eq!(m.meshes[0].positions[2], [0., 1., 0.]);
    }
}

#[test]
fn native_and_xml_vertex_alpha_survive_import_and_saved_models() {
    let fixture = rewrite_rsc7(&rsc_fixture("yft"), |data| {
        let original = data[8192..8192 + 60].to_vec();
        data[0x408..0x40a].copy_from_slice(&24u16.to_le_bytes());
        data[0x600..0x604].copy_from_slice(&((1u32 << 0) | (1 << 4) | (1 << 6)).to_le_bytes());
        data[0x604..0x606].copy_from_slice(&24u16.to_le_bytes());
        data[0x608..0x610].copy_from_slice(&(6u64 | (9 << 16) | (5 << 24)).to_le_bytes());
        for i in 0..3 {
            let o = 8192 + i * 24;
            data[o..o + 12].copy_from_slice(&original[i * 20..i * 20 + 12]);
            data[o + 12..o + 16].copy_from_slice(&[255, 255, 255, (i as u8 + 1) * 51]);
            data[o + 16..o + 24].copy_from_slice(&original[i * 20 + 12..i * 20 + 20]);
        }
    });
    let native = models::parse_rsc7(&fixture, "yft").unwrap();
    assert_eq!(native.meshes[0].vertex_alpha, [51, 102, 153]);
    assert_eq!(native.meshes[0].uvs[0][1], [1., 0.]);
    let xml = "<Drawable><DrawableModelsHigh><Item><Geometries><Item><VertexBuffer><Layout><Position/><Colour0/><TexCoord0/></Layout><Data>0 0 0 255 255 255 51 0 0\n1 0 0 255 255 255 102 1 0\n0 1 0 255 255 255 153 0 1</Data></VertexBuffer><IndexBuffer><Data>0 1 2</Data></IndexBuffer></Item></Geometries></Item></DrawableModelsHigh></Drawable>";
    let model = models::parse_xml(xml).unwrap();
    assert_eq!(model.meshes[0].vertex_alpha, native.meshes[0].vertex_alpha);
    let mut round: models::Model =
        serde_json::from_slice(&serde_json::to_vec(&model).unwrap()).unwrap();
    assert_eq!(round.meshes[0].vertex_alpha, [51, 102, 153]);
    round.meshes[0].vertex_alpha.pop();
    assert!(round.validate().is_err());
    let mut old = serde_json::to_value(&model).unwrap();
    old["meshes"][0]
        .as_object_mut()
        .unwrap()
        .remove("vertex_alpha");
    let old: models::Model = serde_json::from_value(old).unwrap();
    assert!(old.meshes[0].vertex_alpha.is_empty());
    old.validate().unwrap();
    assert!(models::parse_xml(&xml.replace("255 255 255 51", "255 255 255 256")).is_err());
}

#[test]
fn resource_versions_match_the_model_type_and_reject_gen9() {
    for (ext, rejected) in [
        ("yft", vec![159u32, 165, 171]),
        ("ydr", vec![154, 159, 162]),
    ] {
        for version in rejected {
            let mut bytes = rsc_fixture(ext);
            bytes[4..8].copy_from_slice(&version.to_le_bytes());
            assert!(
                models::parse_rsc7(&bytes, ext)
                    .unwrap_err()
                    .contains("Unsupported")
            );
        }
    }
    models::parse_rsc7(&rsc_fixture("yft"), "YFT").unwrap();
}

fn rewrite_rsc7(fixture: &[u8], edit: impl FnOnce(&mut [u8])) -> Vec<u8> {
    use std::io::Read;
    let mut data = vec![];
    flate2::read::DeflateDecoder::new(&fixture[16..])
        .read_to_end(&mut data)
        .unwrap();
    edit(&mut data);
    let mut out = fixture[..16].to_vec();
    let mut encoder = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::default());
    encoder.write_all(&data).unwrap();
    out.extend(encoder.finish().unwrap());
    out
}

#[test]
fn native_siren_bones_apply_parent_transforms() {
    let fixture = rewrite_rsc7(&rsc_fixture("yft"), |data| {
        let ptr =
            |data: &mut [u8], o: usize, p: u64| data[o..o + 8].copy_from_slice(&p.to_le_bytes());
        ptr(data, 0x158, 0x50000700);
        ptr(data, 0x720, 0x50000800);
        data[0x75e..0x760].copy_from_slice(&2u16.to_le_bytes());
        for b in [0x800, 0x850] {
            data[b + 12..b + 16].copy_from_slice(&1f32.to_le_bytes());
            for o in [0x20, 0x24, 0x28] {
                data[b + o..b + o + 4].copy_from_slice(&1f32.to_le_bytes());
            }
        }
        data[0x832..0x834].copy_from_slice(&(-1i16).to_le_bytes());
        data[0x810..0x814].copy_from_slice(&1f32.to_le_bytes());
        data[0x864..0x868].copy_from_slice(&2f32.to_le_bytes());
        ptr(data, 0x888, 0x50000900);
        data[0x900..0x908].copy_from_slice(b"siren32\0");
    });
    let model = models::parse_rsc7(&fixture, "yft").unwrap();
    assert_eq!(model.sirens.len(), 1);
    assert_eq!(model.sirens[0].id, 32);
    assert_eq!(model.sirens[0].position, [1., 2., 0.]);
}

#[test]
fn native_lens_vertices_follow_weighted_and_rigid_siren_bones() {
    let fixture = rewrite_rsc7(&rsc_fixture("yft"), |d| {
        let ptr = |d: &mut [u8], o: usize, p: u64| d[o..o + 8].copy_from_slice(&p.to_le_bytes());
        ptr(d, 0x158, 0x50000c00);
        ptr(d, 0xc20, 0x50000d00);
        d[0xc5e..0xc60].copy_from_slice(&2u16.to_le_bytes());
        for b in [0xd00, 0xd50] {
            d[b + 12..b + 16].copy_from_slice(&1f32.to_le_bytes());
            for o in [0x20, 0x24, 0x28] {
                d[b + o..b + o + 4].copy_from_slice(&1f32.to_le_bytes());
            }
        }
        d[0xd32..0xd34].copy_from_slice(&(-1i16).to_le_bytes());
        ptr(d, 0xd88, 0x50000e00);
        d[0xe00..0xe07].copy_from_slice(b"siren7\0");
        ptr(d, 0x150, 0x50000f00);
        ptr(d, 0xf10, 0x50000f40);
        d[0xf18..0xf1a].copy_from_slice(&8u16.to_le_bytes());
        ptr(d, 0xf78, 0x50000f80);
        d[0xf88..0xf8c].copy_from_slice(&0xe515a6e7u32.to_le_bytes());
        ptr(d, 0x368, 0x50001000);
        d[0x372..0x374].copy_from_slice(&2u16.to_le_bytes());
        d[0x1000..0x1002].copy_from_slice(&1u16.to_le_bytes());
        d[0x408..0x40a].copy_from_slice(&28u16.to_le_bytes());
        d[0x600..0x604]
            .copy_from_slice(&((1u32 << 0) | (1 << 1) | (1 << 2) | (1 << 6)).to_le_bytes());
        ptr(d, 0x608, 6 | (9 << 4) | (9 << 8) | (5 << 24));
        for v in 0..3 {
            let o = 8192 + v * 28;
            d[o..o + 28].fill(0);
            d[o + 14] = 255;
            d[o + 20..o + 24].copy_from_slice(&0.5f32.to_le_bytes());
        }
        d[8192 + 28..8192 + 32].copy_from_slice(&1f32.to_le_bytes());
        d[8192 + 56 + 4..8192 + 56 + 8].copy_from_slice(&1f32.to_le_bytes());
    });
    let model = models::parse_rsc7(&fixture, "yft").unwrap();
    assert_eq!(model.meshes[0].siren_ids, [7, 7, 7]);
    let saved = serde_json::to_value(&model).unwrap();
    let round: models::Model = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(round.meshes[0].siren_ids, [7, 7, 7]);
    let mut legacy = saved;
    legacy["meshes"][0]
        .as_object_mut()
        .unwrap()
        .remove("siren_ids");
    let old: models::Model = serde_json::from_value(legacy).unwrap();
    old.validate().unwrap();
    assert!(old.meshes[0].siren_ids.is_empty());
    let rigid = rewrite_rsc7(&fixture, |d| {
        d[0x600..0x604].copy_from_slice(&((1u32 << 0) | (1 << 6)).to_le_bytes());
        d[0x26b] = 1;
    });
    assert_eq!(
        models::parse_rsc7(&rigid, "yft").unwrap().meshes[0].siren_ids,
        [7, 7, 7]
    );
    let invalid = rewrite_rsc7(&fixture, |d| d[8192 + 18] = 3);
    assert!(models::parse_rsc7(&invalid, "yft").is_err());
}

#[test]
fn native_material_names_resolve_the_livery_sampler() {
    let fixture = rewrite_rsc7(&rsc_fixture("yft"), |data| {
        let ptr = |data: &mut [u8], offset: usize, value: u64| {
            data[offset..offset + 8].copy_from_slice(&value.to_le_bytes())
        };
        let hash = |name: &str| {
            let mut h = 0u32;
            for c in name.bytes() {
                h = h.wrapping_add(c as u32);
                h = h.wrapping_add(h << 10);
                h ^= h >> 6;
            }
            h = h.wrapping_add(h << 3);
            h ^= h >> 11;
            h.wrapping_add(h << 15)
        };
        ptr(data, 0x150, 0x50000700); // drawable ShaderGroup
        ptr(data, 0x710, 0x50000740);
        data[0x718..0x71a].copy_from_slice(&8u16.to_le_bytes());
        ptr(data, 0x778, 0x50000800); // shader index 7
        ptr(data, 0x800, 0x50000900);
        data[0x808..0x80c].copy_from_slice(&hash("vehicle_paint3").to_le_bytes());
        data[0x810] = 3;
        data[0x811] = 1; // preserve native alpha render bucket
        // One vector before the textures tests hash offsets past embedded values.
        data[0x900] = 1;
        ptr(data, 0x918, 0x50000a00);
        ptr(data, 0x928, 0x50000b00);
        data[0x944..0x948].copy_from_slice(&hash("diffusesampler").to_le_bytes());
        data[0x948..0x94c].copy_from_slice(&hash("diffusesampler2").to_le_bytes());
        ptr(data, 0xa28, 0x50000a80);
        data[0xa80..0xa85].copy_from_slice(b"base\0");
        ptr(data, 0xb28, 0x50000b80);
        data[0xb80..0xb8b].copy_from_slice(b"car_sign_1\0");
    });
    let model = models::parse_rsc7(&fixture, "yft").unwrap();
    assert_eq!(model.meshes[0].material, "Shader 7 / car_sign_1");
    let info = &model.material_info[&model.meshes[0].material];
    assert_eq!(info.diffuse_texture, "base");
    assert_eq!(info.secondary_texture, "car_sign_1");
    assert!(info.is_livery());
    assert_eq!(info.render_bucket, 1);
    let malformed = rewrite_rsc7(&fixture, |data| {
        data[0xb28..0xb30].copy_from_slice(&0x50002000u64.to_le_bytes())
    });
    let model = models::parse_rsc7(&malformed, "yft").unwrap();
    assert_eq!(model.triangles(), 1);
    assert!(
        model
            .warnings
            .iter()
            .any(|warning| warning.contains("Material names unavailable"))
    );
}

#[test]
fn empty_vehicle_geometry_slots_do_not_discard_the_model() {
    let fixture = rsc_fixture("yft");
    let with_empty_slot = rewrite_rsc7(&fixture, |data| {
        // Put a second, empty geometry beside the existing triangle.
        data[0x250..0x252].copy_from_slice(&2u16.to_le_bytes());
        data[0x288..0x290].copy_from_slice(&0x50000700u64.to_le_bytes());
        data[0x718..0x720].copy_from_slice(&0x50000800u64.to_le_bytes());
        data[0x738..0x740].copy_from_slice(&0x50000900u64.to_le_bytes());
    });
    let model = models::parse_rsc7(&with_empty_slot, "yft").unwrap();
    assert_eq!(model.meshes.len(), 1);
    assert_eq!(model.triangles(), 1);
    assert_eq!(model.meshes[0].uvs[0][2], [0., 1.]);
    let inconsistent = rewrite_rsc7(&with_empty_slot, |data| {
        data[0x908..0x90c].copy_from_slice(&3u32.to_le_bytes());
    });
    assert!(models::parse_rsc7(&inconsistent, "yft").is_err());
    let all_empty = rewrite_rsc7(&fixture, |data| {
        data[0x418..0x41c].fill(0);
        data[0x508..0x50c].fill(0);
    });
    assert!(
        models::parse_rsc7(&all_empty, "yft")
            .unwrap_err()
            .contains("No triangle")
    );
}

#[test]
fn carcols_large_ids_survive_pattern_project_and_variation_export() {
    use duck_workshop::project::{Project, Session};
    for id in [1, 65535, 9_784_798, u32::MAX] {
        let pattern = Pattern {
            id,
            ..Default::default()
        };
        let xml = pattern.to_xml().unwrap();
        let imported = Pattern::from_xml(&xml, 0).unwrap();
        assert_eq!(imported.id, id);
        assert_eq!(
            Pattern::from_xml(&imported.to_xml().unwrap(), 0)
                .unwrap()
                .id,
            id
        );
        let project = Project {
            format: "duck-workshop".into(),
            version: 1,
            session: Session {
                pattern: imported,
                ..Default::default()
            },
            model: None,
            tool: 1,
            dark_mode: false,
            comparison: Default::default(),
            textures: vec![],
        };
        assert_eq!(
            Project::from_json(&project.to_json().unwrap())
                .unwrap()
                .session
                .pattern
                .id,
            id
        );
        let variations = meta::variations("car", id, 0);
        let doc = duck_workshop::xml_document(&variations).unwrap();
        assert_eq!(
            doc.descendants()
                .find(|n| n.has_tag_name("sirenSettings"))
                .unwrap()
                .attribute("value"),
            Some(id.to_string().as_str())
        );
    }
    let xml = Pattern::default().to_xml().unwrap();
    for id in ["0", "-1", "4294967296"] {
        assert!(
            Pattern::from_xml(
                &xml.replace("<id value=\"1\"", &format!("<id value=\"{id}\"")),
                0
            )
            .is_err()
        );
    }
}

#[test]
fn metadata_type_is_detected_from_the_root() {
    for (kind, xml) in [
        meta::handling("car"),
        meta::vehicles("car", "car"),
        meta::variations("car", 1, 0),
        Pattern::default().to_xml().unwrap(),
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(meta::detect_kind(xml).unwrap(), kind);
        assert!(!meta::entries(xml, kind).unwrap().is_empty());
    }
    assert!(meta::detect_kind("<Other/>").is_err());
    assert!(meta::detect_kind("broken XML").is_err());
}

#[test]
fn carcols_edits_target_lights_kits_and_sirens_without_losing_other_sections() {
    let xml = "<CVehicleModelInfoVarGlobal><!--keep--><Lights><Item><id value=\"714\"/><name>Headlights</name><tailLight><intensity value=\"2\"/></tailLight></Item></Lights><Kits><Item><id value=\"2023\"/><kitName>custom</kitName></Item></Kits><Sirens><Item><id value=\"9784798\"/><name>Vehicle</name><sirens><Item><unknown value=\"keep\"/></Item></sirens></Item></Sirens></CVehicleModelInfoVarGlobal>";
    let names = meta::entries(xml, 3).unwrap();
    assert_eq!(
        names,
        ["Lights / Headlights", "Kits / ID 2023", "Sirens / Vehicle"]
    );
    let mut output = xml.to_string();
    for (index, id) in ["715", "2024", "9784799"].iter().enumerate() {
        let mut fields = meta::fields(&output, 3, index).unwrap();
        fields
            .iter_mut()
            .find(|f| f.path == "id@value")
            .unwrap()
            .value = id.to_string();
        output = meta::update(&output, 3, index, &fields).unwrap();
    }
    let doc = duck_workshop::xml_document(&output).unwrap();
    let ids: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("id"))
        .map(|n| n.attribute("value").unwrap())
        .collect();
    assert_eq!(ids, ["715", "2024", "9784799"]);
    assert!(
        doc.descendants()
            .any(|n| n.is_comment() && n.text().is_some_and(|text| text.trim() == "keep"))
    );
    assert!(output.contains("<unknown value=\"keep\""));
    assert!(output.contains("<intensity value=\"2\""));
    for section in ["Lights", "Kits"] {
        let xml = format!(
            "<CVehicleModelInfoVarGlobal><{section}><Item><id value=\"1\"/></Item></{section}></CVehicleModelInfoVarGlobal>"
        );
        assert_eq!(meta::entries(&xml, 3).unwrap().len(), 1);
        assert_eq!(meta::fields(&xml, 3, 0).unwrap()[0].value, "1");
        meta::update(&xml, 3, 0, &[]).unwrap();
    }
}
#[test]
fn corrupt_gta_resources_are_rejected() {
    let mut b = rsc_fixture("yft");
    b[4..8].copy_from_slice(&999u32.to_le_bytes());
    assert!(
        models::parse_rsc7(&b, "yft")
            .unwrap_err()
            .contains("Unsupported")
    );
    b.truncate(12);
    assert!(models::parse_rsc7(&b, "yft").is_err());
    let mut b = rsc_fixture("ydr");
    b[8..12].copy_from_slice(&0u32.to_le_bytes());
    assert!(models::parse_rsc7(&b, "ydr").is_err());
}
#[test]
fn ieee_half_and_resource_page_sizes() {
    assert_eq!(models::half(0x3c00), 1.);
    assert_eq!(models::half(0xbc00), -1.);
    assert_eq!(models::resource_size(1 << 17), 8192);
    assert_eq!(models::resource_size(1 << 27), 512);
}
#[test]
fn unsigned_sequences_keep_msb_and_full_range() {
    for n in [0, 1, 0x80000000, 4042322160, u32::MAX] {
        assert_eq!(patterns::decimal(&patterns::bits(n)), n)
    }
    assert!(patterns::bits(0x80000000)[0]);
    assert!(!patterns::bits(0x80000000)[31]);
}
#[test]
fn carcols_roundtrip_preserves_unedited_metadata() {
    let p = Pattern::default();
    let xml = p.to_xml().unwrap().replace(
        "<Lights />",
        "<Lights><Item><id value=\"77\" /></Item></Lights>",
    );
    let mut imported = Pattern::from_xml(&xml, 0).unwrap();
    imported.channels[0].bits = [true; 32];
    imported.bpm = 720.;
    let out = imported.to_xml().unwrap();
    let doc = duck_workshop::xml_document(&out).unwrap();
    assert!(
        doc.descendants()
            .any(|n| n.has_tag_name("id") && n.attribute("value") == Some("77"))
    );
    let round = Pattern::from_xml(&out, 0).unwrap();
    assert_eq!(patterns::decimal(&round.channels[0].bits), u32::MAX);
    assert_eq!(round.bpm, 720.);
}
#[test]
fn handling_diff_preserves_sub_float_precision_and_vectors() {
    let a = meta::handling("CAR");
    let b = a
        .replace("0.32", "0.3200000000000000000000001")
        .replace("z=\"-0.1\"", "z=\"-0.2\"");
    let rows = diff::compare(&a, &b).unwrap();
    let changed: Vec<_> = rows.iter().filter(|r| r.status != "same").collect();
    assert_eq!(changed.len(), 2);
    assert!(
        changed
            .iter()
            .any(|r| r.field == "/fInitialDriveForce/@value")
    );
    assert!(
        changed
            .iter()
            .any(|r| r.field == "/vecCentreOfMassOffset/@z")
    );
    assert_eq!(diff::canonical("1.000e2"), diff::canonical("100"));
    assert_ne!(
        diff::canonical("1.00000000000000000001"),
        diff::canonical("1")
    );
}
#[test]
fn handling_diff_matches_by_name_not_position() {
    let item = |name: &str| {
        format!(
            "<Item type=\"CHandlingData\"><handlingName>{name}</handlingName><fMass value=\"1000\"/></Item>"
        )
    };
    let doc = |items: String| {
        format!("<CHandlingDataMgr><HandlingData>{items}</HandlingData></CHandlingDataMgr>")
    };
    let a = doc(item("A") + &item("B"));
    let b = doc(item("B") + &item("A"));
    assert!(
        diff::compare(&a, &b)
            .unwrap()
            .iter()
            .all(|r| r.status == "same")
    );
    assert!(diff::records(&doc(item("A") + &item("A"))).is_err());
    assert!(
        diff::compare(&a, &doc(item("A")))
            .unwrap()
            .iter()
            .any(|r| r.vehicle == "B" && r.status == "removed")
    );
}
#[test]
fn guided_meta_keeps_nested_unknown_fields() {
    let xml = meta::handling("CAR").replace(
        "<SubHandlingData>",
        "<custom value=\"retain\"/><SubHandlingData>",
    );
    let mut fields = meta::fields(&xml, 0, 0).unwrap();
    fields
        .iter_mut()
        .find(|f| f.path == "fMass@value")
        .unwrap()
        .value = "2300".into();
    let out = meta::update(&xml, 0, 0, &fields).unwrap();
    assert!(out.contains("CCarHandlingData"));
    assert!(out.contains("retain"));
    assert!(out.contains("2300"));
}
#[test]
fn uv_export_png_svg_and_boundary_selection() {
    let model = mapped_fixture();
    let mut opt = render::UvOptions {
        size: 256,
        ..Default::default()
    };
    let png = render::uv_png(&model, &opt).unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (256, 256));
    assert!(image.pixels().any(|p| p[3] > 0));
    assert!(image.pixels().any(|p| p[3] == 0));
    let all = render::uv_edges(&model, &opt).len();
    opt.boundaries = true;
    assert!(render::uv_edges(&model, &opt).len() < all);
    let svg = render::uv_svg(&model, &opt).unwrap();
    duck_workshop::xml_document(&svg).unwrap();
    opt.channel = 7;
    assert!(render::uv_png(&model, &opt).is_err());
}
#[test]
fn chevrons_are_symmetric_and_uv_clipping_has_alpha() {
    let c = render::Chevron {
        width: 128,
        height: 128,
        ..Default::default()
    };
    let image = c.image(false, None, &render::UvOptions::default()).unwrap();
    assert_eq!(image.get_pixel(20, 40), image.get_pixel(108, 40));
    let c = render::Chevron { uv_clip: true, ..c };
    assert!(c.image(false, None, &render::UvOptions::default()).is_err());
    let image = c
        .image(
            false,
            Some(&mapped_fixture()),
            &render::UvOptions::default(),
        )
        .unwrap();
    assert!(image.pixels().any(|p| p[3] == 0));
    assert!(image.pixels().any(|p| p[3] > 0));
}
#[test]
fn xml_dtd_and_invalid_chevron_config_are_rejected() {
    assert!(duck_workshop::xml_document("<!DOCTYPE test><test/>").is_err());
    assert!(
        render::Chevron {
            width: 100000,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert_eq!(diff::canonical("--1"), "--1");
    assert_eq!(diff::canonical("1e-2147483648"), "1e-2147483648");
}

#[test]
fn fragment_physics_children_are_recovered() {
    use std::io::Read;
    let fixture = rsc_fixture("yft");
    let mut data = vec![];
    flate2::read::DeflateDecoder::new(&fixture[16..])
        .read_to_end(&mut data)
        .unwrap();
    let write_ptr =
        |b: &mut Vec<u8>, offset, p: u64| b[offset..offset + 8].copy_from_slice(&p.to_le_bytes());
    write_ptr(&mut data, 0xf0, 0x50000800);
    write_ptr(&mut data, 0x810, 0x50000900);
    write_ptr(&mut data, 0x9d0, 0x50000a80);
    data[0xa1d] = 1;
    write_ptr(&mut data, 0xa80, 0x50000b00);
    write_ptr(&mut data, 0xba0, 0x50000c40);
    write_ptr(&mut data, 0xc90, 0x50000200);
    let mut out = fixture[..16].to_vec();
    let mut encoder = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::default());
    encoder.write_all(&data).unwrap();
    out.extend(encoder.finish().unwrap());
    let model = models::parse_rsc7(&out, "yft").unwrap();
    assert_eq!(model.meshes.len(), 2);
    assert_eq!(model.meshes[1].uvs[0][2], [0., 1.]);
}

#[test]
fn glb_embedded_buffer_and_node_transform() {
    let mut data = vec![];
    for vertex in [[0f32, 0., 0.], [1., 0., 0.], [0., 1., 0.]] {
        for value in vertex {
            data.extend(value.to_le_bytes());
        }
    }
    for uv in [[0f32, 0.], [1., 0.], [0., 1.]] {
        for value in uv {
            data.extend(value.to_le_bytes());
        }
    }
    for index in [0u16, 1, 2] {
        data.extend(index.to_le_bytes());
    }
    data.resize(68, 0);
    let json = serde_json::json!({"asset":{"version":"2.0"},"buffers":[{"byteLength":68}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":24},{"buffer":0,"byteOffset":60,"byteLength":6}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},{"bufferView":1,"componentType":5126,"count":3,"type":"VEC2"},{"bufferView":2,"componentType":5123,"count":3,"type":"SCALAR"}],"meshes":[{"name":"Mapped GLB","primitives":[{"attributes":{"POSITION":0,"TEXCOORD_0":1},"indices":2}]}],"nodes":[{"mesh":0,"translation":[2,0,0]}],"scenes":[{"nodes":[0]}],"scene":0});
    let mut text = serde_json::to_vec(&json).unwrap();
    while !text.len().is_multiple_of(4) {
        text.push(b' ');
    }
    let total = 12 + 8 + text.len() + 8 + data.len();
    let mut glb = vec![];
    glb.extend(b"glTF");
    glb.extend(2u32.to_le_bytes());
    glb.extend((total as u32).to_le_bytes());
    glb.extend((text.len() as u32).to_le_bytes());
    glb.extend(b"JSON");
    glb.extend(text);
    glb.extend((data.len() as u32).to_le_bytes());
    glb.extend(b"BIN\0");
    glb.extend(data);
    let model = models::parse_glb(&glb).unwrap();
    assert_eq!(model.triangles(), 1);
    assert_eq!(model.meshes[0].positions[0], [2., 0., 0.]);
    assert_eq!(model.meshes[0].uvs[0][1], [1., 0.]);
}

#[test]
fn hexadecimal_text_flags_do_not_use_decimal_normalization() {
    let a = meta::handling("CAR").replace("440010", "100");
    let b = a.replace(
        "<strModelFlags>100</strModelFlags>",
        "<strModelFlags>1e2</strModelFlags>",
    );
    assert_eq!(
        diff::compare(&a, &b)
            .unwrap()
            .iter()
            .filter(|r| r.status == "changed")
            .count(),
        1
    );
}
fn mapped_fixture() -> models::Model {
    let mut out = models::Model {
        name: "Fixture mesh".into(),
        ..Default::default()
    };
    let panels = [
        (
            "Left side",
            vec![
                [-2., 0., -0.8],
                [2., 0., -0.8],
                [2., 1.2, -0.8],
                [-2., 1.2, -0.8],
            ],
            vec![[0.04, 0.05], [0.94, 0.05], [0.94, 0.3], [0.04, 0.3]],
        ),
        (
            "Right side",
            vec![
                [-2., 0., 0.8],
                [-2., 1.2, 0.8],
                [2., 1.2, 0.8],
                [2., 0., 0.8],
            ],
            vec![[0.04, 0.37], [0.04, 0.62], [0.94, 0.62], [0.94, 0.37]],
        ),
        (
            "Roof",
            vec![
                [-0.8, 1.6, -0.8],
                [-0.8, 1.6, 0.8],
                [1.1, 1.6, 0.8],
                [1.1, 1.6, -0.8],
            ],
            vec![[0.06, 0.7], [0.06, 0.96], [0.5, 0.96], [0.5, 0.7]],
        ),
        (
            "Bonnet",
            vec![
                [1.1, 1.1, -0.8],
                [1.1, 1.1, 0.8],
                [2., 1.1, 0.8],
                [2., 1.1, -0.8],
            ],
            vec![[0.6, 0.7], [0.6, 0.96], [0.93, 0.96], [0.93, 0.7]],
        ),
    ];
    for (name, p, uv) in panels {
        let mut m = models::Mesh {
            vertex_alpha: vec![],
            siren_ids: vec![],
            name: name.into(),
            material: "Livery".into(),
            lod: "High".into(),
            positions: vec![],
            uvs: vec![],
            indices: vec![],
            enabled: true,
        };
        m.positions = p;
        m.uvs = vec![uv];
        m.indices = vec![0, 1, 2, 0, 2, 3];
        out.meshes.push(m)
    }
    out
}
#[test]
fn intact_fragment_instances_use_placement_and_offset_without_damaged_duplicates() {
    let fixture = rewrite_rsc7(&rsc_fixture("yft"), |d| {
        let ptr = |d: &mut [u8], o: usize, p: u64| d[o..o + 8].copy_from_slice(&p.to_le_bytes());
        // Reuse the same geometry as two placed parts. A damaged pointer is present too.
        ptr(d, 0xf0, 0x50000700);
        ptr(d, 0x710, 0x50000800);
        d[0x91d] = 2;
        ptr(d, 0x8d0, 0x50000a00);
        ptr(d, 0x900, 0x50000d00);
        d[0x830..0x834].copy_from_slice(&3f32.to_le_bytes());
        ptr(d, 0xa00, 0x50000b00);
        ptr(d, 0xa08, 0x50000c00);
        ptr(d, 0xba0, 0x50000140);
        ptr(d, 0xba8, 0x50000140);
        ptr(d, 0xca0, 0x50000140);
        d[0xd10..0xd14].copy_from_slice(&2u32.to_le_bytes());
        for (i, x) in [4f32, 9.].into_iter().enumerate() {
            let mat = glam::Mat4::from_translation(glam::Vec3::new(x, 2., 1.)).to_cols_array();
            for (j, v) in mat.iter().enumerate() {
                let o = 0xd20 + i * 64 + j * 4;
                d[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
    });
    let m = models::parse_rsc7(&fixture, "yft").unwrap();
    assert_eq!(m.meshes.len(), 3);
    assert_eq!(m.meshes[0].positions[0], [0., 0., 0.]);
    assert_eq!(m.meshes[1].positions[0], [7., 2., 1.]);
    assert_eq!(m.meshes[2].positions[0], [12., 2., 1.]);
    assert_eq!(m.meshes[0].uvs, m.meshes[2].uvs);
}

#[test]
fn resource_folder_matches_siren_id_and_never_guesses_on_missing_or_duplicate_metadata() {
    use duck_workshop::vehicle_resource as vr;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("stream")).unwrap();
    std::fs::create_dir(root.join("data")).unwrap();
    std::fs::write(root.join("stream/zzcar.yft"), rsc_fixture("yft")).unwrap();
    std::fs::write(root.join("stream/zzcar_hi.yft"), rsc_fixture("yft")).unwrap();
    std::fs::write(root.join("stream/accessory.yft"), rsc_fixture("yft")).unwrap();
    std::fs::write(
        root.join("data/vehicles.meta"),
        meta::vehicles("zzcar", "zzcar"),
    )
    .unwrap();
    std::fs::write(
        root.join("data/carvariations.meta"),
        meta::variations("zzcar", 42, 0),
    )
    .unwrap();
    let pattern = Pattern {
        id: 42,
        name: "Matched".into(),
        ..Default::default()
    };
    std::fs::write(root.join("data/carcols.meta"), pattern.to_xml().unwrap()).unwrap();
    let catalog = vr::discover(root).unwrap();
    assert_eq!(catalog.vehicles.len(), 2);
    assert_eq!(catalog.vehicles[0].name, "zzcar");
    let car = &catalog.vehicles[0];
    let imported = vr::import(car, true).unwrap();
    assert_eq!(imported.model.name, "zzcar_hi.yft");
    assert_eq!(imported.pattern.unwrap().id, 42);
    let bytes = std::fs::read(root.join("data/carcols.meta")).unwrap();
    std::fs::create_dir(root.join("more")).unwrap();
    std::fs::write(root.join("more/carcols.meta"), &bytes).unwrap();
    assert!(vr::import(car, false).unwrap().pattern.is_none());
    std::fs::remove_file(root.join("more/carcols.meta")).unwrap();
    std::fs::write(
        root.join("data/carvariations.meta"),
        "</Item><Item><modelName>zzcar</modelName>",
    )
    .unwrap();
    let missing = vr::import(car, false).unwrap();
    assert!(missing.pattern.is_none());
    assert!(
        missing
            .model
            .warnings
            .iter()
            .any(|w| w.contains("Invalid XML"))
    );
    assert_eq!(
        std::fs::read(root.join("data/carcols.meta")).unwrap(),
        bytes
    );
}
