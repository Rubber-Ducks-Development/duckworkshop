//! Textured 3D model preview drawn with OpenGL inside the egui frame.
#![allow(unsafe_code)]
use super::*;
use eframe::glow::{self, HasContext};
use glam::{Mat4, Vec3};
use image::RgbaImage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
#[path = "viewer/bloom.rs"]
mod bloom;

const FOV: f32 = 0.75;
const FLOATS: usize = 10;

/// Geometry ready for upload: interleaved position, normal and texture coordinate,
/// with triangles grouped by material so each group can use its own texture.
pub(super) struct SceneMesh {
    version: u64,
    vertices: Vec<f32>,
    indices: Vec<u32>,
    lines: Vec<u32>,
    pub groups: Vec<Group>,
    pub center: Vec3,
    radius: f32,
    pub floor: f32,
    pub siren_ids: Vec<u8>,
}
pub(super) struct Group {
    pub material: String,
    start: usize,
    count: usize,
    pub mapped: bool,
    glass: bool,
    cutout: bool,
    alpha: bool,
    vertex_alpha: bool,
    center: Vec3,
    has_sirens: bool,
}

/// Orbit camera in a Y-up space. Distance is measured in model radii.
#[derive(Clone, Copy)]
pub(super) struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub pan: Vec3,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.75,
            pitch: 0.32,
            distance: 1. / (FOV * 0.5).sin() * 1.08,
            pan: Vec3::ZERO,
        }
    }
}
pub(super) const VIEWS: [(&str, f32, f32); 5] = [
    ("Front", 0., 0.),
    ("Back", std::f32::consts::PI, 0.),
    ("Left", std::f32::consts::FRAC_PI_2, 0.),
    ("Right", -std::f32::consts::FRAC_PI_2, 0.),
    ("Top", std::f32::consts::PI, 1.5607),
];

/// Builds the preview geometry for the selected meshes. GTA models are Z-up and face
/// +Y; they are turned so that every model is Y-up and faces +Z in the preview.
pub(super) fn build(
    model: &Model,
    opt: &render::UvOptions,
    z_up: bool,
    version: u64,
) -> Option<SceneMesh> {
    let convert = |p: [f32; 3]| {
        if z_up {
            Vec3::new(-p[0], p[2], p[1])
        } else {
            Vec3::from(p)
        }
    };
    let mut vertices = vec![];
    let mut buckets: Vec<(String, bool, Vec<u32>)> = vec![];
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for m in render::selected(model, opt) {
        let base = (vertices.len() / FLOATS) as u32;
        let positions: Vec<Vec3> = m.positions.iter().map(|p| convert(*p)).collect();
        let mut normals = vec![Vec3::ZERO; positions.len()];
        for tri in m.indices.chunks(3) {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| i as usize);
            let n = (positions[b] - positions[a]).cross(positions[c] - positions[a]);
            for i in [a, b, c] {
                normals[i] += n;
            }
        }
        let uv = m.uvs.get(opt.channel).filter(|u| !u.is_empty());
        for (i, p) in positions.iter().enumerate() {
            min = min.min(*p);
            max = max.max(*p);
            let n = normals[i].normalize_or(Vec3::Y);
            let [u, v] = uv.map_or([0., 0.], |uv| uv[i]);
            let t = if opt.flip_v { 1. - v } else { v };
            let siren = m.siren_ids.get(i).copied().unwrap_or(0);
            let alpha = m.vertex_alpha.get(i).copied().unwrap_or(255) as f32 / 255.;
            vertices.extend_from_slice(&[p.x, p.y, p.z, n.x, n.y, n.z, u, t, siren as f32, alpha]);
        }
        let mapped = uv.is_some();
        let bucket = match buckets
            .iter()
            .position(|(name, has_uv, _)| *name == m.material && *has_uv == mapped)
        {
            Some(i) => i,
            None => {
                buckets.push((m.material.clone(), mapped, vec![]));
                buckets.len() - 1
            }
        };
        buckets[bucket].2.extend(m.indices.iter().map(|i| i + base));
    }
    if vertices.is_empty() {
        return None;
    }
    let mut indices = vec![];
    let mut groups = vec![];
    for (material, mapped, list) in buckets {
        groups.push(Group {
            vertex_alpha: model
                .material_info
                .get(&material)
                .is_some_and(|m| m.shader.ends_with("_alpha") || m.shader.contains("glass")),
            alpha: model.material_info.get(&material).is_some_and(|m| {
                matches!(m.render_bucket, 1 | 2)
                    || m.is_light_emissive()
                    || m.shader.ends_with("_alpha")
            }),
            center: list
                .iter()
                .map(|&i| {
                    let o = i as usize * FLOATS;
                    Vec3::from_slice(&vertices[o..o + 3])
                })
                .sum::<Vec3>()
                / list.len().max(1) as f32,
            has_sirens: list
                .iter()
                .any(|&i| vertices[i as usize * FLOATS + 8] > 0.5),
            cutout: model.material_info.get(&material).is_some_and(|m| {
                m.render_bucket == 3 || m.shader.contains("badges") || m.shader.contains("decal")
            }),
            glass: model
                .material_info
                .get(&material)
                .is_some_and(|m| m.shader.contains("glass")),
            material,
            start: indices.len(),
            count: list.len(),
            mapped,
        });
        indices.extend(list);
    }
    let mut lines = Vec::with_capacity(indices.len() * 2);
    for tri in indices.chunks(3) {
        lines.extend_from_slice(&[tri[0], tri[1], tri[1], tri[2], tri[2], tri[0]]);
    }
    let siren_ids = vertices
        .as_chunks::<FLOATS>()
        .0
        .iter()
        .map(|v| v[8] as u8)
        .filter(|&id| id > 0)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    Some(SceneMesh {
        version,
        vertices,
        indices,
        lines,
        groups,
        center: (min + max) * 0.5,
        radius: ((max - min).length() * 0.5).max(0.001),
        floor: min.y,
        siren_ids,
    })
}

/// Preview the complete assembled vehicle without changing UV export selection.
/// Each material uses its own diffuse/livery channel, with UV 0 as the fallback.
pub(super) fn build_vehicle(
    model: &Model,
    opt: &render::UvOptions,
    z_up: bool,
    version: u64,
) -> Option<SceneMesh> {
    let mut full = model.clone();
    for mesh in &mut full.meshes {
        mesh.enabled = true;
        let channel = if full
            .material_info
            .get(&mesh.material)
            .is_some_and(|m| m.is_livery())
        {
            1
        } else {
            0
        };
        if let Some(uv) = mesh.uvs.get(channel).filter(|uv| !uv.is_empty()).cloned() {
            mesh.uvs[0] = uv;
        }
    }
    let mut options = opt.clone();
    options.material = None;
    options.channel = 0;
    if options.lod == "All" {
        options.lod = ["High", "Med", "Low", "VeryLow"]
            .into_iter()
            .find(|lod| full.meshes.iter().any(|m| m.lod == *lod))
            .unwrap_or("All")
            .into();
    }
    build(&full, &options, z_up, version)
}

/// What one frame draws. Textures are shared and uploaded only when their version changes.
pub(super) struct Frame {
    pub vehicle_opacity: f32,
    pub mesh: Arc<SceneMesh>,
    pub textures: Vec<(PathBuf, u64, Arc<RgbaImage>)>,
    /// Texture per mesh group, by index into `textures`.
    pub bindings: Vec<Option<usize>>,
    pub camera: Camera,
    pub wire: bool,
    pub paint: [u8; 3],
    pub lights: Vec<PreviewLight>,
    pub siren_surfaces: bool,
    pub night: bool,
}
pub(super) struct PreviewLight {
    pub origin: Vec3,
    pub scale: f32,
    pub angle: f32,
    pub id: u8,
    pub position: Vec3,
    pub color: [u8; 3],
    pub intensity: f32,
    pub radius: f32,
}

#[derive(Default)]
pub(super) struct Gpu {
    bloom: Option<bloom::Bloom>,
    ready: Option<Resources>,
    failed: Option<String>,
    mesh_version: u64,
    textures: HashMap<PathBuf, (glow::Texture, u64)>,
}
struct Resources {
    program: glow::Program,
    vao: Option<glow::VertexArray>,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    lbo: glow::Buffer,
    mvp: Option<glow::UniformLocation>,
    light: Option<glow::UniformLocation>,
    mode: Option<glow::UniformLocation>,
    color: Option<glow::UniformLocation>,
    sampler: Option<glow::UniformLocation>,
    light_count: Option<glow::UniformLocation>,
    light_positions: Option<glow::UniformLocation>,
    light_colors: Option<glow::UniformLocation>,
    siren_colors: Option<glow::UniformLocation>,
    siren_transforms: Option<glow::UniformLocation>,
    siren_surfaces: Option<glow::UniformLocation>,
    night: Option<glow::UniformLocation>,
    glass: Option<glow::UniformLocation>,
    cutout: Option<glow::UniformLocation>,
    alpha: Option<glow::UniformLocation>,
    vertex_alpha: Option<glow::UniformLocation>,
    opacity: Option<glow::UniformLocation>,
    lens_pass: Option<glow::UniformLocation>,
    mask: Option<glow::UniformLocation>,
    mipmaps: bool,
    max_texture: u32,
}

pub(super) fn matrices(camera: &Camera, mesh: &SceneMesh, aspect: f32) -> (Mat4, Vec3) {
    let target = mesh.center + camera.pan * mesh.radius;
    let distance = camera.distance * mesh.radius;
    let direction = Vec3::new(
        camera.pitch.cos() * camera.yaw.sin(),
        camera.pitch.sin(),
        camera.pitch.cos() * camera.yaw.cos(),
    );
    let eye = target + direction * distance;
    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
    let near = (distance - mesh.radius * 1.5).max(distance * 0.01);
    let far = distance + mesh.radius * 2.;
    let projection = Mat4::perspective_rh_gl(FOV, aspect.max(0.01), near, far);
    // Light from just above and to the side of the camera.
    let right = Vec3::Y.cross(direction).normalize_or(Vec3::X);
    let light = (direction + Vec3::Y * 0.6 + right * 0.35).normalize();
    (projection * view, light)
}
pub(super) fn convert_position(p: [f32; 3], z_up: bool) -> Vec3 {
    if z_up {
        Vec3::new(-p[0], p[2], p[1])
    } else {
        Vec3::from(p)
    }
}
pub(super) fn project_point(
    camera: &Camera,
    mesh: &SceneMesh,
    rect: Rect,
    p: Vec3,
) -> Option<Pos2> {
    let clip = matrices(camera, mesh, rect.width() / rect.height().max(1.)).0 * p.extend(1.);
    if clip.w <= 0. {
        return None;
    }
    let n = clip.truncate() / clip.w;
    if !n.is_finite() || n.z < -1. || n.z > 1. {
        return None;
    }
    Some(Pos2::new(
        rect.center().x + n.x * rect.width() * 0.5,
        rect.center().y - n.y * rect.height() * 0.5,
    ))
}
pub(super) fn radius(mesh: &SceneMesh) -> f32 {
    mesh.radius
}

/// Moves the orbit target by a screen-space drag of `delta` points.
pub(super) fn pan(camera: &mut Camera, delta: Vec2, height: f32) {
    let direction = Vec3::new(
        camera.pitch.cos() * camera.yaw.sin(),
        camera.pitch.sin(),
        camera.pitch.cos() * camera.yaw.cos(),
    );
    let right = Vec3::Y.cross(direction).normalize_or(Vec3::X);
    let up = direction.cross(right);
    let scale = 2. * camera.distance * (FOV * 0.5).tan() / height.max(1.);
    camera.pan += (-right * delta.x + up * delta.y) * scale;
}

impl Gpu {
    pub(super) fn paint(
        &mut self,
        gl: &glow::Context,
        info: &egui::PaintCallbackInfo,
        frame: &Frame,
    ) {
        if self.failed.is_some() {
            return;
        }
        if self.ready.is_none() {
            match unsafe { Resources::new(gl) } {
                Ok(r) => self.ready = Some(r),
                Err(e) => {
                    self.failed = Some(e);
                    return;
                }
            }
        }
        let r = self.ready.as_ref().unwrap();
        unsafe {
            if self.mesh_version != frame.mesh.version {
                r.upload(gl, &frame.mesh);
                self.mesh_version = frame.mesh.version;
            }
            self.textures.retain(|path, (texture, _)| {
                let keep = frame.textures.iter().any(|(p, ..)| p == path);
                if !keep {
                    gl.delete_texture(*texture);
                }
                keep
            });
            for (path, version, image) in &frame.textures {
                let current = self.textures.get(path).map(|(t, v)| (*t, *v));
                if current.is_none_or(|(_, v)| v != *version) {
                    if let Some((old, _)) = current {
                        gl.delete_texture(old);
                    }
                    if let Ok(texture) = r.texture(gl, image) {
                        self.textures.insert(path.clone(), (texture, *version));
                    } else {
                        self.textures.remove(path);
                    }
                }
            }
            let draw = |mask| {
                r.draw(
                    gl,
                    info,
                    frame,
                    |i| {
                        frame.bindings[i]
                            .and_then(|t| frame.textures.get(t))
                            .and_then(|(p, ..)| self.textures.get(p))
                            .map(|(t, _)| *t)
                    },
                    mask,
                )
            };
            if frame.siren_surfaces && !frame.lights.is_empty() && gl.version().major >= 3 {
                if self.bloom.is_none() {
                    match bloom::Bloom::new(gl) {
                        Ok(bloom) => self.bloom = Some(bloom),
                        Err(error) => {
                            self.failed = Some(format!("Lens glow: {error}"));
                            return;
                        }
                    }
                }
                if let Err(error) = self.bloom.as_mut().unwrap().render(gl, info, draw) {
                    self.failed = Some(error);
                }
            } else {
                draw(false);
            }
        }
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    pub(super) fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
            if let Some(mut bloom) = self.bloom.take() {
                bloom.destroy(gl);
            }
            for (_, (texture, _)) in self.textures.drain() {
                gl.delete_texture(texture);
            }
            if let Some(r) = self.ready.take() {
                gl.delete_program(r.program);
                for buffer in [r.vbo, r.ebo, r.lbo] {
                    gl.delete_buffer(buffer);
                }
                if let Some(vao) = r.vao {
                    gl.delete_vertex_array(vao);
                }
            }
        }
    }
}

/// Carcols delta turns a lens around its siren pivot, including flashing lights.
/// The viewer converts GTA's Z-up space to Y-up before applying this transform.
fn siren_transform(origin: Vec3, scale: f32, angle: f32) -> Mat4 {
    Mat4::from_translation(origin)
        * Mat4::from_rotation_y(angle)
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-origin)
}

const VERTEX: &str = r#"
uniform mat4 u_mvp;
uniform vec4 u_siren_transforms[96];
uniform int u_siren_surfaces;
attribute vec3 a_pos;
attribute vec3 a_normal;
attribute vec2 a_uv;
attribute float a_siren;
attribute float a_alpha;
varying vec3 v_normal;
varying vec2 v_uv;
varying vec3 v_position;
varying float v_siren;
varying float v_alpha;
void main() {
    v_normal = a_normal;
    v_uv = a_uv;
    vec3 position = a_pos;
    if (u_siren_surfaces == 1 && a_siren>0.5) {
        for (int i=0; i<32; i++) {
            if (abs(a_siren-float(i+1))<0.25) {
                vec4 row0=u_siren_transforms[i*3];
                vec4 row1=u_siren_transforms[i*3+1];
                vec4 row2=u_siren_transforms[i*3+2];
                vec4 point=vec4(a_pos,1.0);
                position=vec3(dot(row0,point),dot(row1,point),dot(row2,point));
                v_normal=vec3(dot(row0.xyz,a_normal),dot(row1.xyz,a_normal),dot(row2.xyz,a_normal));
            }
        }
    }
    v_position = position;
    v_siren = a_siren;
    v_alpha = a_alpha;
    gl_Position = u_mvp * vec4(position, 1.0);
}
"#;
const FRAGMENT: &str = r#"
uniform sampler2D u_texture;
uniform vec3 u_light;
uniform vec4 u_color;
uniform int u_mode;
uniform int u_light_count;
uniform vec4 u_light_positions[32];
uniform vec4 u_light_colors[32];
uniform vec4 u_siren_colors[32];
uniform int u_siren_surfaces;
uniform int u_night;
uniform int u_glass;
uniform int u_cutout;
uniform int u_mask;
uniform int u_alpha;
uniform int u_vertex_alpha;
uniform float u_opacity;
uniform int u_lens_pass;
varying vec3 v_normal;
varying vec2 v_uv;
varying vec3 v_position;
varying float v_siren;
varying float v_alpha;
void main() {
    if (u_mode == 2) {
        gl_FragColor = vec4(u_color.rgb * u_color.a, u_color.a);
        return;
    }
    vec4 texel = u_mode == 1 ? texture2D(u_texture, v_uv) : vec4(1.0);
    if (u_cutout==1 && texel.a<0.1) discard;
    float alpha = 1.0;
    if (u_glass == 1) { alpha = u_mode == 1 ? min(texel.a*0.5,0.35) : 0.12; }
    else if (u_alpha == 1 && u_mode == 1) { alpha = texel.a; }
    if (u_vertex_alpha == 1) { alpha *= v_alpha; }
    if (alpha < 0.001) discard;
    vec3 base = u_color.rgb;
    if (u_mode == 1) { base = (u_alpha == 1 || u_glass == 1 || u_cutout == 1) ? texel.rgb : mix(base, texel.rgb, texel.a); }
    float diffuse = abs(dot(normalize(v_normal), u_light));
    vec3 shaded = base * (u_night == 1 ? (0.10 + 0.20 * diffuse) : (0.30 + 0.60 * diffuse));
    vec3 emission = vec3(0.0);
    bool lens = u_siren_surfaces == 1 && v_siren > 0.5;
    if ((u_lens_pass == 1 && !lens) || (u_lens_pass == 2 && lens)) discard;
    if (lens) {
        vec4 lamp = vec4(0.0);
        for (int i=0; i<32; i++) {
            if (abs(v_siren-float(i+1))<0.25) { lamp=u_siren_colors[i]; }
        }
        emission = (u_mode == 1 ? texel.rgb : lamp.rgb) * lamp.a;
        shaded = base * 0.08 + emission * 1.5;
    } else {
        for (int i=0; i<32; i++) {
            if (i<u_light_count) {
                float attenuation=max(0.0,1.0-length(v_position-u_light_positions[i].xyz)/max(u_light_positions[i].w,0.001));
                shaded+=u_light_colors[i].rgb*u_light_colors[i].a*attenuation*attenuation*0.12;
            }
        }
    }
    if (u_mask == 1) {
        if (!lens || max(emission.r,max(emission.g,emission.b))<0.001) discard;
        gl_FragColor=vec4(emission*alpha,1.0);
    } else {
        if (!lens) { alpha *= u_opacity; }
        if (alpha < 0.001) discard;
        gl_FragColor=vec4(shaded*alpha,alpha);
    }
}
"#;

impl Resources {
    unsafe fn new(gl: &glow::Context) -> Result<Self> {
        unsafe {
            let version = egui_glow::ShaderVersion::get(gl);
            let header = version.version_declaration();
            let (vertex_prefix, fragment_prefix) = if version.is_new_shader_interface() {
                (
                    "#define attribute in\n#define varying out\n",
                    "#define varying in\nout vec4 out_color;\n#define gl_FragColor out_color\n#define texture2D texture\n",
                )
            } else {
                ("", "")
            };
            let precision = if version.is_embedded() {
                "precision highp float;\n"
            } else {
                ""
            };
            let program = gl.create_program()?;
            let mut shaders = vec![];
            for (kind, source) in [
                (
                    glow::VERTEX_SHADER,
                    format!("{header}{precision}{vertex_prefix}{VERTEX}"),
                ),
                (
                    glow::FRAGMENT_SHADER,
                    format!("{header}{precision}{fragment_prefix}{FRAGMENT}"),
                ),
            ] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, &source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    return Err(format!(
                        "3D preview shader: {}",
                        gl.get_shader_info_log(shader)
                    ));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }
            for (i, name) in ["a_pos", "a_normal", "a_uv", "a_siren", "a_alpha"]
                .iter()
                .enumerate()
            {
                gl.bind_attrib_location(program, i as u32, name);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(format!(
                    "3D preview shader: {}",
                    gl.get_program_info_log(program)
                ));
            }
            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }
            let v = gl.version();
            let modern = v.major >= 3;
            let vao = if modern {
                gl.create_vertex_array().ok()
            } else {
                None
            };
            let max_texture = gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE).max(1024) as u32;
            Ok(Self {
                mvp: gl.get_uniform_location(program, "u_mvp"),
                light: gl.get_uniform_location(program, "u_light"),
                mode: gl.get_uniform_location(program, "u_mode"),
                color: gl.get_uniform_location(program, "u_color"),
                sampler: gl.get_uniform_location(program, "u_texture"),
                light_count: gl.get_uniform_location(program, "u_light_count"),
                light_positions: gl.get_uniform_location(program, "u_light_positions[0]"),
                light_colors: gl.get_uniform_location(program, "u_light_colors[0]"),
                siren_colors: gl.get_uniform_location(program, "u_siren_colors[0]"),
                siren_transforms: gl.get_uniform_location(program, "u_siren_transforms[0]"),
                siren_surfaces: gl.get_uniform_location(program, "u_siren_surfaces"),
                night: gl.get_uniform_location(program, "u_night"),
                glass: gl.get_uniform_location(program, "u_glass"),
                cutout: gl.get_uniform_location(program, "u_cutout"),
                alpha: gl.get_uniform_location(program, "u_alpha"),
                vertex_alpha: gl.get_uniform_location(program, "u_vertex_alpha"),
                opacity: gl.get_uniform_location(program, "u_opacity"),
                lens_pass: gl.get_uniform_location(program, "u_lens_pass"),
                mask: gl.get_uniform_location(program, "u_mask"),
                program,
                vao,
                vbo: gl.create_buffer()?,
                ebo: gl.create_buffer()?,
                lbo: gl.create_buffer()?,
                mipmaps: modern,
                max_texture,
            })
        }
    }

    unsafe fn upload(&self, gl: &glow::Context, mesh: &SceneMesh) {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes(&mesh.vertices), glow::STATIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            for (buffer, data) in [(self.ebo, &mesh.indices), (self.lbo, &mesh.lines)] {
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(buffer));
                gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, bytes(data), glow::STATIC_DRAW);
            }
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
        }
    }

    unsafe fn texture(&self, gl: &glow::Context, image: &RgbaImage) -> Result<glow::Texture> {
        unsafe {
            let scaled;
            let mut image = image;
            if image.width().max(image.height()) > self.max_texture {
                let scale = self.max_texture as f32 / image.width().max(image.height()) as f32;
                scaled = image::imageops::resize(
                    image,
                    ((image.width() as f32 * scale) as u32).max(1),
                    ((image.height() as f32 * scale) as u32).max(1),
                    image::imageops::FilterType::Triangle,
                );
                image = &scaled;
            }
            let texture = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                image.width() as i32,
                image.height() as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(image.as_raw())),
            );
            let min = if self.mipmaps {
                gl.generate_mipmap(glow::TEXTURE_2D);
                glow::LINEAR_MIPMAP_LINEAR
            } else {
                glow::LINEAR
            };
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, min as i32);
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            for wrap in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, wrap, glow::REPEAT as i32);
            }
            gl.bind_texture(glow::TEXTURE_2D, None);
            Ok(texture)
        }
    }

    unsafe fn draw(
        &self,
        gl: &glow::Context,
        info: &egui::PaintCallbackInfo,
        frame: &Frame,
        texture: impl Fn(usize) -> Option<glow::Texture>,
        mask: bool,
    ) {
        unsafe {
            let viewport = info.viewport_in_pixels();
            let aspect = viewport.width_px as f32 / viewport.height_px.max(1) as f32;
            let (mvp, light) = matrices(&frame.camera, &frame.mesh, aspect);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.depth_mask(!mask);
            if !mask {
                gl.clear_depth(1.);
                gl.clear(glow::DEPTH_BUFFER_BIT);
            }
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(self.mvp.as_ref(), false, &mvp.to_cols_array());
            gl.uniform_3_f32(self.light.as_ref(), light.x, light.y, light.z);
            gl.uniform_1_i32(self.sampler.as_ref(), 0);
            gl.uniform_1_i32(
                self.siren_surfaces.as_ref(),
                i32::from(frame.siren_surfaces),
            );
            gl.uniform_1_i32(self.night.as_ref(), i32::from(frame.night));
            gl.uniform_1_i32(self.mask.as_ref(), i32::from(mask));
            gl.uniform_1_f32(self.opacity.as_ref(), frame.vehicle_opacity.clamp(0., 1.));
            let mut palette = [0f32; 128];
            // Three affine rows per siren also fit the OpenGL 2.1 uniform minimum.
            let mut transforms = [0f32; 32 * 12];
            for matrix in transforms.as_chunks_mut::<12>().0 {
                matrix.copy_from_slice(&Mat4::IDENTITY.to_cols_array()[..12]);
            }
            for lamp in &frame.lights {
                if (1..=32).contains(&lamp.id) {
                    let o = (lamp.id as usize - 1) * 4;
                    let offset = (lamp.id as usize - 1) * 12;
                    transforms[offset..offset + 12].copy_from_slice(
                        &siren_transform(lamp.origin, lamp.scale, lamp.angle)
                            .transpose()
                            .to_cols_array()[..12],
                    );
                    palette[o..o + 4].copy_from_slice(&[
                        lamp.color[0] as f32 / 255.,
                        lamp.color[1] as f32 / 255.,
                        lamp.color[2] as f32 / 255.,
                        lamp.intensity,
                    ]);
                }
            }
            gl.uniform_4_f32_slice(self.siren_colors.as_ref(), &palette);
            gl.uniform_4_f32_slice(self.siren_transforms.as_ref(), &transforms);
            let count = frame.lights.len().min(32);
            gl.uniform_1_i32(self.light_count.as_ref(), count as i32);
            if count > 0 {
                let mut positions = Vec::with_capacity(count * 4);
                let mut colors = Vec::with_capacity(count * 4);
                for light in frame.lights.iter().take(count) {
                    positions.extend([
                        light.position.x,
                        light.position.y,
                        light.position.z,
                        light.radius,
                    ]);
                    colors.extend([
                        light.color[0] as f32 / 255.,
                        light.color[1] as f32 / 255.,
                        light.color[2] as f32 / 255.,
                        light.intensity,
                    ]);
                }
                gl.uniform_4_f32_slice(self.light_positions.as_ref(), &positions);
                gl.uniform_4_f32_slice(self.light_colors.as_ref(), &colors);
            }
            gl.active_texture(glow::TEXTURE0);
            if let Some(vao) = self.vao {
                gl.bind_vertex_array(Some(vao));
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            let stride = (FLOATS * 4) as i32;
            for (location, size, offset) in
                [(0, 3, 0), (1, 3, 12), (2, 2, 24), (3, 1, 32), (4, 1, 36)]
            {
                gl.vertex_attrib_pointer_f32(location, size, glow::FLOAT, false, stride, offset);
                gl.enable_vertex_attrib_array(location);
            }
            let paint = frame.paint.map(|c| c as f32 / 255.);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            if !mask {
                gl.enable(glow::POLYGON_OFFSET_FILL);
                gl.polygon_offset(1., 1.);
            }
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
            // Opaque body, full-strength lenses, then transparent housing/body.
            // Faded body never writes depth, so it cannot hide the edited lights.
            let transparent = |g: &Group| g.glass || g.alpha || frame.vehicle_opacity < 1.;
            let mut order: Vec<_> = frame.mesh.groups.iter().enumerate().collect();
            order.sort_by(|(_, a), (_, b)| {
                mvp.project_point3(b.center)
                    .z
                    .total_cmp(&mvp.project_point3(a.center).z)
            });
            for pass in 0..3 {
                if mask && pass != 1 {
                    continue;
                }
                gl.uniform_1_i32(self.lens_pass.as_ref(), if pass == 1 { 1 } else { 2 });
                for &(i, group) in &order {
                    if match pass {
                        0 => transparent(group),
                        1 => !frame.siren_surfaces || !group.has_sirens,
                        _ => !transparent(group),
                    } {
                        continue;
                    }
                    gl.depth_mask(!mask && pass != 2);
                    gl.uniform_1_i32(self.glass.as_ref(), i32::from(group.glass));
                    gl.uniform_1_i32(self.cutout.as_ref(), i32::from(group.cutout));
                    gl.uniform_1_i32(self.alpha.as_ref(), i32::from(group.alpha));
                    gl.uniform_1_i32(self.vertex_alpha.as_ref(), i32::from(group.vertex_alpha));
                    let bound = if group.mapped { texture(i) } else { None };
                    gl.bind_texture(glow::TEXTURE_2D, bound);
                    gl.uniform_1_i32(self.mode.as_ref(), i32::from(bound.is_some()));
                    gl.uniform_4_f32(self.color.as_ref(), paint[0], paint[1], paint[2], 1.);
                    gl.draw_elements(
                        glow::TRIANGLES,
                        group.count as i32,
                        glow::UNSIGNED_INT,
                        (group.start * 4) as i32,
                    );
                }
            }
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.bind_texture(glow::TEXTURE_2D, None);
            if frame.wire && !mask {
                gl.depth_mask(false);
                gl.uniform_1_i32(self.mode.as_ref(), 2);
                gl.uniform_4_f32(self.color.as_ref(), 0.08, 0.08, 0.09, 0.45);
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.lbo));
                gl.draw_elements(
                    glow::LINES,
                    frame.mesh.lines.len() as i32,
                    glow::UNSIGNED_INT,
                    0,
                );
                gl.depth_mask(true);
            }
            if self.vao.is_some() {
                gl.bind_vertex_array(None);
            } else {
                for location in 0..5 {
                    gl.disable_vertex_attrib_array(location);
                }
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
            gl.disable(glow::DEPTH_TEST);
            gl.depth_mask(true);
        }
    }
}

fn bytes<T: Copy>(data: &[T]) -> &[u8] {
    // Plain f32/u32 arrays; reading them as bytes is always valid.
    unsafe { std::slice::from_raw_parts(data.as_ptr().cast(), std::mem::size_of_val(data)) }
}

/// Shared between the UI thread and the paint callback.
pub(super) type SharedGpu = Arc<Mutex<Gpu>>;

pub(super) fn callback(gpu: &SharedGpu, rect: Rect, frame: Frame) -> egui::PaintCallback {
    let gpu = gpu.clone();
    egui::PaintCallback {
        rect,
        callback: Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
            if let Ok(mut gpu) = gpu.lock() {
                gpu.paint(painter.gl(), &info, &frame);
            }
        })),
    }
}
use eframe::egui_glow;

#[cfg(test)]
mod tests {
    use super::*;

    fn quad_model() -> Model {
        // A 4 x 1 x 2 box face set in GTA axes: long in Y, tall in Z.
        let mesh = |material: &str, z: f32| duck_workshop::models::Mesh {
            vertex_alpha: vec![],
            siren_ids: vec![],
            name: material.into(),
            material: material.into(),
            lod: "High".into(),
            positions: vec![[-1., -2., z], [1., -2., z], [1., 2., z], [-1., 2., z]],
            uvs: vec![vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]]],
            indices: vec![0, 1, 2, 0, 2, 3],
            enabled: true,
        };
        Model {
            name: "test.yft".into(),
            meshes: vec![mesh("body", 0.), mesh("glass", 1.), mesh("body", 2.)],
            warnings: vec![],
            ..Default::default()
        }
    }

    fn project(camera: &Camera, mesh: &SceneMesh, p: Vec3) -> Vec3 {
        let (mvp, _) = matrices(camera, mesh, 1.);
        mvp.project_point3(p)
    }

    #[test]
    fn carcols_facing_rotates_scaled_lenses_about_their_vehicle_pivots() {
        use std::f32::consts::{FRAC_PI_2, PI};
        let pivot = convert_position([-0.72, -0.37, -0.305], true);
        let lens = convert_position([-0.74, -0.367, -0.30], true);
        for (angle, expected) in [
            (0., [-2.72, -0.07, 0.195]),
            (FRAC_PI_2, [-1.02, -2.37, 0.195]),
            (-FRAC_PI_2, [-0.42, 1.63, 0.195]),
            (PI, [1.28, -0.67, 0.195]),
        ] {
            let matrix = siren_transform(pivot, 100., angle);
            assert!(matrix.transform_point3(pivot).abs_diff_eq(pivot, 0.0001));
            assert!(
                matrix
                    .transform_point3(lens)
                    .abs_diff_eq(convert_position(expected, true), 0.0001)
            );
            // Exercise the exact three-row representation uploaded to the shader.
            let rows = matrix.transpose().to_cols_array();
            let point = lens.extend(1.);
            let uploaded = Vec3::new(
                glam::Vec4::from_slice(&rows[..4]).dot(point),
                glam::Vec4::from_slice(&rows[4..8]).dot(point),
                glam::Vec4::from_slice(&rows[8..12]).dot(point),
            );
            assert!(uploaded.abs_diff_eq(matrix.transform_point3(lens), 0.0001));
        }
        assert!(siren_transform(pivot, 1., 0.).abs_diff_eq(Mat4::IDENTITY, 0.0001));
    }

    #[test]
    fn lens_channels_are_uploaded_per_vertex_and_only_for_selected_meshes() {
        let mut model = quad_model();
        model.meshes[0].siren_ids = vec![7; 4];
        model.meshes[0].vertex_alpha = vec![64; 4];
        model.meshes[1].siren_ids = vec![8; 4];
        model.meshes[1].enabled = false;
        let scene = build(&model, &render::UvOptions::default(), true, 5).unwrap();
        assert_eq!(scene.siren_ids, [7]);
        assert_eq!(scene.vertices[9], 64. / 255.);
        assert_eq!(
            scene.vertices[4 * FLOATS + 9],
            1.,
            "old meshes default to opaque vertex alpha"
        );
        assert!(
            scene
                .vertices
                .as_chunks::<FLOATS>()
                .0
                .iter()
                .take(4)
                .all(|v| v[8] == 7.)
        );
        assert!(
            scene
                .vertices
                .as_chunks::<FLOATS>()
                .0
                .iter()
                .skip(4)
                .all(|v| v[8] == 0.)
        );
    }
    #[test]
    fn groups_by_material_and_turns_z_up_models_upright() {
        let opt = render::UvOptions {
            flip_v: false,
            ..Default::default()
        };
        let mesh = build(&quad_model(), &opt, true, 1).unwrap();
        let names: Vec<_> = mesh.groups.iter().map(|g| g.material.as_str()).collect();
        assert_eq!(names, ["body", "glass"]);
        assert_eq!(mesh.groups[0].count, 12);
        // GTA Z (height) becomes preview Y; GTA +Y (forward) becomes +Z.
        assert!((mesh.center.y - 1.).abs() < 1e-5);
        assert_eq!(mesh.vertices[FLOATS * 2 + 2], 2.);
        let flipped = build(&quad_model(), &render::UvOptions::default(), true, 2).unwrap();
        assert_eq!(flipped.vertices[7], 1., "Flip V samples 1 - v");
    }

    #[test]
    fn whole_vehicle_includes_disabled_parts_with_material_uvs_without_changing_exports() {
        let mut model = quad_model();
        model.meshes[1].enabled = false;
        model.meshes[0]
            .uvs
            .push(vec![[0.25, 0.75], [0.5, 0.75], [0.5, 1.], [0.25, 1.]]);
        model.material_info.insert(
            "body".into(),
            duck_workshop::models::MaterialInfo {
                shader: "vehicle_paint3".into(),
                secondary_texture: "sign_1".into(),
                ..Default::default()
            },
        );
        let mut low = model.meshes[1].clone();
        low.lod = "Low".into();
        model.meshes.push(low);
        let opt = render::UvOptions {
            material: Some("body".into()),
            channel: 1,
            flip_v: false,
            lod: "All".into(),
            ..Default::default()
        };
        let before = render::uv_png(&model, &opt).unwrap();
        let full = build_vehicle(&model, &opt, true, 2).unwrap();
        assert_eq!(full.indices.len(), 18, "one LOD, all three parts");
        assert_eq!(&full.vertices[6..8], &[0.25, 0.75]);
        assert_eq!(&full.vertices[4 * FLOATS + 6..4 * FLOATS + 8], &[0., 0.]);
        assert_eq!(build(&model, &opt, true, 3).unwrap().indices.len(), 12);
        assert_eq!(render::uv_png(&model, &opt).unwrap(), before);
        assert!(!model.meshes[1].enabled);
    }

    #[test]
    fn glass_alpha_and_cutout_materials_retain_their_distinct_rendering_roles() {
        let mut model = quad_model();
        model.material_info.insert(
            "glass".into(),
            duck_workshop::models::MaterialInfo {
                shader: "vehicle_vehglass".into(),
                ..Default::default()
            },
        );
        model.material_info.insert(
            "body".into(),
            duck_workshop::models::MaterialInfo {
                shader: "vehicle_mesh".into(),
                render_bucket: 1,
                ..Default::default()
            },
        );
        model.meshes[0].siren_ids = vec![2; 4];
        let mesh = build(&model, &Default::default(), true, 1).unwrap();
        assert!(mesh.groups[0].alpha && mesh.groups[0].has_sirens);
        assert!(!mesh.groups[0].glass && !mesh.groups[0].cutout);
        assert!(mesh.groups[1].glass && !mesh.groups[1].has_sirens);
        assert!(mesh.groups[1].vertex_alpha);
        model.material_info.get_mut("body").unwrap().shader = "vehicle_lightsemissive".into();
        let mesh = build(&model, &Default::default(), true, 2).unwrap();
        assert!(
            mesh.groups[0].alpha && !mesh.groups[0].vertex_alpha,
            "emissive vertex colors are not glass opacity"
        );
        model.material_info.get_mut("body").unwrap().shader = "vehicle_mesh".into();
        model.material_info.get_mut("body").unwrap().render_bucket = 3;
        let mesh = build(&model, &Default::default(), true, 2).unwrap();
        assert!(mesh.groups[0].cutout && !mesh.groups[0].alpha);
    }

    #[test]
    fn orbit_pan_zoom_and_views_move_the_camera() {
        let mesh = build(&quad_model(), &render::UvOptions::default(), true, 1).unwrap();
        let front = Vec3::new(0., 1., 2.);
        let camera = Camera::default();
        let start = project(&camera, &mesh, front);
        let mut turned = camera;
        turned.yaw += 0.8;
        assert!((project(&turned, &mesh, front).x - start.x).abs() > 0.05);

        let mut moved = camera;
        pan(&mut moved, Vec2::new(100., 0.), 500.);
        let shifted = project(&moved, &mesh, mesh.center);
        assert!(shifted.x > 0.1, "dragging right moves the model right");

        let mut closer = camera;
        closer.distance *= 0.5;
        let a =
            project(&camera, &mesh, Vec3::new(1., 1., 2.)) - project(&camera, &mesh, mesh.center);
        let b =
            project(&closer, &mesh, Vec3::new(1., 1., 2.)) - project(&closer, &mesh, mesh.center);
        assert!(
            b.length() > a.length() * 1.5,
            "zooming in enlarges the model"
        );

        let (_, yaw, pitch) = VIEWS[4];
        let top = Camera {
            yaw,
            pitch,
            ..camera
        };
        // From the top, the front of the model is toward the top of the screen.
        let nose = project(&top, &mesh, Vec3::new(0., 1., 2.));
        let tail = project(&top, &mesh, Vec3::new(0., 1., -2.));
        assert!(nose.y > tail.y);
        let (_, yaw, pitch) = VIEWS[0];
        let front_view = Camera {
            yaw,
            pitch,
            ..camera
        };
        assert!(
            project(&front_view, &mesh, Vec3::new(0., 1., 2.)).z
                < project(&front_view, &mesh, Vec3::new(0., 1., -2.)).z
        );
    }
}
