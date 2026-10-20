//! Legacy PC dictionary previews and RGB-only lens texture patches.
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
    let needed: BTreeSet<_> = model
        .material_info
        .values()
        .flat_map(|m| [&m.diffuse_texture, &m.secondary_texture])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let lenses: BTreeSet<_> = model
        .material_info
        .values()
        .filter(|m| m.is_light_emissive())
        .map(|m| m.diffuse_texture.to_ascii_lowercase())
        .collect();
    let mut seen = BTreeSet::new();
    for path in paths {
        if needed.is_empty() || !seen.insert(path.clone()) || !path.is_file() {
            continue;
        }
        let result = crate::read_file(path)
            .and_then(|b| unpack(&b))
            .and_then(|r| {
                // Export provenance is independent of the bounded image cache. An
                // already cached image (or a full cache) must not hide a source YTD.
                for (_, name) in entries(&r)? {
                    if needed.contains(&name)
                        && model
                            .texture_sources
                            .get(&name)
                            .is_none_or(|p| !p.is_file())
                    {
                        model.texture_sources.insert(name, path.clone());
                    }
                }
                let missing = needed
                    .iter()
                    .filter(|n| !model.preview_textures.contains_key(*n))
                    .cloned()
                    .collect();
                read_resource_ordered(&r, &missing, &lenses, 48 * 1024 * 1024)
            });
        match result {
            Ok(textures) => {
                let mut textures: Vec<_> = textures.into_iter().collect();
                textures.sort_by_key(|(name, _)| !lenses.contains(name));
                for (name, data) in textures {
                    if lenses.contains(&name) {
                        while model.preview_textures.len() >= 256
                            || model
                                .preview_textures
                                .values()
                                .map(String::len)
                                .sum::<usize>()
                                + data.len()
                                > 48 * 1024 * 1024
                        {
                            let victim = model
                                .preview_textures
                                .iter()
                                .filter(|(n, _)| !lenses.contains(*n))
                                .max_by_key(|(_, d)| d.len())
                                .map(|(n, _)| n.clone());
                            let Some(victim) = victim else { break };
                            model.preview_textures.remove(&victim);
                        }
                    }
                    let current: usize = model.preview_textures.values().map(String::len).sum();
                    if model.preview_textures.len() >= 256
                        || current + data.len() > 48 * 1024 * 1024
                    {
                        continue;
                    }
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

#[cfg(test)]
pub(crate) fn read(bytes: &[u8], needed: &BTreeSet<String>) -> Result<BTreeMap<String, String>> {
    let r = unpack(bytes)?;
    read_resource(&r, needed)
}

/// Find names without decoding images or applying preview memory limits.
pub(crate) fn source_names(bytes: &[u8], needed: &BTreeSet<String>) -> Result<BTreeSet<String>> {
    Ok(entries(&unpack(bytes)?)?
        .into_iter()
        .map(|(_, n)| n)
        .filter(|n| needed.contains(n))
        .collect())
}

fn entries(r: &Reader) -> Result<Vec<(usize, String)>> {
    let count = r.u16(0x38)? as usize;
    if count > 4096 {
        return Err("Invalid texture count".into());
    }
    if count == 0 {
        return Ok(vec![]);
    }
    let array = r.req(0x30)?;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        if let Some(t) = r.ptr(array + i * 8)? {
            entries.push((t, r.string(r.req(t + 0x28)?)?.to_ascii_lowercase()));
        }
    }
    Ok(entries)
}

fn unpack(bytes: &[u8]) -> Result<Reader> {
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
    Ok(Reader { data, sys, gfx })
}

pub(crate) fn read_for_export(
    bytes: &[u8],
    needed: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>> {
    let r = unpack(bytes)?;
    let count = r.u16(0x38)? as usize;
    if count > 4096 {
        return Err("Invalid texture count".into());
    }
    if count == 0 {
        return Ok(BTreeMap::new());
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
        let format = r.u32(t + 0x58)?;
        if !matches!(format, 21 | 22 | 32 | 0x31545844 | 0x33545844 | 0x35545844) {
            return Err(format!(
                "{name} has an unsupported native lens texture format ({format:#x})"
            ));
        }
        let w = r.u16(t + 0x50)? as u32;
        let h = r.u16(t + 0x52)? as u32;
        if w == 0 || h == 0 || w.max(h) > 4096 {
            return Err(format!("Unsupported native dimensions for {name}"));
        }
    }
    read_resource(&r, needed)
}

fn read_resource(r: &Reader, needed: &BTreeSet<String>) -> Result<BTreeMap<String, String>> {
    read_resource_ordered(r, needed, &BTreeSet::new(), usize::MAX)
}

fn read_resource_ordered(
    r: &Reader,
    needed: &BTreeSet<String>,
    priority: &BTreeSet<String>,
    byte_limit: usize,
) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut entries = entries(r)?;
    entries.sort_by_key(|(_, n)| !priority.contains(n));
    for (t, name) in entries {
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
        if byte_limit != usize::MAX
            && (out.len() >= 256
                || out.values().map(String::len).sum::<usize>() + encoded.len() > byte_limit)
        {
            continue;
        }
        out.insert(name, encoded);
    }
    Ok(out)
}

/// Edit RGB in place, retaining resource pointers, formats, mip counts and encoded alpha.
pub(crate) fn recolor(
    bytes: &[u8],
    edits: &BTreeMap<String, (image::RgbaImage, Vec<bool>)>,
) -> Result<Vec<u8>> {
    use std::io::Write;
    let mut r = unpack(bytes)?;
    let count = r.u16(0x38)? as usize;
    if count > 4096 {
        return Err("Invalid texture count".into());
    }
    let array = r.req(0x30)?;
    let mut changed = BTreeSet::new();
    let mut detached = false;
    let mut allocations = Vec::new();
    for i in 0..count {
        if let Some(t) = r.ptr(array + i * 8)? {
            allocations.push((t, r.req(t + 0x70)?));
        }
    }
    for i in 0..count {
        let Some(t) = r.ptr(array + i * 8)? else {
            continue;
        };
        let name = r.string(r.req(t + 0x28)?)?.to_ascii_lowercase();
        let Some((base, mask)) = edits.get(&name) else {
            continue;
        };
        let w = r.u16(t + 0x50)? as u32;
        let h = r.u16(t + 0x52)? as u32;
        if w == 0 || h == 0 || w.max(h) > 4096 {
            return Err("Invalid lens texture dimensions".into());
        }
        if base.dimensions() != (w, h) || mask.len() != (w * h) as usize {
            return Err(format!(
                "Texture dimensions changed for {name}; reimport the vehicle"
            ));
        }
        let format = r.u32(t + 0x58)?;
        let levels = r.bytes::<1>(t + 0x5d)?[0].max(1) as u32;
        if levels > 1 + w.max(h).ilog2() {
            return Err(format!("Invalid mip count for {name}"));
        }
        let mut offset = r.req(t + 0x70)?;
        if offset < r.sys {
            return Err("Lens pixels must be in the graphics resource".into());
        }
        if allocations
            .iter()
            .any(|(other, p)| *other != t && *p == offset)
        {
            // Equal-color private textures can share immutable pixel storage.
            // Detach before editing one so the other lens keeps its color.
            let size = pixel_bytes(&r, t)?;
            let pixels = r
                .data
                .get(offset..offset + size)
                .ok_or("Truncated shared lens pixels")?
                .to_vec();
            let new = r.data.len().next_multiple_of(16);
            if new + size > 256 * 1024 * 1024 {
                return Err("Lens edit exceeds the native resource memory limit".into());
            }
            r.data.resize(new, 0);
            r.data.extend_from_slice(&pixels);
            r.gfx = r.data.len() - r.sys;
            r.data[t + 0x70..t + 0x78]
                .copy_from_slice(&(0x60000000u64 + (new - r.sys) as u64).to_le_bytes());
            if let Some(allocation) = allocations.iter_mut().find(|(other, _)| *other == t) {
                allocation.1 = new;
            }
            offset = new;
            detached = true;
        }
        let limit = allocations
            .iter()
            .filter(|(_, p)| *p > offset)
            .map(|(_, p)| *p)
            .min()
            .unwrap_or(r.data.len());
        let mut stride = (r.u16(t + 0x56)? as usize).max(w as usize * 4);
        for level in 0..levels {
            let mw = (w >> level).max(1);
            let mh = (h >> level).max(1);
            let image = if level == 0 {
                base.clone()
            } else {
                image::imageops::resize(base, mw, mh, image::imageops::FilterType::Triangle)
            };
            let affected = |x: u32, y: u32| {
                let scale = 1u32 << level;
                (y * scale..((y + 1) * scale).min(h)).any(|by| {
                    (x * scale..((x + 1) * scale).min(w)).any(|bx| mask[(by * w + bx) as usize])
                })
            };
            match format {
                0x31545844 | 0x33545844 | 0x35545844 => {
                    let block_size = if format == 0x31545844 { 8 } else { 16 };
                    let bw = mw.div_ceil(4);
                    let size = bw as usize * mh.div_ceil(4) as usize * block_size;
                    if offset + size > limit {
                        return Err("Texture mip chain overlaps another allocation".into());
                    }
                    let blocks = r
                        .data
                        .get_mut(offset..offset + size)
                        .ok_or("Truncated texture mip chain")?;
                    for (n, block) in blocks.chunks_exact_mut(block_size).enumerate() {
                        let bx = (n as u32 % bw) * 4;
                        let by = (n as u32 / bw) * 4;
                        if !(0..4).any(|dy| {
                            (0..4).any(|dx| {
                                bx + dx < mw && by + dy < mh && affected(bx + dx, by + dy)
                            })
                        }) {
                            continue;
                        }
                        let pixels = std::array::from_fn(|i| {
                            image
                                .get_pixel(
                                    (bx + i as u32 % 4).min(mw - 1),
                                    (by + i as u32 / 4).min(mh - 1),
                                )
                                .0
                        });
                        encode_color_block(
                            &mut block[block_size - 8..],
                            &pixels,
                            format == 0x31545844,
                        );
                    }
                    offset += size;
                }
                21 | 22 | 32 => {
                    let size = stride * mh as usize;
                    if offset + size > limit {
                        return Err("Texture mip chain overlaps another allocation".into());
                    }
                    let pixels = r
                        .data
                        .get_mut(offset..offset + size)
                        .ok_or("Truncated raw texture mip chain")?;
                    for y in 0..mh {
                        for x in 0..mw {
                            if affected(x, y) {
                                let c = image.get_pixel(x, y).0;
                                let o = y as usize * stride + x as usize * 4;
                                let bgra = [c[2], c[1], c[0]];
                                pixels[o..o + 3].copy_from_slice(if format == 32 {
                                    &c[..3]
                                } else {
                                    &bgra
                                });
                            }
                        }
                    }
                    offset += size;
                    stride = (stride / 2).max((mw / 2).max(1) as usize * 4);
                }
                _ => {
                    return Err(format!(
                        "{name} uses an unsupported lens texture format ({format:#x})"
                    ));
                }
            }
        }
        changed.insert(name);
    }
    if changed.len() != edits.len() {
        return Err(
            "A lens texture is missing from its original dictionary; reimport the vehicle".into(),
        );
    }
    if detached
        || r.gfx > super::native_edit::STREAMING_LIMIT
        || r.sys > super::native_edit::STREAMING_LIMIT
    {
        let mut builder = super::native_edit::Builder::new(bytes, &r);
        pack_graphics(&mut builder)?;
        return builder.finish(&[0]);
    }
    let mut compressed =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    compressed.write_all(&r.data).map_err(|e| e.to_string())?;
    let mut out = bytes[..16].to_vec();
    out.extend(compressed.finish().map_err(|e| e.to_string())?);
    Ok(out)
}

#[derive(Clone)]
pub(crate) struct PrivateTexture {
    header: Vec<u8>,
    pixels: Vec<u8>,
    name: String,
}
impl PrivateTexture {
    pub(crate) fn lower_mip(&mut self) -> bool {
        let w = u16::from_le_bytes(self.header[0x50..0x52].try_into().unwrap()) as usize;
        let h = u16::from_le_bytes(self.header[0x52..0x54].try_into().unwrap()) as usize;
        let stride = u16::from_le_bytes(self.header[0x56..0x58].try_into().unwrap()) as usize;
        let format = u32::from_le_bytes(self.header[0x58..0x5c].try_into().unwrap());
        if self.header[0x5d] <= 1 || w <= 1 || h <= 1 {
            return false;
        }
        let base = level_bytes(format, w, h, stride).unwrap();
        if base >= self.pixels.len() {
            return false;
        }
        self.pixels.drain(..base);
        self.header[0x50..0x52].copy_from_slice(&((w / 2) as u16).to_le_bytes());
        self.header[0x52..0x54].copy_from_slice(&((h / 2) as u16).to_le_bytes());
        self.header[0x56..0x58].copy_from_slice(&((stride / 2).max(1) as u16).to_le_bytes());
        self.header[0x5d] -= 1;
        true
    }
}
fn level_bytes(format: u32, w: usize, h: usize, stride: usize) -> Result<usize> {
    Ok(match format {
        0x31545844 | 0x31495441 => w.div_ceil(4) * h.div_ceil(4) * 8,
        0x33545844 | 0x35545844 | 0x32495441 | 0x20374342 => w.div_ceil(4) * h.div_ceil(4) * 16,
        21 | 22 | 32 | 25 | 28 | 50 => stride * h,
        _ => {
            return Err(format!(
                "Cannot safely repack native texture format {format:#x}"
            ));
        }
    })
}
fn pixel_bytes(r: &Reader, t: usize) -> Result<usize> {
    let format = r.u32(t + 0x58)?;
    let mut w = r.u16(t + 0x50)? as usize;
    let mut h = r.u16(t + 0x52)? as usize;
    let mut stride = r.u16(t + 0x56)? as usize;
    let levels = r.bytes::<1>(t + 0x5d)?[0].max(1);
    if w == 0 || h == 0 || r.u16(t + 0x54)? != 1 || levels as u32 > 1 + w.max(h).ilog2() {
        return Err("Invalid native texture dimensions or depth".into());
    }
    let mut size = 0;
    for _ in 0..levels {
        size += level_bytes(format, w, h, stride)?;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        stride = (stride / 2).max(1);
    }
    Ok(size)
}
pub(crate) fn private_lenses(
    bytes: &[u8],
    copies: &super::lens_model::Copies,
    colors: &BTreeMap<u8, [u8; 3]>,
) -> Result<Vec<PrivateTexture>> {
    let r = unpack(bytes)?;
    let original = entries(&r)?;
    let needed = copies.keys().cloned().collect();
    let decoded = read_for_export(bytes, &needed)?;
    let mut out = vec![];
    for (name, channels) in copies {
        let source = original
            .iter()
            .find(|(_, n)| n == name)
            .map(|(t, _)| *t)
            .ok_or("Missing source lens texture")?;
        let image = crate::textures::decode(
            &STANDARD
                .decode(decoded.get(name).ok_or("Unsupported lens texture")?)
                .map_err(|e| e.to_string())?,
        )?;
        let size = pixel_bytes(&r, source)?;
        for (id, new_name) in channels {
            let mut image = image.clone();
            for pixel in image.pixels_mut() {
                crate::lens_colors::tint(&mut pixel.0, colors[id]);
            }
            let mask = vec![true; (image.width() * image.height()) as usize];
            let colored = unpack(&recolor(
                bytes,
                &BTreeMap::from([(name.clone(), (image, mask))]),
            )?)?;
            let offset = colored.req(source + 0x70)?;
            let pixels = colored
                .data
                .get(offset..offset + size)
                .ok_or("Truncated private lens pixels")?
                .to_vec();
            let header = r
                .data
                .get(source..source + 144)
                .ok_or("Truncated lens texture header")?
                .to_vec();
            out.push(PrivateTexture {
                header,
                pixels,
                name: new_name.clone(),
            });
        }
    }
    Ok(out)
}

// Best-fit descending across representable power-of-two pages. Compact existing
// pixels byte-for-byte and share identical encoded allocations. Each allocation
// stays within one physical page; no texture crosses separately loaded pages.
fn pack_graphics(b: &mut super::native_edit::Builder) -> Result<()> {
    use super::native_edit::{STREAMING_LIMIT, page_flags};
    use sha2::{Digest, Sha256};
    let r = Reader {
        data: [b.system.as_slice(), b.graphics.as_slice()].concat(),
        sys: b.system.len(),
        gfx: b.graphics.len(),
    };
    let mut chunks: Vec<Vec<u8>> = vec![];
    let mut references = vec![];
    let mut hashes: BTreeMap<Vec<u8>, Vec<usize>> = BTreeMap::new();
    for (t, _) in entries(&r)? {
        let size = pixel_bytes(&r, t)?;
        let at = r.req(t + 0x70)?;
        let bytes = r
            .data
            .get(at..at + size)
            .ok_or("Truncated native texture allocation")?;
        let hash = Sha256::digest(bytes).to_vec();
        let existing = hashes
            .get(&hash)
            .and_then(|ids| ids.iter().find(|i| chunks[**i] == bytes))
            .copied();
        let id = match existing {
            Some(i) => i,
            None => {
                let i = chunks.len();
                chunks.push(bytes.to_vec());
                hashes.entry(hash).or_default().push(i);
                i
            }
        };
        references.push((t, id));
    }
    if chunks.is_empty() {
        b.set_graphics(vec![], vec![])?;
        return Ok(());
    }
    let mut order: Vec<_> = (0..chunks.len()).collect();
    order.sort_by_key(|i| std::cmp::Reverse(chunks[*i].len()));
    let occupied: usize = chunks.iter().map(|c| c.len().next_multiple_of(16)).sum();
    let quantum = 256 * 1024;
    let mut selected = None;
    for budget in (occupied.next_multiple_of(quantum)..=STREAMING_LIMIT).step_by(quantum) {
        let pages: Vec<_> = (0..=26)
            .rev()
            .filter_map(|bit| (budget & (1 << bit) != 0).then_some(1usize << bit))
            .collect();
        if page_flags(&pages, 0).is_err() {
            continue;
        }
        let mut free = pages.clone();
        let mut locations = vec![0; chunks.len()];
        let mut valid = true;
        let mut starts = vec![];
        let mut start = 0;
        for size in &pages {
            starts.push(start);
            start += *size;
        }
        for &id in &order {
            let size = chunks[id].len().next_multiple_of(16);
            let fit = free
                .iter()
                .enumerate()
                .filter(|(_, f)| **f >= size)
                .min_by_key(|(_, f)| **f)
                .map(|(i, _)| i);
            if let Some(i) = fit {
                locations[id] = starts[i] + pages[i] - free[i];
                free[i] -= size;
            } else {
                valid = false;
                break;
            }
        }
        if valid {
            selected = Some((pages, locations));
            break;
        }
    }
    let (pages,locations)=selected.ok_or("Native lens textures cannot fit below FiveM's 100 MB streaming limit. Export fewer lens colors or provide an HD dictionary.")?;
    let mut data = vec![0u8; pages.iter().sum()];
    for (id, bytes) in chunks.iter().enumerate() {
        data[locations[id]..locations[id] + bytes.len()].copy_from_slice(bytes);
    }
    for (t, id) in references {
        b.put(
            t + 0x70,
            &(0x60000000u64 + locations[id] as u64).to_le_bytes(),
        )?;
    }
    b.set_graphics(data, pages)
}
pub(crate) fn add_private(
    bytes: &[u8],
    textures: &[PrivateTexture],
    keep_originals: bool,
) -> Result<Vec<u8>> {
    use super::native_edit::Builder;
    let r = unpack(bytes)?;
    let mut b = Builder::new(bytes, &r);
    let original = entries(&r)?;
    let mut table = vec![];
    if keep_originals {
        let hash_table = r.req(0x20)?;
        if r.u16(0x28)? as usize != original.len() {
            return Err("Invalid native texture hash table".into());
        }
        for (i, (t, _)) in original.iter().enumerate() {
            table.push((r.u32(hash_table + i * 4)?, 0x50000000 + *t as u64));
        }
    }
    for texture in textures {
        let hash = super::shader_hash(&texture.name);
        if table.iter().any(|(h, _)| *h == hash) {
            return Err(
                "A private lens texture already exists. Reimport the original vehicle.".into(),
            );
        }
        let mut header = texture.header.clone();
        let name = b.string(&texture.name)?;
        let pixels = b.gfx(&texture.pixels)?;
        header[0x28..0x30].copy_from_slice(&name.to_le_bytes());
        header[0x70..0x78].copy_from_slice(&pixels.to_le_bytes());
        let header = b.sys(&header)?;
        table.push((hash, header));
    }
    table.sort_by_key(|e| e.0);
    let count = u16::try_from(table.len()).map_err(|_| "Too many private lens textures")?;
    if count > 4096 {
        return Err("Texture dictionary exceeds 4096 entries".into());
    }
    let hashes: Vec<_> = table.iter().flat_map(|e| e.0.to_le_bytes()).collect();
    let pointers: Vec<_> = table.iter().flat_map(|e| e.1.to_le_bytes()).collect();
    let hashes = b.sys(&hashes)?;
    let pointers = b.sys(&pointers)?;
    b.put(0x20, &hashes.to_le_bytes())?;
    b.put(0x30, &pointers.to_le_bytes())?;
    for at in [0x28, 0x2a, 0x38, 0x3a] {
        b.put(at, &count.to_le_bytes())?;
    }
    pack_graphics(&mut b)?;
    b.finish(&[0])
}
pub(crate) fn compact(bytes: &[u8]) -> Result<Vec<u8>> {
    let r = unpack(bytes)?;
    let mut b = super::native_edit::Builder::new(bytes, &r);
    pack_graphics(&mut b)?;
    b.finish(&[0])
}
#[cfg(test)]
fn clone_lenses(
    bytes: &[u8],
    copies: &super::lens_model::Copies,
    colors: &BTreeMap<u8, [u8; 3]>,
) -> Result<Vec<u8>> {
    add_private(bytes, &private_lenses(bytes, copies, colors)?, true)
}

fn encode_color_block(block: &mut [u8], pixels: &[[u8; 4]; 16], bc1: bool) {
    let old0 = u16::from_le_bytes(block[..2].try_into().unwrap());
    let old1 = u16::from_le_bytes(block[2..4].try_into().unwrap());
    let old_indices = u32::from_le_bytes(block[4..8].try_into().unwrap());
    let transparent = |i: usize| bc1 && old0 <= old1 && (old_indices >> (i * 2)) & 3 == 3;
    if (0..16).all(transparent) {
        return;
    }
    // Choose actual colors, rather than a component-wise box diagonal (which
    // incorrectly turns a red/blue block into magenta/black).
    let distance = |a: usize, b: usize| {
        (0..3)
            .map(|c| (pixels[a][c] as i32 - pixels[b][c] as i32).pow(2))
            .sum::<i32>()
    };
    let first = (0..16).find(|i| !transparent(*i)).unwrap();
    let farthest = |a| {
        (0..16)
            .filter(|i| !transparent(*i))
            .max_by_key(|b| distance(a, *b))
            .unwrap()
    };
    let a = farthest(first);
    let b = farthest(a);
    let max = [pixels[a][0], pixels[a][1], pixels[a][2]];
    let min = [pixels[b][0], pixels[b][1], pixels[b][2]];
    let pack = |p: [u8; 3]| {
        ((p[0] as u16 * 31 + 127) / 255) << 11
            | ((p[1] as u16 * 63 + 127) / 255) << 5
            | ((p[2] as u16 * 31 + 127) / 255)
    };
    let unpack = |p: u16| {
        [
            ((p >> 11) * 255 / 31) as u8,
            (((p >> 5) & 63) * 255 / 63) as u8,
            ((p & 31) * 255 / 31) as u8,
        ]
    };
    let alpha = (0..16).any(transparent);
    let (mut a, mut b) = (pack(max), pack(min));
    if (alpha && a > b) || (!alpha && a < b) {
        std::mem::swap(&mut a, &mut b);
    }
    if !alpha && a == b {
        if a < u16::MAX {
            a += 1;
        } else {
            b -= 1;
        }
    }
    let (p, q) = (unpack(a), unpack(b));
    let mut palette = [p, q, [0; 3], [0; 3]];
    for c in 0..3 {
        palette[2][c] = if alpha {
            ((p[c] as u16 + q[c] as u16) / 2) as u8
        } else {
            ((2 * p[c] as u16 + q[c] as u16) / 3) as u8
        };
        palette[3][c] = ((p[c] as u16 + 2 * q[c] as u16) / 3) as u8;
    }
    let mut indices = 0u32;
    for (i, p) in pixels.iter().enumerate() {
        let index = if transparent(i) {
            3
        } else {
            (0..if alpha { 3 } else { 4 })
                .min_by_key(|&n| {
                    (0..3)
                        .map(|c| (p[c] as i32 - palette[n][c] as i32).pow(2))
                        .sum::<i32>()
                })
                .unwrap()
        };
        indices |= (index as u32) << (i * 2);
    }
    block[..2].copy_from_slice(&a.to_le_bytes());
    block[2..4].copy_from_slice(&b.to_le_bytes());
    block[4..8].copy_from_slice(&indices.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    fn resource(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut out = b"RSC7".to_vec();
        for v in [13u32, 1 << 27, 1 << 27] {
            out.extend(v.to_le_bytes());
        }
        let mut encoder = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::fast());
        encoder.write_all(data).unwrap();
        out.extend(encoder.finish().unwrap());
        out
    }
    fn fixture(format: u32, levels: u8) -> Vec<u8> {
        let mut d = vec![0u8; 1024];
        for (o, v) in [
            (0x30, 0x50000080u64),
            (0x80, 0x50000100),
            (0x128, 0x500001a0),
            (0x170, 0x60000000),
        ] {
            d[o..o + 8].copy_from_slice(&v.to_le_bytes());
        }
        d[0x38..0x3a].copy_from_slice(&1u16.to_le_bytes());
        d[0x1a0..0x1a5].copy_from_slice(b"lens\0");
        d[0x150..0x152].copy_from_slice(&8u16.to_le_bytes());
        d[0x152..0x154].copy_from_slice(&8u16.to_le_bytes());
        d[0x156..0x158].copy_from_slice(&36u16.to_le_bytes());
        d[0x158..0x15c].copy_from_slice(&format.to_le_bytes());
        d[0x15d] = levels;
        d
    }
    #[test]
    fn private_color_copies_keep_original_texture_storage_and_alpha() {
        let mut raw = fixture(21, 1);
        raw[0x20..0x28].copy_from_slice(&0x500001b0u64.to_le_bytes());
        raw[0x28..0x2a].copy_from_slice(&1u16.to_le_bytes());
        raw[0x1b0..0x1b4].copy_from_slice(&super::super::shader_hash("lens").to_le_bytes());
        raw[0x154..0x156].copy_from_slice(&1u16.to_le_bytes());
        for y in 0..8 {
            for x in 0..8 {
                raw[512 + y * 36 + x * 4..512 + y * 36 + x * 4 + 4].copy_from_slice(&[
                    10,
                    20,
                    100,
                    (x + y) as u8,
                ]);
            }
        }
        let original = resource(&raw);
        let copies = BTreeMap::from([(
            "lens".into(),
            BTreeMap::from([(7, "private_green".into()), (8, "private_blue".into())]),
        )]);
        let output = clone_lenses(
            &original,
            &copies,
            &BTreeMap::from([(7, [0, 255, 0]), (8, [0, 0, 255])]),
        )
        .unwrap();
        let after = unpack(&output).unwrap();
        assert!(after.gfx <= super::super::native_edit::STREAMING_LIMIT);
        assert_eq!(after.u16(0x38).unwrap(), 3);
        let decoded = read_for_export(
            &output,
            &BTreeSet::from(["lens".into(), "private_green".into(), "private_blue".into()]),
        )
        .unwrap();
        let decode = |s: &str| crate::textures::decode(&STANDARD.decode(s).unwrap()).unwrap();
        let before = decode(&decoded["lens"]);
        let green = decode(&decoded["private_green"]);
        let blue = decode(&decoded["private_blue"]);
        let shared = clone_lenses(
            &original,
            &copies,
            &BTreeMap::from([(7, [0, 255, 0]), (8, [0, 255, 0])]),
        )
        .unwrap();
        let edit = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 100, 0]));
        let detached = recolor(
            &shared,
            &BTreeMap::from([("private_green".into(), (edit, vec![true; 64]))]),
        )
        .unwrap();
        let decoded_detached = read_for_export(
            &detached,
            &BTreeSet::from(["private_green".into(), "private_blue".into()]),
        )
        .unwrap();
        let independent = decode(&decoded_detached["private_green"]);
        let unchanged = decode(&decoded_detached["private_blue"]);
        assert_eq!(independent.get_pixel(2, 2).0, [0, 0, 100, 4]);
        assert_eq!(unchanged.get_pixel(2, 2).0, [0, 100, 0, 4]);
        for ((a, g), b) in before.pixels().zip(green.pixels()).zip(blue.pixels()) {
            assert_eq!(a[3], g[3]);
            assert_eq!(a[3], b[3]);
            assert_eq!([g[0], g[2]], [0, 0]);
            assert_eq!([b[0], b[1]], [0, 0]);
        }
    }
    #[test]
    #[ignore = "Explicit local crash-patch regression; no user assets are bundled"]
    fn reported_archive_repair_keeps_all_encoded_pixels_and_user_metadata() {
        use std::io::{Cursor, Read};
        let path = std::env::var_os("DUCK_WORKSHOP_TEST_CRASH_PATCH")
            .expect("Set the crash-producing ZIP");
        let source = std::fs::read(path).unwrap();
        let output = crate::models::lens_model::repair_patch(&source).unwrap();
        let mut before = zip::ZipArchive::new(Cursor::new(&source)).unwrap();
        let mut after = zip::ZipArchive::new(Cursor::new(&output)).unwrap();
        assert_eq!(before.len(), after.len());
        let mut count = 0;
        for i in 0..before.len() {
            let mut original = before.by_index(i).unwrap();
            let name = original.name().to_string();
            let mut b = vec![];
            original.read_to_end(&mut b).unwrap();
            let mut a = vec![];
            after.by_name(&name).unwrap().read_to_end(&mut a).unwrap();
            if !name.ends_with(".ytd") {
                assert_eq!(a, b, "Non-texture patch member changed: {name}");
                continue;
            }
            let old = unpack(&b).unwrap();
            let new = unpack(&a).unwrap();
            println!("Repaired {name}: {} -> {} graphics bytes", old.gfx, new.gfx);
            assert!(new.gfx <= 100_000_000);
            assert!(new.sys <= 100_000_000);
            let flags = u32::from_le_bytes(a[12..16].try_into().unwrap());
            let mut page_ends = vec![];
            let mut end = 0;
            for (i, (shift, mask)) in [4, 5, 7, 11, 17, 24, 25, 26, 27]
                .into_iter()
                .zip([1, 3, 15, 63, 127, 1, 1, 1, 1])
                .enumerate()
            {
                for _ in 0..((flags >> shift) & mask) {
                    end += (512usize << (flags & 15)) << (8 - i);
                    page_ends.push(end);
                }
            }
            let page_info = new.req(8).unwrap();
            assert_eq!(new.data[page_info + 9] as usize, page_ends.len());
            for (texture, _) in entries(&new).unwrap() {
                let start = new.req(texture + 0x70).unwrap() - new.sys;
                let size = pixel_bytes(&new, texture).unwrap();
                assert!(
                    start + size <= *page_ends.iter().find(|end| **end > start).unwrap(),
                    "A texture crosses physical pages"
                );
            }
            let old_entries = entries(&old).unwrap();
            let new_entries = entries(&new).unwrap();
            assert_eq!(old_entries.len(), new_entries.len());
            for (t, name) in old_entries {
                let nt = new_entries.iter().find(|(_, n)| *n == name).unwrap().0;
                let size = pixel_bytes(&old, t).unwrap();
                assert_eq!(size, pixel_bytes(&new, nt).unwrap());
                let op = old.req(t + 0x70).unwrap();
                let np = new.req(nt + 0x70).unwrap();
                assert_eq!(
                    &old.data[op..op + size],
                    &new.data[np..np + size],
                    "Encoded pixels or mip alpha changed: {name}"
                );
                assert_eq!(
                    &old.data[t + 0x50..t + 0x70],
                    &new.data[nt + 0x50..nt + 0x70],
                    "Texture format/dimensions changed: {name}"
                );
                count += 1;
            }
        }
        assert!(count > 0);
        println!(
            "Validated {count} complete native texture/mip allocations; model files and metadata identical"
        );
    }
    #[test]
    fn hd_fallback_uses_existing_encoded_mips_without_resampling() {
        let mut header = vec![0u8; 144];
        header[0x50..0x52].copy_from_slice(&8u16.to_le_bytes());
        header[0x52..0x54].copy_from_slice(&8u16.to_le_bytes());
        header[0x56..0x58].copy_from_slice(&8u16.to_le_bytes());
        header[0x58..0x5c].copy_from_slice(&0x35545844u32.to_le_bytes());
        header[0x5d] = 2;
        let mut pixels = vec![41; 64];
        pixels.extend([2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32]);
        let mut texture = PrivateTexture {
            header,
            pixels: pixels.clone(),
            name: "lens".into(),
        };
        assert!(texture.lower_mip());
        assert_eq!(&texture.pixels, &pixels[64..]);
        assert_eq!(
            u16::from_le_bytes(texture.header[0x50..0x52].try_into().unwrap()),
            4
        );
        assert_eq!(texture.header[0x5d], 1);
        assert!(!texture.lower_mip());
    }
    fn source_model() -> Model {
        Model {
            material_info: BTreeMap::from([(
                "lens-material".into(),
                MaterialInfo {
                    shader: "vehicle_mesh".into(),
                    diffuse_texture: "lens".into(),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        }
    }
    #[test]
    fn export_sources_are_recorded_even_when_preview_cache_is_full_or_already_loaded() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vehicle.ytd");
        std::fs::write(&path, resource(&fixture(0x35545844, 1))).unwrap();
        let mut model = source_model();
        model
            .preview_textures
            .insert("body".into(), "x".repeat(48 * 1024 * 1024));
        load_paths(std::slice::from_ref(&path), &mut model);
        assert_eq!(model.texture_sources["lens"], path);
        assert!(!model.preview_textures.contains_key("lens"));
        model.preview_textures.clear();
        model
            .preview_textures
            .insert("lens".into(), "cached image".into());
        model.texture_sources.clear();
        load_paths(std::slice::from_ref(&path), &mut model);
        assert_eq!(model.texture_sources["lens"], path);
        assert_eq!(model.preview_textures["lens"], "cached image");
    }
    #[test]
    fn emergency_lens_preview_takes_priority_without_dropping_body_export_sources() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vehicle.ytd");
        std::fs::write(&path, resource(&fixture(0x35545844, 1))).unwrap();
        let mut model = source_model();
        model.material_info.get_mut("lens-material").unwrap().shader =
            "vehicle_lightsemissive".into();
        model
            .preview_textures
            .insert("body".into(), "x".repeat(48 * 1024 * 1024));
        model.texture_sources.insert("body".into(), path.clone());
        load_paths(std::slice::from_ref(&path), &mut model);
        assert!(model.preview_textures.contains_key("lens"));
        assert!(!model.preview_textures.contains_key("body"));
        assert!(model.texture_sources.contains_key("body"));
        assert_eq!(model.texture_sources["lens"], path);
    }
    #[test]
    fn native_bc_mips_alpha_metadata_and_untouched_blocks_survive_recoloring() {
        for format in [0x31545844u32, 0x33545844, 0x35545844] {
            let mut d = fixture(format, 2);
            let size = if format == 0x31545844 { 8 } else { 16 };
            for block in d[512..512 + size * 5].chunks_exact_mut(size) {
                if size == 16 {
                    block[..8].copy_from_slice(&[255, 8, 1, 2, 3, 4, 5, 6]);
                }
                block[size - 8..size - 6].copy_from_slice(&0xf800u16.to_le_bytes());
            }
            let encoded = resource(&d);
            let mut image = crate::textures::decode(
                &STANDARD
                    .decode(&read(&encoded, &BTreeSet::from(["lens".into()])).unwrap()["lens"])
                    .unwrap(),
            )
            .unwrap();
            let mut mask = vec![false; 64];
            for y in 0..4 {
                for x in 0..4 {
                    image.get_pixel_mut(x, y).0[..3].copy_from_slice(&[0, 0, 255]);
                    mask[(y * 8 + x) as usize] = true;
                }
            }
            let patched =
                recolor(&encoded, &BTreeMap::from([("lens".into(), (image, mask))])).unwrap();
            let after = unpack(&patched).unwrap();
            assert_eq!(&patched[..16], &encoded[..16]);
            assert_eq!(&after.data[..512], &d[..512]);
            assert_eq!(
                &after.data[512 + size..512 + size * 4],
                &d[512 + size..512 + size * 4]
            );
            if size == 16 {
                assert_eq!(&after.data[512..520], &d[512..520]);
                assert_eq!(
                    &after.data[512 + size * 4..520 + size * 4],
                    &d[512 + size * 4..520 + size * 4]
                );
            }
            let decoded = read(&patched, &BTreeSet::from(["lens".into()])).unwrap();
            let image =
                crate::textures::decode(&STANDARD.decode(&decoded["lens"]).unwrap()).unwrap();
            assert!(image.get_pixel(0, 0)[2] > 240);
            assert!(image.get_pixel(0, 0)[0] < 10);
            assert_ne!(
                &after.data[512 + size * 4..512 + size * 5],
                &d[512 + size * 4..512 + size * 5],
                "Mip was not updated"
            );
        }
    }
    #[test]
    fn bc1_transparent_indices_remain_transparent_and_mixed_hues_stay_distinct() {
        let mut block = [0u8; 8];
        block[2..4].copy_from_slice(&0xf800u16.to_le_bytes());
        block[4..8].copy_from_slice(&3u32.to_le_bytes());
        let mut pixels = [[0, 0, 255, 255]; 16];
        pixels[0] = [0, 0, 0, 0];
        encode_color_block(&mut block, &pixels, true);
        assert!(
            u16::from_le_bytes(block[..2].try_into().unwrap())
                <= u16::from_le_bytes(block[2..4].try_into().unwrap())
        );
        assert_eq!(block[4] & 3, 3);
        let mut block = [0u8; 8];
        for p in &mut pixels[..8] {
            *p = [255, 0, 0, 255];
        }
        encode_color_block(&mut block, &pixels, false);
        let endpoints = [
            u16::from_le_bytes(block[..2].try_into().unwrap()),
            u16::from_le_bytes(block[2..4].try_into().unwrap()),
        ];
        assert!(endpoints.contains(&0xf800));
        assert!(endpoints.contains(&0x001f));
    }
    #[test]
    fn raw_formats_keep_alpha_padding_and_bounds_are_checked() {
        for format in [21, 22, 32] {
            let mut d = fixture(format, 1);
            d[512..].fill(77);
            let encoded = resource(&d);
            let image = image::RgbaImage::from_pixel(8, 8, image::Rgba([1, 2, 3, 99]));
            let mut mask = vec![false; 64];
            mask[0] = true;
            let after = unpack(
                &recolor(
                    &encoded,
                    &BTreeMap::from([("lens".into(), (image.clone(), mask.clone()))]),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(
                &after.data[512..515],
                if format == 32 { &[1, 2, 3] } else { &[3, 2, 1] }
            );
            assert_eq!(&after.data[515..], &d[515..]);
            d[0x15d] = 99;
            assert!(
                recolor(
                    &resource(&d),
                    &BTreeMap::from([("lens".into(), (image, mask))])
                )
                .is_err()
            );
        }
    }
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
