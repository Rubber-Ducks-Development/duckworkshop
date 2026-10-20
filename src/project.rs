use crate::{
    Result, meta,
    models::Model,
    patterns::Pattern,
    render::{Chevron, UvOptions},
};
use serde::{Deserialize, Serialize};
use std::io::Read;

const MAX_PROJECT_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(default = "whole_vehicle_default")]
    pub uv_whole_vehicle: bool,
    #[serde(default)]
    pub siren_preview: crate::sirens::PreviewSettings,
    pub pattern: Pattern,
    pub chevron: Chevron,
    pub uv: UvOptions,
    pub metas: [String; 4],
    pub model_name: String,
    pub handling_name: String,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            uv_whole_vehicle: true,
            siren_preview: Default::default(),
            pattern: Pattern::default(),
            chevron: Chevron::default(),
            uv: UvOptions::default(),
            metas: std::array::from_fn(|_| String::new()),
            model_name: String::new(),
            handling_name: String::new(),
        }
    }
}
fn whole_vehicle_default() -> bool {
    true
}
impl Session {
    pub fn validate(&self) -> Result<()> {
        self.siren_preview.validate()?;
        self.pattern.validate()?;
        self.chevron.validate()?;
        if !(64..=8192).contains(&self.uv.size)
            || self.uv.channel > 7
            || !self.uv.width.is_finite()
            || !(0.1..=20.).contains(&self.uv.width)
        {
            return Err("Invalid UV export settings in this project.".into());
        }
        for (kind, text) in self.metas.iter().enumerate() {
            if !text.trim().is_empty() {
                meta::entries(text, kind)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    pub format: String,
    pub version: u32,
    pub session: Session,
    pub model: Option<Model>,
    pub tool: usize,
    pub dark_mode: bool,
    #[serde(default)]
    pub comparison: [String; 2],
    /// Material name and the texture file shown on it. Files stay where they are.
    #[serde(default)]
    pub textures: Vec<(String, std::path::PathBuf)>,
}
impl Project {
    /// Projects include expanded geometry and image data, so their bounded
    /// input allowance is larger than the limit for a single vehicle file.
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_PROJECT_BYTES as u64 {
            return Err("Project exceeds the 512 MB workspace limit".into());
        }
        let mut bytes = Vec::new();
        file.take((MAX_PROJECT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_PROJECT_BYTES {
            return Err("Project exceeds the 512 MB workspace limit".into());
        }
        Self::from_json(&bytes)
    }
    pub fn validate(&self) -> Result<()> {
        if self.format != "duck-workshop" || self.version != 1 || self.tool >= 5 {
            return Err("This file is not a supported Duck Workshop project.".into());
        }
        self.session.validate()?;
        if let Some(model) = &self.model {
            model.validate()?;
        }
        Ok(())
    }
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let project: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid project: {e}"))?;
        project.validate()?;
        Ok(project)
    }
    pub fn to_json(&self) -> Result<Vec<u8>> {
        self.validate()?;
        // Native geometry is large; keep formatting overhead out of saved workspaces.
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_PROJECT_BYTES {
            return Err("Project exceeds the 512 MB workspace limit".into());
        }
        Ok(bytes)
    }
}
