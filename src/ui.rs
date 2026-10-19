use duck_workshop::project::{Project, Session};
use duck_workshop::{
    Result, diff, meta,
    models::{self, Model},
    patterns::{self, Pattern},
    render::{self, Chevron},
};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
mod live;
mod sequencer;
mod sirens;
mod stripes;
mod theme;
mod updates;
mod vehicle_resource;
mod viewer;
mod workspace;
use std::{io::Write, path::PathBuf, sync::mpsc};
use theme::*;

const ACCENT: Color32 = Color32::from_rgb(0xD7, 0xB9, 0x74);
const MUTED: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x9C);
const TOOLS: [(&str, &str); 5] = [
    ("UV Editor", "Models, textures and UVs"),
    ("Lightboard", "Build lightbar sequences"),
    ("Stripe Designer", "Create warning textures"),
    ("Meta Editor", "Edit vehicle metadata"),
    ("Handling Compare", "Compare handling fields"),
];
const META_NAMES: [&str; 4] = ["handling", "vehicles", "carvariations", "carcols"];

enum JobResult {
    Model(Model),
    VehicleCatalog(duck_workshop::vehicle_resource::Catalog),
    Vehicle(Box<duck_workshop::vehicle_resource::Imported>),
    VehicleFile(Box<duck_workshop::vehicle_resource::Imported>),
    Saved(String),
    Project(Box<Project>, PathBuf),
    ProjectSaved(PathBuf),
}
pub struct Workbench {
    vehicle_catalog: Option<duck_workshop::vehicle_resource::Catalog>,
    vehicle_choice: usize,
    vehicle_high: bool,
    session: Session,
    tool: usize,
    status: String,
    error: bool,
    job: Option<mpsc::Receiver<Result<JobResult>>>,
    model: Option<Model>,
    uv_edges: Vec<([f32; 2], [f32; 2])>,
    uv_faces: Vec<render::UvFace>,
    uv_zoom: f32,
    uv_pan: Vec2,
    uv_texture: bool,
    camera: viewer::Camera,
    scene: Option<std::sync::Arc<viewer::SceneMesh>>,
    scene_version: u64,
    gpu: viewer::SharedGpu,
    z_up: bool,
    paint: [u8; 3],
    live: live::Live,
    ctx: egui::Context,
    view: usize,
    wire: bool,
    blink_play: bool,
    blink_start: f64,
    blink_step: usize,
    blink_selected: usize,
    blink_decimal: Vec<String>,
    blink_source_settings: Vec<String>,
    blink_undo: Vec<Pattern>,
    blink_redo: Vec<Pattern>,
    paint_value: Option<Option<usize>>,
    blink_brush: usize,
    chevron_texture: Option<egui::TextureHandle>,
    stripe_presets: Vec<egui::TextureHandle>,
    chevron_dirty: bool,
    siren_camera: viewer::Camera,
    siren_scene: Option<std::sync::Arc<viewer::SceneMesh>>,
    siren_embedded: std::collections::BTreeMap<String, std::sync::Arc<image::RgbaImage>>,
    siren_version: u64,
    siren_gpu: viewer::SharedGpu,
    siren_popout: bool,
    siren_was_lightboard: bool,
    siren_pin: bool,
    meta_kind: usize,
    meta_index: usize,
    meta_fields: Vec<meta::Field>,
    meta_entries: Vec<String>,
    meta_filter: String,
    meta_raw: bool,
    diff_text: [String; 2],
    diff_paths: [String; 2],
    diff_rows: Vec<diff::Row>,
    diff_filter: String,
    diff_show_same: bool,
    project_path: Option<PathBuf>,
    notice_time: Option<std::time::Instant>,
    show_help: bool,
    show_about: bool,
    confirm_new: bool,
    dark_mode: bool,
    palette_color: [u8; 3],
    show_values: bool,
    sequence_clipboard: Option<patterns::Channel>,
    paint_started: bool,
    paint_previous: Option<(usize, usize)>,
    show_tools: bool,
    confirm_close: bool,
    allow_close: bool,
    window_title: String,
    logo: egui::TextureHandle,
    updater: updates::Updater,
    update_after_save: bool,
}
impl Workbench {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let logo = image::load_from_memory(include_bytes!("../assets/duck-workshop.png"))
            .expect("embedded duck logo")
            .resize(128, 128, image::imageops::FilterType::Nearest)
            .into_rgba8();
        let logo = cc.egui_ctx.load_texture(
            "duck-logo",
            egui::ColorImage::from_rgba_unmultiplied(
                [logo.width() as usize, logo.height() as usize],
                logo.as_raw(),
            ),
            egui::TextureOptions::NEAREST,
        );
        let mut session = cc
            .storage
            .and_then(|s| s.get_string("duck-workshop-v2"))
            .and_then(|s| serde_json::from_str::<Session>(&s).ok())
            .unwrap_or_default();
        if session.pattern.validate().is_err() || session.chevron.validate().is_err() {
            session = Session::default();
        }
        if session.siren_preview.validate().is_err() {
            session.siren_preview = Default::default();
        }
        let mut app = Self {
            vehicle_catalog: None,
            vehicle_choice: 0,
            vehicle_high: true,
            session,
            tool: 0,
            status: String::new(),
            error: false,
            job: None,
            model: None,
            uv_edges: vec![],
            uv_faces: vec![],
            uv_zoom: 1.,
            uv_pan: Vec2::ZERO,
            uv_texture: true,
            camera: viewer::Camera::default(),
            scene: None,
            scene_version: 0,
            gpu: Default::default(),
            z_up: true,
            paint: [214, 216, 220],
            live: live::Live::new(&cc.egui_ctx),
            ctx: cc.egui_ctx.clone(),
            view: 1,
            wire: true,
            blink_play: false,
            blink_start: 0.,
            blink_step: 0,
            blink_selected: 0,
            blink_decimal: vec![],
            blink_source_settings: vec![],
            blink_undo: vec![],
            blink_redo: vec![],
            paint_value: None,
            blink_brush: 0,
            chevron_texture: None,
            stripe_presets: vec![],
            chevron_dirty: true,
            siren_camera: Self::siren_fit_camera(),
            siren_scene: None,
            siren_embedded: Default::default(),
            siren_version: u64::MAX,
            siren_gpu: Default::default(),
            siren_popout: false,
            siren_was_lightboard: false,
            siren_pin: false,
            meta_kind: 0,
            meta_index: 0,
            meta_fields: vec![],
            meta_entries: vec![],
            meta_filter: String::new(),
            meta_raw: false,
            diff_text: [String::new(), String::new()],
            diff_paths: ["Before".into(), "After".into()],
            diff_rows: vec![],
            diff_filter: String::new(),
            diff_show_same: false,
            project_path: None,
            notice_time: None,
            show_help: false,
            show_about: false,
            confirm_new: false,
            dark_mode: false,
            palette_color: [255, 45, 55],
            show_values: false,
            sequence_clipboard: None,
            paint_started: false,
            paint_previous: None,
            show_tools: true,
            confirm_close: false,
            allow_close: false,
            window_title: String::new(),
            logo,
            updater: updates::Updater::new(cc.storage),
            update_after_save: false,
        };
        app.refresh_meta();
        app.sync_decimal();
        app
    }
    fn message(&mut self, result: Result<String>) {
        self.notice_time = Some(std::time::Instant::now());
        match result {
            Ok(s) => {
                self.status = s;
                self.error = false
            }
            Err(e) => {
                self.status = e;
                self.error = true
            }
        }
    }
    fn start_job(&mut self, f: impl FnOnce() -> Result<JobResult> + Send + 'static) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
                .unwrap_or_else(|_| Err("This file could not be processed safely.".into()));
            let _ = tx.send(result);
        });
        self.job = Some(rx);
        self.status = "Processing locally…".into();
        self.error = false;
    }
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.job {
            match rx.try_recv() {
                Ok(result) => {
                    self.job = None;
                    match result {
                        Ok(JobResult::Model(model)) => self.accept_model(model),
                        Ok(JobResult::VehicleCatalog(catalog)) => {
                            self.vehicle_choice = 0;
                            self.vehicle_catalog = Some(catalog);
                            self.status = "Choose the vehicle to import".into();
                        }
                        Ok(JobResult::Vehicle(imported)) => self.accept_vehicle(*imported, true),
                        Ok(JobResult::VehicleFile(imported)) => {
                            self.accept_vehicle(*imported, false)
                        }
                        Ok(JobResult::Saved(name)) => self.message(Ok(format!("Saved {name}"))),
                        Ok(JobResult::Project(project, path)) => {
                            self.restore_project(*project, path)
                        }
                        Ok(JobResult::ProjectSaved(path)) => {
                            self.project_path = Some(path.clone());
                            self.message(Ok(format!(
                                "Saved {}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            )));
                            if self.update_after_save {
                                self.install_update(ctx);
                            }
                        }
                        Err(e) => {
                            self.update_after_save = false;
                            self.message(Err(e));
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.job = None;
                    self.update_after_save = false;
                    self.message(Err("Background task stopped.".into()));
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(40))
                }
            }
        }
    }
    fn accept_model(&mut self, model: Model) {
        self.status = format!(
            "Loaded {} · {} meshes · {} triangles",
            model.name,
            model.meshes.len(),
            model.triangles()
        );
        self.notice_time = Some(std::time::Instant::now());
        self.orient(&model);
        let materials: Vec<String> = render::materials(&model).into_iter().collect();
        self.live.retain_materials(&materials);
        render::select_template(&model, &mut self.session.uv);
        self.model = Some(model);
        self.camera = viewer::Camera::default();
        self.siren_camera = Self::siren_fit_camera();
        self.session.siren_preview.positions.clear();
        self.view = 1;
        self.refresh_uv();
        self.uv_zoom = 1.;
        self.uv_pan = Vec2::ZERO;
        self.error = false;
    }
    fn import_model(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Mapped models",
                &["yft", "ydr", "xml", "obj", "glb", "json"],
            )
            .pick_file()
        {
            self.start_job(move || Self::load_model_file(&path));
        }
    }
    fn load_model_file(path: &std::path::Path) -> Result<JobResult> {
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("yft"))
        {
            Ok(JobResult::VehicleFile(Box::new(
                duck_workshop::vehicle_resource::import_file(path)?,
            )))
        } else {
            Ok(JobResult::Model(models::load(path)?))
        }
    }
    fn refresh_uv(&mut self) {
        self.uv_faces = self
            .model
            .as_ref()
            .filter(|_| self.session.uv.surface_colors)
            .map(|model| render::uv_faces(model, &self.session.uv))
            .unwrap_or_default();
        self.uv_edges = self
            .model
            .as_ref()
            .map(|m| render::uv_edges(m, &self.session.uv))
            .unwrap_or_default();
        self.scene_version += 1;
        self.scene = self
            .model
            .as_ref()
            .and_then(|m| {
                if self.session.uv_whole_vehicle {
                    viewer::build_vehicle(m, &self.session.uv, self.z_up, self.scene_version)
                } else {
                    viewer::build(m, &self.session.uv, self.z_up, self.scene_version)
                }
            })
            .map(std::sync::Arc::new);
        self.chevron_dirty = true;
    }
    /// Picks the up axis and V direction that suit the model's source format.
    fn orient(&mut self, model: &Model) {
        let ext = model.name.rsplit('.').next().unwrap_or("").to_lowercase();
        let mut min = glam::Vec3::splat(f32::MAX);
        let mut max = glam::Vec3::splat(f32::MIN);
        for p in model.meshes.iter().flat_map(|m| &m.positions) {
            min = min.min(glam::Vec3::from(*p));
            max = max.max(glam::Vec3::from(*p));
        }
        let extent = max - min;
        // Vehicles are longer than they are tall: a long Y axis means Z is up.
        self.z_up = match ext.as_str() {
            "yft" | "ydr" | "xml" => true,
            "glb" => false,
            _ => extent.y > extent.z,
        };
        match ext.as_str() {
            "obj" => self.session.uv.flip_v = true,
            "yft" | "ydr" | "xml" | "glb" => self.session.uv.flip_v = false,
            _ => {}
        }
    }
    fn save_bytes(&mut self, name: &str, ext: &str, bytes: Vec<u8>) {
        if let Some(path) = save_dialog(name, ext) {
            self.start_job(move || {
                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                Ok(JobResult::Saved(path.display().to_string()))
            });
        }
    }
    fn uv_export(&mut self, svg: bool) {
        let Some(model) = self.model.clone() else {
            self.message(Err("Import a mapped model first.".into()));
            return;
        };
        let opt = self.session.uv.clone();
        let ext = if svg { "svg" } else { "png" };
        if let Some(path) = save_dialog(&format!("uv-template.{ext}"), ext) {
            self.start_job(move || {
                let bytes = if svg {
                    render::uv_svg(&model, &opt)?.into_bytes()
                } else {
                    render::uv_png(&model, &opt)?
                };
                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                Ok(JobResult::Saved(path.display().to_string()))
            });
        }
    }
    fn uv_zip(&mut self) {
        let Some(model) = self.model.clone() else {
            return;
        };
        let mut opt = self.session.uv.clone();
        opt.material = None;
        if let Some(path) = save_dialog("uv-materials.zip", "zip") {
            self.start_job(move || {
                let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                let mut zip = zip::ZipWriter::new(file);
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                let mut count = 0;
                for (i, material) in render::materials(&model).into_iter().enumerate() {
                    let mut selection = model.clone();
                    for m in &mut selection.meshes {
                        m.enabled = m.enabled && m.material == material;
                    }
                    if render::uv_edges(&selection, &opt).is_empty() {
                        continue;
                    }
                    zip.start_file(
                        format!("{:02}-{}.png", i + 1, safe_name(&material)),
                        options,
                    )
                    .map_err(|e| e.to_string())?;
                    zip.write_all(&render::uv_png(&selection, &opt)?)
                        .map_err(|e| e.to_string())?;
                    count += 1;
                }
                if count == 0 {
                    return Err("No selected materials contain UVs.".into());
                }
                zip.finish().map_err(|e| e.to_string())?;
                Ok(JobResult::Saved(path.display().to_string()))
            });
        }
    }
    fn uv(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if primary(ui, icon_text(icon::IMPORT, "Import Model")).clicked() && self.job.is_none()
            {
                self.import_model()
            }
            if ui.button("Import folder").clicked() && self.job.is_none() {
                self.import_vehicle_folder();
            }
            if ui
                .add_enabled(
                    self.model.is_some(),
                    egui::Button::new(icon_text(icon::OPEN, "Load Texture")),
                )
                .on_hover_text("PNG, JPG, PSD, DDS, TGA, BMP, TIFF, WebP or GIF")
                .on_disabled_hover_text("Import a model first")
                .clicked()
            {
                self.pick_texture(None);
            }
            if ui
                .add_enabled(
                    self.model.is_some() && self.job.is_none(),
                    egui::Button::new(icon_text(icon::SAVE, "Save UV Project")),
                )
                .clicked()
            {
                match serde_json::to_vec_pretty(self.model.as_ref().unwrap()) {
                    Ok(bytes) => self.save_bytes("uv-project.json", "json", bytes),
                    Err(e) => self.message(Err(e.to_string())),
                }
            }
            ui.label(
                RichText::new(
                    "Models: YFT / YDR / XML / OBJ / GLB    Textures: PNG / JPG / PSD / DDS / TGA",
                )
                .color(MUTED)
                .small(),
            );
        });
        ui.add_space(4.);
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            let width = (ui.available_width() - 290.).max(300.);
            ui.allocate_ui_with_layout(
                Vec2::new(width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.horizontal(|ui| {
                        tabs(ui, &mut self.view, &[(1, "3D Preview"), (0, "UV Template")]);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Fit View").clicked() {
                                self.fit_view();
                            }
                            if self.view == 1 {
                                for (name, yaw, pitch) in viewer::VIEWS.iter().rev() {
                                    if ui.small_button(*name).clicked() {
                                        self.camera.yaw = *yaw;
                                        self.camera.pitch = *pitch;
                                    }
                                }
                                ui.checkbox(&mut self.wire, "Wireframe");
                            } else {
                                ui.checkbox(&mut self.uv_texture, "Show texture")
                                    .on_hover_text("Show your linked artwork instead of the colored surface guide");
                            }
                        });
                    });
                    if self.view == 1 {
                        let previous = self.session.uv_whole_vehicle;
                        ui.horizontal(|ui| {
                            ui.label("Preview");
                            ui.selectable_value(&mut self.session.uv_whole_vehicle, true, "Whole vehicle");
                            ui.selectable_value(&mut self.session.uv_whole_vehicle, false, "Shell / UV selection")
                                .on_hover_text("Show only the material and meshes selected for the UV template");
                        });
                        if previous != self.session.uv_whole_vehicle {
                            self.refresh_uv();
                            self.camera = viewer::Camera::default();
                        }
                    }
                    let available = Vec2::new(
                        ui.available_width(),
                        (ui.available_height() - 30.).max(100.),
                    );
                    if self.view == 0 {
                        self.uv_canvas(ui, available)
                    } else {
                        self.model_view(ui, available)
                    }
                    ui.horizontal(|ui| match &self.model {
                        Some(m) => {
                            ui.label(RichText::new(&m.name).strong());
                            ui.label(format!("{} meshes", m.meshes.len()));
                            ui.label(format!("{} triangles", m.triangles()));
                            ui.label(format!("{} UV edges selected", self.uv_edges.len()));
                        }
                        None => {
                            ui.label("Import a model, then load a texture to see it in 3D.");
                        }
                    });
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                Vec2::new(270., height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("uv-inspector")
                        .show(ui, |ui| self.uv_inspector(ui));
                },
            );
        });
    }

    fn uv_inspector(&mut self, ui: &mut egui::Ui) {
        ui.set_width(258.);
        section(ui, "TEXTURES");
        let materials: Vec<String> = self
            .model
            .as_ref()
            .map(|m| {
                let mut list: Vec<String> = vec![];
                for mesh in &m.meshes {
                    if !list.contains(&mesh.material) {
                        list.push(mesh.material.clone());
                    }
                }
                list
            })
            .unwrap_or_default();
        if materials.is_empty() {
            ui.label(
                RichText::new("Import a model to add textures.")
                    .small()
                    .color(MUTED),
            );
        }
        let mut pick = None;
        let mut clear = None;
        let mut reload = None;
        egui::ScrollArea::vertical()
            .id_salt("texture-list")
            .max_height(230.)
            .show(ui, |ui| {
                for material in &materials {
                    egui::Frame::new()
                        .fill(LIST_BG)
                        .stroke(Stroke::new(1.0_f32, LIST_LINE))
                        .inner_margin(egui::Margin::symmetric(8, 5))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(material).color(TEXT))
                                        .truncate(),
                                );
                            });
                            let file = self.live.for_material(material);
                            let (text, color) = match file {
                                None if self.siren_embedded.contains_key(material) => ("Imported vehicle texture".to_string(), CAPTION),
                                None => ("No texture".to_string(), MUTED),
                                Some(f) if f.loading && f.image.is_none() => {
                                    (format!("Loading {}…", f.name()), CAPTION)
                                }
                                Some(f) if f.image.is_none() => {
                                    (format!("{} could not be loaded", f.name()), ERROR)
                                }
                                Some(f) => (f.name(), ACCENT),
                            };
                            ui.add(
                                egui::Label::new(RichText::new(text).small().color(color))
                                    .truncate(),
                            )
                            .on_hover_text(
                                file.map(|f| {
                                    let mut tip = f.path.display().to_string();
                                    if let Some(e) = &f.error {
                                        tip += &format!("\n{e}");
                                    }
                                    tip
                                })
                                .unwrap_or_else(|| if self.siren_embedded.contains_key(material) {
                                    "Imported from the vehicle texture dictionary. Choose an image to override it.".into()
                                } else {
                                    "Choose an image for this material".into()
                                }),
                            );
                            ui.horizontal(|ui| {
                                if ui.small_button("Choose…").clicked() {
                                    pick = Some(material.clone());
                                }
                                if let Some(f) = file {
                                    if ui.small_button("Reload").clicked() {
                                        reload = Some(f.path.clone());
                                    }
                                    if ui.small_button("Remove").clicked() {
                                        clear = Some(material.clone());
                                    }
                                }
                            });
                        });
                    ui.add_space(3.);
                }
            });
        if let Some(material) = pick {
            self.pick_texture(Some(material));
        }
        if let Some(material) = clear {
            self.live.unassign(&material);
        }
        if let Some(path) = reload {
            self.live.reload(ui.ctx(), &path);
        }
        if !self.live.files.is_empty() {
            ui.label(
                RichText::new("Textures update on the model each time the file is saved, e.g. from Photoshop.")
                    .small()
                    .color(MUTED),
            );
        }
        ui.add_space(4.);
        egui::Grid::new("preview-settings")
            .num_columns(2)
            .spacing([12., 8.])
            .show(ui, |ui| {
                ui.label("Base paint");
                ui.color_edit_button_srgb(&mut self.paint).on_hover_text(
                    "Shown where the model has no texture or the texture is transparent",
                );
                ui.end_row();
            });
        let before = (self.z_up, self.session.uv.flip_v);
        ui.checkbox(&mut self.z_up, "Z axis is up (GTA models)");
        ui.checkbox(&mut self.session.uv.flip_v, "Flip V")
            .on_hover_text("Turn on if the texture appears upside down on the model");
        if before != (self.z_up, self.session.uv.flip_v) {
            self.refresh_uv();
        }
        ui.separator();
        section(ui, "TEMPLATE");
        let before = serde_json::to_string(&self.session.uv).unwrap();
        let previous_style = self.session.uv.surface_colors;
        egui::ComboBox::from_label("Style")
            .selected_text(if previous_style {
                "Colored surface + wireframe"
            } else {
                "Lines only"
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.session.uv.surface_colors,
                    true,
                    "Colored surface + wireframe",
                );
                ui.selectable_value(&mut self.session.uv.surface_colors, false, "Lines only");
            });
        if previous_style != self.session.uv.surface_colors {
            let colored = self.session.uv.surface_colors;
            render::set_template_style(&mut self.session.uv, colored);
        }
        if let Some(model) = &self.model {
            let previous = self.session.uv.material.clone();
            egui::ComboBox::from_label("Material")
                .width(210.)
                .selected_text(previous.as_deref().unwrap_or("All materials (overlaid)"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.session.uv.material,
                        None,
                        "All materials (overlaid)",
                    );
                    for material in render::materials(model) {
                        ui.selectable_value(
                            &mut self.session.uv.material,
                            Some(material.clone()),
                            &material,
                        );
                    }
                });
            if previous != self.session.uv.material {
                let material = self.session.uv.material.clone();
                render::select_material(model, &mut self.session.uv, material);
            }
            if ui.button("Auto select livery / material").clicked() {
                render::select_template(model, &mut self.session.uv);
            }
            if let Some(info) = self
                .session
                .uv
                .material
                .as_ref()
                .and_then(|name| model.material_info.get(name))
            {
                let text = if info.is_livery() {
                    format!("Livery: {} · use UV 1", info.secondary_texture)
                } else {
                    format!("{} · {}", info.diffuse_texture, info.shader)
                };
                ui.label(RichText::new(text).small().color(MUTED));
            } else if self.session.uv.material.is_none() {
                ui.label(
                    RichText::new(
                        "Materials overlap here. Choose one texture's material before painting.",
                    )
                    .small()
                    .color(MUTED),
                );
            }
        }
        egui::ComboBox::from_label("LOD")
            .selected_text(&self.session.uv.lod)
            .show_ui(ui, |ui| {
                for l in ["All", "High", "Med", "Low", "VeryLow"] {
                    ui.selectable_value(&mut self.session.uv.lod, l.into(), l);
                }
            });
        egui::ComboBox::from_label("UV Channel")
            .selected_text(format!("UV {}", self.session.uv.channel))
            .show_ui(ui, |ui| {
                let channels = self
                    .model
                    .as_ref()
                    .map(|model| render::uv_channels(model, &self.session.uv))
                    .unwrap_or_else(|| (0..8).collect());
                for c in channels {
                    ui.selectable_value(&mut self.session.uv.channel, c, format!("UV {c}"));
                }
            });
        ui.horizontal(|ui| {
            ui.label("Resolution");
            ui.add(
                egui::DragValue::new(&mut self.session.uv.size)
                    .range(64..=8192)
                    .suffix(" px"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Line width");
            ui.add(
                egui::DragValue::new(&mut self.session.uv.width)
                    .range(0.1..=20.)
                    .speed(0.1),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Line color");
            ui.color_edit_button_srgba_unmultiplied(&mut self.session.uv.color);
        });
        ui.horizontal(|ui| {
            ui.label("Background");
            ui.color_edit_button_srgba_unmultiplied(&mut self.session.uv.background);
        });
        ui.checkbox(&mut self.session.uv.boundaries, "Island outlines only")
            .on_hover_text("Hide internal triangle edges for a cleaner painting guide");
        if before != serde_json::to_string(&self.session.uv).unwrap() {
            self.refresh_uv()
        }
        ui.separator();
        section(ui, "MESH SELECTION");
        let mut changed = false;
        if let Some(model) = &mut self.model {
            ui.horizontal(|ui| {
                if ui.small_button("All").clicked() {
                    for m in &mut model.meshes {
                        m.enabled = true
                    }
                    changed = true;
                }
                if ui.small_button("None").clicked() {
                    for m in &mut model.meshes {
                        m.enabled = false
                    }
                    changed = true;
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("mesh-list")
                .max_height(200.)
                .show(ui, |ui| {
                    for m in &mut model.meshes {
                        changed |= ui
                            .checkbox(&mut m.enabled, &m.name)
                            .on_hover_text(format!(
                                "{} · {} · {} triangles",
                                m.material,
                                m.lod,
                                m.indices.len() / 3
                            ))
                            .changed();
                    }
                });
        }
        if changed {
            self.refresh_uv()
        }
        ui.separator();
        section(ui, "EXPORT");
        let enabled = self.model.is_some() && self.job.is_none();
        if ui
            .add_enabled_ui(enabled, |ui| {
                primary(ui, icon_text(icon::EXPORT, "Export PNG"))
            })
            .inner
            .clicked()
        {
            self.uv_export(false)
        }
        if ui
            .add_enabled(enabled, egui::Button::new("Export SVG"))
            .on_hover_text("Editable vector lines; colored surface shading is embedded as a PNG")
            .clicked()
        {
            self.uv_export(true)
        }
        if ui
            .add_enabled(enabled, egui::Button::new("PNG per Material (ZIP)"))
            .clicked()
        {
            self.uv_zip()
        }
        ui.label(
            RichText::new(
                "Exports clip to the 0–1 tile. UVs are recovered without changing the model.",
            )
            .small()
            .color(MUTED),
        );
        if let Some(m) = &self.model {
            for warning in &m.warnings {
                ui.label(RichText::new(warning).small().color(MUTED));
            }
        }
    }

    /// Opens a texture for one material, or for every selected material when `None`.
    fn pick_texture(&mut self, material: Option<String>) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Textures", &duck_workshop::textures::EXTENSIONS)
            .pick_file()
        else {
            return;
        };
        self.apply_texture(material, path);
    }

    fn apply_texture(&mut self, material: Option<String>, path: PathBuf) {
        let Some(model) = &self.model else {
            self.message(Err("Import a model before loading a texture.".into()));
            return;
        };
        let materials = match material {
            Some(m) => vec![m],
            None => {
                let mut list: Vec<String> = vec![];
                for m in render::selected(model, &self.session.uv) {
                    let mapped = m
                        .uvs
                        .get(self.session.uv.channel)
                        .is_some_and(|u| !u.is_empty());
                    if mapped && !list.contains(&m.material) {
                        list.push(m.material.clone());
                    }
                }
                if list.is_empty() {
                    list = render::materials(model).into_iter().collect();
                }
                list
            }
        };
        self.status = format!("Loading {}…", live::file_name(&path));
        self.error = false;
        self.notice_time = Some(std::time::Instant::now());
        self.live.assign(&self.ctx, materials, path);
        self.view = 1;
    }

    fn fit_view(&mut self) {
        self.uv_zoom = 1.;
        self.uv_pan = Vec2::ZERO;
        let (yaw, pitch) = (self.camera.yaw, self.camera.pitch);
        self.camera = viewer::Camera {
            yaw,
            pitch,
            ..Default::default()
        };
    }

    fn uv_canvas(&mut self, ui: &mut egui::Ui, size: Vec2) {
        let (response, painter) = ui.allocate_painter(size, Sense::drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0., VIEWPORT);
        if response.dragged() {
            self.uv_pan += ui.input(|i| i.pointer.delta());
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0. {
                self.uv_zoom = (self.uv_zoom * (scroll * 0.003).exp()).clamp(0.2, 15.);
            }
        }
        let side = rect.width().min(rect.height()) * 0.86 * self.uv_zoom;
        let tile = Rect::from_center_size(rect.center() + self.uv_pan, Vec2::splat(side));
        let p = painter.with_clip_rect(rect);
        let checker = (side / 16.).max(2.);
        for y in 0..16 {
            for x in 0..16 {
                let r = Rect::from_min_size(
                    tile.min + Vec2::new(x as f32 * checker, y as f32 * checker),
                    Vec2::splat(checker),
                );
                p.rect_filled(
                    r,
                    0.,
                    if (x + y) % 2 == 0 {
                        Color32::WHITE
                    } else {
                        Color32::from_gray(241)
                    },
                );
            }
        }
        p.rect_filled(
            tile,
            0.,
            Color32::from_rgba_unmultiplied(
                self.session.uv.background[0],
                self.session.uv.background[1],
                self.session.uv.background[2],
                self.session.uv.background[3],
            ),
        );
        let background = self
            .model
            .as_ref()
            .filter(|_| self.uv_texture)
            .and_then(|m| {
                render::selected(m, &self.session.uv)
                    .find_map(|mesh| self.live.assigned.get(&mesh.material))
                    .cloned()
            });
        let mut has_texture = false;
        if let Some(path) = background
            && let Some(texture) = self.live.preview(ui.ctx(), &path)
        {
            has_texture = true;
            p.image(
                texture,
                tile,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
        }
        let clipped = p.with_clip_rect(rect.intersect(tile));
        let map = |v: [f32; 2]| {
            Pos2::new(
                tile.left() + v[0] * side,
                tile.top()
                    + (if self.session.uv.flip_v {
                        1. - v[1]
                    } else {
                        v[1]
                    }) * side,
            )
        };
        let color = Color32::from_rgba_unmultiplied(
            self.session.uv.color[0],
            self.session.uv.color[1],
            self.session.uv.color[2],
            self.session.uv.color[3],
        );
        if self.model.is_some() {
            if !has_texture && !self.uv_faces.is_empty() {
                let mut mesh = egui::Mesh::default();
                for face in &self.uv_faces {
                    let base = mesh.vertices.len() as u32;
                    for i in 0..3 {
                        mesh.colored_vertex(
                            map(face.uv[i]),
                            Color32::from_rgb(
                                face.colors[i][0],
                                face.colors[i][1],
                                face.colors[i][2],
                            ),
                        );
                    }
                    mesh.add_triangle(base, base + 1, base + 2);
                }
                clipped.add(egui::Shape::mesh(mesh));
            }
            for (a, b) in &self.uv_edges {
                let width =
                    (self.session.uv.width * side / self.session.uv.size as f32).clamp(0.45, 4.);
                clipped.line_segment([map(*a), map(*b)], Stroke::new(width, color));
            }
        } else {
            p.text(
                tile.center(),
                Align2::CENTER_CENTER,
                "Your UV template appears here",
                egui::FontId::proportional(21.),
                MUTED,
            );
        }
        p.rect_stroke(
            tile,
            0.,
            Stroke::new(1.0_f32, Color32::from_gray(150)),
            egui::StrokeKind::Inside,
        );
        p.text(
            rect.left_bottom() + Vec2::new(12., -12.),
            Align2::LEFT_BOTTOM,
            "Drag to pan · Scroll to zoom · 0–1 UV tile",
            egui::FontId::proportional(12.),
            MUTED,
        );
    }

    fn model_view(&mut self, ui: &mut egui::Ui, size: Vec2) {
        if self.model.is_some() {
            self.prepare_siren_scene();
        }
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0., VIEWPORT);
        let Some(scene) = self.scene.clone() else {
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                if self.model.is_some() {
                    "No meshes selected"
                } else {
                    "Import a model to preview it in 3D"
                },
                egui::FontId::proportional(20.),
                CAPTION,
            );
            return;
        };
        let pointer = ui.input(|i| (i.pointer.delta(), i.modifiers.shift));
        if response.dragged_by(egui::PointerButton::Primary) && !pointer.1 {
            self.camera.yaw -= pointer.0.x * 0.008;
            self.camera.pitch = (self.camera.pitch + pointer.0.y * 0.008).clamp(-1.55, 1.55);
        } else if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Primary)
        {
            viewer::pan(&mut self.camera, pointer.0, rect.height());
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0. {
                self.camera.distance =
                    (self.camera.distance * (-scroll * 0.0018).exp()).clamp(0.05, 40.);
            }
        }
        if response.double_clicked() {
            self.fit_view();
        }
        let mut textures: Vec<(PathBuf, u64, std::sync::Arc<image::RgbaImage>)> = vec![];
        let bindings = scene
            .groups
            .iter()
            .map(|g| {
                let (path, version, image) = if let Some(file) = self
                    .live
                    .for_material(&g.material)
                    .filter(|f| f.image.is_some())
                {
                    (file.path.clone(), file.version, file.image.clone()?)
                } else {
                    (
                        PathBuf::from(format!("@vehicle/{}", g.material)),
                        self.scene_version,
                        self.siren_embedded.get(&g.material)?.clone(),
                    )
                };
                Some(match textures.iter().position(|(p, ..)| *p == path) {
                    Some(i) => i,
                    None => {
                        textures.push((path, version, image));
                        textures.len() - 1
                    }
                })
            })
            .collect();
        let frame = viewer::Frame {
            vehicle_opacity: 1.,
            siren_surfaces: false,
            night: false,
            mesh: scene,
            textures,
            bindings,
            camera: self.camera,
            wire: self.wire,
            paint: self.paint,
            lights: vec![],
        };
        p.add(viewer::callback(&self.gpu, rect, frame));
        let mut y = rect.top() + 12.;
        for file in &self.live.files {
            let (text, color) = if let Some(e) = &file.error {
                (format!("{}: {e}", file.name()), ERROR)
            } else if file.loading {
                (format!("Loading {}…", file.name()), CAPTION)
            } else if file.updated.is_some_and(|t| t.elapsed().as_secs_f32() < 3.) {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(500));
                (format!("{} updated", file.name()), SUCCESS)
            } else {
                (format!("{} · updates when saved", file.name()), CAPTION)
            };
            p.text(
                Pos2::new(rect.left() + 12., y),
                Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(12.),
                color,
            );
            y += 17.;
        }
        if let Some(error) = self
            .gpu
            .lock()
            .ok()
            .and_then(|g| g.error().map(String::from))
        {
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                format!("3D preview unavailable: {error}"),
                egui::FontId::proportional(14.),
                ERROR,
            );
        }
        p.text(
            rect.left_bottom() + Vec2::new(12., -12.),
            Align2::LEFT_BOTTOM,
            "Drag to rotate · Right-drag or Shift+drag to move · Scroll to zoom · Double-click to fit",
            egui::FontId::proportional(12.),
            CAPTION,
        );
    }
    fn blink_snapshot(&mut self) {
        self.blink_undo.push(self.session.pattern.clone());
        if self.blink_undo.len() > 100 {
            self.blink_undo.remove(0);
        }
        self.blink_redo.clear();
    }
    fn sync_decimal(&mut self) {
        self.blink_selected = self
            .blink_selected
            .min(self.session.pattern.channels.len().saturating_sub(1));
        let channel = &self.session.pattern.channels[self.blink_selected];
        self.blink_decimal = (0..channel.color_count())
            .map(|i| patterns::decimal(channel.bits_at(i)).to_string())
            .collect();
        self.blink_brush = self.blink_brush.min(channel.color_count() - 1);
    }
    fn refresh_meta(&mut self) {
        let text = &self.session.metas[self.meta_kind];
        if text.trim().is_empty() {
            self.meta_fields.clear();
            self.meta_entries.clear();
            return;
        }
        match meta::entries(text, self.meta_kind) {
            Ok(entries) => {
                self.meta_entries = entries;
                self.meta_index = self.meta_index.min(self.meta_entries.len() - 1);
                self.meta_fields =
                    meta::fields(text, self.meta_kind, self.meta_index).unwrap_or_default();
            }
            Err(e) => {
                self.meta_fields.clear();
                self.meta_entries.clear();
                self.message(Err(e));
            }
        }
    }
    fn apply_meta(&mut self) -> Result<()> {
        if self.session.metas[self.meta_kind].trim().is_empty() {
            return Ok(());
        }
        if self.meta_raw {
            meta::entries(&self.session.metas[self.meta_kind], self.meta_kind)?;
            return Ok(());
        }
        let text = meta::update(
            &self.session.metas[self.meta_kind],
            self.meta_kind,
            self.meta_index,
            &self.meta_fields,
        )?;
        self.session.metas[self.meta_kind] = text;
        Ok(())
    }
    fn meta(&mut self, ui: &mut egui::Ui) {
        let mut choice = self.meta_kind;
        let files: Vec<_> = META_NAMES.iter().map(|n| format!("{n}.meta")).collect();
        let items: Vec<_> = files
            .iter()
            .enumerate()
            .map(|(i, f)| (i, f.as_str()))
            .collect();
        tabs(ui, &mut choice, &items);
        ui.add_space(4.);
        if choice != self.meta_kind {
            let result = self.apply_meta();
            if result.is_ok() {
                self.meta_kind = choice;
                self.meta_index = 0;
                self.refresh_meta()
            } else {
                self.message(result.map(|_| String::new()));
            }
        }
        ui.horizontal(|ui| {
            if icon_button(ui, icon::IMPORT, "Import .meta").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Metadata", &["meta", "xml"])
                    .pick_file()
            {
                let result = (|| {
                    let bytes = duck_workshop::read_file(&path)?;
                    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                    let kind = meta::detect_kind(&text)?;
                    meta::entries(&text, kind)?;
                    if kind != self.meta_kind {
                        self.apply_meta()?;
                    }
                    self.session.metas[kind] = text;
                    self.meta_kind = kind;
                    self.meta_index = 0;
                    self.refresh_meta();
                    Ok("Imported file. Unknown fields are retained.".into())
                })();
                self.message(result);
            }
            if primary(ui, icon_text(icon::EXPORT, "Export Current File")).clicked() {
                let result = self.apply_meta();
                match result {
                    Ok(()) => self.save_bytes(
                        &format!("{}.meta", META_NAMES[self.meta_kind]),
                        "meta",
                        self.session.metas[self.meta_kind].clone().into_bytes(),
                    ),
                    Err(e) => self.message(Err(e)),
                }
            }
            if ui.button("Export Resource ZIP").clicked() {
                self.meta_zip()
            }
            if ui.button("Use Light Sequence").clicked() {
                match self.session.pattern.to_xml() {
                    Ok(x) => {
                        self.session.metas[3] = x;
                        self.meta_kind = 3;
                        self.meta_index = 0;
                        self.refresh_meta()
                    }
                    Err(e) => self.message(Err(e)),
                }
            }
            if ui.checkbox(&mut self.meta_raw, "XML Editor").changed() {
                if self.meta_raw {
                    self.meta_raw = false;
                    let result = self.apply_meta();
                    self.meta_raw = true;
                    if let Err(e) = result {
                        self.message(Err(e));
                    }
                } else {
                    let result = meta::entries(&self.session.metas[self.meta_kind], self.meta_kind);
                    if result.is_ok() {
                        self.refresh_meta()
                    } else {
                        self.meta_raw = true;
                        self.message(result.map(|_| String::new()));
                    }
                }
            }
        });
        ui.separator();
        ui.collapsing("Create Vehicle Metadata", |ui| {
            ui.horizontal(|ui| {
                ui.label("Model name"); ui.add(egui::TextEdit::singleline(&mut self.session.model_name).hint_text("Your model filename"));
                ui.label("Handling ID"); ui.add(egui::TextEdit::singleline(&mut self.session.handling_name).hint_text("Your handling identifier"));
            });
            if ui.button("Create Metadata Files").clicked() {
                let valid_name = |name: &str| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
                if !valid_name(&self.session.model_name) || !valid_name(&self.session.handling_name) {
                    self.message(Err("Enter a model name and handling ID using letters, numbers, underscores or hyphens.".into()));
                } else {
                    self.session.metas = [meta::format_xml(&meta::handling(&self.session.handling_name)).unwrap(),meta::format_xml(&meta::vehicles(&self.session.model_name,&self.session.handling_name)).unwrap(),meta::format_xml(&meta::variations(&self.session.model_name,self.session.pattern.id,0)).unwrap(),self.session.pattern.to_xml().unwrap()];
                    self.meta_index=0; self.refresh_meta(); self.message(Ok("Vehicle metadata created".into()));
                }
            }
        });
        if self.session.metas[self.meta_kind].trim().is_empty() {
            ui.add_space(24.);
            ui.label("Import a metadata file or create files for your vehicle.");
            return;
        }
        if self.meta_raw {
            ui.label("Edit XML, then Validate & Apply to update the guided fields.");
            if ui.button("Validate & Apply XML").clicked() {
                let result = meta::entries(&self.session.metas[self.meta_kind], self.meta_kind)
                    .map(|_| "XML validated.".into());
                if result.is_ok() {
                    self.refresh_meta()
                }
                self.message(result);
            }
            egui::ScrollArea::both().id_salt("meta-raw").show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.session.metas[self.meta_kind])
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(30),
                );
            });
            return;
        }
        let previous = self.meta_index;
        egui::ComboBox::from_label("Entry")
            .selected_text(
                self.meta_entries
                    .get(self.meta_index)
                    .map(String::as_str)
                    .unwrap_or("None"),
            )
            .show_ui(ui, |ui| {
                for (i, n) in self.meta_entries.iter().enumerate() {
                    ui.selectable_value(&mut self.meta_index, i, n);
                }
            });
        if previous != self.meta_index {
            let next = self.meta_index;
            self.meta_index = previous;
            let result = self.apply_meta();
            self.meta_index = next;
            if result.is_ok() {
                self.refresh_meta()
            } else {
                self.message(result.map(|_| String::new()));
            }
        }
        ui.horizontal(|ui| {
            ui.label("Find field");
            ui.text_edit_singleline(&mut self.meta_filter);
            if ui.button("Apply Changes").clicked() {
                let result = self
                    .apply_meta()
                    .map(|_| "Changes applied to the local editor.".into());
                if result.is_ok() {
                    self.refresh_meta()
                }
                self.message(result)
            }
        });
        let filter = self.meta_filter.to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("meta-fields")
            .show(ui, |ui| {
                egui::Grid::new("meta-grid")
                    .num_columns(2)
                    .striped(true)
                    .min_col_width(220.)
                    .show(ui, |ui| {
                        for f in &mut self.meta_fields {
                            if !f.path.to_lowercase().contains(&filter) {
                                continue;
                            }
                            ui.label(&f.path);
                            ui.add_sized(
                                [ui.available_width().min(550.), 26.],
                                egui::TextEdit::singleline(&mut f.value),
                            );
                            ui.end_row();
                        }
                    });
            });
    }
    fn meta_zip(&mut self) {
        if let Err(e) = self.apply_meta() {
            self.message(Err(e));
            return;
        }
        let texts = self.session.metas.clone();
        let model = self.session.model_name.clone();
        let manifest = match meta::manifest(&model) {
            Ok(m) => m,
            Err(e) => {
                self.message(Err(e));
                return;
            }
        };
        for (i, t) in texts.iter().enumerate() {
            if let Err(e) = meta::entries(t, i) {
                self.message(Err(e));
                return;
            }
        }
        if let Some(path) = save_dialog(&format!("{model}-metadata.zip"), "zip") {
            self.start_job(move||{let mut zip=zip::ZipWriter::new(std::fs::File::create(&path).map_err(|e|e.to_string())?);let options=zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);for(i,text)in texts.iter().enumerate(){zip.start_file(format!("data/{}.meta",META_NAMES[i]),options).map_err(|e|e.to_string())?;zip.write_all(text.as_bytes()).map_err(|e|e.to_string())?;}zip.start_file("fxmanifest.lua",options).map_err(|e|e.to_string())?;zip.write_all(manifest.as_bytes()).map_err(|e|e.to_string())?;zip.add_directory("stream/",options).map_err(|e|e.to_string())?;zip.start_file("README.txt",options).map_err(|e|e.to_string())?;zip.write_all(b"Add your YFT and YTD files to stream/. Check modelName, handlingId, sirenSettings and carcols IDs match. Test in FiveM before release.\n").map_err(|e|e.to_string())?;zip.finish().map_err(|e|e.to_string())?;Ok(JobResult::Saved(path.display().to_string()))});
        }
    }
    fn diff(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui|{if primary(ui,icon_text(icon::COMPARE,"Compare Files")).clicked(){match diff::compare(&self.diff_text[0],&self.diff_text[1]){Ok(rows)=>{let changes=rows.iter().filter(|r|r.status!="same").count();self.diff_rows=rows;self.message(Ok(format!("{changes} changed fields. Numeric comparisons preserve exact decimal precision.")));},Err(e)=>{self.diff_rows.clear();self.message(Err(e));}}}
if ui.button("Swap Before / After").clicked(){self.diff_text.swap(0,1);self.diff_paths.swap(0,1);self.diff_rows.clear();}
if ui.button("Export CSV").clicked()&&!self.diff_rows.is_empty(){self.save_bytes("handling-diff.csv","csv",diff::csv(&self.diff_rows).into_bytes())}
if ui.button("Export JSON").clicked()&&!self.diff_rows.is_empty(){self.save_bytes("handling-diff.json","json",serde_json::to_vec_pretty(&self.diff_rows).unwrap())}});
        ui.separator();
        ui.columns(2, |columns| {
            for (i, col) in columns.iter_mut().enumerate() {
                col.horizontal(|ui| {
                    ui.label(
                        RichText::new(if i == 0 { "BEFORE" } else { "AFTER" })
                            .small()
                            .strong()
                            .color(CAPTION),
                    );
                    if icon_button(ui, icon::OPEN, "Open handling.meta").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Handling", &["meta", "xml"])
                            .pick_file()
                    {
                        let result = (|| {
                            let bytes = duck_workshop::read_file(&path)?;
                            let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                            diff::records(&text)?;
                            self.diff_text[i] = text;
                            self.diff_paths[i] = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into();
                            self.diff_rows.clear();
                            Ok("Handling file loaded.".into())
                        })();
                        self.message(result);
                    }
                });
                col.label(RichText::new(&self.diff_paths[i]).small().color(MUTED));
                if col
                    .add(
                        egui::TextEdit::multiline(&mut self.diff_text[i])
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(7)
                            .desired_width(f32::INFINITY)
                            .hint_text("Paste handling.meta XML here"),
                    )
                    .changed()
                {
                    self.diff_rows.clear();
                }
            }
        });
        ui.separator();
        let changed = self.diff_rows.iter().filter(|r| r.status != "same").count();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{changed} changes · {} fields",
                    self.diff_rows.len()
                ))
                .strong(),
            );
            ui.checkbox(&mut self.diff_show_same, "Show identical");
            ui.label("Filter");
            ui.text_edit_singleline(&mut self.diff_filter);
        });
        let filter = self.diff_filter.to_lowercase();
        let visible: Vec<_> = self
            .diff_rows
            .iter()
            .filter(|r| {
                (self.diff_show_same || r.status != "same")
                    && format!("{} {}", r.vehicle, r.field)
                        .to_lowercase()
                        .contains(&filter)
            })
            .collect();
        let row_height = 28.;
        ui.horizontal(|ui| {
            ui.add_sized([110., 24.], egui::Label::new("Vehicle"));
            ui.add_sized([350., 24.], egui::Label::new("Field"));
            ui.add_sized([160., 24.], egui::Label::new("Before"));
            ui.add_sized([160., 24.], egui::Label::new("After"));
            ui.label("Status");
        });
        egui::ScrollArea::both().id_salt("diff-results").show_rows(
            ui,
            row_height,
            visible.len(),
            |ui, range| {
                for index in range {
                    let r = visible[index];
                    ui.horizontal(|ui| {
                        ui.add_sized([110., row_height], egui::Label::new(&r.vehicle).truncate());
                        ui.add_sized(
                            [350., row_height],
                            egui::Label::new(&r.field).truncate().sense(Sense::hover()),
                        )
                        .on_hover_text(&r.field);
                        ui.add_sized([160., row_height], egui::Label::new(&r.before).truncate())
                            .on_hover_text(&r.before);
                        ui.add_sized([160., row_height], egui::Label::new(&r.after).truncate())
                            .on_hover_text(&r.after);
                        ui.colored_label(
                            match r.status.as_str() {
                                "added" => SUCCESS,
                                "removed" => ERROR,
                                "changed" => ACCENT,
                                _ => MUTED,
                            },
                            &r.status,
                        );
                    });
                }
            },
        );
    }
}
impl eframe::App for Workbench {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        self.vehicle_resource_dialog(ctx);
        self.advance_sirens(ctx);
        for message in self.live.poll(ctx) {
            self.message(message);
        }
        self.shortcuts(ctx);
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(path) = dropped.into_iter().find_map(|f| f.path)
            && self.job.is_none()
        {
            if path.is_dir() {
                self.start_job(move || {
                    Ok(JobResult::VehicleCatalog(
                        duck_workshop::vehicle_resource::discover(&path)?,
                    ))
                });
            } else if path.extension().is_some_and(|ext| ext == "duckworkshop") {
                self.start_job(move || {
                    let project = Project::from_json(&duck_workshop::read_file(&path)?)?;
                    Ok(JobResult::Project(Box::new(project), path))
                });
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("yft"))
            {
                self.start_job(move || Self::load_model_file(&path));
            } else if self.tool == 0 {
                let ext = path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if duck_workshop::textures::EXTENSIONS.contains(&ext.as_str()) {
                    self.apply_texture(None, path);
                } else {
                    self.start_job(move || Self::load_model_file(&path));
                }
            }
        }
        self.chrome(ctx);
        if self.tool == 1 && !self.siren_was_lightboard {
            self.siren_popout = true;
        }
        self.siren_was_lightboard = self.tool == 1;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin {
                left: 14,
                right: 14,
                top: 8,
                bottom: 12,
            }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(TOOLS[self.tool].0).size(13.).color(TEXT));
                    ui.label(RichText::new(TOOLS[self.tool].1).small().color(CAPTION));
                });
                ui.add_space(4.);
                match self.tool {
                    0 => self.uv(ui),
                    1 => self.sequencer(ui),
                    2 => self.chevron(ui),
                    3 => self.meta(ui),
                    _ => self.diff(ui),
                }
            });
        self.overlays(ctx);
        self.updates_ui(ctx);
        self.siren_popout_ui(ctx);
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let (Some(gl), Ok(mut gpu)) = (gl, self.gpu.lock()) {
            gpu.destroy(gl);
        }
        if let (Some(gl), Ok(mut gpu)) = (gl, self.siren_gpu.lock()) {
            gpu.destroy(gl);
        }
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.updater.save(storage);
        let _ = self.apply_meta();
        if let Ok(json) = serde_json::to_string(&self.session) {
            storage.set_string("duck-workshop-v2", json)
        }
    }
}
fn primary(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    primary_button(ui, text.into())
}
fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.);
    ui.label(RichText::new(text).size(11.).strong().color(CAPTION));
    ui.add_space(4.);
}
fn save_dialog(name: &str, ext: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_file_name(name)
        .add_filter(ext, &[ext])
        .save_file()
}
fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect()
}
fn checkerboard(p: &egui::Painter, rect: Rect, cell: f32) {
    let p = p.with_clip_rect(rect);
    for y in 0..(rect.height() / cell).ceil() as i32 {
        for x in 0..(rect.width() / cell).ceil() as i32 {
            p.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(x as f32 * cell, y as f32 * cell),
                    Vec2::splat(cell),
                ),
                0.,
                if (x + y) % 2 == 0 {
                    Color32::from_gray(0x44)
                } else {
                    Color32::from_gray(0x3A)
                },
            );
        }
    }
}
