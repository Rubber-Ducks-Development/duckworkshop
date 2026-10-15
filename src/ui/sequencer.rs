use super::*;

const GRID_BG: Color32 = VIEWPORT;
const PANEL_BG: Color32 = Color32::from_rgb(0x4A, 0x4C, 0x52);
const LIGHT_TEXT: Color32 = TEXT;
const DIM_TEXT: Color32 = CAPTION;
const CELL_OFF: [Color32; 2] = [
    Color32::from_rgb(0x33, 0x33, 0x35),
    Color32::from_rgb(0x3B, 0x3B, 0x3D),
];

impl Workbench {
    pub(super) fn toggle_play(&mut self, ctx: &egui::Context) {
        self.blink_play = !self.blink_play;
        self.blink_start =
            ctx.input(|i| i.time) - self.blink_step as f64 * 60. / self.session.pattern.bpm as f64;
    }

    pub(super) fn import_sequence(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Light pattern / carcols", &["json", "meta", "xml"])
            .pick_file()
        else {
            return;
        };
        let result = (|| {
            let text =
                String::from_utf8(duck_workshop::read_file(&path)?).map_err(|e| e.to_string())?;
            let pattern = if path.extension().is_some_and(|e| e == "json") {
                serde_json::from_str::<Pattern>(&text).map_err(|e| e.to_string())?
            } else {
                Pattern::from_xml(&text, 0)?
            };
            pattern.validate()?;
            self.blink_snapshot();
            self.session.pattern = pattern;
            self.blink_source_settings = self
                .session
                .pattern
                .source_xml
                .as_ref()
                .and_then(|x| Pattern::settings(x).ok())
                .unwrap_or_default();
            self.blink_play = false;
            self.blink_step = 0;
            self.blink_selected = 0;
            self.sync_decimal();
            Ok("Light pattern imported".into())
        })();
        self.message(result);
    }

    fn export_sequence(&mut self, kind: usize) {
        match kind {
            0 => match self.session.pattern.to_xml() {
                Ok(xml) => {
                    self.session.metas[3] = xml.clone();
                    if self.meta_kind == 3 {
                        self.refresh_meta();
                    }
                    self.save_bytes("carcols.meta", "meta", xml.into_bytes());
                }
                Err(e) => self.message(Err(e)),
            },
            1 => {
                let text = self
                    .session
                    .pattern
                    .sirens()
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        format!(
                            "siren{}\t{}\t{}",
                            i + 1,
                            c.bits
                                .iter()
                                .map(|b| if *b { '1' } else { '0' })
                                .collect::<String>(),
                            patterns::decimal(&c.bits)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.save_bytes("sequencers.txt", "txt", text.into_bytes());
            }
            _ => match serde_json::to_vec_pretty(&self.session.pattern) {
                Ok(bytes) => self.save_bytes("light-pattern.json", "json", bytes),
                Err(e) => self.message(Err(e.to_string())),
            },
        }
    }

    fn sequence_operation(&mut self, operation: usize) {
        self.blink_snapshot();
        let brush = self.blink_brush;
        let channel = &mut self.session.pattern.channels[self.blink_selected];
        match operation {
            0 => {
                let steps = channel
                    .steps()
                    .map(|s| if s.is_some() { None } else { Some(brush) });
                channel.set_steps(&steps);
            }
            1 => channel.rotate(true),
            2 => channel.rotate(false),
            _ => channel.set_steps(&[None; 32]),
        }
        self.sync_decimal();
    }

    pub(super) fn sequencer(&mut self, ui: &mut egui::Ui) {
        ui.set_style(sequence_style(ui));
        if self.blink_play {
            self.blink_step = (((ui.input(|i| i.time) - self.blink_start)
                * self.session.pattern.bpm as f64
                / 60.)
                .floor() as usize)
                % 32;
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
        ui.horizontal(|ui| {
            if primary(
                ui,
                if self.blink_play {
                    icon_text(icon::PAUSE, "Pause")
                } else {
                    icon_text(icon::PLAY, "Play")
                },
            )
            .on_hover_text("Space")
            .clicked()
            {
                self.toggle_play(ui.ctx());
            }
            if icon_button(ui, icon::STOP, "Stop").clicked() {
                self.blink_play = false;
                self.blink_step = 0;
            }
            ui.separator();
            if icon_button(ui, icon::IMPORT, "Import Pattern…").clicked() {
                self.import_sequence();
            }
            ui.menu_button(icon_text(icon::EXPORT, "Export"), |ui| {
                for (kind, label) in [
                    "carcols.meta",
                    "Binary + Decimal Sequencers",
                    "Pattern JSON",
                ]
                .iter()
                .enumerate()
                {
                    if ui.button(*label).clicked() {
                        ui.close();
                        self.export_sequence(kind);
                    }
                }
            });
            ui.separator();
            if ui
                .add_enabled(
                    !self.blink_undo.is_empty(),
                    egui::Button::new(icon_text(icon::UNDO, "Undo")),
                )
                .clicked()
            {
                self.undo_sequence();
            }
            if ui
                .add_enabled(
                    !self.blink_redo.is_empty(),
                    egui::Button::new(icon_text(icon::REDO, "Redo")),
                )
                .clicked()
            {
                self.redo_sequence();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{:02} / 32", self.blink_step + 1))
                        .monospace()
                        .color(ACCENT),
                );
                ui.label(RichText::new("STEP").small().color(DIM_TEXT));
            });
        });
        ui.add_space(12.);
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(245., height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(245.);
                    ui.set_max_width(245.);
                    egui::ScrollArea::vertical()
                        .id_salt("sequence-inspector")
                        .show(ui, |ui| self.sequence_inspector(ui));
                },
            );
            ui.add_space(8.);
            let width = ui.available_width();
            ui.allocate_ui_with_layout(
                Vec2::new(width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(width);
                    ui.set_max_width(width);
                    self.light_preview(ui);
                    ui.add_space(14.);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("PATTERN").strong().small().color(DIM_TEXT));
                        ui.label(
                            RichText::new(format!(
                                "{} lights · 32 steps",
                                self.session.pattern.channels.len()
                            ))
                            .small()
                            .color(DIM_TEXT),
                        );
                    });
                    ui.add_space(4.);
                    self.sequence_grid(ui);
                },
            );
        });
    }

    fn sequence_inspector(&mut self, ui: &mut egui::Ui) {
        let before = self.session.pattern.clone();
        let mut changed = false;
        dark_section(ui, "SEQUENCE");
        changed |= ui
            .add_sized(
                [ui.available_width(), 28.],
                egui::TextEdit::singleline(&mut self.session.pattern.name)
                    .hint_text("Sequence name"),
            )
            .changed();
        ui.add_space(6.);
        egui::Grid::new("sequence-settings")
            .num_columns(2)
            .spacing([14., 10.])
            .show(ui, |ui| {
                ui.label("Siren ID");
                changed |= ui
                    .add(egui::DragValue::new(&mut self.session.pattern.id).range(1..=65535))
                    .changed();
                ui.end_row();
                ui.label("BPM");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut self.session.pattern.bpm)
                            .range(1.0..=5000.0)
                            .speed(5.),
                    )
                    .changed();
                ui.end_row();
            });
        if self.blink_source_settings.len() > 1 {
            let previous = self.session.pattern.source_index;
            egui::ComboBox::from_id_salt("carcols-setting")
                .width(230.)
                .selected_text(
                    self.blink_source_settings
                        .get(previous)
                        .map(String::as_str)
                        .unwrap_or("Siren setting"),
                )
                .show_ui(ui, |ui| {
                    for (i, name) in self.blink_source_settings.iter().enumerate() {
                        ui.selectable_value(&mut self.session.pattern.source_index, i, name);
                    }
                });
            if previous != self.session.pattern.source_index {
                let source = self.session.pattern.source_xml.clone().unwrap();
                match Pattern::from_xml(&source, self.session.pattern.source_index) {
                    Ok(pattern) => {
                        self.blink_undo.push(before.clone());
                        self.blink_redo.clear();
                        self.session.pattern = pattern;
                        self.blink_selected = 0;
                        self.sync_decimal();
                    }
                    Err(error) => {
                        self.session.pattern.source_index = previous;
                        self.message(Err(error));
                    }
                }
            }
        }
        ui.add_space(10.);
        ui.separator();
        dark_section(ui, &format!("LIGHT {:02}", self.blink_selected + 1));
        let selected = self.blink_selected;
        changed |= ui
            .add_sized(
                [ui.available_width(), 28.],
                egui::TextEdit::singleline(&mut self.session.pattern.channels[selected].name),
            )
            .changed();
        ui.add_space(6.);
        changed |= self.light_colors(ui);
        ui.add_space(4.);
        egui::Grid::new("selected-light-settings")
            .num_columns(2)
            .spacing([14., 10.])
            .show(ui, |ui| {
                ui.label("Intensity");
                changed |= ui
                    .add(
                        egui::DragValue::new(
                            &mut self.session.pattern.channels[selected].intensity,
                        )
                        .range(0.0..=1000.0)
                        .speed(0.1),
                    )
                    .changed();
                ui.end_row();
                ui.label("Direction");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut self.session.pattern.channels[selected].delta)
                            .range(-std::f32::consts::TAU..=std::f32::consts::TAU)
                            .speed(0.01)
                            .suffix(" rad"),
                    )
                    .changed();
                ui.end_row();
            });
        if changed {
            self.blink_undo.push(before);
            self.blink_redo.clear();
            self.sync_decimal();
        }
        ui.add_space(10.);
        ui.separator();
        dark_section(ui, "EDIT PATTERN");
        ui.horizontal(|ui| {
            if ui.button("Invert").clicked() {
                self.sequence_operation(0);
            }
            if ui.button("Clear").clicked() {
                self.sequence_operation(3);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Shift Left").clicked() {
                self.sequence_operation(1);
            }
            if ui.button("Shift Right").clicked() {
                self.sequence_operation(2);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Copy").clicked() {
                self.sequence_clipboard = Some(self.session.pattern.channels[selected].clone());
            }
            if ui
                .add_enabled(
                    self.sequence_clipboard.is_some(),
                    egui::Button::new("Paste"),
                )
                .on_hover_text("Pastes the copied colors and steps")
                .clicked()
            {
                let copied = self.sequence_clipboard.clone().unwrap();
                let current = &self.session.pattern.channels[selected];
                let total = self.session.pattern.siren_count() - current.color_count()
                    + copied.color_count();
                if total > patterns::MAX_SIRENS {
                    self.message(Err(format!(
                        "That would use {total} sirens; carcols allows {}.",
                        patterns::MAX_SIRENS
                    )));
                } else {
                    self.blink_snapshot();
                    let channel = &mut self.session.pattern.channels[selected];
                    channel.color = copied.color;
                    channel.bits = copied.bits;
                    channel.extra = copied.extra;
                    self.sync_decimal();
                }
            }
        });
        ui.add_space(10.);
        ui.separator();
        dark_section(ui, "LIGHTS");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.session.pattern.siren_count() < patterns::MAX_SIRENS,
                    egui::Button::new(icon_text(icon::ADD, "Add Light")),
                )
                .clicked()
            {
                self.blink_snapshot();
                let i = self.session.pattern.channels.len();
                self.session.pattern.channels.push(patterns::Channel::new(
                    format!("Light {:02}", i + 1),
                    self.palette_color,
                ));
                self.blink_selected = i;
                self.blink_brush = 0;
                self.sync_decimal();
            }
            if ui
                .add_enabled(
                    self.session.pattern.channels.len() > 1,
                    egui::Button::new(icon_text(icon::REMOVE, "Remove")),
                )
                .clicked()
            {
                self.blink_snapshot();
                self.session.pattern.channels.remove(selected);
                self.sync_decimal();
            }
        });
        if ui
            .add_enabled(selected > 0, egui::Button::new("Merge Into Light Above"))
            .on_hover_text("Adds this light's colors and steps to the light above it")
            .clicked()
        {
            let before = self.session.pattern.clone();
            match self.session.pattern.merge_up(selected) {
                Ok(conflicts) => {
                    self.blink_undo.push(before);
                    self.blink_redo.clear();
                    self.blink_selected = selected - 1;
                    self.sync_decimal();
                    self.message(Ok(if conflicts == 0 {
                        "Lights merged.".into()
                    } else {
                        format!("Lights merged. {conflicts} overlapping steps kept the color of the light above.")
                    }));
                }
                Err(e) => self.message(Err(e)),
            }
        }
        ui.add_space(10.);
        ui.separator();
        if ui
            .checkbox(&mut self.show_values, "Sequence Values")
            .changed()
        {
            self.sync_decimal();
        }
        if self.show_values {
            ui.label(
                RichText::new("Unsigned decimal per color")
                    .small()
                    .color(DIM_TEXT),
            );
            let mut apply = None;
            for i in 0..self.blink_decimal.len() {
                let color = self.session.pattern.channels[selected].color_at(i);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.), Sense::hover());
                    ui.painter().rect_filled(rect, 2., rgb(color));
                    ui.add_sized(
                        [130., 26.],
                        egui::TextEdit::singleline(&mut self.blink_decimal[i])
                            .font(egui::TextStyle::Monospace),
                    );
                    if ui.button("Apply").clicked() {
                        apply = Some(i);
                    }
                });
                let binary = self.session.pattern.channels[selected]
                    .bits_at(i)
                    .iter()
                    .map(|b| if *b { '1' } else { '0' })
                    .collect::<String>();
                ui.label(RichText::new(binary).monospace().small().color(DIM_TEXT));
            }
            if let Some(i) = apply {
                match self.blink_decimal[i].trim().parse::<u32>() {
                    Ok(n) => {
                        self.blink_snapshot();
                        self.session.pattern.channels[selected]
                            .set_color_bits(i, patterns::bits(n));
                        self.sync_decimal();
                    }
                    Err(_) => self.message(Err("Enter an integer from 0 to 4294967295.".into())),
                }
            }
        }
    }

    /// Color chips of the selected light. The highlighted chip is the paint color.
    fn light_colors(&mut self, ui: &mut egui::Ui) -> bool {
        let selected = self.blink_selected;
        let mut changed = false;
        let count = self.session.pattern.channels[selected].color_count();
        ui.label(RichText::new("Colors").small().color(DIM_TEXT));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 5.;
            for i in 0..count {
                let color = self.session.pattern.channels[selected].color_at(i);
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(30.), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::Button,
                        true,
                        self.blink_brush == i,
                        format!("Paint with color {}", i + 1),
                    )
                });
                ui.painter().rect_filled(rect.shrink(3.), 3., rgb(color));
                if self.blink_brush == i {
                    ui.painter().rect_stroke(
                        rect,
                        4.,
                        Stroke::new(2.0_f32, Color32::WHITE),
                        egui::StrokeKind::Inside,
                    );
                }
                if response.clicked() {
                    self.blink_brush = i;
                }
                response.on_hover_text(format!("Color {} · click to paint with it", i + 1));
            }
            let room = count < patterns::MAX_COLORS
                && self.session.pattern.siren_count() < patterns::MAX_SIRENS;
            let add = ui
                .add_enabled(
                    room,
                    egui::Button::new(icon_text(icon::ADD, "")).min_size(Vec2::splat(30.)),
                )
                .on_hover_text("Add another color to this light")
                .on_disabled_hover_text(format!(
                    "Up to {} colors per light and {} sirens in total",
                    patterns::MAX_COLORS,
                    patterns::MAX_SIRENS
                ));
            if add.clicked() {
                let used: Vec<_> = (0..count)
                    .map(|i| self.session.pattern.channels[selected].color_at(i))
                    .collect();
                // The first palette color clearly different from the light's colors.
                let distance = |a: [u8; 3], b: [u8; 3]| {
                    (0..3)
                        .map(|i| (a[i] as i32 - b[i] as i32).pow(2))
                        .sum::<i32>()
                };
                let next = PALETTE
                    .into_iter()
                    .find(|c| used.iter().all(|u| distance(*c, *u) > 100 * 100))
                    .unwrap_or(PALETTE[0]);
                self.session.pattern.channels[selected].add_color(next);
                self.blink_brush = count;
                changed = true;
            }
        });
        ui.add_space(2.);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.;
            for color in PALETTE {
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        format!("Color #{:02X}{:02X}{:02X}", color[0], color[1], color[2]),
                    )
                });
                ui.painter().rect_filled(rect.shrink(3.), 3., rgb(color));
                if response.hovered() {
                    ui.painter().rect_stroke(
                        rect,
                        3.,
                        Stroke::new(1.0_f32, ROW_HOVER),
                        egui::StrokeKind::Inside,
                    );
                }
                if response.clicked() {
                    self.palette_color = color;
                    self.session.pattern.channels[selected].set_color(self.blink_brush, color);
                    changed = true;
                }
            }
        });
        ui.horizontal(|ui| {
            let mut color = self.session.pattern.channels[selected].color_at(self.blink_brush);
            if ui.color_edit_button_srgb(&mut color).changed() {
                self.session.pattern.channels[selected].set_color(self.blink_brush, color);
                changed = true;
            }
            ui.label(RichText::new(format!("Color {}", self.blink_brush + 1)).color(DIM_TEXT));
            if count > 1 && ui.small_button("Remove color").clicked() {
                self.session.pattern.channels[selected].remove_color(self.blink_brush);
                self.blink_brush = self.blink_brush.saturating_sub(1);
                changed = true;
            }
        });
        if count > 1 {
            let first = self.session.pattern.channels[..selected]
                .iter()
                .map(|c| c.color_count())
                .sum::<usize>()
                + 1;
            ui.label(
                RichText::new(format!(
                    "Each step shows one color. carcols has one color per siren, so this light exports as sirens {first}–{}. Give each of them a siren bone in the same spot on the model.",
                    first + count - 1
                ))
                .small()
                .color(DIM_TEXT),
            );
        }
        changed
    }

    fn light_preview(&mut self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(Vec2::new(ui.available_width(), 140.), Sense::hover());
        let rect = response.rect;
        painter.rect_filled(rect, 6., GRID_BG);
        painter.text(
            rect.left_top() + Vec2::new(16., 13.),
            Align2::LEFT_TOP,
            "LIGHT PREVIEW",
            egui::FontId::proportional(11.),
            DIM_TEXT,
        );
        let count = self.session.pattern.channels.len();
        let gap = 6.;
        let width = ((rect.width() - 44. - gap * (count - 1) as f32) / count as f32).min(80.);
        let origin = rect.center().x - (count as f32 * width + (count - 1) as f32 * gap) / 2.;
        let mut select = None;
        for (i, channel) in self.session.pattern.channels.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(origin + i as f32 * (width + gap), rect.top() + 52.),
                Vec2::new(width, 32.),
            );
            let lit = channel.step(self.blink_step).map(|i| channel.color_at(i));
            let on = lit.is_some();
            let color = rgb(lit.unwrap_or(channel.color));
            if let Some(c) = lit {
                for spread in (1..=4).rev() {
                    painter.rect_filled(
                        r.expand(spread as f32 * 3.),
                        5.,
                        Color32::from_rgba_unmultiplied(c[0], c[1], c[2], (14 / spread) as u8),
                    );
                }
            }
            painter.rect_filled(
                r,
                3.,
                if on {
                    color
                } else {
                    Color32::from_rgb(0x3A, 0x3A, 0x3C)
                },
            );
            if on {
                painter.rect_filled(r.shrink(5.), 1., Color32::from_white_alpha(150));
            }
            if i == self.blink_selected {
                painter.rect_stroke(
                    r.expand(3.),
                    5.,
                    Stroke::new(1.0_f32, ACCENT),
                    egui::StrokeKind::Outside,
                );
            }
            painter.text(
                r.center_bottom() + Vec2::new(0., 12.),
                Align2::CENTER_TOP,
                format!("{:02}", i + 1),
                egui::FontId::monospace(11.),
                DIM_TEXT,
            );
            let response = ui.interact(r, ui.id().with(("preview-light", i)), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    format!("Select {}", channel.name),
                )
            });
            if response.clicked() {
                select = Some(i)
            }
        }
        if let Some(i) = select {
            self.blink_selected = i;
            self.sync_decimal();
        }
    }

    fn sequence_grid(&mut self, ui: &mut egui::Ui) {
        let cell = ((ui.available_width() - 116.) / 32.).clamp(17., 38.);
        let gap = 6.;
        let left = 72.;
        let total = left + cell * 32. + gap * 3.;
        let mut selection = None;
        let mut paint = None;
        let down = ui.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
        let pressed = ui.input(|i| i.pointer.primary_pressed() || i.pointer.secondary_pressed());
        if pressed {
            self.paint_value = None;
            self.paint_started = false;
            self.paint_previous = None;
        }
        egui::Frame::new()
            .fill(GRID_BG)
            .corner_radius(6.)
            .inner_margin(10.)
            .show(ui, |ui| {
                let height = ui.available_height().max(100.);
                egui::ScrollArea::both()
                    .id_salt("sequence-grid")
                    .max_height(height)
                    .show(ui, |ui| {
                        let (header, _) =
                            ui.allocate_exact_size(Vec2::new(total, 28.), Sense::hover());
                        for step in 0..32 {
                            let x =
                                header.left() + left + step as f32 * cell + (step / 8) as f32 * gap;
                            let r = Rect::from_min_size(
                                Pos2::new(x, header.top()),
                                Vec2::new(cell, 28.),
                            );
                            if step == self.blink_step {
                                ui.painter().rect_filled(
                                    r.shrink2(Vec2::new(2., 3.)),
                                    3.,
                                    Color32::from_rgb(0x55, 0x4C, 0x36),
                                );
                            }
                            ui.painter().text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                format!("{:02}", step + 1),
                                egui::FontId::monospace(11.),
                                if step == self.blink_step {
                                    ACCENT
                                } else {
                                    DIM_TEXT
                                },
                            );
                            let response =
                                ui.interact(r, ui.id().with(("step", step)), Sense::click());
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!("Preview step {}", step + 1),
                                )
                            });
                            if response.clicked() {
                                self.blink_step = step;
                                self.blink_play = false;
                            }
                        }
                        for (i, channel) in self.session.pattern.channels.iter().enumerate() {
                            let (row, _) =
                                ui.allocate_exact_size(Vec2::new(total, 31.), Sense::hover());
                            if i == self.blink_selected {
                                ui.painter().rect_filled(row, 3., PANEL_BG);
                            }
                            let label = Rect::from_min_size(row.min, Vec2::new(left - 5., 31.));
                            let dots = channel.color_count().min(3);
                            for d in 0..dots {
                                ui.painter().circle_filled(
                                    label.left_center()
                                        + Vec2::new(10., (d as f32 - (dots - 1) as f32 / 2.) * 7.),
                                    3.,
                                    rgb(channel.color_at(d)),
                                );
                            }
                            ui.painter().text(
                                label.center() + Vec2::new(5., 0.),
                                Align2::CENTER_CENTER,
                                format!("{:02}", i + 1),
                                egui::FontId::monospace(12.),
                                if i == self.blink_selected {
                                    ACCENT
                                } else {
                                    LIGHT_TEXT
                                },
                            );
                            let response = ui
                                .interact(label, ui.id().with(("light-row", i)), Sense::click())
                                .on_hover_text(&channel.name);
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!("Select {}", channel.name),
                                )
                            });
                            if response.clicked() {
                                selection = Some(i)
                            }
                            for step in 0..32 {
                                let x = row.left()
                                    + left
                                    + step as f32 * cell
                                    + (step / 8) as f32 * gap;
                                let r = Rect::from_min_size(
                                    Pos2::new(x, row.top() + 3.),
                                    Vec2::new(cell - 3., 25.),
                                );
                                let lit = channel.step(step);
                                let active = lit.is_some();
                                let color = match lit {
                                    Some(c) => rgb(channel.color_at(c)),
                                    None => CELL_OFF[(step / 4) % 2],
                                };
                                ui.painter().rect_filled(r, 2., color);
                                if step == self.blink_step {
                                    ui.painter().rect_stroke(
                                        r,
                                        2.,
                                        Stroke::new(
                                            1.0_f32,
                                            if active {
                                                Color32::WHITE
                                            } else {
                                                Color32::from_rgb(0x8A, 0x7A, 0x50)
                                            },
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                let response = ui.interact(
                                    r,
                                    ui.id().with(("sequence-cell", i, step)),
                                    Sense::click_and_drag(),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::WidgetType::Checkbox,
                                        true,
                                        active,
                                        format!("{} step {}", channel.name, step + 1),
                                    )
                                });
                                let inside = ui
                                    .input(|input| input.pointer.interact_pos())
                                    .is_some_and(|p| r.contains(p) && ui.clip_rect().contains(p));
                                // The light's paint color; lights with fewer colors use their last.
                                let brush = if i == self.blink_selected {
                                    self.blink_brush
                                } else {
                                    0
                                }
                                .min(channel.color_count() - 1);
                                if inside && pressed {
                                    self.paint_value = Some(
                                        if ui.input(|i| i.pointer.secondary_down())
                                            || lit == Some(brush)
                                        {
                                            None
                                        } else {
                                            Some(brush)
                                        },
                                    );
                                    self.paint_started = false;
                                }
                                if inside
                                    && down
                                    && let Some(value) = self.paint_value
                                {
                                    paint = Some((i, step, value));
                                }
                                if (response.clicked() || response.secondary_clicked()) && !down {
                                    let value = self.paint_value.unwrap_or_else(|| {
                                        if response.secondary_clicked() || lit == Some(brush) {
                                            None
                                        } else {
                                            Some(brush)
                                        }
                                    });
                                    paint = Some((i, step, value));
                                }
                            }
                        }
                    });
            });
        if let Some(i) = selection {
            self.blink_selected = i;
            self.sync_decimal();
        }
        if let Some((i, step, value)) = paint {
            // Fill skipped cells when fast pointer movement spans multiple steps.
            let previous = self
                .paint_previous
                .filter(|(row, _)| *row == i)
                .map_or(step, |(_, previous)| previous);
            let range = previous.min(step)..=previous.max(step);
            if range
                .clone()
                .any(|s| self.session.pattern.channels[i].step(s) != value)
            {
                if !self.paint_started {
                    self.blink_snapshot();
                    self.paint_started = true;
                }
                for s in range {
                    self.session.pattern.channels[i].set_step(s, value);
                }
                if self.blink_selected != i {
                    self.blink_selected = i;
                    self.blink_brush = value.unwrap_or(0);
                }
                self.sync_decimal();
            }
            self.paint_previous = Some((i, step));
        }
        if !down {
            self.paint_value = None;
            self.paint_started = false;
            self.paint_previous = None;
        }
    }
}

const PALETTE: [[u8; 3]; 8] = [
    [255, 45, 55],
    [45, 125, 255],
    [255, 174, 30],
    [245, 247, 255],
    [35, 215, 135],
    [170, 95, 230],
    [25, 205, 225],
    [255, 230, 45],
];
fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
fn dark_section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.);
    ui.label(RichText::new(text).strong().small().color(DIM_TEXT));
    ui.add_space(6.);
}
fn sequence_style(ui: &egui::Ui) -> egui::Style {
    let mut style = (**ui.style()).clone();
    style.spacing.item_spacing = Vec2::new(8., 7.);
    style.spacing.button_padding = Vec2::new(10., 5.);
    style
}
