//! Local repair utility for old development lens patches. Original source
//! vehicles are untouched; replaces the named output atomically without backups.
use std::{io::Write, path::PathBuf};
fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(
        args.next()
            .ok_or("Pass input patch ZIP and optional output ZIP")?,
    );
    let output = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| input.clone());
    let bytes = duck_workshop::read_file(&input)?;
    let repaired = duck_workshop::models::lens_model::repair_patch(&bytes)?;
    let parent = output.parent().ok_or("Output has no parent folder")?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(&repaired).map_err(|e| e.to_string())?;
    file.persist(&output).map_err(|e| e.to_string())?;
    println!(
        "Repaired patch: {} ({} bytes)",
        output.display(),
        repaired.len()
    );
    Ok(())
}
