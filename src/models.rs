use crate::{Result, xml_document};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Mesh {
    pub name: String,
    pub material: String,
    pub lod: String,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<Vec<[f32; 2]>>,
    pub indices: Vec<u32>,
    pub enabled: bool,
}
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Model {
    pub name: String,
    pub meshes: Vec<Mesh>,
    pub warnings: Vec<String>,
}
impl Model {
    pub fn validate(&self) -> Result<()> {
        if self.meshes.is_empty() {
            return Err("No triangle geometry found.".into());
        }
        let mut count = 0;
        for m in &self.meshes {
            count += m.indices.len();
            if m.positions.is_empty()
                || m.positions.iter().flatten().any(|v| !v.is_finite())
                || m.indices.len() % 3 != 0
                || m.indices.iter().any(|i| *i as usize >= m.positions.len())
            {
                return Err("Invalid triangle positions or indices.".into());
            }
            for uv in &m.uvs {
                if !uv.is_empty()
                    && (uv.len() != m.positions.len()
                        || uv.iter().flatten().any(|v| !v.is_finite()))
                {
                    return Err("Invalid or incomplete UV coordinates.".into());
                }
            }
        }
        if count > 6_000_000 {
            return Err("Model exceeds two million triangles.".into());
        }
        if !self
            .meshes
            .iter()
            .any(|m| m.uvs.iter().any(|uv| !uv.is_empty()))
        {
            return Err("No UV coordinates. Map this model in Blender first; UV Editor recovers existing UVs.".into());
        }
        Ok(())
    }
    pub fn triangles(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }
}
pub fn load(path: &Path) -> Result<Model> {
    let data = crate::read_file(path)?;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let mut model =
        match ext.as_str() {
            "obj" => parse_obj(std::str::from_utf8(&data).map_err(|_| "OBJ must be UTF-8 text")?)?,
            "xml" => parse_xml(std::str::from_utf8(&data).map_err(|_| "XML must be UTF-8 text")?)?,
            "yft" | "ydr" => parse_rsc7(&data, &ext)?,
            "glb" => parse_glb(&data)?,
            "json" => serde_json::from_slice::<Model>(&data).map_err(|e| e.to_string())?,
            _ => return Err(
                "Supported models: YFT, YDR, CodeWalker XML, OBJ, GLB or a saved UV JSON project."
                    .into(),
            ),
        };
    model.validate()?;
    model.name = filename;
    Ok(model)
}
fn mesh(name: &str, material: &str) -> Mesh {
    Mesh {
        name: name.into(),
        material: material.into(),
        lod: "High".into(),
        positions: vec![],
        uvs: vec![],
        indices: vec![],
        enabled: true,
    }
}
pub fn parse_obj(text: &str) -> Result<Model> {
    let mut positions = Vec::new();
    let mut coords = Vec::new();
    let mut out = Model::default();
    let mut name = "Object".to_string();
    let mut material = "Default".to_string();
    let mut current = None;
    fn idx(raw: &str, n: usize) -> Result<usize> {
        let i = raw.parse::<i64>().map_err(|_| "Invalid OBJ index")?;
        let at = if i > 0 { i - 1 } else { n as i64 + i };
        if at < 0 || at >= n as i64 {
            return Err("OBJ index out of range.".into());
        }
        Ok(at as usize)
    }
    for line in text.lines() {
        let parts: Vec<_> = line
            .split('#')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "v" => {
                if parts.len() < 4 {
                    return Err("Invalid OBJ vertex.".into());
                }
                positions.push([number(parts[1])?, number(parts[2])?, number(parts[3])?]);
            }
            "vt" => {
                if parts.len() < 3 {
                    return Err("Invalid OBJ UV.".into());
                }
                coords.push([number(parts[1])?, number(parts[2])?]);
            }
            "o" | "g" => {
                name = parts[1..].join(" ");
                current = None;
            }
            "usemtl" => {
                material = parts[1..].join(" ");
                current = None;
            }
            "f" => {
                if parts.len() < 4 {
                    continue;
                }
                let mi = *current.get_or_insert_with(|| {
                    out.meshes.push(mesh(&name, &material));
                    out.meshes.len() - 1
                });
                for t in 2..parts.len() - 1 {
                    for raw in [parts[1], parts[t], parts[t + 1]] {
                        let p: Vec<_> = raw.split('/').collect();
                        let v = positions[idx(p[0], positions.len())?];
                        let uv = if p.len() > 1 && !p[1].is_empty() {
                            Some(coords[idx(p[1], coords.len())?])
                        } else {
                            None
                        };
                        let m = &mut out.meshes[mi];
                        m.indices.push(m.positions.len() as u32);
                        m.positions.push(v);
                        if m.uvs.is_empty() {
                            m.uvs.push(Vec::new());
                        }
                        m.uvs[0].push(uv.unwrap_or([f32::NAN; 2]));
                    }
                }
            }
            _ => {}
        }
    }
    for m in &mut out.meshes {
        if m.uvs
            .first()
            .is_some_and(|uv| uv.iter().flatten().all(|v| v.is_nan()))
        {
            m.uvs.clear();
        }
    }
    out.validate()?;
    Ok(out)
}
fn number(s: &str) -> Result<f32> {
    s.parse::<f32>().map_err(|_| format!("Invalid number: {s}"))
}
pub fn parse_xml(text: &str) -> Result<Model> {
    let doc = xml_document(text)?;
    let mut out = Model::default();
    for vb in doc.descendants().filter(|n| n.has_tag_name("VertexBuffer")) {
        let Some(g) = vb.parent() else { continue };
        let layout = vb
            .children()
            .find(|n| n.has_tag_name("Layout"))
            .ok_or("XML VertexBuffer missing Layout")?;
        let mut offsets = std::collections::HashMap::new();
        let mut width = 0;
        for c in layout.children().filter(|n| n.is_element()) {
            let name = c.tag_name().name();
            offsets.insert(name, width);
            width += if name.starts_with("TexCoord") {
                2
            } else if name.starts_with("Colour")
                || ["BlendWeights", "BlendIndices", "Tangent"].contains(&name)
            {
                4
            } else {
                3
            };
        }
        let Some(&p) = offsets.get("Position") else {
            continue;
        };
        let data = vb
            .children()
            .find(|n| n.has_tag_name("Data"))
            .or_else(|| vb.children().find(|n| n.has_tag_name("Data2")))
            .and_then(|n| n.text())
            .ok_or("Missing XML vertex data")?;
        let shader = g
            .children()
            .find(|n| n.has_tag_name("ShaderIndex"))
            .and_then(|n| n.attribute("value"))
            .unwrap_or("0");
        let mut m = mesh(
            &format!("Geometry {}", out.meshes.len() + 1),
            &format!("Shader {shader}"),
        );
        for a in g.ancestors() {
            if a.is_element() {
                let n = a.tag_name().name();
                if let Some(l) = n.strip_prefix("DrawableModels") {
                    m.lod = l.replace("Medium", "Med");
                    break;
                }
            }
        }
        for row in data.lines().filter(|l| !l.trim().is_empty()) {
            let nums = row
                .split_whitespace()
                .map(number)
                .collect::<Result<Vec<_>>>()?;
            if nums.len() != width {
                return Err("XML vertex row does not match Layout.".into());
            }
            m.positions.push([nums[p], nums[p + 1], nums[p + 2]]);
            for c in 0..8 {
                if let Some(&u) = offsets.get(format!("TexCoord{c}").as_str()) {
                    while m.uvs.len() <= c {
                        m.uvs.push(vec![])
                    }
                    m.uvs[c].push([nums[u], nums[u + 1]]);
                }
            }
        }
        let indices = g
            .children()
            .find(|n| n.has_tag_name("IndexBuffer"))
            .and_then(|n| n.children().find(|c| c.has_tag_name("Data")))
            .and_then(|n| n.text())
            .unwrap_or("");
        m.indices = indices
            .split_whitespace()
            .map(|s| s.parse::<u32>().map_err(|_| "Invalid XML index".into()))
            .collect::<Result<Vec<_>>>()?;
        if !m.indices.is_empty() {
            out.meshes.push(m);
        }
    }
    out.warnings.push("CodeWalker XML preview uses raw mesh positions; bone and fragment transforms are not applied.".into());
    out.validate()?;
    Ok(out)
}
pub fn parse_glb(bytes: &[u8]) -> Result<Model> {
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let blob = gltf
        .blob
        .as_ref()
        .ok_or("GLB must contain an embedded binary buffer")?;
    let mut out = Model::default();
    for b in gltf.document.buffers() {
        if !matches!(b.source(), gltf::buffer::Source::Bin) {
            return Err("External buffers are unsupported. Export a self-contained GLB.".into());
        }
    }
    fn visit(node: gltf::Node<'_>, parent: glam::Mat4, blob: &[u8], out: &mut Model) -> Result<()> {
        let transform = parent * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
        if let Some(source) = node.mesh() {
            for primitive in source.primitives() {
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    continue;
                }
                let reader = primitive.reader(|_| Some(blob));
                let mut m = mesh(
                    source.name().unwrap_or("Mesh"),
                    primitive.material().name().unwrap_or("Default"),
                );
                m.positions = reader
                    .read_positions()
                    .ok_or("Missing GLB positions")?
                    .map(|p| transform.transform_point3(glam::Vec3::from(p)).to_array())
                    .collect();
                for c in 0..8 {
                    if let Some(uv) = reader.read_tex_coords(c) {
                        while m.uvs.len() <= c as usize {
                            m.uvs.push(vec![]);
                        }
                        m.uvs[c as usize] = uv.into_f32().collect();
                    }
                }
                m.indices = reader
                    .read_indices()
                    .map(|i| i.into_u32().collect())
                    .unwrap_or_else(|| (0..m.positions.len() as u32).collect());
                out.meshes.push(m);
            }
        }
        for c in node.children() {
            visit(c, transform, blob, out)?
        }
        Ok(())
    }
    let scene = gltf
        .default_scene()
        .or_else(|| gltf.scenes().next())
        .ok_or("GLB has no scene")?;
    for n in scene.nodes() {
        visit(n, glam::Mat4::IDENTITY, blob, &mut out)?
    }
    out.validate()?;
    Ok(out)
}
pub fn resource_size(flags: u32) -> usize {
    let shifts = [4, 5, 7, 11, 17, 24, 25, 26, 27];
    let masks = [1, 3, 15, 63, 127, 1, 1, 1, 1];
    (512usize << (flags & 15))
        * shifts
            .iter()
            .enumerate()
            .map(|(i, s)| (((flags >> s) & masks[i]) as usize) << (8 - i))
            .sum::<usize>()
}
pub fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = (bits >> 10) & 31;
    let mantissa = (bits & 1023) as f32;
    sign * if exponent == 0 {
        2f32.powi(-14) * mantissa / 1024.0
    } else if exponent == 31 {
        if mantissa == 0.0 {
            f32::INFINITY
        } else {
            f32::NAN
        }
    } else {
        2f32.powi(exponent as i32 - 15) * (1.0 + mantissa / 1024.0)
    }
}
struct Reader {
    data: Vec<u8>,
    sys: usize,
    gfx: usize,
}
impl Reader {
    fn bytes<const N: usize>(&self, o: usize) -> Result<[u8; N]> {
        self.data
            .get(o..o.checked_add(N).ok_or("Buffer overflow")?)
            .ok_or("Model points outside its buffers")?
            .try_into()
            .map_err(|_| "Truncated model".into())
    }
    fn u16(&self, o: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(o)?))
    }
    fn u32(&self, o: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(o)?))
    }
    fn u64(&self, o: usize) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(o)?))
    }
    fn f32(&self, o: usize) -> Result<f32> {
        Ok(f32::from_le_bytes(self.bytes(o)?))
    }
    fn ptr(&self, o: usize) -> Result<Option<usize>> {
        let p = self.u64(o)?;
        if p == 0 {
            return Ok(None);
        }
        let x = if (0x50000000..0x60000000).contains(&p) {
            let x = (p - 0x50000000) as usize;
            if x >= self.sys {
                return Err("Invalid system pointer".into());
            }
            x
        } else if (0x60000000..0x70000000).contains(&p) {
            let x = (p - 0x60000000) as usize;
            if x >= self.gfx {
                return Err("Invalid graphics pointer".into());
            }
            self.sys + x
        } else {
            return Err("Unsupported resource pointer".into());
        };
        Ok(Some(x))
    }
    fn req(&self, o: usize) -> Result<usize> {
        self.ptr(o)?.ok_or("Missing required buffer".into())
    }
    fn component(&self, o: usize, ty: usize, i: usize) -> Result<f32> {
        match ty {
            1 | 3 => Ok(half(self.u16(o + i * 2)?)),
            2 | 5 | 6 | 7 => self.f32(o + i * 4),
            _ => Err("Unsupported position/UV type. Use CodeWalker XML.".into()),
        }
    }
}
// Original read-only implementation of the documented legacy PC RSC7 layouts.
// Format research: CodeWalker Drawable.cs, Frag.cs and RpfResourcePageFlags.
pub fn parse_rsc7(bytes: &[u8], ext: &str) -> Result<Model> {
    if bytes.len() < 16 || &bytes[0..4] != b"RSC7" {
        return Err("Expected a standalone RSC7 model. Export from the RPF archive first.".into());
    }
    let h = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let version = h(4);
    if ![159, 165].contains(&version) {
        return Err(format!(
            "Unsupported RSC7 version {version}. Use legacy PC YFT/YDR or CodeWalker XML."
        ));
    }
    let sys = resource_size(h(8));
    let gfx = resource_size(h(12));
    let total = sys + gfx;
    if sys == 0 || total > 256 * 1024 * 1024 {
        return Err("Invalid resource size or exceeds 256 MB decompressed.".into());
    }
    let mut data = Vec::new();
    flate2::read::DeflateDecoder::new(&bytes[16..])
        .take((total + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|e| format!("Damaged model: {e}"))?;
    if data.len() != total {
        return Err("Decompressed size does not match resource header.".into());
    }
    let r = Reader { data, sys, gfx };
    let mut out = Model::default();
    let mut drawables = Vec::new();
    if ext.eq_ignore_ascii_case("ydr") {
        drawables.push(0)
    } else {
        if let Some(d) = r.ptr(0x30)? {
            drawables.push(d)
        }
        let count = r.u32(0x48)?;
        if count > 4096 {
            return Err("Invalid drawable count".into());
        }
        if let Some(a) = r.ptr(0x38)? {
            for i in 0..count as usize {
                if let Some(d) = r.ptr(a + i * 8)? {
                    drawables.push(d)
                }
            }
        }
        // Fragment physics contains separate pristine/damaged part drawables.
        // Include their geometry for UV recovery, deduplicating shared pointers.
        if let Some(group) = r.ptr(0xf0)? {
            for level in 0..3 {
                if let Some(lod) = r.ptr(group + 0x10 + level * 8)? {
                    let count = r.bytes::<1>(lod + 0x11d)?[0] as usize;
                    if let Some(array) = r.ptr(lod + 0xd0)? {
                        for i in 0..count {
                            if let Some(child) = r.ptr(array + i * 8)? {
                                for offset in [0xa0, 0xa8] {
                                    if let Some(drawable) = r.ptr(child + offset)? {
                                        drawables.push(drawable);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    drawables.sort_unstable();
    drawables.dedup();
    for (di, d) in drawables.iter().enumerate() {
        for li in 0..4 {
            let Some(list) = r.ptr(d + 0x50 + li * 8)? else {
                continue;
            };
            let count = r.u16(list + 8)?;
            if count == 0 {
                continue;
            }
            if count > 4096 {
                return Err("Invalid model count".into());
            }
            let array = r.req(list)?;
            for mi in 0..count as usize {
                let Some(model) = r.ptr(array + mi * 8)? else {
                    continue;
                };
                let n = r.u16(model + 0x10)?;
                if n == 0 {
                    continue;
                }
                let geoms = r.req(model + 8)?;
                if n > 4096 {
                    return Err("Invalid geometry count".into());
                }
                let mapping = r.ptr(model + 0x20)?;
                for gi in 0..n as usize {
                    let Some(g) = r.ptr(geoms + gi * 8)? else {
                        continue;
                    };
                    let vb = r.req(g + 0x18)?;
                    let ib = r.req(g + 0x38)?;
                    let stride = r.u16(vb + 8)? as usize;
                    let vc = r.u32(vb + 0x18)? as usize;
                    let decl = r.req(vb + 0x30)?;
                    let vdata = match r.ptr(vb + 0x10)? {
                        Some(p) => p,
                        None => r.req(vb + 0x20)?,
                    };
                    let idata = r.req(ib + 0x10)?;
                    let ic = r.u32(ib + 8)? as usize;
                    if stride == 0
                        || stride > 512
                        || vc > 2_000_000
                        || ic > 6_000_000
                        || !ic.is_multiple_of(3)
                    {
                        return Err("Invalid geometry dimensions".into());
                    }
                    if vdata + vc * stride > total || idata + ic * 2 > total {
                        return Err("Truncated geometry data".into());
                    }
                    let flags = r.u32(decl)?;
                    let types = r.u64(decl + 8)?;
                    let sizes = [0, 4, 4, 8, 0, 8, 12, 16, 4, 4, 4, 0, 0, 0, 0, 0];
                    let mut attrs = [None; 16];
                    let mut off = 0;
                    for (s, attr) in attrs.iter_mut().enumerate() {
                        if flags & (1 << s) != 0 {
                            let ty = ((types >> (s * 4)) & 15) as usize;
                            if sizes[ty] == 0 {
                                return Err(format!("Unsupported vertex component {ty}. Use XML."));
                            }
                            *attr = Some((off, ty));
                            off += sizes[ty];
                        }
                    }
                    let Some((po, pt)) = attrs[0] else {
                        return Err("Missing vertex positions".into());
                    };
                    if off > stride {
                        return Err("Invalid vertex stride".into());
                    }
                    let shader = if let Some(s) = mapping {
                        r.u16(s + gi * 2)?
                    } else {
                        0
                    };
                    let mut m = mesh(
                        &format!("Drawable {} / mesh {}.{}", di + 1, mi + 1, gi + 1),
                        &format!("Shader {shader}"),
                    );
                    m.lod = ["High", "Med", "Low", "VeryLow"][li].into();
                    for v in 0..vc {
                        let o = vdata + v * stride;
                        m.positions.push([
                            r.component(o + po, pt, 0)?,
                            r.component(o + po, pt, 1)?,
                            r.component(o + po, pt, 2)?,
                        ]);
                        for c in 0..8 {
                            if let Some((uo, ut)) = attrs[c + 6] {
                                while m.uvs.len() <= c {
                                    m.uvs.push(vec![])
                                }
                                m.uvs[c].push([
                                    r.component(o + uo, ut, 0)?,
                                    r.component(o + uo, ut, 1)?,
                                ]);
                            }
                        }
                    }
                    for i in 0..ic {
                        m.indices.push(r.u16(idata + i * 2)? as u32)
                    }
                    out.meshes.push(m);
                    if out.triangles() > 2_000_000 {
                        return Err("Model exceeds two million triangles.".into());
                    }
                }
            }
        }
    }
    out.warnings.push("Legacy PC model, including fragment part geometry. Preview uses raw mesh positions without articulated transforms. Enhanced/Gen9 models require CodeWalker XML.".into());
    out.validate()?;
    Ok(out)
}
