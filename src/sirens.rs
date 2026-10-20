use crate::{
    Result,
    models::{Mesh, Model},
    patterns::Pattern,
};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SirenAnchor {
    pub id: u8,
    pub position: [f32; 3],
    /// The bone's authored rotation relative to its parent. The game replaces it
    /// with the carcols facing angle, so lenses authored on a turned axis keep
    /// their modeled place instead of being turned twice.
    #[serde(default = "identity_rotation")]
    pub rest_rotation: [f32; 4],
    /// World rotation of the bone's parent, the frame the facing angle turns in.
    #[serde(default = "identity_rotation")]
    pub parent_rotation: [f32; 4],
}
fn identity_rotation() -> [f32; 4] {
    Quat::IDENTITY.to_array()
}
impl SirenAnchor {
    pub fn new(id: u8, position: [f32; 3]) -> Self {
        Self {
            id,
            position,
            rest_rotation: identity_rotation(),
            parent_rotation: identity_rotation(),
        }
    }
    /// Model-space pose of this siren's lens surfaces, as the game poses the bone:
    /// carcols delta replaces the authored bone rotation (around the parent's Z axis)
    /// and a lit siren scales by its scale factor about the bone pivot.
    pub fn lens_transform(&self, scale: f32, angle: f32) -> Mat4 {
        lens_transform(
            Vec3::from(self.position),
            quat(self.parent_rotation),
            quat(self.rest_rotation),
            scale,
            angle,
        )
    }
}
fn quat(q: [f32; 4]) -> Quat {
    let q = Quat::from_array(q);
    if q.is_finite() && q.length_squared() > 1e-8 {
        q.normalize()
    } else {
        Quat::IDENTITY
    }
}
/// Lens pose about `pivot`. With identity rotations this is a plain turn by `angle`.
pub fn lens_transform(pivot: Vec3, parent: Quat, rest: Quat, scale: f32, angle: f32) -> Mat4 {
    let posed = parent * Quat::from_rotation_z(angle);
    let authored = parent * rest;
    Mat4::from_translation(pivot)
        * Mat4::from_quat(posed)
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_quat(authored.inverse())
        * Mat4::from_translation(-pivot)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PreviewSettings {
    pub night: bool,
    pub labels: bool,
    pub ground_glow: bool,
    pub scale: f32,
    pub vehicle_opacity: u8,
    pub positions: BTreeMap<u8, [f32; 3]>,
}
impl Default for PreviewSettings {
    fn default() -> Self {
        Self {
            night: false,
            labels: true,
            ground_glow: true,
            scale: 1.,
            vehicle_opacity: 100,
            positions: BTreeMap::new(),
        }
    }
}
impl PreviewSettings {
    pub fn validate(&self) -> Result<()> {
        if !self.scale.is_finite()
            || !(0.25..=4.).contains(&self.scale)
            || self.vehicle_opacity > 100
            || self.positions.len() > 32
            || self.positions.iter().any(|(&id, p)| {
                !(1..=32).contains(&id) || p.iter().any(|v| !v.is_finite() || v.abs() > 10000.)
            })
        {
            return Err("Invalid siren preview settings".into());
        }
        Ok(())
    }
}

pub(crate) struct BonePose {
    pub name: String,
    pub parent: i16,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}
pub(crate) fn bone_channels(bones: &[BonePose]) -> Result<Vec<u8>> {
    (0..bones.len())
        .map(|index| {
            let mut current = Some(index);
            let mut seen = vec![false; bones.len()];
            while let Some(i) = current {
                if i >= bones.len() || seen[i] {
                    return Err("Invalid siren bone hierarchy".into());
                }
                seen[i] = true;
                let bone = &bones[i];
                if let Some(id) = bone
                    .name
                    .to_ascii_lowercase()
                    .strip_prefix("siren")
                    .and_then(|name| name.parse::<u8>().ok())
                    .filter(|id| (1..=32).contains(id))
                {
                    return Ok(id);
                }
                current = (bone.parent >= 0).then_some(bone.parent as usize);
            }
            Ok(0)
        })
        .collect()
}
pub(crate) fn bone_worlds(bones: &[BonePose]) -> Result<Vec<Mat4>> {
    (0..bones.len())
        .map(|index| {
            let mut current = Some(index);
            let mut matrix = Mat4::IDENTITY;
            let mut seen = vec![false; bones.len()];
            while let Some(i) = current {
                if i >= bones.len() || seen[i] {
                    return Err("Invalid or cyclic bone hierarchy".into());
                }
                seen[i] = true;
                let b = &bones[i];
                let q = Quat::from_array(b.rotation);
                if !q.is_finite()
                    || q.length_squared() < 1e-8
                    || b.position.iter().chain(&b.scale).any(|v| !v.is_finite())
                {
                    return Err("Invalid bone transform".into());
                }
                matrix = Mat4::from_scale_rotation_translation(
                    Vec3::from(b.scale),
                    q.normalize(),
                    Vec3::from(b.position),
                ) * matrix;
                current = (b.parent >= 0).then_some(b.parent as usize);
            }
            if !matrix.is_finite() {
                return Err("Invalid bone transform".into());
            }
            Ok(matrix)
        })
        .collect()
}
pub(crate) fn bone_anchors(bones: &[BonePose]) -> Result<Vec<SirenAnchor>> {
    let worlds = bone_worlds(bones)?;
    let mut anchors: Vec<_> = bones
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let id = b
                .name
                .to_ascii_lowercase()
                .strip_prefix("siren")?
                .parse::<u8>()
                .ok()?;
            let rotation = |m: &Mat4| m.to_scale_rotation_translation().1;
            let parent = usize::try_from(b.parent)
                .ok()
                .and_then(|p| worlds.get(p))
                .map_or(Quat::IDENTITY, rotation);
            (1..=32).contains(&id).then(|| SirenAnchor {
                id,
                position: worlds[i].transform_point3(Vec3::ZERO).to_array(),
                rest_rotation: (parent.inverse() * rotation(&worlds[i]))
                    .normalize()
                    .to_array(),
                parent_rotation: parent.normalize().to_array(),
            })
        })
        .collect();
    anchors.sort_by_key(|a| a.id);
    Ok(anchors)
}

#[derive(Clone, Debug)]
pub struct Marker {
    pub surface_scale: f32,
    pub id: u8,
    pub channel: usize,
    pub layer: usize,
    pub position: [f32; 3],
    pub color: [u8; 3],
    pub active: bool,
    pub intensity: f32,
    pub inferred: bool,
}

pub fn markers(
    model: Option<&Model>,
    pattern: &Pattern,
    settings: &PreviewSettings,
    step: usize,
) -> Vec<Marker> {
    let scales = surface_scale_settings(pattern);
    let mut min = Vec3::new(-1.2, -0.2, 0.);
    let mut max = Vec3::new(1.2, 0.2, 0.2);
    if let Some(model) = model {
        min = Vec3::splat(f32::INFINITY);
        max = Vec3::splat(f32::NEG_INFINITY);
        for p in model.meshes.iter().flat_map(|m| &m.positions) {
            min = min.min(Vec3::from(*p));
            max = max.max(Vec3::from(*p));
        }
    }
    let mut markers = vec![];
    let mut id = 1;
    for (channel, light) in pattern.channels.iter().enumerate() {
        for layer in 0..light.color_count() {
            let native = model.and_then(|m| m.sirens.iter().find(|a| a.id == id));
            let custom = settings.positions.get(&id);
            // A real vehicle must never acquire fabricated roof lights for missing IDs.
            if model.is_some() && native.is_none() && custom.is_none() {
                id += 1;
                continue;
            }
            let position = custom
                .copied()
                .or_else(|| native.map(|a| a.position))
                .unwrap_or_else(|| {
                    let columns = pattern.channels.len().div_ceil(2);
                    let col = channel % columns;
                    let row = channel / columns;
                    [
                        min.x
                            + (max.x - min.x) * (0.12 + 0.76 * (col as f32 + 0.5) / columns as f32),
                        (min.y + max.y) * 0.5 + (row as f32 - 0.5) * (max.y - min.y) * 0.25,
                        max.z + (max - min).length() * 0.02,
                    ]
                });
            markers.push(Marker {
                // A new sequence's export default is not evidence of a vehicle's
                // authored scale. Avoid enlarging real lenses until settings exist.
                surface_scale: scales[id as usize - 1].unwrap_or(if model.is_some() {
                    1.
                } else {
                    100.
                }),
                id,
                channel,
                layer,
                position,
                color: light.color_at(layer),
                active: light.bits_at(layer)[step % 32],
                intensity: light.intensity,
                inferred: native.is_none() && custom.is_none(),
            });
            id += 1;
        }
    }
    markers
}

/// Match the scaling metadata retained by Pattern::to_xml, including unedited imported settings.
pub fn surface_scales(pattern: &Pattern) -> [f32; 32] {
    surface_scale_settings(pattern).map(|s| s.unwrap_or(100.))
}

/// Warn only about sizes actually needed by this model, including partial metadata.
pub fn missing_surface_scales(model: &Model, pattern: &Pattern) -> bool {
    let scales = surface_scale_settings(pattern);
    model
        .meshes
        .iter()
        .flat_map(|m| &m.siren_ids)
        .any(|&id| (1..=32).contains(&id) && scales[id as usize - 1].is_none())
}

fn surface_scale_settings(pattern: &Pattern) -> [Option<f32>; 32] {
    let mut scales = [None; 32];
    let Some(source) = &pattern.source_xml else {
        return scales;
    };
    let Ok(doc) = crate::xml_document(source) else {
        return scales;
    };
    let Some(settings) = doc
        .descendants()
        .find(|n| n.has_tag_name("Sirens"))
        .and_then(|n| {
            n.children()
                .filter(|c| c.has_tag_name("Item"))
                .nth(pattern.source_index)
        })
        .and_then(|n| n.children().find(|c| c.has_tag_name("sirens")))
    else {
        return scales;
    };
    for (i, node) in settings
        .children()
        .filter(|n| n.has_tag_name("Item"))
        .take(32)
        .enumerate()
    {
        let value = |name| {
            node.children()
                .find(|n| n.has_tag_name(name))
                .and_then(|n| n.attribute("value").or(n.text()))
        };
        scales[i] = if value("scale").is_some_and(|s| s.eq_ignore_ascii_case("false")) {
            Some(1.)
        } else {
            value("scaleFactor")
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|n| n.is_finite() && (0.0..=1000.).contains(n))
        };
    }
    scales
}

/// A clearly identified fallback so the 3D light player works before importing a vehicle.
pub fn schematic_model() -> Model {
    let p = vec![
        [-1.2, -0.2, 0.],
        [1.2, -0.2, 0.],
        [1.2, 0.2, 0.],
        [-1.2, 0.2, 0.],
        [-1.2, -0.2, 0.2],
        [1.2, -0.2, 0.2],
        [1.2, 0.2, 0.2],
        [-1.2, 0.2, 0.2],
    ];
    Model {
        name: "Schematic lightbar".into(),
        meshes: vec![Mesh {
            vertex_alpha: vec![],
            siren_ids: vec![],
            name: "Lightbar housing".into(),
            material: "Housing".into(),
            lod: "High".into(),
            uvs: vec![vec![[0., 0.]; 8]],
            positions: p,
            indices: vec![
                0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2,
                7, 6, 3, 0, 4, 3, 4, 7,
            ],
            enabled: true,
        }],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parent_rotation_and_translation_place_sirens_in_model_space() {
        let bones = vec![
            BonePose {
                name: "root".into(),
                parent: -1,
                position: [1., 2., 3.],
                rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array(),
                scale: [1.; 3],
            },
            BonePose {
                name: "siren32".into(),
                parent: 0,
                position: [1., 0., 0.],
                rotation: Quat::IDENTITY.to_array(),
                scale: [1.; 3],
            },
        ];
        let anchors = bone_anchors(&bones).unwrap();
        assert_eq!(anchors[0].id, 32);
        assert!((Vec3::from(anchors[0].position) - Vec3::new(1., 3., 3.)).length() < 0.0001);
        // The authored turn is kept relative to the parent the facing angle replaces.
        assert!(Quat::from_array(anchors[0].rest_rotation).abs_diff_eq(Quat::IDENTITY, 0.0001));
        assert!(
            Quat::from_array(anchors[0].parent_rotation)
                .abs_diff_eq(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), 0.0001)
        );
        let mut bad = bones;
        bad[0].parent = 1;
        assert!(bone_anchors(&bad).is_err());
    }
    #[test]
    fn ids_follow_export_order_and_multicolor_channels_share_schematic_positions() {
        let mut p = Pattern::default();
        p.channels.truncate(2);
        p.channels[0].add_color([0, 0, 255]);
        p.channels[0].set_step(3, Some(1));
        let m = markers(None, &p, &PreviewSettings::default(), 3);
        assert_eq!(m.len(), 3);
        assert_eq!((m[1].id, m[1].channel, m[1].layer), (2, 0, 1));
        assert!(m[1].active);
        assert!(!m[0].active);
        assert_eq!(m[0].position, m[1].position);
        let model = Model {
            sirens: vec![SirenAnchor::new(2, [1., 2., 3.])],
            ..schematic_model()
        };
        let mut settings = PreviewSettings::default();
        settings.positions.insert(1, [4., 5., 6.]);
        let m = markers(Some(&model), &p, &settings, 35);
        assert_eq!(m[0].position, [4., 5., 6.]);
        assert_eq!(m[1].position, [1., 2., 3.]);
        assert!(!m[1].inferred);
        assert!(m[1].active);
    }
}
