use crate::{Result, xml_document};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};
pub mod lens_model;
pub(crate) mod native_edit;
mod placement;
pub(crate) mod texture_dict;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Mesh {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vertex_alpha: Vec<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub siren_ids: Vec<u8>,
    /// Vehicle extra (`extra_N` bone) each vertex belongs to, 0 for none.
    /// Empty when the model has no extras.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_ids: Vec<u8>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_source: Option<std::path::PathBuf>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub texture_sources: std::collections::BTreeMap<String, std::path::PathBuf>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub preview_textures: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sirens: Vec<crate::sirens::SirenAnchor>,
    pub name: String,
    pub meshes: Vec<Mesh>,
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub material_info: std::collections::BTreeMap<String, MaterialInfo>,
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct MaterialInfo {
    #[serde(default)]
    pub render_bucket: u8,
    pub shader: String,
    pub diffuse_texture: String,
    pub secondary_texture: String,
}
impl MaterialInfo {
    pub fn is_light_emissive(&self) -> bool {
        self.shader.starts_with("vehicle_lightsemissive")
    }
    pub fn is_livery(&self) -> bool {
        self.shader == "vehicle_paint3" && !self.secondary_texture.is_empty()
    }
    fn label(&self, index: usize) -> String {
        let texture = if self.is_livery() {
            &self.secondary_texture
        } else {
            &self.diffuse_texture
        };
        if texture.is_empty() {
            format!("Shader {index} / {}", self.shader)
        } else {
            format!("Shader {index} / {texture}")
        }
    }
}

fn shader_hash(name: &str) -> u32 {
    let mut h = 0u32;
    for c in name.bytes() {
        h = h.wrapping_add(c as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}

fn shader_name(hash: u32) -> String {
    [
        "vehicle_paint1",
        "vehicle_paint2",
        "vehicle_paint3",
        "vehicle_paint4",
        "vehicle_paint5",
        "vehicle_paint6",
        "vehicle_paint7",
        "vehicle_mesh",
        "vehicle_badges",
        "vehicle_tire",
        "vehicle_vehglass",
        "vehicle_vehglass_inner",
        "vehicle_lightsemissive",
        "vehicle_interior",
        "vehicle_decal",
        "vehicle_licenseplate",
        "ped_alpha",
    ]
    .into_iter()
    .find(|name| shader_hash(name) == hash)
    .map(str::to_string)
    .unwrap_or_else(|| format!("0x{hash:08X}"))
}
impl Model {
    /// Vehicle extras with geometry in this model, in ascending order.
    pub fn extras(&self) -> std::collections::BTreeSet<u8> {
        self.meshes
            .iter()
            .flat_map(|m| &m.extra_ids)
            .copied()
            .filter(|&id| id > 0)
            .collect()
    }
    /// Removes the triangles of hidden extras. A triangle touching a hidden
    /// extra belongs to that part. Vertices are kept, so bindings stay aligned.
    pub fn hide_extras(&mut self, hidden: &std::collections::BTreeSet<u8>) {
        if hidden.is_empty() {
            return;
        }
        for mesh in self.meshes.iter_mut().filter(|m| !m.extra_ids.is_empty()) {
            let ids = &mesh.extra_ids;
            mesh.indices = mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .filter(|tri| {
                    !tri.iter()
                        .any(|&i| ids.get(i as usize).is_some_and(|id| hidden.contains(id)))
                })
                .flatten()
                .copied()
                .collect();
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.preview_textures.len() > 256
            || self
                .preview_textures
                .values()
                .map(String::len)
                .sum::<usize>()
                > 48 * 1024 * 1024
        {
            return Err("Preview textures exceed the project limit".into());
        }
        if self.sirens.len() > 1024
            || self
                .sirens
                .iter()
                .any(|a| !(1..=32).contains(&a.id) || a.position.iter().any(|v| !v.is_finite()))
        {
            return Err("Invalid model siren positions".into());
        }
        if self.meshes.is_empty() {
            return Err("No triangle geometry found.".into());
        }
        let mut count = 0;
        for m in &self.meshes {
            if !m.vertex_alpha.is_empty() && m.vertex_alpha.len() != m.positions.len() {
                return Err("Invalid vertex alpha channel".into());
            }
            if !m.siren_ids.is_empty()
                && (m.siren_ids.len() != m.positions.len() || m.siren_ids.iter().any(|&id| id > 32))
            {
                return Err("Invalid siren surface bindings".into());
            }
            if !m.extra_ids.is_empty()
                && (m.extra_ids.len() != m.positions.len()
                    || m.extra_ids.iter().any(|&id| id > crate::sirens::MAX_EXTRA))
            {
                return Err("Invalid vehicle extra bindings".into());
            }
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
    if ext == "yft" || ext == "ydr" {
        model.native_source = Some(path.to_owned());
        texture_dict::load_nearby(path, &mut model);
    }
    model.validate()?;
    Ok(model)
}
fn mesh(name: &str, material: &str) -> Mesh {
    Mesh {
        vertex_alpha: vec![],
        siren_ids: vec![],
        extra_ids: vec![],
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
    let mut bone_channels = vec![];
    let mut bone_extras = vec![];
    let materials: Vec<_> = doc
        .descendants()
        .find(|n| n.has_tag_name("Shaders"))
        .map(|shaders| {
            shaders
                .children()
                .filter(|n| n.has_tag_name("Item"))
                .map(|node| {
                    let shader = node
                        .children()
                        .find(|n| n.has_tag_name("Name"))
                        .and_then(|n| n.text())
                        .unwrap_or("")
                        .trim_end_matches(".sps")
                        .to_ascii_lowercase();
                    let texture = |sampler| {
                        node.descendants()
                            .find(|n| {
                                n.attribute("name")
                                    .is_some_and(|s| s.eq_ignore_ascii_case(sampler))
                            })
                            .and_then(|n| n.children().find(|n| n.has_tag_name("Name")))
                            .and_then(|n| n.text())
                            .unwrap_or("")
                            .to_string()
                    };
                    MaterialInfo {
                        render_bucket: node
                            .children()
                            .find(|n| n.has_tag_name("RenderBucket"))
                            .and_then(|n| n.attribute("value"))
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0),
                        shader,
                        diffuse_texture: texture("DiffuseSampler"),
                        secondary_texture: texture("DiffuseSampler2"),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(bones) = doc
        .descendants()
        .find(|n| n.has_tag_name("Skeleton"))
        .and_then(|n| n.children().find(|c| c.has_tag_name("Bones")))
    {
        let poses = (|| -> Result<Vec<crate::sirens::BonePose>> {
            let mut poses = vec![];
            for bone in bones.children().filter(|n| n.has_tag_name("Item")) {
                let child = |name| bone.children().find(|n| n.has_tag_name(name));
                let component = |tag, attribute, default| -> Result<f32> {
                    child(tag)
                        .and_then(|n| n.attribute(attribute))
                        .map(number)
                        .unwrap_or(Ok(default))
                };
                poses.push(crate::sirens::BonePose {
                    name: child("Name").and_then(|n| n.text()).unwrap_or("").into(),
                    parent: child("ParentIndex")
                        .and_then(|n| n.attribute("value").or(n.text()))
                        .unwrap_or("-1")
                        .trim()
                        .parse()
                        .map_err(|_| "Invalid bone parent")?,
                    position: [
                        component("Translation", "x", 0.)?,
                        component("Translation", "y", 0.)?,
                        component("Translation", "z", 0.)?,
                    ],
                    rotation: [
                        component("Rotation", "x", 0.)?,
                        component("Rotation", "y", 0.)?,
                        component("Rotation", "z", 0.)?,
                        component("Rotation", "w", 1.)?,
                    ],
                    scale: [
                        component("Scale", "x", 1.)?,
                        component("Scale", "y", 1.)?,
                        component("Scale", "z", 1.)?,
                    ],
                });
                if poses.len() > 4096 {
                    return Err("Invalid skeleton bone count".into());
                }
            }
            Ok(poses)
        })();
        match poses.and_then(|p| {
            bone_channels = crate::sirens::bone_channels(&p)?;
            bone_extras = crate::sirens::bone_extras(&p)?;
            crate::sirens::bone_anchors(&p)
        }) {
            Ok(anchors) => out.sirens = anchors,
            Err(error) => out
                .warnings
                .push(format!("Siren bones unavailable: {error}")),
        }
    }
    let has_extras = bone_extras.iter().any(|&id| id > 0);
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
        let info = shader
            .parse::<usize>()
            .ok()
            .and_then(|i| materials.get(i).map(|m| (i, m)));
        let material = if let Some((index, info)) = info {
            let label = info.label(index);
            out.material_info.insert(label.clone(), info.clone());
            label
        } else {
            format!("Shader {shader}")
        };
        let emissive = info.is_some_and(|(_, info)| info.is_light_emissive());
        let rigid_bone = g
            .ancestors()
            .skip(1)
            .find_map(|n| n.children().find(|n| n.has_tag_name("BoneIndex")))
            .and_then(|n| n.attribute("value"))
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(0);
        let palette = g
            .children()
            .find(|n| n.has_tag_name("BoneIDs"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .split_whitespace()
            .map(|s| {
                s.parse::<usize>()
                    .map_err(|_| "Invalid XML bone palette".into())
            })
            .collect::<Result<Vec<_>>>()?;
        let mut m = mesh(&format!("Geometry {}", out.meshes.len() + 1), &material);
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
            if let Some(&color) = offsets.get("Colour0") {
                let alpha = nums[color + 3];
                if !(0.0..=255.).contains(&alpha) {
                    return Err("Invalid XML vertex alpha".into());
                }
                m.vertex_alpha.push(alpha.round() as u8);
            }
            if emissive || has_extras {
                // The bone with the most weight, or None for an unusable blend index.
                let bone = if let (Some(&w), Some(&i)) =
                    (offsets.get("BlendWeights"), offsets.get("BlendIndices"))
                {
                    let slot = (0..4)
                        .max_by(|&a, &b| nums[w + a].total_cmp(&nums[w + b]))
                        .unwrap_or(0);
                    let index = nums[i + slot];
                    if !index.is_finite() || index < 0. || index.fract() != 0. || index > 4095. {
                        None
                    } else if palette.is_empty() {
                        Some(index as usize)
                    } else {
                        palette.get(index as usize).copied()
                    }
                } else {
                    Some(rigid_bone)
                };
                if emissive {
                    let bone = bone.ok_or("Invalid XML blend index or bone palette index")?;
                    m.siren_ids
                        .push(bone_channels.get(bone).copied().unwrap_or(0));
                }
                if has_extras {
                    m.extra_ids
                        .push(bone.and_then(|b| bone_extras.get(b)).copied().unwrap_or(0));
                }
            }
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
    fn bone_poses(&self, drawable: usize) -> Result<Vec<crate::sirens::BonePose>> {
        let Some(skeleton) = self.ptr(drawable + 0x18)? else {
            return Ok(vec![]);
        };
        let count = self.u16(skeleton + 0x5e)? as usize;
        if count == 0 {
            return Ok(vec![]);
        }
        if count > 4096 {
            return Err("Invalid skeleton bone count".into());
        }
        let bones = self.req(skeleton + 0x20)?;
        let mut poses = Vec::with_capacity(count);
        for i in 0..count {
            let b = bones + i * 80;
            let name = self
                .ptr(b + 0x38)?
                .map(|n| self.string(n))
                .transpose()?
                .unwrap_or_default();
            poses.push(crate::sirens::BonePose {
                name,
                parent: i16::from_le_bytes(self.bytes(b + 0x32)?),
                position: [
                    self.f32(b + 0x10)?,
                    self.f32(b + 0x14)?,
                    self.f32(b + 0x18)?,
                ],
                rotation: [
                    self.f32(b)?,
                    self.f32(b + 4)?,
                    self.f32(b + 8)?,
                    self.f32(b + 12)?,
                ],
                scale: [
                    self.f32(b + 0x20)?,
                    self.f32(b + 0x24)?,
                    self.f32(b + 0x28)?,
                ],
            });
        }
        Ok(poses)
    }
    fn siren_bones(&self, drawable: usize) -> Result<Vec<crate::sirens::SirenAnchor>> {
        crate::sirens::bone_anchors(&self.bone_poses(drawable)?)
    }
    fn string(&self, offset: usize) -> Result<String> {
        let bytes = self
            .data
            .get(offset..)
            .ok_or("Texture name is outside the model")?;
        let length = bytes
            .iter()
            .take(256)
            .position(|&b| b == 0)
            .ok_or("Invalid texture name")?;
        Ok(String::from_utf8_lossy(&bytes[..length]).into_owned())
    }
    fn materials(&self, drawable: usize) -> Result<Vec<MaterialInfo>> {
        let Some(group) = self.ptr(drawable + 0x10)? else {
            return Ok(vec![]);
        };
        let count = self.u16(group + 0x18)? as usize;
        if count == 0 {
            return Ok(vec![]);
        }
        if count > 4096 {
            return Err("Invalid shader count".into());
        }
        let array = self.req(group + 0x10)?;
        let mut materials = vec![MaterialInfo::default(); count];
        for (index, info) in materials.iter_mut().enumerate() {
            let Some(shader) = self.ptr(array + index * 8)? else {
                continue;
            };
            info.shader = shader_name(self.u32(shader + 8)?);
            info.render_bucket = self.bytes::<1>(shader + 0x11)?[0];
            let count = self.bytes::<1>(shader + 0x10)?[0] as usize;
            if count == 0 {
                continue;
            }
            let parameters = self.req(shader)?;
            let mut hashes = parameters + count * 16;
            for i in 0..count {
                hashes += self.bytes::<1>(parameters + i * 16)?[0] as usize * 16;
            }
            for i in 0..count {
                if self.bytes::<1>(parameters + i * 16)?[0] != 0 {
                    continue;
                }
                let hash = self.u32(hashes + i * 4)?;
                if hash != shader_hash("diffusesampler") && hash != shader_hash("diffusesampler2") {
                    continue;
                }
                let Some(texture) = self.ptr(parameters + i * 16 + 8)? else {
                    continue;
                };
                let Some(name) = self.ptr(texture + 0x28)? else {
                    continue;
                };
                let name = self.string(name)?;
                if hash == shader_hash("diffusesampler2") {
                    info.secondary_texture = name;
                } else {
                    info.diffuse_texture = name;
                }
            }
        }
        Ok(materials)
    }
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
    parse_rsc7_traced(bytes, ext, None)
}
fn parse_rsc7_traced(
    bytes: &[u8],
    ext: &str,
    mut trace: Option<&mut Vec<lens_model::NativeMesh>>,
) -> Result<Model> {
    if bytes.len() < 16 || &bytes[0..4] != b"RSC7" {
        return Err("Expected a standalone RSC7 model. Export from the RPF archive first.".into());
    }
    let h = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let version = h(4);
    // Versions are specific to the resource type: 162 is legacy PC YFT,
    // 165 is legacy PC YDR. YDR 159/154 and YFT 171 use Gen9 layouts.
    let supported = match ext.to_ascii_lowercase().as_str() {
        "yft" => version == 162,
        "ydr" => version == 165,
        _ => return Err("Expected a YFT or YDR model extension.".into()),
    };
    if !supported {
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
    let drawables = r.placed_drawables(ext)?;
    let root = drawables
        .first()
        .map(|d| d.address)
        .ok_or("No drawable in model")?;
    // Always use the actual root skeleton, never whichever pointer sorts first.
    out.sirens = r.siren_bones(root)?;
    let root_bones = r.bone_poses(root)?;
    let root_materials = r.materials(root).unwrap_or_default();
    for (di, placed) in drawables.iter().enumerate() {
        let d = &placed.address;
        let materials = match r.materials(*d) {
            Ok(materials) if materials.is_empty() && *d != root => root_materials.clone(),
            Ok(materials) => materials,
            Err(error) => {
                out.warnings.push(format!(
                    "Material names unavailable for drawable {}: {error}",
                    di + 1
                ));
                vec![]
            }
        };
        let own_bones = r.bone_poses(*d)?;
        let bones = if own_bones.is_empty() {
            &root_bones
        } else {
            &own_bones
        };
        let bone_channels = crate::sirens::bone_channels(bones)?;
        let bone_extras = crate::sirens::bone_extras(bones)?;
        let has_extras = bone_extras.iter().any(|&id| id > 0);
        let worlds = crate::sirens::bone_worlds(bones)?;
        let skeleton_drawable = if own_bones.is_empty() { root } else { *d };
        let skin = r.skin_matrices(skeleton_drawable, &worlds)?;
        let rigid = r.rigid_matrices(skeleton_drawable, ext, &worlds)?;
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
                let binding = r.u32(model + 0x28)?;
                let rigid_bone = (binding >> 24) as usize;
                let skinned = (binding >> 8) & 255 != 0;
                let transform = placed.transform.unwrap_or_else(|| {
                    rigid
                        .get(rigid_bone)
                        .copied()
                        .unwrap_or(glam::Mat4::IDENTITY)
                });
                for gi in 0..n as usize {
                    let Some(g) = r.ptr(geoms + gi * 8)? else {
                        continue;
                    };
                    let vb = r.req(g + 0x18)?;
                    let ib = r.req(g + 0x38)?;
                    let stride = r.u16(vb + 8)? as usize;
                    let vc = r.u32(vb + 0x18)? as usize;
                    let ic = r.u32(ib + 8)? as usize;
                    // Vehicle exports can retain empty geometry slots. They have
                    // no triangles to recover and may omit their data pointers.
                    if vc == 0 && ic == 0 {
                        continue;
                    }
                    if vc == 0
                        || ic == 0
                        || stride == 0
                        || stride > 512
                        || vc > 2_000_000
                        || ic > 6_000_000
                        || !ic.is_multiple_of(3)
                    {
                        return Err("Invalid geometry dimensions".into());
                    }
                    let decl = r.req(vb + 0x30)?;
                    let vdata = match r.ptr(vb + 0x10)? {
                        Some(p) => p,
                        None => r.req(vb + 0x20)?,
                    };
                    let idata = r.req(ib + 0x10)?;
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
                    let material = if let Some(info) = materials
                        .get(shader as usize)
                        .filter(|info| !info.shader.is_empty())
                    {
                        let label = info.label(shader as usize);
                        out.material_info.insert(label.clone(), info.clone());
                        label
                    } else {
                        format!("Shader {shader}")
                    };
                    let mut m = mesh(
                        &format!("Drawable {} / mesh {}.{}", di + 1, mi + 1, gi + 1),
                        &material,
                    );
                    let emissive = out
                        .material_info
                        .get(&material)
                        .is_some_and(MaterialInfo::is_light_emissive);
                    let palette_count = r.u16(g + 0x72)? as usize;
                    if palette_count > 4096 {
                        return Err("Invalid bone palette".into());
                    }
                    let palette = if palette_count > 0 {
                        let start = r.req(g + 0x68)?;
                        (0..palette_count)
                            .map(|i| r.u16(start + i * 2))
                            .collect::<Result<Vec<_>>>()?
                    } else {
                        vec![]
                    };
                    m.lod = ["High", "Med", "Low", "VeryLow"][li].into();
                    for v in 0..vc {
                        let o = vdata + v * stride;
                        let raw = glam::Vec3::new(
                            r.component(o + po, pt, 0)?,
                            r.component(o + po, pt, 1)?,
                            r.component(o + po, pt, 2)?,
                        );
                        let position = if skinned {
                            if let (Some((wo, 8 | 9)), Some((io, 8 | 9))) = (attrs[1], attrs[2]) {
                                placement::skin_position(
                                    raw,
                                    r.bytes::<4>(o + wo)?,
                                    r.bytes::<4>(o + io)?,
                                    &palette,
                                    &skin,
                                )?
                            } else {
                                raw
                            }
                        } else {
                            transform.transform_point3(raw)
                        };
                        m.positions.push(position.to_array());
                        if let Some((co, ct)) = attrs[4] {
                            let alpha = match ct {
                                8 | 9 => r.bytes::<4>(o + co)?[3],
                                3 | 7 => {
                                    (r.component(o + co, ct, 3)?.clamp(0., 1.) * 255.).round() as u8
                                }
                                _ => 255,
                            };
                            m.vertex_alpha.push(alpha);
                        }
                        if emissive || has_extras {
                            // The bone with the most weight, or None for a bad palette index.
                            let bone = if let (Some((wo, 8 | 9)), Some((io, 8 | 9))) =
                                (attrs[1], attrs[2])
                            {
                                let weights = r.bytes::<4>(o + wo)?;
                                let indices = r.bytes::<4>(o + io)?;
                                let slot = (0..4).max_by_key(|&i| weights[i]).unwrap_or(0);
                                let index = indices[slot] as usize;
                                if weights[slot] == 0 {
                                    Some(rigid_bone)
                                } else if palette.is_empty() {
                                    Some(index)
                                } else {
                                    palette.get(index).map(|&b| b as usize)
                                }
                            } else {
                                Some(rigid_bone)
                            };
                            if emissive {
                                let bone = bone.ok_or("Invalid vertex bone palette index")?;
                                m.siren_ids
                                    .push(bone_channels.get(bone).copied().unwrap_or(0));
                            }
                            if has_extras {
                                m.extra_ids.push(
                                    bone.and_then(|b| bone_extras.get(b)).copied().unwrap_or(0),
                                );
                            }
                        }
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
                    if let Some(trace) = trace.as_mut() {
                        let material_drawable = if r.materials(*d)?.is_empty() {
                            root
                        } else {
                            *d
                        };
                        trace.push(lens_model::NativeMesh {
                            model,
                            geometry: g,
                            slot: gi,
                            shader: shader as usize,
                            group: r.req(material_drawable + 0x10)?,
                        });
                    }
                    out.meshes.push(m);
                    if out.triangles() > 2_000_000 {
                        return Err("Model exceeds two million triangles.".into());
                    }
                }
            }
        }
    }
    out.warnings.push("Legacy PC model with rest-pose skinning and pristine fragment placement. Runtime suspension, damage and scripted attachments are not simulated. Enhanced/Gen9 models require CodeWalker XML.".into());
    out.validate()?;
    Ok(out)
}

pub fn load_preview_textures(paths: &[std::path::PathBuf], model: &mut Model) {
    texture_dict::load_paths(paths, model);
}
