use crate::{Result, models::Model};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke, Transform};
#[derive(Clone, Serialize, Deserialize)]
pub struct UvOptions {
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
    model
        .meshes
        .iter()
        .filter(move |m| m.enabled && (lod == "All" || m.lod == lod))
}
pub fn uv_edges(model: &Model, opt: &UvOptions) -> Vec<([f32; 2], [f32; 2])> {
    let mut edges = std::collections::BTreeMap::new();
    for m in selected(model, opt) {
        let Some(uv) = m.uvs.get(opt.channel).filter(|u| !u.is_empty()) else {
            continue;
        };
        for tri in m.indices.as_chunks::<3>().0 {
            for i in 0..3 {
                let a = uv[tri[i] as usize];
                let b = uv[tri[(i + 1) % 3] as usize];
                let key = |v: [f32; 2]| [v[0].to_bits(), v[1].to_bits()];
                let ka = key(a);
                let kb = key(b);
                let key = if ka < kb { (ka, kb) } else { (kb, ka) };
                let entry = edges.entry(key).or_insert((a, b, 0u32));
                entry.2 += 1;
            }
        }
    }
    edges
        .into_values()
        .filter(|(_, _, count)| !opt.boundaries || *count == 1)
        .map(|(a, b, _)| (a, b))
        .collect()
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
    let mut pix = Pixmap::new(opt.size, opt.size).ok_or("Cannot allocate image")?;
    pix.fill(tiny_skia::Color::from_rgba8(
        opt.background[0],
        opt.background[1],
        opt.background[2],
        opt.background[3],
    ));
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
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{s}\" height=\"{s}\" viewBox=\"0 0 {s} {s}\"><defs><clipPath id=\"tile\"><rect width=\"{s}\" height=\"{s}\"/></clipPath></defs><rect width=\"{s}\" height=\"{s}\" fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{}\"/><path d=\"{path}\" clip-path=\"url(#tile)\" fill=\"none\" stroke=\"#{:02x}{:02x}{:02x}\" stroke-opacity=\"{}\" stroke-width=\"{}\"/></svg>",
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
    pub width: u32,
    pub height: u32,
    pub stripe: f32,
    pub angle: f32,
    pub color_a: [u8; 3],
    pub color_b: [u8; 3],
    pub opacity: u8,
    pub center: f32,
    pub offset: f32,
    pub direction: usize,
    pub uv_clip: bool,
}
impl Default for Chevron {
    fn default() -> Self {
        Self {
            width: 2048,
            height: 1024,
            stripe: 120.,
            angle: 45.,
            color_a: [255, 210, 0],
            color_b: [220, 25, 25],
            opacity: 255,
            center: 0.5,
            offset: 0.,
            direction: 0,
            uv_clip: false,
        }
    }
}
impl Chevron {
    pub fn validate(&self) -> Result<()> {
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
            let factor = 512. / self.width.max(self.height) as f32;
            (
                (self.width as f32 * factor).round() as u32,
                (self.height as f32 * factor).round() as u32,
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
                let q = if self.direction == 2 {
                    py * rad.cos() + dx * rad.sin()
                } else {
                    py * rad.cos()
                        + dx.abs() * rad.sin() * if self.direction == 0 { 1. } else { -1. }
                } + self.offset;
                let color = if (q / self.stripe).floor() as i64 % 2 == 0 {
                    self.color_a
                } else {
                    self.color_b
                };
                *p = image::Rgba([color[0], color[1], color[2], self.opacity]);
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
pub fn materials(model: &Model) -> BTreeSet<String> {
    model.meshes.iter().map(|m| m.material.clone()).collect()
}
