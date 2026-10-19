//! Blur only visible emissive lens pixels, preserving the vehicle depth buffer.
use super::*;

#[derive(Default)]
struct Targets {
    size: [i32; 2],
    textures: Vec<glow::Texture>,
    buffers: Vec<glow::Framebuffer>,
    depth: Option<glow::Renderbuffer>,
}
impl Targets {
    unsafe fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
            for t in self.textures.drain(..) {
                gl.delete_texture(t);
            }
            for b in self.buffers.drain(..) {
                gl.delete_framebuffer(b);
            }
            if let Some(d) = self.depth.take() {
                gl.delete_renderbuffer(d);
            }
        }
    }
    unsafe fn new(gl: &glow::Context, size: [i32; 2]) -> Result<Self> {
        unsafe {
            let mut target = Self {
                size,
                ..Default::default()
            };
            let result = (|| -> Result<()> {
                let limit = gl
                    .get_parameter_i32(glow::MAX_TEXTURE_SIZE)
                    .min(gl.get_parameter_i32(glow::MAX_RENDERBUFFER_SIZE));
                if size.iter().any(|&n| n < 1 || n > limit) {
                    return Err("Lighting viewport exceeds GPU limits".into());
                }
                let depth = gl.create_renderbuffer()?;
                target.depth = Some(depth);
                gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
                gl.renderbuffer_storage(
                    glow::RENDERBUFFER,
                    glow::DEPTH_COMPONENT24,
                    size[0],
                    size[1],
                );
                for i in 0..3 {
                    let texture = gl.create_texture()?;
                    target.textures.push(texture);
                    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                    gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::RGBA8 as i32,
                        size[0],
                        size[1],
                        0,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(None),
                    );
                    for parameter in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
                        gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, glow::LINEAR as i32);
                    }
                    for parameter in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
                        gl.tex_parameter_i32(
                            glow::TEXTURE_2D,
                            parameter,
                            glow::CLAMP_TO_EDGE as i32,
                        );
                    }
                    let buffer = gl.create_framebuffer()?;
                    target.buffers.push(buffer);
                    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(buffer));
                    gl.framebuffer_texture_2d(
                        glow::FRAMEBUFFER,
                        glow::COLOR_ATTACHMENT0,
                        glow::TEXTURE_2D,
                        Some(texture),
                        0,
                    );
                    if i < 2 {
                        gl.framebuffer_renderbuffer(
                            glow::FRAMEBUFFER,
                            glow::DEPTH_ATTACHMENT,
                            glow::RENDERBUFFER,
                            Some(depth),
                        );
                    }
                    if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE
                    {
                        return Err("Lighting framebuffer is incomplete".into());
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                target.destroy(gl);
                return Err(error);
            }
            Ok(target)
        }
    }
}

pub(super) struct Bloom {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    source: Option<glow::UniformLocation>,
    scene: Option<glow::UniformLocation>,
    direction: Option<glow::UniformLocation>,
    composite: Option<glow::UniformLocation>,
    targets: Targets,
}
impl Bloom {
    pub unsafe fn new(gl: &glow::Context) -> Result<Self> {
        unsafe {
            let version = egui_glow::ShaderVersion::get(gl);
            let header = version.version_declaration();
            let precision = if version.is_embedded() {
                "precision highp float;\n"
            } else {
                ""
            };
            let vertex = "attribute vec2 a_position; varying vec2 v_uv; void main(){v_uv=a_position*0.5+0.5;gl_Position=vec4(a_position,0.0,1.0);}";
            let fragment = r#"
                uniform sampler2D u_source; uniform sampler2D u_scene;
                uniform vec2 u_direction; uniform int u_composite; varying vec2 v_uv;
                void main(){
                    vec3 glow=texture2D(u_source,v_uv).rgb*0.227027;
                    glow+=(texture2D(u_source,v_uv+u_direction*1.384615).rgb+texture2D(u_source,v_uv-u_direction*1.384615).rgb)*0.316216;
                    glow+=(texture2D(u_source,v_uv+u_direction*3.230769).rgb+texture2D(u_source,v_uv-u_direction*3.230769).rgb)*0.070270;
                    vec4 scene=u_composite==1?texture2D(u_scene,v_uv):vec4(0.0);
                    gl_FragColor=vec4(scene.rgb+glow,scene.a);
                }"#;
            let program = gl.create_program()?;
            for (kind, prefix, source) in [
                (
                    glow::VERTEX_SHADER,
                    if version.is_new_shader_interface() {
                        "#define attribute in\n#define varying out\n"
                    } else {
                        ""
                    },
                    vertex,
                ),
                (
                    glow::FRAGMENT_SHADER,
                    if version.is_new_shader_interface() {
                        "#define varying in\nout vec4 out_color;\n#define gl_FragColor out_color\n#define texture2D texture\n"
                    } else {
                        ""
                    },
                    fragment,
                ),
            ] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, &format!("{header}{precision}{prefix}{source}"));
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let error = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    gl.delete_program(program);
                    return Err(error);
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.bind_attrib_location(program, 0, "a_position");
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                let error = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(error);
            }
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            let quad: [f32; 12] = [-1., -1., 1., -1., 1., 1., -1., -1., 1., 1., -1., 1.];
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes(&quad), glow::STATIC_DRAW);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
            gl.enable_vertex_attrib_array(0);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            Ok(Self {
                source: gl.get_uniform_location(program, "u_source"),
                scene: gl.get_uniform_location(program, "u_scene"),
                direction: gl.get_uniform_location(program, "u_direction"),
                composite: gl.get_uniform_location(program, "u_composite"),
                program,
                vao,
                vbo,
                targets: Targets::default(),
            })
        }
    }
    pub unsafe fn render(
        &mut self,
        gl: &glow::Context,
        info: &egui::PaintCallbackInfo,
        draw: impl Fn(bool),
    ) -> Result<()> {
        unsafe {
            let viewport = info.viewport_in_pixels();
            let size = [viewport.width_px.max(1), viewport.height_px.max(1)];
            let framebuffer = gl.get_parameter_framebuffer(glow::DRAW_FRAMEBUFFER_BINDING);
            let mut previous_viewport = [0; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut previous_viewport);
            let scissor = gl.is_enabled(glow::SCISSOR_TEST);
            let result = (|| -> Result<()> {
                if self.targets.size != size {
                    self.targets.destroy(gl);
                    self.targets = Targets::new(gl, size)?;
                }
                gl.disable(glow::SCISSOR_TEST);
                gl.viewport(0, 0, size[0], size[1]);
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.targets.buffers[0]));
                gl.clear_color(0., 0., 0., 0.);
                gl.depth_mask(true);
                gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
                draw(false);
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.targets.buffers[1]));
                gl.clear(glow::COLOR_BUFFER_BIT);
                draw(true);
                gl.disable(glow::DEPTH_TEST);
                gl.disable(glow::BLEND);
                gl.use_program(Some(self.program));
                gl.bind_vertex_array(Some(self.vao));
                gl.uniform_1_i32(self.source.as_ref(), 0);
                gl.uniform_1_i32(self.scene.as_ref(), 1);
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.targets.textures[1]));
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.targets.buffers[2]));
                gl.uniform_1_i32(self.composite.as_ref(), 0);
                let blur = 1.7 * info.pixels_per_point;
                gl.uniform_2_f32(self.direction.as_ref(), blur / size[0] as f32, 0.);
                gl.draw_arrays(glow::TRIANGLES, 0, 6);
                gl.bind_framebuffer(glow::FRAMEBUFFER, framebuffer);
                gl.viewport(
                    previous_viewport[0],
                    previous_viewport[1],
                    previous_viewport[2],
                    previous_viewport[3],
                );
                if scissor {
                    gl.enable(glow::SCISSOR_TEST);
                }
                gl.active_texture(glow::TEXTURE1);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.targets.textures[0]));
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.targets.textures[2]));
                gl.uniform_1_i32(self.composite.as_ref(), 1);
                gl.uniform_2_f32(self.direction.as_ref(), 0., blur / size[1] as f32);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
                gl.draw_arrays(glow::TRIANGLES, 0, 6);
                Ok(())
            })();
            gl.bind_framebuffer(glow::FRAMEBUFFER, framebuffer);
            gl.viewport(
                previous_viewport[0],
                previous_viewport[1],
                previous_viewport[2],
                previous_viewport[3],
            );
            if scissor {
                gl.enable(glow::SCISSOR_TEST);
            } else {
                gl.disable(glow::SCISSOR_TEST);
            }
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_vertex_array(None);
            gl.bind_renderbuffer(glow::RENDERBUFFER, None);
            gl.depth_mask(true);
            result
        }
    }
    pub unsafe fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
            self.targets.destroy(gl);
            gl.delete_buffer(self.vbo);
            gl.delete_vertex_array(self.vao);
            gl.delete_program(self.program);
        }
    }
}
