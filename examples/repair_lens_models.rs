//! Repair existing development YFTs from an original vehicle. No backup files.
use duck_workshop::{models::lens_model, read_file};
use std::{io::Write, path::PathBuf};
fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let source = PathBuf::from(args.next().ok_or("Pass original vehicle folder")?).join("stream");
    let target = PathBuf::from(args.next().ok_or("Pass current vehicle folder")?).join("stream");
    let write = args.next().is_some_and(|a| a == "--write");
    let mut outputs = vec![];
    for name in ["eladderlt.yft", "eladderlt_hi.yft"] {
        let original = std::fs::read_dir(&source)
            .map_err(|e| e.to_string())?
            .flatten()
            .find(|e| e.file_name().eq_ignore_ascii_case(name))
            .ok_or("Original YFT missing")?
            .path();
        let current = target.join(name);
        let edited = read_file(&current)?;
        println!(
            "Current {name}: {:?}",
            lens_model::validate_lens_fixups(&edited)
        );
        let output =
            lens_model::repair_model_from_original(&read_file(&original)?, &edited, "eladderlt")?;
        lens_model::validate_lens_fixups(&output)?;
        println!(
            "Validated repaired {name}: {} bytes, unchanged geometry/materials",
            output.len()
        );
        outputs.push((current, output));
    }
    if write {
        for (path, data) in outputs {
            let mut file = tempfile::NamedTempFile::new_in(&target).map_err(|e| e.to_string())?;
            file.write_all(&data).map_err(|e| e.to_string())?;
            file.persist(&path).map_err(|e| e.to_string())?;
            println!("Updated {}", path.display());
        }
    }
    Ok(())
}
