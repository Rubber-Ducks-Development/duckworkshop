fn main() {
    println!("cargo:rerun-if-changed=assets/duck-workshop.png");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icon = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("app.ico");
        image::open("assets/duck-workshop.png")
            .expect("read supplied duck logo")
            .resize(256, 256, image::imageops::FilterType::Nearest)
            .save_with_format(&icon, image::ImageFormat::Ico)
            .expect("encode Windows icon");
        winresource::WindowsResource::new()
            .set_icon(icon.to_str().unwrap())
            .set("ProductName", "Duck Workshop")
            .set(
                "FileDescription",
                "Rubber Ducks Development - Duck Workshop",
            )
            .set("OriginalFilename", "Duck-Workshop.exe")
            .compile()
            .expect("embed Windows branding");
    }
}
