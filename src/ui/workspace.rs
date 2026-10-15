use super::*;

impl Workbench {
    pub(super) fn project(&mut self) -> Result<Project> {
        self.apply_meta()?;
        Ok(Project {
            format: "duck-workshop".into(),
            version: 1,
            session: self.session.clone(),
            model: self.model.clone(),
            tool: self.tool,
            dark_mode: self.dark_mode,
            comparison: self.diff_text.clone(),
            textures: self
                .live
                .assigned
                .iter()
                .map(|(m, p)| (m.clone(), p.clone()))
                .collect(),
        })
    }

    pub(super) fn open_project(&mut self) {
        if self.job.is_some() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Duck Workshop project", &["duckworkshop"])
            .pick_file()
        {
            self.start_job(move || {
                let bytes = duck_workshop::read_file(&path)?;
                Ok(JobResult::Project(
                    Box::new(Project::from_json(&bytes)?),
                    path,
                ))
            });
        }
    }

    pub(super) fn save_project(&mut self, save_as: bool) {
        if self.job.is_some() {
            return;
        }
        let project = match self.project() {
            Ok(p) => p,
            Err(e) => {
                self.message(Err(e));
                return;
            }
        };
        let path = if save_as || self.project_path.is_none() {
            save_dialog("Untitled.duckworkshop", "duckworkshop")
        } else {
            self.project_path.clone()
        };
        if let Some(path) = path {
            self.start_job(move || {
                let bytes = project.to_json()?;
                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                Ok(JobResult::ProjectSaved(path))
            });
        }
    }

    pub(super) fn restore_project(&mut self, project: Project, path: PathBuf) {
        let flip_v = project.session.uv.flip_v;
        self.session = project.session;
        self.model = project.model;
        self.tool = project.tool;
        self.dark_mode = project.dark_mode;
        self.diff_text = project.comparison;
        self.diff_rows.clear();
        self.project_path = Some(path);
        self.live.clear();
        if let Some(model) = self.model.take() {
            self.orient(&model);
            self.model = Some(model);
        }
        self.session.uv.flip_v = flip_v;
        self.camera = viewer::Camera::default();
        let ctx = self.ctx.clone();
        for (material, file) in project.textures {
            self.live.assign(&ctx, vec![material], file);
        }
        self.blink_play = false;
        self.blink_selected = 0;
        self.blink_brush = 0;
        self.blink_undo.clear();
        self.blink_redo.clear();
        self.blink_source_settings = self
            .session
            .pattern
            .source_xml
            .as_ref()
            .and_then(|s| Pattern::settings(s).ok())
            .unwrap_or_default();
        self.refresh_uv();
        self.refresh_meta();
        self.sync_decimal();
        self.uv_pan = Vec2::ZERO;
        self.uv_zoom = 1.;
        self.message(Ok("Project opened".into()));
    }

    fn reset_project(&mut self) {
        self.session = Session::default();
        self.model = None;
        self.live.clear();
        self.camera = viewer::Camera::default();
        self.tool = 0;
        self.project_path = None;
        self.blink_play = false;
        self.blink_step = 0;
        self.blink_selected = 0;
        self.blink_undo.clear();
        self.blink_redo.clear();
        self.blink_source_settings.clear();
        self.sequence_clipboard = None;
        self.diff_text = [String::new(), String::new()];
        self.diff_rows.clear();
        self.meta_index = 0;
        self.meta_raw = false;
        self.meta_kind = 0;
        self.refresh_uv();
        self.refresh_meta();
        self.sync_decimal();
        self.status.clear();
        self.error = false;
    }

    pub(super) fn undo_sequence(&mut self) {
        if let Some(previous) = self.blink_undo.pop() {
            self.blink_redo.push(self.session.pattern.clone());
            self.session.pattern = previous;
            self.sync_decimal();
        }
    }
    pub(super) fn redo_sequence(&mut self) {
        if let Some(next) = self.blink_redo.pop() {
            self.blink_undo.push(self.session.pattern.clone());
            self.session.pattern = next;
            self.sync_decimal();
        }
    }

    pub(super) fn shortcuts(&mut self, ctx: &egui::Context) {
        let key = |key, shift| {
            egui::KeyboardShortcut::new(
                egui::Modifiers {
                    ctrl: true,
                    shift,
                    ..Default::default()
                },
                key,
            )
        };
        if ctx.input_mut(|i| i.consume_shortcut(&key(egui::Key::S, true))) {
            self.save_project(true);
        } else if ctx.input_mut(|i| i.consume_shortcut(&key(egui::Key::S, false))) {
            self.save_project(false);
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&key(egui::Key::O, true))
                || i.consume_shortcut(&key(egui::Key::O, false))
        }) {
            self.open_project();
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&key(egui::Key::N, true))
                || i.consume_shortcut(&key(egui::Key::N, false))
        }) {
            self.confirm_new = true;
        }
        let tool_keys = [
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
        ];
        for (i, k) in tool_keys.into_iter().enumerate() {
            if ctx.input_mut(|input| input.consume_shortcut(&key(k, false))) {
                self.select_tool(i);
            }
        }
        if self.tool == 1 && !ctx.wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_shortcut(&key(egui::Key::Z, false))) {
                self.undo_sequence();
            }
            if ctx.input_mut(|i| i.consume_shortcut(&key(egui::Key::Y, false))) {
                self.redo_sequence();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.toggle_play(ctx);
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.show_help = true;
        }
    }

    pub(super) fn select_tool(&mut self, tool: usize) {
        self.tool = tool;
        self.blink_play = false;
    }

    /// Duck Studio layout: menu bar, tool list on the left and a status bar with actions.
    pub(super) fn chrome(&mut self, ctx: &egui::Context) {
        self.sync_title(ctx);
        egui::TopBottomPanel::top("menu-bar")
            .frame(
                egui::Frame::new()
                    .fill(MENU_BG)
                    .inner_margin(egui::Margin::symmetric(4, 2)),
            )
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
            });
        self.status_bar(ctx);
        if self.show_tools {
            egui::SidePanel::left("tools-pane")
                .resizable(true)
                .default_width(270.)
                .width_range(230.0..=420.)
                .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin {
                    left: 12,
                    right: 10,
                    top: 8,
                    bottom: 12,
                }))
                .show(ctx, |ui| self.tools_pane(ui));
        }
    }

    fn sync_title(&mut self, ctx: &egui::Context) {
        let name = self
            .project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "New project".into());
        let title = format!("Duck Workshop - {name}");
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        let busy = self.job.is_some();
        let item = |text: &str, shortcut: &str| egui::Button::new(text).shortcut_text(shortcut);
        ui.menu_button("Project", |ui| {
            if ui.add(item("New", "Ctrl+Shift+N")).clicked() {
                self.confirm_new = true;
            }
            if ui
                .add_enabled(!busy, item("Open...", "Ctrl+Shift+O"))
                .clicked()
            {
                self.open_project();
            }
            ui.separator();
            if ui.add_enabled(!busy, item("Save", "Ctrl+S")).clicked() {
                self.save_project(false);
            }
            if ui
                .add_enabled(!busy, item("Save as...", "Ctrl+Shift+S"))
                .clicked()
            {
                self.save_project(true);
            }
            ui.separator();
            if ui.button("Exit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.menu_button("Edit", |ui| {
            let lightboard = self.tool == 1;
            if ui
                .add_enabled(
                    lightboard && !self.blink_undo.is_empty(),
                    item("Undo", "Ctrl+Z"),
                )
                .clicked()
            {
                self.undo_sequence();
            }
            if ui
                .add_enabled(
                    lightboard && !self.blink_redo.is_empty(),
                    item("Redo", "Ctrl+Y"),
                )
                .clicked()
            {
                self.redo_sequence();
            }
            if !lightboard {
                ui.label(
                    RichText::new("Undo history is kept in Lightboard.")
                        .small()
                        .color(CAPTION),
                );
            }
        });
        ui.menu_button("Tools", |ui| {
            for (i, (name, _)) in TOOLS.iter().enumerate() {
                let button = egui::Button::new(icon_text(TOOL_ICONS[i], name))
                    .selected(self.tool == i)
                    .shortcut_text(format!("Ctrl+{}", i + 1));
                if ui.add(button).clicked() {
                    self.select_tool(i);
                }
            }
        });
        ui.menu_button("View", |ui| {
            ui.checkbox(&mut self.show_tools, "Tools pane");
            ui.separator();
            if ui.button("UV template").clicked() {
                self.select_tool(0);
                self.view = 0;
            }
            if ui.button("3D preview").clicked() {
                self.select_tool(0);
                self.view = 1;
            }
            if ui.button("Fit view").clicked() {
                self.fit_view();
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Check for updates").clicked() {
                self.updater.check(ui.ctx(), true);
            }
            if ui.button("Update settings...").clicked() {
                self.updater.show_settings = true;
            }
            ui.separator();
            if ui.add(item("Workshop guide", "F1")).clicked() {
                self.show_help = true;
            }
            if ui.button("About Duck Workshop").clicked() {
                self.show_about = true;
            }
        });
    }

    fn tools_pane(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::splat(32.)));
            ui.label(RichText::new("Duck Workshop").size(15.).color(TEXT));
        });
        ui.add_space(8.);
        ui.label(RichText::new("Tools").size(13.).color(TEXT));
        ui.add_space(1.);
        let mut pick = None;
        egui::Frame::new()
            .fill(LIST_BG)
            .stroke(Stroke::new(1.0_f32, LIST_LINE))
            .inner_margin(1.)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.;
                for (i, (name, about)) in TOOLS.iter().enumerate() {
                    let selected = self.tool == i;
                    let (rect, response) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 46.), Sense::click());
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::SelectableLabel,
                            true,
                            selected,
                            *name,
                        )
                    });
                    let p = ui.painter();
                    if selected {
                        p.rect_filled(rect, 0., ROW_SELECTED);
                    }
                    if response.hovered() || response.has_focus() {
                        p.rect_stroke(
                            rect,
                            0.,
                            Stroke::new(1.0_f32, ROW_HOVER),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let mut x = rect.left() + 12.;
                    if let Some(font) = icon_font(18.) {
                        p.text(
                            Pos2::new(x + 9., rect.center().y),
                            Align2::CENTER_CENTER,
                            TOOL_ICONS[i],
                            font,
                            if selected { Color32::WHITE } else { CAPTION },
                        );
                        x += 32.;
                    }
                    p.text(
                        Pos2::new(x, rect.center().y),
                        Align2::LEFT_BOTTOM,
                        *name,
                        egui::FontId::proportional(13.),
                        if selected { Color32::WHITE } else { TEXT },
                    );
                    p.text(
                        Pos2::new(x, rect.center().y + 1.),
                        Align2::LEFT_TOP,
                        *about,
                        egui::FontId::proportional(11.),
                        CAPTION,
                    );
                    if response.clicked() {
                        pick = Some(i);
                    }
                }
            });
        if let Some(i) = pick {
            self.select_tool(i);
        }
        ui.add_space(14.);
        ui.label(RichText::new("Project").size(13.).color(TEXT));
        ui.add_space(1.);
        let file = self
            .project_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Not saved yet".into());
        let model = self
            .model
            .as_ref()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "None".into());
        let metas = self
            .session
            .metas
            .iter()
            .filter(|m| !m.trim().is_empty())
            .count();
        let details = [
            ("File", file),
            ("Model", model),
            (
                "Lights",
                format!(
                    "{} lights ({} sirens)",
                    self.session.pattern.channels.len(),
                    self.session.pattern.siren_count()
                ),
            ),
            (
                "Textures",
                match self.live.files.len() {
                    0 => "None".into(),
                    1 => self.live.files[0].name(),
                    n => format!("{n} files"),
                },
            ),
            ("Metadata", format!("{metas} of 4 files")),
        ];
        egui::Frame::new()
            .fill(LIST_BG)
            .stroke(Stroke::new(1.0_f32, LIST_LINE))
            .inner_margin(10.)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::Grid::new("project-details")
                    .num_columns(2)
                    .spacing([14., 6.])
                    .show(ui, |ui| {
                        for (label, value) in details {
                            ui.label(RichText::new(label).small().color(CAPTION));
                            ui.add(egui::Label::new(value).truncate());
                            ui.end_row();
                        }
                    });
            });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.label(
                RichText::new("Ctrl+S saves the workspace, including imported geometry.")
                    .small()
                    .color(MUTED),
            );
        });
    }

    fn summary(&self) -> String {
        match self.tool {
            0 => match &self.model {
                Some(m) => format!(
                    "{}  /  {} meshes  /  {} triangles  /  {} UV edges",
                    m.name,
                    m.meshes.len(),
                    m.triangles(),
                    self.uv_edges.len()
                ),
                None => "No model loaded".into(),
            },
            1 => format!(
                "{} lights  /  {} sirens  /  32 steps  /  {} BPM",
                self.session.pattern.channels.len(),
                self.session.pattern.siren_count(),
                self.session.pattern.bpm
            ),
            2 => format!(
                "{} x {} px  /  {}",
                self.session.chevron.width,
                self.session.chevron.height,
                ["Down chevron", "Up chevron", "Diagonal"][self.session.chevron.direction]
            ),
            3 if self.session.metas[self.meta_kind].trim().is_empty() => {
                format!("{}.meta  /  empty", META_NAMES[self.meta_kind])
            }
            3 => format!(
                "{}.meta  /  {} entries  /  {} fields",
                META_NAMES[self.meta_kind],
                self.meta_entries.len(),
                self.meta_fields.len()
            ),
            _ if self.diff_rows.is_empty() => "No comparison yet".into(),
            _ => format!(
                "{} changes  /  {} fields",
                self.diff_rows.iter().filter(|r| r.status != "same").count(),
                self.diff_rows.len()
            ),
        }
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let busy = self.job.is_some();
        let recent = self.notice_time.is_some_and(|t| t.elapsed().as_secs() < 6);
        let notice = !self.status.is_empty() && (self.error || busy || recent);
        let (text, color) = if notice {
            (
                self.status.clone(),
                if self.error { ERROR } else { STATUS_TEXT },
            )
        } else {
            (self.summary(), STATUS_TEXT)
        };
        egui::TopBottomPanel::bottom("status-bar")
            .frame(
                egui::Frame::new()
                    .fill(STATUS_BG)
                    .inner_margin(egui::Margin::symmetric(9, 6)),
            )
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let size = Vec2::new(120., 26.);
                    let save =
                        egui::Button::new(icon_text(icon::SAVE, "Save project")).min_size(size);
                    if ui.add_enabled(!busy, save).clicked() {
                        self.save_project(false);
                    }
                    let open =
                        egui::Button::new(icon_text(icon::OPEN, "Open project")).min_size(size);
                    if ui.add_enabled(!busy, open).clicked() {
                        self.open_project();
                    }
                    let guide = egui::Button::new(icon_text(icon::HELP, "Guide"))
                        .min_size(Vec2::new(96., 26.));
                    if ui.add(guide).on_hover_text("F1").clicked() {
                        self.show_help = true;
                    }
                    ui.add_space(6.);
                    ui.label(
                        RichText::new("Editor settings kept between sessions")
                            .small()
                            .color(STATUS_TEXT),
                    );
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if busy {
                            ui.spinner();
                        }
                        ui.add(
                            egui::Label::new(RichText::new(text).small().color(color)).truncate(),
                        );
                        if notice && self.error && ui.small_button("Dismiss").clicked() {
                            self.status.clear();
                            self.error = false;
                        }
                    });
                });
            });
        if notice && !self.error {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
    }

    pub(super) fn overlays(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        if self.confirm_close {
            match dialog(
                ctx,
                "close-dialog",
                "Close Duck Workshop?",
                "Are you sure you want to close?",
                "Editor settings are kept. Imported models and texture links are only kept in saved projects.",
                &["Close", "Cancel"],
            ) {
                Some(0) => {
                    self.confirm_close = false;
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Some(_) => self.confirm_close = false,
                None => {}
            }
        }
        if self.confirm_new {
            match dialog(
                ctx,
                "new-dialog",
                "New project",
                "Start a new project?",
                "Save your current project first if you want to keep it.",
                &["Save Project", "Start New", "Cancel"],
            ) {
                Some(0) => self.save_project(false),
                Some(1) => {
                    self.reset_project();
                    self.confirm_new = false;
                }
                Some(_) => self.confirm_new = false,
                None => {}
            }
        }
        egui::Window::new("Workshop Guide").open(&mut self.show_help).default_width(520.).show(ctx, |ui| {
            ui.label(RichText::new("Duck Workshop").size(17.).color(TEXT)); ui.add_space(6.);
            ui.label("Project: Save all editor settings and the imported model in a .duckworkshop file. Ctrl+O opens a project; Ctrl+S saves it. Ctrl+1 to Ctrl+5 switch tools.");
            ui.separator();
            for (name, text) in [
                ("UV Editor", "Import YFT, YDR, XML, OBJ or GLB, then Load Texture (PNG, JPG, PSD, DDS, TGA, BMP, TIFF, WebP) to see it on the model in 3D. Drag to rotate, right-drag or Shift+drag to move, scroll to zoom, double-click to fit. Textures reload automatically each time the file is saved, so you can keep painting in Photoshop. Choose a different texture per material in the Textures list. The UV Template tab shows the texture under the UV lines and exports PNG or SVG templates."),
                ("Lightboard", "Select a light and paint the 32-step grid with its highlighted color. Add colors to a light with + to make it flash in several colors; each step shows one color. Drag to draw a continuous stroke; right-click to erase. Space plays or pauses. Each color of a light is exported as its own carcols siren."),
                ("Stripe Designer", "Adjust stripe colors and dimensions. Use the current UV selection as a mask when a model is loaded."),
                ("Meta Editor", "Import a .meta file to edit it, or enter your own vehicle name under Create Vehicle Metadata. Nested fields remain editable in the XML editor."),
                ("Handling Compare", "Open or paste two handling files, compare, filter, then export the differences."),
            ] { ui.label(RichText::new(name).color(ACCENT)); ui.label(text); ui.add_space(6.); }
        });
        egui::Window::new("About Duck Workshop")
            .open(&mut self.show_about)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::splat(96.)));
                ui.label(RichText::new("Duck Workshop").size(17.).color(TEXT));
                ui.label(
                    RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(CAPTION),
                );
                ui.add_space(6.);
                ui.label("Native vehicle development tools. MIT licensed.");
                ui.label(
                    "Project source and third-party license notices are included with this app.",
                );
            });
    }
}

const STATUS_TEXT: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xD0);

/// Modal confirmation in the style of Duck Studio's dialogs. The last button cancels.
pub(super) fn dialog(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    heading: &str,
    body: &str,
    buttons: &[&str],
) -> Option<usize> {
    let mut choice = None;
    let modal = egui::Modal::new(egui::Id::new(id))
        .frame(egui::Frame::window(&ctx.style()).inner_margin(0.))
        .show(ctx, |ui| {
            ui.set_width(460.);
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(14, 9))
                .show(ui, |ui| {
                    ui.label(RichText::new(title).size(12.).color(TEXT));
                });
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 20,
                    right: 18,
                    top: 4,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    ui.label(RichText::new(heading).size(17.).color(TEXT));
                    ui.add_space(2.);
                    ui.label(RichText::new(body).color(STATUS_TEXT));
                    ui.add_space(16.);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for (i, label) in buttons.iter().enumerate().rev() {
                            let button = egui::Button::new(*label).min_size(Vec2::new(72., 26.));
                            if ui.add(button).clicked() {
                                choice = Some(i);
                            }
                        }
                    });
                });
        });
    if choice.is_none() && modal.should_close() {
        choice = Some(buttons.len() - 1);
    }
    choice
}
