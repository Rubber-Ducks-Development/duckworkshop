//! Append-only legacy RSC7 builder. Existing virtual offsets stay stable.
use super::*;
use std::io::Write;

pub(super) fn unpack(bytes: &[u8]) -> Result<Reader> {
    if bytes.len() < 16 || &bytes[..4] != b"RSC7" {
        return Err("Expected a native RSC7 resource".into());
    }
    let sys = resource_size(u32::from_le_bytes(bytes[8..12].try_into().unwrap()));
    let gfx = resource_size(u32::from_le_bytes(bytes[12..16].try_into().unwrap()));
    if sys < 16 || sys + gfx > 256 * 1024 * 1024 {
        return Err("Invalid native resource size".into());
    }
    let mut data = vec![];
    flate2::read::DeflateDecoder::new(&bytes[16..])
        .take((sys + gfx + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() != sys + gfx {
        return Err("Truncated native resource".into());
    }
    Ok(Reader { data, sys, gfx })
}
pub(super) struct Builder {
    pub system: Vec<u8>,
    pub graphics: Vec<u8>,
    header: Vec<u8>,
    graphics_pages: Option<Vec<usize>>,
}
impl Builder {
    pub fn new(bytes: &[u8], r: &Reader) -> Self {
        Self {
            system: r.data[..r.sys].to_vec(),
            graphics: r.data[r.sys..].to_vec(),
            header: bytes[..16].to_vec(),
            graphics_pages: None,
        }
    }
    fn alloc(data: &mut Vec<u8>, bytes: &[u8], base: u64) -> Result<u64> {
        let offset = data.len().next_multiple_of(16);
        let end = offset
            .checked_add(bytes.len())
            .ok_or("Native allocation overflow")?;
        if end > 256 * 1024 * 1024 {
            return Err("Edited resource exceeds 256 MB".into());
        }
        data.resize(offset, 0);
        data.extend_from_slice(bytes);
        Ok(base + offset as u64)
    }
    pub fn sys(&mut self, bytes: &[u8]) -> Result<u64> {
        Self::alloc(&mut self.system, bytes, 0x50000000)
    }
    pub fn gfx(&mut self, bytes: &[u8]) -> Result<u64> {
        Self::alloc(&mut self.graphics, bytes, 0x60000000)
    }
    pub fn string(&mut self, value: &str) -> Result<u64> {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        self.sys(&bytes)
    }
    pub fn put(&mut self, offset: usize, bytes: &[u8]) -> Result<()> {
        let out = self
            .system
            .get_mut(
                offset
                    ..offset
                        .checked_add(bytes.len())
                        .ok_or("Native write overflow")?,
            )
            .ok_or("Native write outside the system resource")?;
        out.copy_from_slice(bytes);
        Ok(())
    }
    pub fn set_graphics(&mut self, data: Vec<u8>, pages: Vec<usize>) -> Result<()> {
        if data.len() != pages.iter().sum::<usize>() {
            return Err("Invalid packed graphics size".into());
        }
        self.graphics = data;
        self.graphics_pages = Some(pages);
        Ok(())
    }
    pub fn finish(mut self, page_roots: &[usize]) -> Result<Vec<u8>> {
        // Reserve enough space before calculating either segment's final flags.
        // RAGE/FiveM checks streamed segment sizes as well as page metadata.
        let pages = self.sys(&[0u8; 16 + 8 * 128])?;
        for root in page_roots {
            self.put(root + 8, &pages.to_le_bytes())?;
        }
        let single = |len: usize| -> Result<Vec<usize>> {
            if len == 0 {
                return Ok(vec![]);
            }
            let size = len.max(512).next_power_of_two();
            if size > 64 * 1024 * 1024 {
                return Err(
                    "Native segment needs repacking to stay below FiveM's 100 MB streaming limit"
                        .into(),
                );
            }
            Ok(vec![size])
        };
        let sys_pages = single(self.system.len())?;
        let gfx_pages = match self.graphics_pages.take() {
            Some(p) => p,
            None => single(self.graphics.len())?,
        };
        let old_sys = u32::from_le_bytes(self.header[8..12].try_into().unwrap());
        let old_gfx = u32::from_le_bytes(self.header[12..16].try_into().unwrap());
        let sflags = page_flags(&sys_pages, old_sys)?;
        let gflags = page_flags(&gfx_pages, old_gfx)?;
        validate_streaming_flags(sflags, gflags)?;
        if sys_pages.len() + gfx_pages.len() > 128 {
            return Err("Native resource exceeds 128 streaming pages".into());
        }
        let offset = (pages - 0x50000000) as usize;
        self.system[offset + 8] = sys_pages.len() as u8;
        self.system[offset + 9] = gfx_pages.len() as u8;
        self.system.resize(resource_size(sflags), 0);
        self.graphics.resize(resource_size(gflags), 0);
        self.header[8..12].copy_from_slice(&sflags.to_le_bytes());
        self.header[12..16].copy_from_slice(&gflags.to_le_bytes());
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&self.system).map_err(|e| e.to_string())?;
        encoder
            .write_all(&self.graphics)
            .map_err(|e| e.to_string())?;
        self.header
            .extend(encoder.finish().map_err(|e| e.to_string())?);
        Ok(self.header)
    }
}
pub(super) const STREAMING_LIMIT: usize = 100_000_000;
pub(super) fn page_flags(pages: &[usize], old: u32) -> Result<u32> {
    const SHIFTS: [u32; 9] = [4, 5, 7, 11, 17, 24, 25, 26, 27];
    const LIMITS: [usize; 9] = [1, 3, 15, 63, 127, 1, 1, 1, 1];
    for base in 0..=15 {
        let mut counts = [0usize; 9];
        let mut valid = true;
        for size in pages {
            if let Some(i) = (0..9).find(|i| ((512usize << base) << (8 - i)) == *size) {
                counts[i] += 1;
            } else {
                valid = false;
                break;
            }
        }
        if valid && counts.iter().zip(LIMITS).all(|(n, max)| *n <= max) {
            let mut flags = (old & 0xf0000000) | base;
            for (count, shift) in counts.into_iter().zip(SHIFTS) {
                flags |= (count as u32) << shift;
            }
            if resource_size(flags) == pages.iter().sum::<usize>() {
                return Ok(flags);
            }
        }
    }
    Err("Native page layout cannot be represented in RSC7 flags".into())
}
pub(crate) fn validate_streaming_flags(system: u32, graphics: u32) -> Result<()> {
    for (name, flags) in [("system", system), ("graphics", graphics)] {
        let size = resource_size(flags);
        if size > STREAMING_LIMIT {
            return Err(format!(
                "The {name} segment would stream {:.2} MB; FiveM supports at most 100 MB. Reduce the edited texture set or provide an HD dictionary.",
                size as f64 / 1_000_000.
            ));
        }
        let base = 512usize << (flags & 15);
        for (i, (shift, mask)) in [4, 5, 7, 11, 17, 24, 25, 26, 27]
            .into_iter()
            .zip([1, 3, 15, 63, 127, 1, 1, 1, 1])
            .enumerate()
        {
            if ((flags >> shift) & mask) != 0 && (base << (8 - i)) > STREAMING_LIMIT {
                return Err("A native streaming page exceeds FiveM's 100 MB limit".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_the_reported_134_mb_streaming_chunk_even_with_small_pages() {
        assert!(validate_streaming_flags(0, 0xd000001a).is_err());
        let split = page_flags(&[64 * 1024 * 1024, 64 * 1024 * 1024], 0xd0000000).unwrap();
        assert!(validate_streaming_flags(0, split).is_err());
        let legal = page_flags(
            &[
                64 * 1024 * 1024,
                16 * 1024 * 1024,
                8 * 1024 * 1024,
                4 * 1024 * 1024,
                2 * 1024 * 1024,
                512 * 1024,
                256 * 1024,
            ],
            0xd0000000,
        )
        .unwrap();
        assert_eq!(resource_size(legal), 99_352_576);
        assert!(validate_streaming_flags(0, legal).is_ok());
    }
    #[test]
    fn appended_system_and_graphics_keep_old_virtual_offsets_and_version() {
        let mut data = vec![0u8; 1024];
        data[16] = 91;
        data[512] = 37;
        let mut bytes = b"RSC7".to_vec();
        for n in [162u32, 0xa8000000, 0x28000000] {
            bytes.extend(n.to_le_bytes());
        }
        let mut encoder = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::fast());
        encoder.write_all(&data).unwrap();
        bytes.extend(encoder.finish().unwrap());
        let r = unpack(&bytes).unwrap();
        let mut b = Builder::new(&bytes, &r);
        let s = b.sys(&[2; 64]).unwrap();
        let g = b.gfx(&[3; 64]).unwrap();
        let out = b.finish(&[0]).unwrap();
        let r = unpack(&out).unwrap();
        assert_eq!(r.data[16], 91);
        assert_eq!(r.data[r.sys], 37);
        assert_eq!(r.data[(s - 0x50000000) as usize], 2);
        assert_eq!(r.data[r.sys + (g - 0x60000000) as usize], 3);
        assert_eq!(&out[4..8], &bytes[4..8]);
        assert_eq!(u32::from_le_bytes(out[8..12].try_into().unwrap()) >> 28, 10);
        let info = r.req(8).unwrap();
        assert_eq!(r.data[info + 8..info + 10], [1, 1]);
    }
}
