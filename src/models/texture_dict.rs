//! Read-only legacy PC dictionary textures needed by the vehicle preview.
//! Format research: CodeWalker.Core/GameFiles/Resources/Texture.cs (original reader).
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn load_nearby(path: &Path, model: &mut Model) {
    let Some(parent) = path.parent() else { return };
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let stem = stem.strip_suffix("_hi").unwrap_or(&stem);
    let mut paths = vec![
        parent.join(format!("{stem}+hi.ytd")),
        parent.join(format!("{stem}.ytd")),
    ];
    if let Ok(entries) = std::fs::read_dir(parent) {
        let mut others: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|s| s.eq_ignore_ascii_case("ytd")))
            .collect();
        others.sort();
        paths.extend(others.into_iter().take(16));
    }
    load_paths(&paths, model);
}
pub(super) fn load_paths(paths: &[std::path::PathBuf], model: &mut Model) {
    let mut needed: BTreeSet<_> = model
        .material_info
        .values()
        .flat_map(|m| [&m.diffuse_texture, &m.secondary_texture])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect();
    needed.retain(|n| !model.preview_textures.contains_key(n));
    let mut seen = BTreeSet::new();
    for path in paths {
        if needed.is_empty() || !seen.insert(path.clone()) || !path.is_file() {
            continue;
        }
        let result = crate::read_file(path).and_then(|b| read(&b, &needed));
        match result {
            Ok(textures) => {
                for (name, data) in textures {
                    if model.preview_textures.len() >= 256 {
                        break;
                    }
                    let current: usize = model.preview_textures.values().map(String::len).sum();
                    if current + data.len() > 48 * 1024 * 1024 {
                        break;
                    }
                    needed.remove(&name);
                    model.preview_textures.insert(name, data);
                }
            }
            Err(error) => model.warnings.push(format!(
                "Preview textures in {}: {error}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }
}

fn read(bytes: &[u8], needed: &BTreeSet<String>) -> Result<BTreeMap<String, String>> {
    if bytes.len() < 16
        || &bytes[..4] != b"RSC7"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 13
    {
        return Err("Only legacy PC YTD version 13 is supported".into());
    }
    let sys = resource_size(u32::from_le_bytes(bytes[8..12].try_into().unwrap()));
    let gfx = resource_size(u32::from_le_bytes(bytes[12..16].try_into().unwrap()));
    let total = sys + gfx;
    if sys < 64 || total > 256 * 1024 * 1024 {
        return Err("Invalid texture dictionary size".into());
    }
    let mut data = vec![];
    flate2::read::DeflateDecoder::new(&bytes[16..])
        .take((total + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() != total {
        return Err("Truncated texture dictionary".into());
    }
    let r = Reader { data, sys, gfx };
    let count = r.u16(0x38)? as usize;
    if count > 4096 {
        return Err("Invalid texture count".into());
    }
    let mut out = BTreeMap::new();
    if count == 0 {
        return Ok(out);
    }
    let array = r.req(0x30)?;
    for i in 0..count {
        let Some(t) = r.ptr(array + i * 8)? else {
            continue;
        };
        let name = r.string(r.req(t + 0x28)?)?.to_ascii_lowercase();
        if !needed.contains(&name) {
            continue;
        }
        let (w, h) = (r.u16(t + 0x50)? as u32, r.u16(t + 0x52)? as u32);
        if w == 0 || h == 0 || w.max(h) > 4096 {
            continue;
        }
        let format = r.u32(t + 0x58)?;
        let data = r.req(t + 0x70)?;
        let encoded = match format {
            0x31545844 | 0x33545844 | 0x35545844 => {
                let size = w.div_ceil(4) as usize
                    * h.div_ceil(4) as usize
                    * if format == 0x31545844 { 8 } else { 16 };
                let pixels = r
                    .data
                    .get(data..data + size)
                    .ok_or("Truncated compressed texture")?;
                let mut dds = vec![0u8; 128];
                dds[..4].copy_from_slice(b"DDS ");
                for (offset, value) in [
                    (4, 124),
                    (8, 0x81007),
                    (12, h),
                    (16, w),
                    (20, size as u32),
                    (28, 1),
                    (76, 32),
                    (80, 4),
                    (84, format),
                    (108, 0x1000),
                ] {
                    dds[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
                dds.extend_from_slice(pixels);
                dds
            }
            21 | 22 | 32 => {
                let stride = (r.u16(t + 0x56)? as usize).max(w as usize * 4);
                let mut image = image::RgbaImage::new(w, h);
                for (x, y, pixel) in image.enumerate_pixels_mut() {
                    let color = r.bytes::<4>(data + y as usize * stride + x as usize * 4)?;
                    *pixel = image::Rgba(if format == 32 {
                        color
                    } else {
                        [
                            color[2],
                            color[1],
                            color[0],
                            if format == 22 { 255 } else { color[3] },
                        ]
                    });
                }
                let mut encoded = std::io::Cursor::new(vec![]);
                image
                    .write_to(&mut encoded, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                encoded.into_inner()
            }
            _ => continue,
        };
        let encoded = STANDARD.encode(encoded);
        if out.len() >= 256
            || out.values().map(String::len).sum::<usize>() + encoded.len() > 48 * 1024 * 1024
        {
            break;
        }
        out.insert(name, encoded);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_dictionary_decodes_dxt_and_bgra_diffuse_textures() {
        use std::io::Write;
        let mut data = vec![0u8; 1024];
        let ptr = |d: &mut [u8], o: usize, v: u64| d[o..o + 8].copy_from_slice(&v.to_le_bytes());
        ptr(&mut data, 0x30, 0x50000080);
        data[0x38..0x3a].copy_from_slice(&1u16.to_le_bytes());
        ptr(&mut data, 0x80, 0x50000100);
        ptr(&mut data, 0x128, 0x500001a0);
        data[0x1a0..0x1a5].copy_from_slice(b"lens\0");
        data[0x150..0x152].copy_from_slice(&4u16.to_le_bytes());
        data[0x152..0x154].copy_from_slice(&4u16.to_le_bytes());
        data[0x158..0x15c].copy_from_slice(&0x31545844u32.to_le_bytes());
        ptr(&mut data, 0x170, 0x60000000);
        data[512..514].copy_from_slice(&0xf800u16.to_le_bytes());
        let resource = |data: &[u8]| {
            let mut bytes = b"RSC7".to_vec();
            for v in [13u32, 1 << 27, 1 << 27] {
                bytes.extend(v.to_le_bytes());
            }
            let mut zip = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::fast());
            zip.write_all(data).unwrap();
            bytes.extend(zip.finish().unwrap());
            bytes
        };
        let needed = BTreeSet::from(["lens".to_string()]);
        let decoded = read(&resource(&data), &needed).unwrap();
        let image = crate::textures::decode(&STANDARD.decode(&decoded["lens"]).unwrap()).unwrap();
        assert_eq!(image.dimensions(), (4, 4));
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
        data[0x158..0x15c].copy_from_slice(&21u32.to_le_bytes());
        data[512..516].copy_from_slice(&[11, 22, 33, 255]);
        let decoded = read(&resource(&data), &needed).unwrap();
        let image = crate::textures::decode(&STANDARD.decode(&decoded["lens"]).unwrap()).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [33, 22, 11, 255]);
        ptr(&mut data, 0x170, 0x60001000);
        assert!(read(&resource(&data), &needed).is_err());
    }
    #[test]
    fn dictionary_bounds_and_empty_resources_are_checked() {
        assert!(read(b"invalid", &BTreeSet::new()).is_err());
        let mut header = b"RSC7".to_vec();
        for value in [13u32, 1 << 27, 0] {
            header.extend_from_slice(&value.to_le_bytes());
        }
        let mut data = vec![0u8; 512];
        let compress = |raw: &[u8]| {
            use std::io::Write;
            let mut encoder =
                flate2::write::DeflateEncoder::new(vec![], flate2::Compression::fast());
            encoder.write_all(raw).unwrap();
            encoder.finish().unwrap()
        };
        let mut resource = header.clone();
        resource.extend(compress(&data));
        assert!(read(&resource, &BTreeSet::new()).unwrap().is_empty());
        data[0x38..0x3a].copy_from_slice(&5000u16.to_le_bytes());
        let mut resource = header;
        resource.extend(compress(&data));
        assert!(read(&resource, &BTreeSet::new()).is_err());
    }
}
