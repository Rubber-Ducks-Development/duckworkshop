#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ui;
fn main() -> eframe::Result {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--apply-update")) {
        if let (Some(target), Some(pid)) = (args.next(), args.next())
            && let Some(pid) = pid.to_str().and_then(|s| s.parse().ok())
        {
            duck_workshop::updates::helper_entry(target.into(), pid);
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1480., 940.])
            .with_min_inner_size([1180., 760.])
            .with_icon(icon()),
        // The UV Editor's 3D preview draws with depth testing.
        depth_buffer: 24,
        ..Default::default()
    };
    eframe::run_native(
        "Duck Workshop",
        options,
        Box::new(|cc| Ok(Box::new(ui::Workbench::new(cc)))),
    )
}

fn icon() -> eframe::egui::IconData {
    let image = image::load_from_memory(include_bytes!("../assets/duck-workshop.png"))
        .expect("embedded duck logo")
        .resize(256, 256, image::imageops::FilterType::Nearest)
        .into_rgba8();
    eframe::egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}
