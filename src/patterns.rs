use crate::{Result, xml_document, xml_escape};
use serde::{Deserialize, Serialize};
/// Most colors one light can show. Each color is exported as its own siren.
pub const MAX_COLORS: usize = 6;
/// carcols supports 32 sirens per setting.
pub const MAX_SIRENS: usize = 32;

/// A light. `color`/`bits` are its first color; `extra` holds any further colors.
/// A step shows at most one color.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    pub color: [u8; 3],
    pub bits: [bool; 32],
    pub intensity: f32,
    pub delta: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<ColorLayer>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorLayer {
    pub color: [u8; 3],
    pub bits: [bool; 32],
}
impl Channel {
    pub fn new(name: String, color: [u8; 3]) -> Self {
        Self {
            name,
            color,
            bits: [false; 32],
            intensity: 1.,
            delta: 0.,
            extra: vec![],
        }
    }
    pub fn color_count(&self) -> usize {
        1 + self.extra.len()
    }
    pub fn color_at(&self, index: usize) -> [u8; 3] {
        if index == 0 {
            self.color
        } else {
            self.extra[index - 1].color
        }
    }
    pub fn set_color(&mut self, index: usize, color: [u8; 3]) {
        if index == 0 {
            self.color = color
        } else {
            self.extra[index - 1].color = color
        }
    }
    pub fn bits_at(&self, index: usize) -> &[bool; 32] {
        if index == 0 {
            &self.bits
        } else {
            &self.extra[index - 1].bits
        }
    }
    fn layers_mut(&mut self) -> impl Iterator<Item = &mut [bool; 32]> {
        std::iter::once(&mut self.bits).chain(self.extra.iter_mut().map(|l| &mut l.bits))
    }
    /// The color index lit at `step`, if any.
    pub fn step(&self, step: usize) -> Option<usize> {
        (0..self.color_count()).find(|&i| self.bits_at(i)[step])
    }
    pub fn steps(&self) -> [Option<usize>; 32] {
        std::array::from_fn(|s| self.step(s))
    }
    /// Lights `step` in one color (clearing the others) or turns it off.
    pub fn set_step(&mut self, step: usize, value: Option<usize>) {
        for (i, bits) in self.layers_mut().enumerate() {
            bits[step] = value == Some(i);
        }
    }
    pub fn set_steps(&mut self, steps: &[Option<usize>; 32]) {
        for (s, value) in steps.iter().enumerate() {
            self.set_step(s, *value);
        }
    }
    /// Replaces the steps of one color, taking those steps from the other colors.
    pub fn set_color_bits(&mut self, index: usize, bits: [bool; 32]) {
        for (s, on) in bits.iter().enumerate() {
            if *on {
                self.set_step(s, Some(index));
            } else if self.step(s) == Some(index) {
                self.set_step(s, None);
            }
        }
    }
    pub fn rotate(&mut self, left: bool) {
        for bits in self.layers_mut() {
            if left {
                bits.rotate_left(1)
            } else {
                bits.rotate_right(1)
            }
        }
    }
    pub fn add_color(&mut self, color: [u8; 3]) -> bool {
        if self.color_count() >= MAX_COLORS {
            return false;
        }
        self.extra.push(ColorLayer {
            color,
            bits: [false; 32],
        });
        true
    }
    /// Removes a color and its steps. The last remaining color cannot be removed.
    pub fn remove_color(&mut self, index: usize) {
        if self.extra.is_empty() || index >= self.color_count() {
            return;
        }
        if index == 0 {
            let next = self.extra.remove(0);
            self.color = next.color;
            self.bits = next.bits;
        } else {
            self.extra.remove(index - 1);
        }
    }
    /// One single-color siren per color, in color order.
    pub fn sirens(&self) -> Vec<Channel> {
        (0..self.color_count())
            .map(|i| Channel {
                name: if self.extra.is_empty() {
                    self.name.clone()
                } else {
                    format!("{} color {}", self.name, i + 1)
                },
                color: self.color_at(i),
                bits: *self.bits_at(i),
                intensity: self.intensity,
                delta: self.delta,
                extra: vec![],
            })
            .collect()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    pub version: u32,
    pub name: String,
    pub id: u32,
    pub bpm: f32,
    pub channels: Vec<Channel>,
    pub source_xml: Option<String>,
    pub source_index: usize,
}
impl Default for Pattern {
    fn default() -> Self {
        Self {
            version: 1,
            name: "Untitled Sequence".into(),
            id: 1,
            bpm: 600.,
            channels: (0..8)
                .map(|i| {
                    Channel::new(
                        format!("Light {:02}", i + 1),
                        if i < 4 { [255, 35, 35] } else { [35, 110, 255] },
                    )
                })
                .collect(),
            source_xml: None,
            source_index: 0,
        }
    }
}
pub fn decimal(bits: &[bool; 32]) -> u32 {
    bits.iter().fold(0, |n, b| (n << 1) | u32::from(*b))
}
pub fn bits(decimal: u32) -> [bool; 32] {
    std::array::from_fn(|i| (decimal >> (31 - i)) & 1 != 0)
}
impl Pattern {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.channels.is_empty()
            || self.channels.len() > 32
            || self.id == 0
            || !self.bpm.is_finite()
            || !(1.0..=5000.0).contains(&self.bpm)
        {
            return Err("Requires 1–32 channels, a nonzero siren ID and 1–5000 BPM.".into());
        }
        if self.siren_count() > MAX_SIRENS {
            return Err(format!(
                "A siren setting holds {MAX_SIRENS} sirens. Each color of a light uses one siren."
            ));
        }
        for c in &self.channels {
            if c.color_count() > MAX_COLORS {
                return Err(format!("A light can have up to {MAX_COLORS} colors."));
            }
            if !c.intensity.is_finite()
                || !(0.0..=1000.0).contains(&c.intensity)
                || !c.delta.is_finite()
                || c.delta.abs() > std::f32::consts::TAU
            {
                return Err("Invalid light intensity or angle.".into());
            }
        }
        Ok(())
    }
    /// Sirens used in carcols: one per color of each light.
    pub fn siren_count(&self) -> usize {
        self.channels.iter().map(Channel::color_count).sum()
    }
    pub fn sirens(&self) -> Vec<Channel> {
        self.channels.iter().flat_map(Channel::sirens).collect()
    }
    /// Moves light `index` into the light above it as extra colors. Steps the light
    /// above already lights keep its color; returns how many steps that affected.
    pub fn merge_up(&mut self, index: usize) -> Result<usize> {
        if index == 0 || index >= self.channels.len() {
            return Err("Select a light below another light to merge.".into());
        }
        let source = self.channels[index].clone();
        let mut target = self.channels[index - 1].clone();
        let mut map = vec![];
        for i in 0..source.color_count() {
            let color = source.color_at(i);
            match (0..target.color_count()).find(|&k| target.color_at(k) == color) {
                Some(k) => map.push(k),
                None => {
                    if !target.add_color(color) {
                        return Err(format!("A light can have up to {MAX_COLORS} colors."));
                    }
                    map.push(target.color_count() - 1);
                }
            }
        }
        let mut conflicts = 0;
        for (s, value) in source.steps().iter().enumerate() {
            if let Some(i) = value {
                if target.step(s).is_some() {
                    conflicts += 1;
                } else {
                    target.set_step(s, Some(map[*i]));
                }
            }
        }
        self.channels[index - 1] = target;
        self.channels.remove(index);
        Ok(conflicts)
    }
    pub fn settings(text: &str) -> Result<Vec<String>> {
        let doc = xml_document(text)?;
        let container = doc
            .root_element()
            .children()
            .find(|n| n.has_tag_name("Sirens"))
            .ok_or("No Sirens container in carcols.meta")?;
        Ok(container
            .children()
            .filter(|n| n.has_tag_name("Item"))
            .map(|n| format!("{} (ID {})", val(n, "name"), val(n, "id")))
            .collect())
    }
    pub fn from_xml(text: &str, index: usize) -> Result<Self> {
        let doc = xml_document(text)?;
        let item = doc
            .root_element()
            .children()
            .find(|n| n.has_tag_name("Sirens"))
            .and_then(|n| n.children().filter(|n| n.has_tag_name("Item")).nth(index))
            .ok_or("No siren setting found")?;
        let mut channels = vec![];
        let sirens = item
            .children()
            .find(|n| n.has_tag_name("sirens"))
            .ok_or("Missing sirens")?;
        for (s, n) in sirens
            .children()
            .filter(|n| n.has_tag_name("Item"))
            .enumerate()
        {
            let flash = n
                .children()
                .find(|n| n.has_tag_name("flashiness"))
                .ok_or("Missing flashiness")?;
            let color = val(n, "color");
            let digits = color
                .strip_prefix("0x")
                .or_else(|| color.strip_prefix("0X"))
                .unwrap_or(&color);
            let parsed = u32::from_str_radix(digits, 16).map_err(|_| {
                let color_node = n.children().find(|c| c.has_tag_name("color"));
                let raw = color_node
                    .and_then(|c| c.attribute("value").or_else(|| c.text()))
                    .unwrap_or("");
                let pos = doc.text_pos_at(color_node.unwrap_or(n).range().start);
                let value: String = raw.chars().take(96).collect();
                let suffix = if raw.chars().count() > 96 { " (truncated)" } else { "" };
                let id = val(item, "id").parse::<u32>().ok();
                let setting = id.map_or_else(|| format!("entry {}", index + 1), |id| format!("ID {id}"));
                format!("Invalid ARGB color {value:?}{suffix} in siren settings {setting}, siren {}, line {}, column {}. Expected a 32-bit hexadecimal ARGB color such as 0xFFFF0000.", s + 1, pos.row, pos.col)
            })?;
            let sequence = val(flash, "sequencer")
                .parse::<u32>()
                .map_err(|_| "Invalid unsigned sequencer")?;
            channels.push(Channel {
                bits: bits(sequence),
                intensity: val(n, "intensity").parse().unwrap_or(1.),
                delta: val(flash, "delta").parse().unwrap_or(0.),
                ..Channel::new(
                    format!("Siren {}", s + 1),
                    [(parsed >> 16) as u8, (parsed >> 8) as u8, parsed as u8],
                )
            });
        }
        let p = Self {
            version: 1,
            name: val(item, "name"),
            id: val(item, "id")
                .parse()
                .map_err(|_| "Siren ID must be an unsigned 32-bit integer")?,
            bpm: val(item, "sequencerBpm").parse().unwrap_or(600.),
            channels,
            source_xml: Some(text.into()),
            source_index: index,
        };
        p.validate()?;
        Ok(p)
    }
    pub fn to_xml(&self) -> Result<String> {
        self.validate()?;
        let lights = self.sirens();
        if let Some(source) = &self.source_xml {
            let mut root = crate::meta::parse_element(source)?;
            let sirens = root.get_mut_child("Sirens").ok_or("Missing Sirens")?;
            let item = sirens
                .children
                .iter_mut()
                .filter_map(|n| n.as_mut_element())
                .filter(|n| n.name == "Item")
                .nth(self.source_index)
                .ok_or("Missing imported siren entry")?;
            crate::meta::set_value(item, "id", &self.id.to_string(), true);
            crate::meta::set_value(item, "sequencerBpm", &self.bpm.to_string(), true);
            crate::meta::set_value(item, "name", &self.name, false);
            let nodes = item.get_mut_child("sirens").ok_or("Missing sirens")?;
            while nodes
                .children
                .iter()
                .filter_map(|n| n.as_element())
                .filter(|n| n.name == "Item")
                .count()
                < lights.len()
            {
                let n = crate::meta::parse_element(&channel_xml(&lights[0]))?;
                nodes.children.push(xmltree::XMLNode::Element(n));
            }
            let mut ci = 0;
            nodes.children.retain(|n| {
                if n.as_element().is_some_and(|e| e.name == "Item") {
                    ci += 1;
                    ci <= lights.len()
                } else {
                    true
                }
            });
            for (n, c) in nodes
                .children
                .iter_mut()
                .filter_map(|n| n.as_mut_element())
                .filter(|n| n.name == "Item")
                .zip(&lights)
            {
                for part in ["rotation", "flashiness"] {
                    if n.get_child(part).is_none() {
                        n.children
                            .push(xmltree::XMLNode::Element(xmltree::Element::new(part)));
                    }
                    let section = n.get_mut_child(part).unwrap();
                    crate::meta::set_value(
                        section,
                        "sequencer",
                        &decimal(&c.bits).to_string(),
                        true,
                    );
                    crate::meta::set_value(section, "delta", &c.delta.to_string(), true);
                }
                crate::meta::set_value(
                    n,
                    "color",
                    &format!("0xFF{:02X}{:02X}{:02X}", c.color[0], c.color[1], c.color[2]),
                    true,
                );
                crate::meta::set_value(n, "intensity", &c.intensity.to_string(), true);
            }
            return crate::meta::write_element(&root);
        }
        let xml = format!(
            "<CVehicleModelInfoVarGlobal><Kits/><Lights/><Sirens><Item><id value=\"{}\"/><name>{}</name><timeMultiplier value=\"1\"/><lightFalloffMax value=\"80\"/><lightFalloffExponent value=\"52\"/><lightInnerConeAngle value=\"2.3\"/><lightOuterConeAngle value=\"70\"/><lightOffset value=\"0\"/><textureName>VehicleLight_sirenlight</textureName><sequencerBpm value=\"{}\"/><leftHeadLight><sequencer value=\"0\"/></leftHeadLight><rightHeadLight><sequencer value=\"0\"/></rightHeadLight><leftTailLight><sequencer value=\"0\"/></leftTailLight><rightTailLight><sequencer value=\"0\"/></rightTailLight><leftHeadLightMultiples value=\"1\"/><rightHeadLightMultiples value=\"1\"/><leftTailLightMultiples value=\"1\"/><rightTailLightMultiples value=\"1\"/><useRealLights value=\"true\"/><sirens>{}</sirens></Item></Sirens></CVehicleModelInfoVarGlobal>",
            self.id,
            xml_escape(&self.name),
            self.bpm,
            lights.iter().map(channel_xml).collect::<String>()
        );
        crate::meta::format_xml(&xml)
    }
}
fn val(n: roxmltree::Node<'_, '_>, tag: &str) -> String {
    n.children()
        .find(|n| n.has_tag_name(tag))
        .map(|n| {
            n.attribute("value")
                .unwrap_or(n.text().unwrap_or(""))
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}
fn channel_xml(c: &Channel) -> String {
    let seq = decimal(&c.bits);
    let section = |name| {
        format!(
            "<{name}><delta value=\"{}\"/><start value=\"0\"/><speed value=\"3\"/><sequencer value=\"{seq}\"/><multiples value=\"1\"/><direction value=\"false\"/><syncToBpm value=\"true\"/></{name}>",
            c.delta
        )
    };
    format!(
        "<Item>{}{}<corona><intensity value=\"0\"/><size value=\"0\"/><pull value=\"0.15\"/><faceCamera value=\"false\"/></corona><color value=\"0xFF{:02X}{:02X}{:02X}\"/><intensity value=\"{}\"/><lightGroup value=\"0\"/><rotate value=\"false\"/><scale value=\"true\"/><scaleFactor value=\"100\"/><flash value=\"true\"/><light value=\"true\"/><spotLight value=\"true\"/><castShadows value=\"false\"/></Item>",
        section("rotation"),
        section("flashiness"),
        c.color[0],
        c.color[1],
        c.color[2],
        c.intensity
    )
}
