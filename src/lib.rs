pub mod diff;
pub mod meta;
pub mod models;
pub mod patterns;
pub mod project;
pub mod render;
pub mod textures;
pub mod updates;

pub type Result<T> = std::result::Result<T, String>;

pub fn read_file(path: &std::path::Path) -> Result<Vec<u8>> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() > 100 * 1024 * 1024 {
        return Err("File exceeds the 100 MB input limit.".into());
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

pub fn xml_document(text: &str) -> Result<roxmltree::Document<'_>> {
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return Err("DTD and entities are unsupported. Use plain XML.".into());
    }
    roxmltree::Document::parse(text).map_err(|e| format!("Invalid XML: {e}"))
}

pub fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
