use duck_workshop::{
    patterns::{self, Channel, Pattern},
    textures,
};

#[test]
fn a_step_shows_one_color_and_each_color_exports_as_a_siren() {
    let mut p = Pattern::default();
    p.channels.truncate(2);
    let light = &mut p.channels[0];
    assert!(light.add_color([45, 125, 255]));
    light.set_step(0, Some(0));
    light.set_step(1, Some(1));
    light.set_step(0, Some(1));
    assert_eq!(light.step(0), Some(1));
    assert!(
        !light.bits[0],
        "repainting a step clears its previous color"
    );
    light.set_step(31, Some(0));
    assert_eq!(p.siren_count(), 3);

    let round = Pattern::from_xml(&p.to_xml().unwrap(), 0).unwrap();
    assert_eq!(round.channels.len(), 3);
    assert_eq!(round.channels[0].color, p.channels[0].color);
    assert_eq!(round.channels[1].color, [45, 125, 255]);
    assert_eq!(patterns::decimal(&round.channels[0].bits), 1);
    assert_eq!(patterns::decimal(&round.channels[1].bits), 0xC000_0000);

    // Re-exporting an imported carcols keeps the expanded siren count.
    let mut imported = Pattern::from_xml(&p.to_xml().unwrap(), 0).unwrap();
    imported.merge_up(1).unwrap();
    assert_eq!(imported.channels.len(), 2);
    assert_eq!(imported.siren_count(), 3);
    let again = Pattern::from_xml(&imported.to_xml().unwrap(), 0).unwrap();
    assert_eq!(again.channels.len(), 3);
}

#[test]
fn merging_lights_keeps_the_upper_light_on_overlaps() {
    let mut p = Pattern::default();
    p.channels[0].set_step(0, Some(0));
    p.channels[0].set_step(1, Some(0));
    p.channels[1] = Channel::new("Blue".into(), [0, 0, 255]);
    p.channels[1].set_step(1, Some(0));
    p.channels[1].set_step(2, Some(0));
    let conflicts = p.merge_up(1).unwrap();
    assert_eq!(conflicts, 1);
    assert_eq!(p.channels.len(), 7);
    let merged = &p.channels[0];
    assert_eq!(merged.color_count(), 2);
    assert_eq!(merged.steps()[..3], [Some(0), Some(0), Some(1)]);
    assert!(p.merge_up(0).is_err());
}

#[test]
fn color_limits_and_older_files_without_extra_colors() {
    let mut light = Channel::new("A".into(), [1, 2, 3]);
    for _ in 1..patterns::MAX_COLORS {
        assert!(light.add_color([9, 9, 9]));
    }
    assert!(!light.add_color([9, 9, 9]));
    light.set_step(5, Some(2));
    light.remove_color(0);
    assert_eq!(light.color_count(), patterns::MAX_COLORS - 1);
    assert_eq!(light.step(5), Some(1));

    let mut p = Pattern::default();
    while p.channels.len() < 32 {
        p.channels.push(Channel::new("L".into(), [1, 1, 1]));
    }
    p.channels[0].add_color([2, 2, 2]);
    assert!(p.validate().is_err(), "33 sirens exceed carcols' limit");

    let old = serde_json::to_string(&Pattern::default())
        .unwrap()
        .replace(",\"extra\":[]", "");
    let parsed: Pattern = serde_json::from_str(&old).unwrap();
    assert_eq!(parsed.siren_count(), 8);
}

#[test]
fn textures_decode_png_and_merged_psd_image() {
    let mut png = vec![];
    image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128])
        .unwrap()
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let decoded = textures::decode(&png).unwrap();
    assert_eq!(decoded.dimensions(), (2, 1));
    assert_eq!(decoded.get_pixel(1, 0).0, [0, 0, 255, 128]);

    // Minimal 2 x 1 RGBA PSD: no layers, raw merged image data in planar order.
    let mut psd = b"8BPS".to_vec();
    psd.extend_from_slice(&1u16.to_be_bytes());
    psd.extend_from_slice(&[0; 6]);
    psd.extend_from_slice(&4u16.to_be_bytes());
    psd.extend_from_slice(&1u32.to_be_bytes());
    psd.extend_from_slice(&2u32.to_be_bytes());
    psd.extend_from_slice(&8u16.to_be_bytes());
    psd.extend_from_slice(&3u16.to_be_bytes());
    for _ in 0..3 {
        psd.extend_from_slice(&0u32.to_be_bytes());
    }
    psd.extend_from_slice(&0u16.to_be_bytes());
    psd.extend_from_slice(&[10, 200, 20, 210, 30, 220, 255, 255]);
    let image = textures::decode_psd(&psd).unwrap();
    assert_eq!(image.dimensions(), (2, 1));
    assert_eq!(image.get_pixel(0, 0).0, [10, 20, 30, 255]);
    assert_eq!(image.get_pixel(1, 0).0, [200, 210, 220, 255]);

    assert!(textures::decode(b"not an image").is_err());
    assert!(textures::decode_psd(b"8BPS\0\x02rest").is_err());
}
