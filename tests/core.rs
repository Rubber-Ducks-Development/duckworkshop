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
    out.extend_from_slice(&(if ext == "yft" { 159u32 } else { 165 }).to_le_bytes());
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
