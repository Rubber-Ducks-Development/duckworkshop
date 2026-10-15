//! Textured 3D model preview drawn with OpenGL inside the egui frame.
#![allow(unsafe_code)]
use super::*;
use eframe::glow::{self, HasContext};
use glam::{Mat4, Vec3};
use image::RgbaImage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const FOV: f32 = 0.75;
const FLOATS: usize = 8;

/// Geometry ready for upload: interleaved position, normal and texture coordinate,
/// with triangles grouped by material so each group can use its own texture.
pub(super) struct SceneMesh {
    version: u64,
    vertices: Vec<f32>,
    indices: Vec<u32>,
    lines: Vec<u32>,
    pub groups: Vec<Group>,
    center: Vec3,
    radius: f32,
}
pub(super) struct Group {
    pub material: String,
    start: usize,
    count: usize,
    pub mapped: bool,
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
            vertices.extend_from_slice(&[p.x, p.y, p.z, n.x, n.y, n.z, u, t]);
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
    Some(SceneMesh {
        version,
        vertices,
        indices,
        lines,
        groups,
        center: (min + max) * 0.5,
        radius: ((max - min).length() * 0.5).max(0.001),
    })
}

/// What one frame draws. Textures are shared and uploaded only when their version changes.
pub(super) struct Frame {
    pub mesh: Arc<SceneMesh>,
    pub textures: Vec<(PathBuf, u64, Arc<RgbaImage>)>,
    /// Texture per mesh group, by index into `textures`.
    pub bindings: Vec<Option<usize>>,
    pub camera: Camera,
    pub wire: bool,
    pub paint: [u8; 3],
}

#[derive(Default)]
pub(super) struct Gpu {
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
            r.draw(gl, info, frame, |i| {
                frame.bindings[i]
                    .and_then(|t| frame.textures.get(t))
                    .and_then(|(p, ..)| self.textures.get(p))
                    .map(|(t, _)| *t)
            });
        }
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    pub(super) fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
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

const VERTEX: &str = r#"
uniform mat4 u_mvp;
attribute vec3 a_pos;
attribute vec3 a_normal;
attribute vec2 a_uv;
varying vec3 v_normal;
varying vec2 v_uv;
void main() {
    v_normal = a_normal;
    v_uv = a_uv;
    gl_Position = u_mvp * vec4(a_pos, 1.0);
}
"#;
const FRAGMENT: &str = r#"
uniform sampler2D u_texture;
uniform vec3 u_light;
uniform vec4 u_color;
uniform int u_mode;
varying vec3 v_normal;
varying vec2 v_uv;
void main() {
    if (u_mode == 2) {
        gl_FragColor = vec4(u_color.rgb * u_color.a, u_color.a);
        return;
    }
    vec3 base = u_color.rgb;
    if (u_mode == 1) {
        vec4 texel = texture2D(u_texture, v_uv);
        base = mix(base, texel.rgb, texel.a);
    }
    float diffuse = abs(dot(normalize(v_normal), u_light));
    gl_FragColor = vec4(base * (0.36 + 0.64 * diffuse), 1.0);
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
            for (i, name) in ["a_pos", "a_normal", "a_uv"].iter().enumerate() {
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
    ) {
        unsafe {
            let viewport = info.viewport_in_pixels();
            let aspect = viewport.width_px as f32 / viewport.height_px.max(1) as f32;
            let (mvp, light) = matrices(&frame.camera, &frame.mesh, aspect);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.depth_mask(true);
            gl.clear_depth(1.);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(self.mvp.as_ref(), false, &mvp.to_cols_array());
            gl.uniform_3_f32(self.light.as_ref(), light.x, light.y, light.z);
            gl.uniform_1_i32(self.sampler.as_ref(), 0);
            gl.active_texture(glow::TEXTURE0);
            if let Some(vao) = self.vao {
                gl.bind_vertex_array(Some(vao));
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            let stride = (FLOATS * 4) as i32;
            for (location, size, offset) in [(0, 3, 0), (1, 3, 12), (2, 2, 24)] {
                gl.vertex_attrib_pointer_f32(location, size, glow::FLOAT, false, stride, offset);
                gl.enable_vertex_attrib_array(location);
            }
            let paint = frame.paint.map(|c| c as f32 / 255.);
            gl.enable(glow::POLYGON_OFFSET_FILL);
            gl.polygon_offset(1., 1.);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
            for (i, group) in frame.mesh.groups.iter().enumerate() {
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
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.bind_texture(glow::TEXTURE_2D, None);
            if frame.wire {
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
                for location in 0..3 {
                    gl.disable_vertex_attrib_array(location);
                }
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
            gl.disable(glow::DEPTH_TEST);
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
        }
    }

    fn project(camera: &Camera, mesh: &SceneMesh, p: Vec3) -> Vec3 {
        let (mvp, _) = matrices(camera, mesh, 1.);
        mvp.project_point3(p)
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
