use super::*;

impl Workbench {
    pub(super) fn chevron(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if primary(ui, icon_text(icon::EXPORT, "Export PNG")).clicked() && self.job.is_none() {
                let c = self.session.chevron.clone();
                let model = self.model.clone();
                let uv = self.session.uv.clone();
                if let Some(path) = save_dialog("chevron.png", "png") {
                    self.start_job(move || {
                        std::fs::write(&path, c.png(model.as_ref(), &uv)?)
                            .map_err(|e| e.to_string())?;
                        Ok(JobResult::Saved(path.display().to_string()))
                    });
                }
            }
            if icon_button(ui, icon::SAVE, "Save Settings").clicked() {
                match serde_json::to_vec_pretty(&self.session.chevron) {
                    Ok(bytes) => self.save_bytes("chevron-project.json", "json", bytes),
                    Err(e) => self.message(Err(e.to_string())),
                }
            }
            if icon_button(ui, icon::IMPORT, "Import Settings").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Stripe settings", &["json"])
                    .pick_file()
            {
                let result = (|| {
                    let c: Chevron = serde_json::from_slice(&duck_workshop::read_file(&path)?)
                        .map_err(|e| e.to_string())?;
                    c.validate()?;
                    self.session.chevron = c;
                    self.chevron_dirty = true;
                    Ok("Stripe settings imported".into())
                })();
                self.message(result);
            }
        });
        ui.separator();
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            let width=(ui.available_width()-280.).max(200.);
            ui.allocate_ui_with_layout(Vec2::new(width,height),egui::Layout::top_down(egui::Align::Min),|ui| {
                if self.chevron_dirty {
                    match self.session.chevron.image(true,self.model.as_ref(),&self.session.uv) {
                        Ok(image)=>self.chevron_texture=Some(ui.ctx().load_texture("stripe-preview",egui::ColorImage::from_rgba_unmultiplied([image.width()as usize,image.height()as usize],image.as_raw()),egui::TextureOptions::LINEAR)),
                        Err(e)=>{self.chevron_texture=None;self.message(Err(e));}
                    }
                    self.chevron_dirty=false;
                }
                let (rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),(height-62.).max(100.)),Sense::hover());
                checkerboard(ui.painter(),rect,24.);
                if let Some(texture)=&self.chevron_texture {
                    let ratio=self.session.chevron.width as f32/self.session.chevron.height as f32;
                    let w=rect.width().min(rect.height()*ratio)*0.94;
                    let image_rect=Rect::from_center_size(rect.center(),Vec2::new(w,w/ratio));
                    ui.painter().image(texture.id(),image_rect,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::WHITE);
                }
                ui.label(RichText::new("Honeycomb and reflective finish are included in PNG exports. The seam line is a preview guide.").small().color(MUTED));
            });
            ui.separator();
            ui.allocate_ui_with_layout(Vec2::new(255.,height),egui::Layout::top_down(egui::Align::Min),|ui| {
                egui::ScrollArea::vertical().id_salt("stripe-inspector").show(ui,|ui|self.stripe_inspector(ui));
            });
        });
    }
    fn stripe_inspector(&mut self, ui: &mut egui::Ui) {
        let before = serde_json::to_string(&self.session.chevron).unwrap();
        if self.stripe_presets.is_empty() {
            for i in 0..render::CHEVRON_PRESETS.len() {
                let mut c = Chevron {
                    width: 256,
                    height: 64,
                    ..Default::default()
                };
                c.apply_preset(i);
                c.stripe = 20.;
                if let Ok(image) = c.image(false, None, &self.session.uv) {
                    self.stripe_presets.push(ui.ctx().load_texture(
                        format!("stripe-preset-{i}"),
                        egui::ColorImage::from_rgba_unmultiplied([256, 64], image.as_raw()),
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }
        section(ui, "PRESETS");
        for (i, preset) in render::CHEVRON_PRESETS.iter().enumerate() {
            if let Some(texture) = self.stripe_presets.get(i) {
                let button = egui::Button::image_and_text(
                    egui::Image::new((texture.id(), Vec2::new(76., 28.))),
                    preset.name,
                )
                .min_size(Vec2::new(ui.available_width(), 34.));
                if ui.add(button).on_hover_text(preset.description).clicked() {
                    self.session.chevron.apply_preset(i);
                }
            }
        }
        section(ui, "PATTERN");
        ui.horizontal(|ui| {
            ui.label("Color A");
            ui.color_edit_button_srgb(&mut self.session.chevron.color_a);
            ui.label("Color B");
            ui.color_edit_button_srgb(&mut self.session.chevron.color_b);
        });
        let mut remove = None;
        let color_count = self.session.chevron.extra_colors.len();
        for (i, color) in self.session.chevron.extra_colors.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("Color {}", i + 3));
                ui.color_edit_button_srgb(color);
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.session.chevron.extra_colors.remove(i);
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(color_count < 14, egui::Button::new("Add color"))
                .clicked()
            {
                self.session.chevron.extra_colors.push([255, 255, 255]);
            }
            if ui.button("Red / White / Blue").clicked() {
                self.session.chevron.color_a = [220, 25, 25];
                self.session.chevron.color_b = [255, 255, 255];
                self.session.chevron.extra_colors = vec![[25, 65, 220]];
            }
        });
        ui.label(
            RichText::new("Stripes repeat in the color order above.")
                .small()
                .color(MUTED),
        );
        let mut opacity = self.session.chevron.opacity as f32 * 100. / 255.;
        if ui
            .add(
                egui::Slider::new(&mut opacity, 0.0..=100.)
                    .text("Opacity")
                    .suffix("%")
                    .fixed_decimals(0),
            )
            .changed()
        {
            self.session.chevron.opacity = (opacity * 255. / 100.).round() as u8;
        }
        for (label, value) in [
            ("Width", &mut self.session.chevron.width),
            ("Height", &mut self.session.chevron.height),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(egui::DragValue::new(value).range(64..=8192).suffix(" px"));
            });
        }
        ui.add(
            egui::Slider::new(&mut self.session.chevron.stripe, 1.0..=500.).text("Stripe width"),
        );
        ui.add(egui::Slider::new(&mut self.session.chevron.angle, 5.0..=85.).text("Angle"));
        ui.add(egui::Slider::new(&mut self.session.chevron.center, 0.0..=1.).text("Seam position"));
        ui.add(egui::Slider::new(&mut self.session.chevron.offset, -1000.0..=1000.).text("Offset"));
        let previous = self.session.chevron.direction;
        egui::ComboBox::from_label("Direction")
            .selected_text(["Down chevron", "Up chevron", "Diagonal"][previous])
            .show_ui(ui, |ui| {
                for (i, label) in ["Down chevron", "Up chevron", "Diagonal"]
                    .iter()
                    .enumerate()
                {
                    ui.selectable_value(&mut self.session.chevron.direction, i, *label);
                }
            });
        if previous != self.session.chevron.direction {
            self.session.chevron.mirror_over_seam = Some(self.session.chevron.direction != 2);
        }
        section(ui, "EFFECTS");
        let mut mirrored = self.session.chevron.mirrored();
        if ui.checkbox(&mut mirrored, "Mirror over seam").changed() {
            self.session.chevron.mirror_over_seam = Some(mirrored);
        }
        ui.checkbox(&mut self.session.chevron.show_seam, "Show seam line")
            .on_hover_text("Preview guide only; excluded from exported artwork");
        ui.checkbox(&mut self.session.chevron.honeycomb, "Honeycomb overlay");
        if self.session.chevron.honeycomb {
            ui.add(
                egui::Slider::new(&mut self.session.chevron.cell_size, 3.0..=80.).text("Cell size"),
            );
        }
        ui.checkbox(
            &mut self.session.chevron.reflective_983,
            "3M 983-style reflective finish",
        )
        .on_hover_text("Embossed prism and sheen simulation baked into the artwork");
        ui.separator();
        ui.checkbox(
            &mut self.session.chevron.uv_clip,
            "Clip to current UV selection",
        );
        ui.label(
            RichText::new(
                "Use the UV Editor's material, LOD and channel selection as the export mask.",
            )
            .small()
            .color(MUTED),
        );
        if before != serde_json::to_string(&self.session.chevron).unwrap() {
            self.chevron_dirty = true;
            ui.ctx().request_repaint();
        }
    }
}
