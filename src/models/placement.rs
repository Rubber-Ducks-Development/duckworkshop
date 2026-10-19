//! Original implementation following CodeWalker's rest-pose/fragment placement conventions.
use super::*;
use glam::{Mat4, Vec3, Vec4};

pub(super) struct PlacedDrawable {
    pub address: usize,
    pub transform: Option<Mat4>,
}
impl Reader {
    fn matrix(&self, offset: usize) -> Result<Mat4> {
        let mut v = [0.; 16];
        for (i, x) in v.iter_mut().enumerate() {
            *x = self.f32(offset + i * 4)?;
        }
        // RAGE row-vector matrices become glam column-vector matrices with this layout.
        // The fourth column in the file can contain flags, not float values.
        v[3] = 0.;
        v[7] = 0.;
        v[11] = 0.;
        v[15] = 1.;
        let m = Mat4::from_cols_array(&v);
        if !m.is_finite() {
            return Err("Invalid model placement matrix".into());
        }
        Ok(m)
    }
    pub(super) fn rigid_matrices(
        &self,
        drawable: usize,
        ext: &str,
        worlds: &[Mat4],
    ) -> Result<Vec<Mat4>> {
        let mut result = worlds.to_vec();
        if ext.eq_ignore_ascii_case("yft")
            && let Some(pose) = self.ptr(0xa8)?
        {
            let count = self.bytes::<1>(pose + 16)?[0] as usize;
            for (i, m) in result.iter_mut().enumerate().take(count) {
                let base = pose + 32 + i * 48;
                let mut rows = [Vec4::ZERO; 3];
                for (r, row) in rows.iter_mut().enumerate() {
                    *row = Vec4::new(
                        self.f32(base + r * 16)?,
                        self.f32(base + r * 16 + 4)?,
                        self.f32(base + r * 16 + 8)?,
                        self.f32(base + r * 16 + 12)?,
                    );
                }
                *m = Mat4::from_cols(rows[0], rows[1], rows[2], Vec4::W).transpose();
                if !m.is_finite() {
                    return Err("Invalid fragment pose".into());
                }
            }
        } else if let Some(sk) = self.ptr(drawable + 0x18)?
            && let Some(array) = self.ptr(sk + 0x30)?
        {
            let bones = self.bone_poses(drawable)?;
            for (i, m) in result.iter_mut().enumerate() {
                *m = self.matrix(array + i * 64)?;
                let mut parent = bones[i].parent;
                let mut visited = 0;
                while parent >= 0 {
                    let index = parent as usize;
                    if index >= bones.len() || visited >= bones.len() {
                        return Err("Invalid bone parent".into());
                    }
                    *m = self.matrix(array + index * 64)? * *m;
                    parent = bones[index].parent;
                    visited += 1;
                }
            }
        }
        Ok(result)
    }
    pub(super) fn skin_matrices(&self, drawable: usize, worlds: &[Mat4]) -> Result<Vec<Mat4>> {
        let inverse = self
            .ptr(drawable + 0x18)?
            .map(|sk| self.ptr(sk + 0x28))
            .transpose()?
            .flatten();
        worlds
            .iter()
            .enumerate()
            .map(|(i, world)| {
                // Without inverse binds, the skeleton's authored pose is the bind pose.
                let m = if let Some(array) = inverse {
                    *world * self.matrix(array + i * 64)?
                } else {
                    Mat4::IDENTITY
                };
                if !m.is_finite() {
                    return Err("Invalid skin transform".into());
                }
                Ok(m)
            })
            .collect()
    }
    pub(super) fn placed_drawables(&self, ext: &str) -> Result<Vec<PlacedDrawable>> {
        if ext.eq_ignore_ascii_case("ydr") {
            return Ok(vec![PlacedDrawable {
                address: 0,
                transform: None,
            }]);
        }
        let mut result = vec![PlacedDrawable {
            address: self.req(0x30)?,
            transform: None,
        }];
        // Only intact LOD1 physics parts belong in the rest-pose preview. The other
        // physics LODs and damaged alternatives are not additional vehicle parts.
        if let Some(group) = self.ptr(0xf0)?
            && let Some(lod) = self.ptr(group + 0x10)?
        {
            let count = self.bytes::<1>(lod + 0x11d)?[0] as usize;
            let offset = Vec3::new(
                self.f32(lod + 0x30)?,
                self.f32(lod + 0x34)?,
                self.f32(lod + 0x38)?,
            );
            let transforms = self.ptr(lod + 0x100)?;
            let transform_count = transforms
                .map(|t| self.u32(t + 16))
                .transpose()?
                .unwrap_or(0) as usize;
            if transform_count > 4096 {
                return Err("Invalid fragment transform count".into());
            }
            if let Some(children) = self.ptr(lod + 0xd0)? {
                for i in 0..count {
                    let Some(child) = self.ptr(children + i * 8)? else {
                        continue;
                    };
                    let Some(address) = self.ptr(child + 0xa0)? else {
                        continue;
                    };
                    let transform = if let Some(t) = transforms.filter(|_| i < transform_count) {
                        let mut m = self.matrix(t + 32 + i * 64)?;
                        // Native right wheel assets are authored facing the other side.
                        if [26418, 5857, 5858, 5859, 26398].contains(&self.u16(child + 0x12)?) {
                            m.x_axis = -Vec4::X;
                            m.y_axis = Vec4::Y;
                            m.z_axis = -Vec4::Z;
                        }
                        m.w_axis += offset.extend(0.);
                        if !m.is_finite() {
                            return Err("Invalid fragment placement".into());
                        }
                        Some(m)
                    } else {
                        None
                    };
                    // Preserve instances: two children may share geometry but have different placements.
                    result.push(PlacedDrawable { address, transform });
                }
            }
        }
        Ok(result)
    }
}
pub(super) fn skin_position(
    raw: Vec3,
    weights: [u8; 4],
    indices: [u8; 4],
    palette: &[u16],
    skin: &[Mat4],
) -> Result<Vec3> {
    let total: f32 = weights.iter().map(|&w| w as f32).sum();
    if total == 0. || skin.is_empty() {
        return Ok(raw);
    }
    let mut result = Vec3::ZERO;
    for i in 0..4 {
        if weights[i] == 0 {
            continue;
        }
        let index = indices[i] as usize;
        let bone = if palette.is_empty() {
            index
        } else {
            *palette
                .get(index)
                .ok_or("Invalid vertex bone palette index")? as usize
        };
        let matrix = skin.get(bone).ok_or("Invalid vertex bone index")?;
        result += matrix.transform_point3(raw) * (weights[i] as f32 / total);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weighted_skinning_uses_palette_and_ignores_zero_weight_slots() {
        let skin = [
            Mat4::from_translation(Vec3::X * 2.),
            Mat4::from_translation(Vec3::Y * 4.),
        ];
        let p = skin_position(
            Vec3::ONE,
            [128, 128, 0, 0],
            [0, 1, 255, 255],
            &[1, 0],
            &skin,
        )
        .unwrap();
        assert!(p.abs_diff_eq(Vec3::new(2., 3., 1.), 1e-6));
        assert!(skin_position(Vec3::ZERO, [255, 0, 0, 0], [2, 0, 0, 0], &[1, 0], &skin).is_err());
    }
}
