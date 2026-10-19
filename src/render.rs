use crate::{Result, models::Model};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke, Transform};
#[derive(Clone, Serialize, Deserialize)]
pub struct UvOptions {
    #[serde(default)]
    pub surface_colors: bool,
    #[serde(default)]
    pub material: Option<String>,
    pub size: u32,
    pub channel: usize,
    pub lod: String,
    pub color: [u8; 4],
    pub background: [u8; 4],
    pub width: f32,
    pub flip_v: bool,
    pub boundaries: bool,
}
impl Default for UvOptions {
    fn default() -> Self {
        Self {
            surface_colors: false,
            material: None,
            size: 2048,
            channel: 0,
            lod: "High".into(),
            color: [20, 24, 28, 255],
            background: [255, 255, 255, 0],
            width: 1.,
            flip_v: true,
            boundaries: false,
        }
    }
}
pub fn selected<'a>(
    model: &'a Model,
    opt: &UvOptions,
) -> impl Iterator<Item = &'a crate::models::Mesh> {
    let lod = opt.lod.clone();
    let material = opt.material.clone();
    model.meshes.iter().filter(move |m| {
        m.enabled
            && (lod == "All" || m.lod == lod)
            && material.as_ref().is_none_or(|name| m.material == *name)
    })
}

pub fn uv_channels(model: &Model, opt: &UvOptions) -> Vec<usize> {
    (0..8)
        .filter(|&channel| {
            selected(model, opt).any(|m| m.uvs.get(channel).is_some_and(|uv| !uv.is_empty()))
        })
        .collect()
}

pub fn select_material(model: &Model, opt: &mut UvOptions, material: Option<String>) {
    opt.material = material;
    let channels = uv_channels(model, opt);
    let livery = opt
        .material
        .as_ref()
        .and_then(|name| model.material_info.get(name))
        .is_some_and(|info| info.is_livery());
    opt.channel = if livery && channels.contains(&1) {
        1
    } else {
        channels.first().copied().unwrap_or(0)
    };
}

/// Start on a single texture's highest available LOD. Vehicle paint3's second
/// diffuse sampler uses UV 1 for the livery; base textures normally use UV 0.
pub fn select_template(model: &Model, opt: &mut UvOptions) {
    opt.lod = ["High", "Med", "Low", "VeryLow"]
        .into_iter()
        .find(|lod| {
            model
                .meshes
                .iter()
                .any(|m| m.enabled && m.lod == *lod && m.uvs.iter().any(|uv| !uv.is_empty()))
        })
        .unwrap_or("All")
        .into();
    let mut candidates = std::collections::BTreeMap::<String, (bool, usize)>::new();
    for m in model.meshes.iter().filter(|m| {
        m.enabled && (opt.lod == "All" || m.lod == opt.lod) && m.uvs.iter().any(|uv| !uv.is_empty())
    }) {
        let is_livery = model
            .material_info
            .get(&m.material)
            .is_some_and(|info| info.is_livery())
            && m.uvs.get(1).is_some_and(|uv| !uv.is_empty());
        let entry = candidates.entry(m.material.clone()).or_default();
        entry.0 |= is_livery;
        entry.1 += m.indices.len();
    }
    let material = candidates
        .into_iter()
        .max_by_key(|(_, rank)| *rank)
        .map(|(name, _)| name);
    select_material(model, opt, material);
    set_template_style(opt, true);
    opt.size = 4096;
}

pub fn set_template_style(opt: &mut UvOptions, colored: bool) {
    opt.surface_colors = colored;
    opt.boundaries = !colored;
    opt.background = if colored {
        [0, 0, 0, 255]
    } else {
        [255, 255, 255, 0]
    };
    opt.color = if colored {
        [18, 18, 24, 205]
    } else {
        [20, 24, 28, 255]
    };
    opt.width = 1.;
}

#[derive(Clone, Copy)]
pub struct UvFace {
    pub uv: [[f32; 2]; 3],
    pub colors: [[u8; 3]; 3],
}

/// Bake an object-space normal guide into the original UV layout. Face normals
/// are averaged at each vertex to reveal curved panels without changing UVs.
pub fn uv_faces(model: &Model, opt: &UvOptions) -> Vec<UvFace> {
    let mut faces = vec![];
    for mesh in selected(model, opt) {
        let Some(uv) = mesh.uvs.get(opt.channel).filter(|uv| !uv.is_empty()) else {
            continue;
        };
        let mut normals = vec![glam::Vec3::ZERO; mesh.positions.len()];
        for tri in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = tri.map(|i| glam::Vec3::from(mesh.positions[i as usize]));
            let normal = (b - a).cross(c - a);
            for &i in tri {
                normals[i as usize] += normal;
            }
        }
        for tri in mesh.indices.as_chunks::<3>().0 {
            faces.push(UvFace {
                uv: tri.map(|i| uv[i as usize]),
                colors: tri.map(|i| {
                    normals[i as usize]
                        .normalize_or(glam::Vec3::Z)
                        .to_array()
                        .map(|n| {
                            ((0.25 + 0.75 * (n * 0.5 + 0.5)) * 255.)
                                .round()
                                .clamp(0., 255.) as u8
                        })
                }),
            });
        }
    }
    faces
}

fn surface_pixmap(model: &Model, opt: &UvOptions, background: bool) -> Result<Pixmap> {
    let mut pix = Pixmap::new(opt.size, opt.size).ok_or("Cannot allocate image")?;
    if background {
        pix.fill(tiny_skia::Color::from_rgba8(
            opt.background[0],
            opt.background[1],
            opt.background[2],
            opt.background[3],
        ));
    }
    if !opt.surface_colors {
        return Ok(pix);
    }
    let size = opt.size as f32;
    let pixels = pix.data_mut();
    for face in uv_faces(model, opt) {
        let p = face.uv.map(|uv| {
            [
                uv[0] * size,
                (if opt.flip_v { 1. - uv[1] } else { uv[1] }) * size,
            ]
        });
        let edge = |a: [f32; 2], b: [f32; 2], x: f32, y: f32| {
            (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
        };
        let area = edge(p[0], p[1], p[2][0], p[2][1]);
        if !area.is_finite() || area.abs() < 1e-8 {
            continue;
        }
        let min_x = p
            .iter()
            .map(|p| p[0])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let min_y = p
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let max_x = p
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .clamp(0., size) as u32;
        let max_y = p
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .clamp(0., size) as u32;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w = [
                    edge(p[1], p[2], px, py) / area,
                    edge(p[2], p[0], px, py) / area,
                    edge(p[0], p[1], px, py) / area,
                ];
                if w.iter().any(|&w| w < -1e-5) {
                    continue;
                }
                let at = ((y * opt.size + x) * 4) as usize;
                for c in 0..3 {
                    pixels[at + c] = (w[0] * face.colors[0][c] as f32
                        + w[1] * face.colors[1][c] as f32
                        + w[2] * face.colors[2][c] as f32)
                        .round()
                        .clamp(0., 255.) as u8;
                }
                pixels[at + 3] = 255;
            }
        }
    }
    Ok(pix)
}
pub fn uv_edges(model: &Model, opt: &UvOptions) -> Vec<([f32; 2], [f32; 2])> {
    let mut edges = std::collections::BTreeMap::new();
    for m in selected(model, opt) {
        let Some(uv) = m.uvs.get(opt.channel).filter(|u| !u.is_empty()) else {
            continue;
        };
        // Count adjacency in each surface's 3D+UV topology. Counting solely by
        // UV coordinates erases boundaries when mirrored parts overlap in UVs.
        let mut topology = std::collections::BTreeMap::new();
        let mut triangles = BTreeSet::new();
        let vertex = |index: u32| {
            let p = m.positions[index as usize];
            let t = uv[index as usize];
            [p[0], p[1], p[2], t[0], t[1]].map(|v| if v == 0. { 0 } else { v.to_bits() })
        };
        for tri in m.indices.as_chunks::<3>().0 {
            let mut triangle = [vertex(tri[0]), vertex(tri[1]), vertex(tri[2])];
            triangle.sort_unstable();
            if !triangles.insert(triangle) {
                continue;
            }
            for i in 0..3 {
                let a = uv[tri[i] as usize];
                let b = uv[tri[(i + 1) % 3] as usize];
                if a == b {
                    continue;
                }
                let ka = vertex(tri[i]);
                let kb = vertex(tri[(i + 1) % 3]);
                let key = if ka < kb { (ka, kb) } else { (kb, ka) };
                let entry = topology.entry(key).or_insert((a, b, 0u32));
                entry.2 += 1;
            }
        }
        for (a, b, count) in topology.into_values() {
            if opt.boundaries && count != 1 {
                continue;
            }
            let key = |v: [f32; 2]| v.map(|v| if v == 0. { 0 } else { v.to_bits() });
            let (ka, kb) = (key(a), key(b));
            let key = if ka < kb { (ka, kb) } else { (kb, ka) };
            edges.entry(key).or_insert((a, b));
        }
    }
    edges.into_values().collect()
}
fn check(opt: &UvOptions) -> Result<()> {
    if !(64..=8192).contains(&opt.size) || !(0.1..=20.).contains(&opt.width) || opt.channel > 7 {
        return Err("UV export requires 64–8192 pixels, 0.1–20 px lines and channel 0–7.".into());
    }
    Ok(())
}
pub fn uv_png(model: &Model, opt: &UvOptions) -> Result<Vec<u8>> {
    model.validate()?;
    check(opt)?;
    let edges = uv_edges(model, opt);
    if edges.is_empty() {
        return Err("No UVs in the selected meshes, LOD and channel.".into());
    }
    let mut pix = surface_pixmap(model, opt, true)?;
    let mut path = PathBuilder::new();
    for (a, b) in edges {
        let y = |v: f32| if opt.flip_v { 1. - v } else { v };
        path.move_to(a[0] * opt.size as f32, y(a[1]) * opt.size as f32);
        path.line_to(b[0] * opt.size as f32, y(b[1]) * opt.size as f32);
    }
    let mut paint = Paint::default();
    paint.set_color_rgba8(opt.color[0], opt.color[1], opt.color[2], opt.color[3]);
    if let Some(path) = path.finish() {
        pix.stroke_path(
            &path,
            &paint,
            &Stroke {
                width: opt.width,
                ..Default::default()
            },
            Transform::identity(),
            None,
        );
    }
    pix.encode_png().map_err(|e| e.to_string())
}
pub fn uv_svg(model: &Model, opt: &UvOptions) -> Result<String> {
    model.validate()?;
    check(opt)?;
    let edges = uv_edges(model, opt);
    if edges.is_empty() {
        return Err("No UVs in the selection.".into());
    }
    let s = opt.size;
    let surface = if opt.surface_colors {
        use base64::Engine;
        let png = surface_pixmap(model, opt, false)?
            .encode_png()
            .map_err(|e| e.to_string())?;
        format!(
            "<image width=\"{s}\" height=\"{s}\" href=\"data:image/png;base64,{}\"/>",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    } else {
        String::new()
    };
    let path = edges
        .iter()
        .map(|(a, b)| {
            format!(
                "M{:.5} {:.5}L{:.5} {:.5}",
                a[0] * s as f32,
                if opt.flip_v { 1. - a[1] } else { a[1] } * s as f32,
                b[0] * s as f32,
                if opt.flip_v { 1. - b[1] } else { b[1] } * s as f32
            )
        })
        .collect::<String>();
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{s}\" height=\"{s}\" viewBox=\"0 0 {s} {s}\"><defs><clipPath id=\"tile\"><rect width=\"{s}\" height=\"{s}\"/></clipPath></defs><rect width=\"{s}\" height=\"{s}\" fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{}\"/>{surface}<path d=\"{path}\" clip-path=\"url(#tile)\" fill=\"none\" stroke=\"#{:02x}{:02x}{:02x}\" stroke-opacity=\"{}\" stroke-width=\"{}\"/></svg>",
        opt.background[0],
        opt.background[1],
        opt.background[2],
        opt.background[3] as f32 / 255.,
        opt.color[0],
        opt.color[1],
        opt.color[2],
        opt.color[3] as f32 / 255.,
        opt.width
    ))
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Chevron {
    #[serde(default)]
    pub mirror_over_seam: Option<bool>,
    #[serde(default)]
    pub show_seam: bool,
    #[serde(default)]
    pub honeycomb: bool,
    #[serde(default)]
    pub reflective_983: bool,
    #[serde(default = "default_cell_size")]
    pub cell_size: f32,
    pub width: u32,
    pub height: u32,
    pub stripe: f32,
    pub angle: f32,
    pub color_a: [u8; 3],
    pub color_b: [u8; 3],
    /// Additional stripes, repeated after A and B. Empty in older projects.
    #[serde(default)]
    pub extra_colors: Vec<[u8; 3]>,
    pub opacity: u8,
    pub center: f32,
    pub offset: f32,
    pub direction: usize,
    pub uv_clip: bool,
}
impl Default for Chevron {
    fn default() -> Self {
        Self {
            mirror_over_seam: Some(true),
            show_seam: false,
            honeycomb: false,
            reflective_983: false,
            cell_size: default_cell_size(),
            width: 2048,
            height: 1024,
            stripe: 120.,
            angle: 45.,
            color_a: [255, 210, 0],
            color_b: [220, 25, 25],
            extra_colors: vec![],
            opacity: 255,
            center: 0.5,
            offset: 0.,
            direction: 0,
            uv_clip: false,
        }
    }
}
impl Chevron {
    pub fn mirrored(&self) -> bool {
        self.mirror_over_seam.unwrap_or(self.direction != 2)
    }
    pub fn apply_preset(&mut self, index: usize) {
        if let Some(preset) = CHEVRON_PRESETS.get(index) {
            self.color_a = preset.a;
            self.color_b = preset.b;
            self.extra_colors.clear();
            self.angle = 45.;
            self.direction = if preset.mirrored { 0 } else { 2 };
            self.mirror_over_seam = Some(preset.mirrored);
            self.center = 0.5;
            self.offset = 0.;
            self.stripe = self.width as f32 / 16.;
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.extra_colors.len() > 14 {
            return Err("Use at most 16 chevron colors.".into());
        }
        if !(64..=8192).contains(&self.width)
            || !(64..=8192).contains(&self.height)
            || !self.stripe.is_finite()
            || !(1.0..=2000.).contains(&self.stripe)
            || !self.angle.is_finite()
            || !(5.0..=85.).contains(&self.angle)
            || !self.center.is_finite()
            || !(0.0..=1.).contains(&self.center)
            || !self.offset.is_finite()
            || self.offset.abs() > 10000.
            || self.direction > 2
            || !self.cell_size.is_finite()
            || !(3.0..=80.).contains(&self.cell_size)
        {
            return Err("Invalid chevron size, stripe, angle or center.".into());
        }
        Ok(())
    }
    pub fn image(
        &self,
        preview: bool,
        model: Option<&Model>,
        uv: &UvOptions,
    ) -> Result<image::RgbaImage> {
        self.validate()?;
        let (w, h) = if preview {
            let factor = (1024. / self.width.max(self.height) as f32).min(1.);
            (
                (self.width as f32 * factor).round() as u32,
                (self.height as f32 * factor).round().max(1.) as u32,
            )
        } else {
            (self.width, self.height)
        };
        let mut img = image::RgbaImage::new(w, h);
        let rad = self.angle.to_radians();
        let factor = w as f32 / self.width as f32;
        for (y, row) in img.rows_mut().enumerate() {
            for (x, p) in row.enumerate() {
                let px = x as f32 / factor;
                let py = y as f32 / factor;
                let dx = px - self.width as f32 * self.center;
                let across = if self.mirrored() { dx.abs() } else { dx };
                let q = py * rad.cos()
                    + across * rad.sin() * if self.direction == 1 { -1. } else { 1. }
                    + self.offset;
                let index = ((q / self.stripe).floor() as i64)
                    .rem_euclid((2 + self.extra_colors.len()) as i64)
                    as usize;
                let color = match index {
                    0 => self.color_a,
                    1 => self.color_b,
                    i => self.extra_colors[i - 2],
                };
                let mut shade = 1.;
                if self.honeycomb {
                    let r = self.cell_size;
                    let col = (across / (1.5 * r)).round();
                    let mut distance = f32::INFINITY;
                    for c in -1..=1 {
                        let column = col + c as f32;
                        let cy_offset = column.rem_euclid(2.) * 0.8660254 * r;
                        let row = ((py - cy_offset) / (1.7320508 * r)).round();
                        for rr in -1..=1 {
                            let ox = (across - column * 1.5 * r).abs();
                            let oy = (py - ((row + rr as f32) * 1.7320508 * r + cy_offset)).abs();
                            let side = oy.max(0.8660254 * ox + 0.5 * oy) - 0.8660254 * r;
                            distance = distance.min(side.abs());
                        }
                    }
                    let edge = (1. - distance / (r * 0.13).max(0.6)).clamp(0., 1.);
                    shade *= 0.92 - 0.30 * edge;
                }
                if self.reflective_983 {
                    let prism = ((across / 4.).floor() + (py / 4.).floor()).rem_euclid(3.);
                    let micro = if prism < 1. {
                        0.82
                    } else if prism < 2. {
                        1.02
                    } else {
                        0.94
                    };
                    let sweep = 0.94 + 0.11 * ((across + py * 0.6) / 180.).sin().powi(8);
                    shade *= micro * sweep;
                }
                *p = image::Rgba([color[0], color[1], color[2], self.opacity]);
                if self.honeycomb || self.reflective_983 {
                    for c in 0..3 {
                        p[c] = (color[c] as f32 * shade).round().clamp(0., 255.) as u8;
                    }
                }
            }
        }
        if self.uv_clip {
            let model =
                model.ok_or("Import a mapped model in UV Editor to clip chevrons to UVs.")?;
            let mut mask = Pixmap::new(w, h).ok_or("Cannot allocate UV mask")?;
            let mut path = PathBuilder::new();
            let mut found = false;
            for m in selected(model, uv) {
                if let Some(coords) = m.uvs.get(uv.channel).filter(|u| !u.is_empty()) {
                    for tri in m.indices.as_chunks::<3>().0 {
                        for (i, id) in tri.iter().enumerate() {
                            let p = coords[*id as usize];
                            let yy = if uv.flip_v { 1. - p[1] } else { p[1] };
                            if i == 0 {
                                path.move_to(p[0] * w as f32, yy * h as f32)
                            } else {
                                path.line_to(p[0] * w as f32, yy * h as f32)
                            }
                        }
                        path.close();
                        found = true;
                    }
                }
            }
            if !found {
                return Err("Selected UVs are empty.".into());
            }
            let mut paint = Paint::default();
            paint.set_color_rgba8(255, 255, 255, 255);
            if let Some(path) = path.finish() {
                mask.fill_path(
                    &path,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
            for (i, p) in img.pixels_mut().enumerate() {
                p[3] = ((p[3] as u16 * mask.data()[i * 4 + 3] as u16) / 255) as u8;
            }
        }
        if preview && self.show_seam {
            let seam = (w as f32 * self.center).round().clamp(0., (w - 1) as f32) as u32;
            for y in 0..h {
                if y % 12 < 7 {
                    img.put_pixel(seam, y, image::Rgba([245, 245, 245, 220]));
                }
            }
        }
        Ok(img)
    }
    pub fn png(&self, model: Option<&Model>, uv: &UvOptions) -> Result<Vec<u8>> {
        let img = self.image(false, model, uv)?;
        let mut out = std::io::Cursor::new(vec![]);
        img.write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }
}
fn default_cell_size() -> f32 {
    12.
}

pub struct ChevronPreset {
    pub name: &'static str,
    pub description: &'static str,
    pub a: [u8; 3],
    pub b: [u8; 3],
    pub mirrored: bool,
}
pub const CHEVRON_PRESETS: [ChevronPreset; 6] = [
    ChevronPreset {
        name: "Fire Standard",
        description: "Red / yellow · 45° mirrored",
        a: [215, 30, 38],
        b: [255, 215, 30],
        mirrored: true,
    },
    ChevronPreset {
        name: "EMS White",
        description: "Orange / white · mirrored",
        a: [235, 150, 35],
        b: [245, 245, 245],
        mirrored: true,
    },
    ChevronPreset {
        name: "DOT Highway",
        description: "Black / yellow · hazard",
        a: [20, 22, 24],
        b: [255, 211, 61],
        mirrored: false,
    },
    ChevronPreset {
        name: "Police Vis",
        description: "Blue / silver · mirrored",
        a: [40, 65, 190],
        b: [225, 232, 243],
        mirrored: true,
    },
    ChevronPreset {
        name: "Tow Warning",
        description: "Amber / black · full rear",
        a: [245, 170, 24],
        b: [20, 22, 24],
        mirrored: false,
    },
    ChevronPreset {
        name: "Rescue Red",
        description: "Red / white · mirrored",
        a: [205, 28, 45],
        b: [250, 250, 250],
        mirrored: true,
    },
];
pub fn materials(model: &Model) -> BTreeSet<String> {
    model.meshes.iter().map(|m| m.material.clone()).collect()
}
