use super::*;
use duck_workshop::sirens;
use glam::Vec3;

/// Compact badges stay near their sirens; only colliding badges move to nearby slots.
fn label_layout(view: Rect, points: &[Pos2]) -> Vec<Rect> {
    let size = Vec2::new(24., 17.);
    let bounds = Rect::from_min_max(view.min + Vec2::splat(8.), view.max - Vec2::new(8., 28.));
    let mut offsets: Vec<_> = (-8..=8)
        .flat_map(|y| (-8..=8).map(move |x| Vec2::new(x as f32 * 27., y as f32 * 21. - 16.)))
        .collect();
    offsets.sort_by(|a, b| {
        let cost = |offset: Vec2| (offset + Vec2::new(0., 16.)).length_sq();
        cost(*a).total_cmp(&cost(*b))
    });
    let mut order: Vec<_> = (0..points.len()).collect();
    order.sort_by(|&a, &b| {
        points[a]
            .y
            .total_cmp(&points[b].y)
            .then(points[a].x.total_cmp(&points[b].x))
    });
    let mut labels = vec![Rect::NOTHING; points.len()];
    let mut placed: Vec<Rect> = vec![];
    for index in order {
        let label = offsets
            .iter()
            .find_map(|offset| {
                let center = points[index] + *offset;
                let center = Pos2::new(
                    center
                        .x
                        .clamp(bounds.left() + size.x * 0.5, bounds.right() - size.x * 0.5),
                    center
                        .y
                        .clamp(bounds.top() + size.y * 0.5, bounds.bottom() - size.y * 0.5),
                );
                let candidate = Rect::from_center_size(center, size);
                placed
                    .iter()
                    .all(|r| !r.expand(2.).intersects(candidate))
                    .then_some(candidate)
            })
            .unwrap_or_else(|| Rect::from_center_size(bounds.center(), size));
        labels[index] = label;
        placed.push(label);
    }
    labels
}

impl Workbench {
    pub(super) fn siren_fit_camera() -> viewer::Camera {
        viewer::Camera {
            distance: viewer::Camera::default().distance * 0.9,
            ..Default::default()
        }
    }
    pub(super) fn advance_sirens(&mut self, ctx: &egui::Context) {
        if self.blink_play && (self.tool == 1 || self.siren_popout) {
            self.blink_step = (((ctx.input(|i| i.time) - self.blink_start)
                * self.session.pattern.bpm as f64
                / 60.)
                .floor() as usize)
                % 32;
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
    pub(super) fn siren_preview_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("3D SIREN PREVIEW").small().strong());
            if ui.small_button("Import folder").clicked() {
                self.import_vehicle_folder();
            }
            if ui.small_button("Model file").on_hover_text("Load only a model and keep the current sequence. Import folder matches vehicle data automatically.").clicked() {
                self.import_model();
            }
            if ui
                .small_button(if self.siren_popout {
                    "Hide preview"
                } else {
                    "Open 3D preview"
                })
                .clicked()
            {
                self.siren_popout = !self.siren_popout;
            }
        });
        if self.siren_popout {
            ui.label(
                RichText::new("Preview is open in its own resizable window.")
                    .small()
                    .color(MUTED),
            );
        } else {
            ui.label(
                RichText::new("Open the separate preview window to view vehicle lighting.")
                    .small()
                    .color(MUTED),
            );
        }
    }
    pub(super) fn prepare_siren_scene(&mut self) {
        if self.siren_scene.is_some() && self.siren_version == self.scene_version {
            return;
        }
        let full = self.model.clone().unwrap_or_else(sirens::schematic_model);
        self.siren_embedded.clear();
        {
            use base64::Engine;
            let mut decoded = std::collections::BTreeMap::new();
            for (name, bytes) in &full.preview_textures {
                if let Ok(image) = base64::engine::general_purpose::STANDARD
                    .decode(bytes)
                    .map_err(|e| e.to_string())
                    .and_then(|data| duck_workshop::textures::decode(&data))
                {
                    decoded.insert(name.clone(), std::sync::Arc::new(image));
                }
            }
            for (name, material) in &full.material_info {
                let texture = if material.is_livery() {
                    &material.secondary_texture
                } else {
                    &material.diffuse_texture
                };
                if let Some(image) = decoded.get(&texture.to_ascii_lowercase()) {
                    self.siren_embedded.insert(name.clone(), image.clone());
                }
            }
        }
        let opt = render::UvOptions {
            flip_v: self
                .model
                .as_ref()
                .is_none_or(|m| m.material_info.is_empty()),
            lod: ["High", "Med", "Low", "VeryLow"]
                .into_iter()
                .find(|lod| full.meshes.iter().any(|m| m.lod == *lod))
                .unwrap_or("All")
                .into(),
            ..Default::default()
        };
        self.siren_scene = viewer::build_vehicle(
            &full,
            &opt,
            self.model.is_none() || self.z_up,
            self.scene_version,
        )
        .map(std::sync::Arc::new);
        self.siren_version = self.scene_version;
    }
    pub(super) fn focus_siren_lightbar(&mut self) {
        self.prepare_siren_scene();
        if let Some(scene) = &self.siren_scene
            && let Some(model) = &self.model
        {
            let points: Vec<_> = model
                .sirens
                .iter()
                .map(|a| viewer::convert_position(a.position, self.z_up))
                .collect();
            let top = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
            let roof: Vec<_> = points.into_iter().filter(|p| p.y > top - 0.25).collect();
            if !roof.is_empty() {
                let center = roof.iter().copied().sum::<Vec3>() / roof.len() as f32;
                let radius = roof.iter().map(|p| p.distance(center)).fold(0.15, f32::max);
                self.siren_camera.pan = (center - scene.center) / viewer::radius(scene);
                self.siren_camera.distance =
                    (radius * 3.3 / viewer::radius(scene)).clamp(0.15, 10.);
            }
        }
    }
    fn siren_preview_content(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(if self.blink_play { "Pause" } else { "Play" })
                .clicked()
            {
                self.toggle_play(ui.ctx());
            }
            ui.label(format!(
                "Step {:02}/32 · {} BPM",
                self.blink_step + 1,
                self.session.pattern.bpm
            ));
            ui.checkbox(&mut self.session.siren_preview.labels, "Siren labels");
            ui.checkbox(&mut self.session.siren_preview.ground_glow, "Ground glow");
            ui.checkbox(&mut self.session.siren_preview.night, "Night");
            if ui.small_button("Fit").clicked() {
                self.siren_camera = Self::siren_fit_camera();
            }
            if ui
                .small_button("Lightbar view")
                .on_hover_text("Zoom in on the highest mounted siren lenses")
                .clicked()
            {
                self.focus_siren_lightbar();
            }
            if self.siren_popout {
                ui.checkbox(&mut self.siren_pin, "Pin window");
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::Slider::new(&mut self.session.siren_preview.scale, 0.25..=4.)
                    .text("Light scale")
                    .fixed_decimals(2),
            );
            ui.add(
                egui::Slider::new(&mut self.session.siren_preview.vehicle_opacity, 0..=100)
                    .text("Vehicle opacity")
                    .suffix("%"),
            )
            .on_hover_text(
                "Fade the body and lightbar housing to see the siren lenses. Lights stay visible.",
            );
        });
        self.prepare_siren_scene();
        let markers = sirens::markers(
            self.model.as_ref(),
            &self.session.pattern,
            &self.session.siren_preview,
            self.blink_step,
        );
        let description = match &self.model {
            None => "Schematic lightbar · import a vehicle for model siren positions".to_string(),
            Some(model) => format!(
                "{} · {} model siren bones · {} light surfaces{}",
                model.name,
                model.sirens.len(),
                self.siren_scene
                    .as_ref()
                    .map_or(0, |scene| scene.siren_ids.len()),
                if markers.iter().any(|m| m.inferred) {
                    " · unassigned sirens use editable roof positions"
                } else {
                    ""
                }
            ),
        };
        ui.label(RichText::new(description).small().color(MUTED));
        if let Some(model) = &self.model {
            egui::CollapsingHeader::new(format!("Import notes ({})", model.warnings.len()))
                .id_salt("siren-import-notes")
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(120.)
                        .show(ui, |ui| {
                            for warning in &model.warnings {
                                ui.label(warning);
                            }
                        });
                });
        }
        let size = Vec2::new(
            ui.available_width(),
            height.min(ui.available_height() - 45.).max(100.),
        );
        self.siren_view(ui, size, &markers);
        if let Some(marker) = markers
            .iter()
            .find(|m| m.channel == self.blink_selected && m.layer == self.blink_brush)
        {
            let (id, mut position) = (marker.id, marker.position);
            egui::CollapsingHeader::new(format!("Siren {id} position"))
                .id_salt("siren-position")
                .show(ui, |ui| {
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        for (axis, value) in ["X", "Y", "Z"].into_iter().zip(&mut position) {
                            ui.label(axis);
                            changed |= ui
                                .add(
                                    egui::DragValue::new(value)
                                        .speed(0.02)
                                        .range(-10000.0..=10000.)
                                        .fixed_decimals(3),
                                )
                                .changed();
                        }
                    });
                    if changed {
                        self.session.siren_preview.positions.insert(id, position);
                    }
                    if ui.small_button("Use model / automatic position").clicked() {
                        self.session.siren_preview.positions.remove(&id);
                    }
                });
        }
    }
    fn siren_view(&mut self, ui: &mut egui::Ui, size: Vec2, markers: &[sirens::Marker]) {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 3., VIEWPORT);
        let Some(scene) = self.siren_scene.clone() else {
            return;
        };
        let (delta, shift) = ui.input(|i| (i.pointer.delta(), i.modifiers.shift));
        if response.dragged_by(egui::PointerButton::Primary) && !shift {
            self.siren_camera.yaw -= delta.x * 0.008;
            self.siren_camera.pitch =
                (self.siren_camera.pitch + delta.y * 0.008).clamp(-1.55, 1.55);
        } else if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Primary)
        {
            viewer::pan(&mut self.siren_camera, delta, rect.height());
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            self.siren_camera.distance =
                (self.siren_camera.distance * (-scroll * 0.0018).exp()).clamp(0.1, 30.);
        }
        if response.double_clicked() {
            self.siren_camera = Self::siren_fit_camera();
        }
        let radius = viewer::radius(&scene);
        let z_up = self.model.is_none() || self.z_up;
        let project = |point| viewer::project_point(&self.siren_camera, &scene, rect, point);
        let floor = scene.floor - radius * 0.02;
        for i in -10..=10 {
            let offset = i as f32 * radius * 0.3;
            let extent = radius * 3.;
            for (a, b) in [
                (
                    Vec3::new(scene.center.x - extent, floor, scene.center.z + offset),
                    Vec3::new(scene.center.x + extent, floor, scene.center.z + offset),
                ),
                (
                    Vec3::new(scene.center.x + offset, floor, scene.center.z - extent),
                    Vec3::new(scene.center.x + offset, floor, scene.center.z + extent),
                ),
            ] {
                if let (Some(a), Some(b)) = (project(a), project(b)) {
                    p.line_segment([a, b], Stroke::new(0.6_f32, Color32::from_gray(67)));
                }
            }
        }
        if self.session.siren_preview.ground_glow {
            for marker in markers.iter().filter(|m| m.active) {
                let mut point = viewer::convert_position(marker.position, z_up);
                point.y = floor;
                if let Some(pos) = project(point) {
                    let r = 20. * self.session.siren_preview.scale;
                    for ring in (1..=5).rev() {
                        p.add(egui::Shape::ellipse_filled(
                            pos,
                            Vec2::new(r * ring as f32 / 2., r * ring as f32 / 5.),
                            Color32::from_rgba_unmultiplied(
                                marker.color[0],
                                marker.color[1],
                                marker.color[2],
                                7,
                            ),
                        ));
                    }
                }
            }
        }
        let mut textures = vec![];
        let bindings = scene
            .groups
            .iter()
            .map(|group| {
                let (path, version, image) = if let Some(file) = self
                    .live
                    .for_material(&group.material)
                    .filter(|f| f.image.is_some())
                {
                    (file.path.clone(), file.version, file.image.clone()?)
                } else {
                    (
                        PathBuf::from(format!("@vehicle/{}", group.material)),
                        self.scene_version,
                        self.siren_embedded.get(&group.material)?.clone(),
                    )
                };
                Some(match textures.iter().position(|(p, _, _)| *p == path) {
                    Some(i) => i,
                    None => {
                        textures.push((path, version, image));
                        textures.len() - 1
                    }
                })
            })
            .collect();
        let lights = markers
            .iter()
            .filter(|m| m.active)
            .map(|m| viewer::PreviewLight {
                origin: viewer::convert_position(
                    self.model
                        .as_ref()
                        .and_then(|model| model.sirens.iter().find(|a| a.id == m.id))
                        .map_or(m.position, |a| a.position),
                    z_up,
                ),
                scale: m.surface_scale * self.session.siren_preview.scale,
                angle: self.session.pattern.channels[m.channel].delta,
                id: m.id,
                position: viewer::convert_position(m.position, z_up),
                color: m.color,
                intensity: m.intensity.clamp(0., 4.) * 0.55,
                radius: radius * 0.18 * self.session.siren_preview.scale,
            })
            .collect();
        p.add(viewer::callback(
            &self.siren_gpu,
            rect,
            viewer::Frame {
                vehicle_opacity: self.session.siren_preview.vehicle_opacity as f32 / 100.,
                siren_surfaces: true,
                night: self.session.siren_preview.night,
                mesh: scene.clone(),
                textures,
                bindings,
                camera: self.siren_camera,
                wire: false,
                paint: [165, 172, 181],
                lights,
            },
        ));
        let visible: Vec<_> = markers
            .iter()
            .filter_map(|marker| {
                let pos = project(viewer::convert_position(marker.position, z_up))?;
                rect.contains(pos).then_some((marker, pos))
            })
            .collect();
        let labels = if self.session.siren_preview.labels {
            label_layout(
                rect,
                &visible.iter().map(|(_, pos)| *pos).collect::<Vec<_>>(),
            )
        } else {
            vec![]
        };
        let mut selected = None;
        for (marker, pos) in visible.iter().copied() {
            if scene.siren_ids.contains(&marker.id) {
                continue;
            }
            let color = if marker.active {
                Color32::from_rgb(marker.color[0], marker.color[1], marker.color[2])
            } else {
                Color32::from_gray(75)
            };
            let r = (if marker.active { 4.5 } else { 2.5 }) * self.session.siren_preview.scale;
            if marker.active {
                for ring in (1..=4).rev() {
                    p.circle_filled(
                        pos,
                        r + ring as f32 * 3.,
                        Color32::from_rgba_unmultiplied(
                            marker.color[0],
                            marker.color[1],
                            marker.color[2],
                            12,
                        ),
                    );
                }
            }
            p.circle_filled(pos, r, color);
            if marker.active {
                p.circle_filled(pos, r * 0.35, Color32::WHITE);
            }
        }
        // Draw every badge after the light coronas so the numbers remain readable.
        for (index, (marker, _)) in visible.into_iter().enumerate() {
            if self.session.siren_preview.labels {
                let label = labels[index];
                p.rect_filled(label, 3., Color32::from_rgb(17, 25, 39));
                p.rect_stroke(
                    label,
                    3.,
                    Stroke::new(
                        1.0_f32,
                        if marker.channel == self.blink_selected {
                            ACCENT
                        } else if marker.active {
                            Color32::from_rgb(95, 138, 225)
                        } else {
                            Color32::from_rgb(57, 77, 112)
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
                p.text(
                    label.center(),
                    Align2::CENTER_CENTER,
                    marker.id.to_string(),
                    egui::FontId::monospace(11.),
                    TEXT,
                );
                if ui
                    .interact(
                        label,
                        ui.id().with(("siren-label", marker.id)),
                        Sense::click(),
                    )
                    .on_hover_text(format!(
                        "Siren {} · {}",
                        marker.id, self.session.pattern.channels[marker.channel].name
                    ))
                    .clicked()
                {
                    selected = Some((marker.channel, marker.layer));
                }
            }
        }
        if let Some((channel, layer)) = selected {
            self.blink_selected = channel;
            self.blink_brush = layer;
            self.sync_decimal();
        }
        if let Some(error) = self
            .siren_gpu
            .lock()
            .ok()
            .and_then(|g| g.error().map(str::to_string))
        {
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                error,
                egui::FontId::proportional(14.),
                ERROR,
            );
        }
        p.text(
            rect.left_bottom() + Vec2::new(10., -10.),
            Align2::LEFT_BOTTOM,
            "Drag: orbit · Right-drag: pan · Scroll: zoom · Click label: select siren",
            egui::FontId::proportional(11.),
            MUTED,
        );
    }
    pub(super) fn siren_popout_ui(&mut self, ctx: &egui::Context) {
        if !self.siren_popout {
            return;
        }
        let mut open = true;
        let builder = egui::ViewportBuilder::default()
            .with_title("Duck Workshop · Siren preview")
            .with_inner_size([960., 700.])
            .with_min_inner_size([520., 400.])
            .with_window_level(if self.siren_pin {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            });
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("siren-player"),
            builder,
            |ctx, class| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    open = false;
                }
                if class == egui::ViewportClass::Embedded {
                    egui::Window::new("3D siren preview")
                        .open(&mut open)
                        .default_size([900., 600.])
                        .show(ctx, |ui| {
                            let height = (ui.available_height() - 130.).max(200.);
                            self.siren_preview_content(ui, height);
                        });
                } else {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::new().fill(BG).inner_margin(12.))
                        .show(ctx, |ui| {
                            let height = (ui.available_height() - 145.).max(100.);
                            self.siren_preview_content(ui, height);
                        });
                }
            },
        );
        self.siren_popout = open;
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;

    fn assert_spaced(view: Rect, points: &[Pos2]) {
        let labels = label_layout(view, points);
        assert_eq!(labels.len(), points.len());
        for (i, label) in labels.iter().enumerate() {
            assert!(view.shrink(8.).contains_rect(*label));
            for other in &labels[i + 1..] {
                assert!(
                    !label.expand(0.9).intersects(other.expand(0.9)),
                    "Labels {label:?} and {other:?} need a clear gap"
                );
            }
        }
        assert_eq!(
            labels,
            label_layout(view, points),
            "Placement is deterministic"
        );
    }

    #[test]
    fn coincident_sirens_keep_compact_separate_badges() {
        for size in [
            Vec2::new(904., 280.),
            Vec2::new(936., 531.),
            Vec2::new(496., 200.),
        ] {
            let view = Rect::from_min_size(Pos2::new(20., 40.), size);
            let points = vec![view.center(); 32];
            assert_spaced(view, &points);
            assert!(
                label_layout(view, &points)
                    .iter()
                    .all(|label| label.center().distance(view.center()) < 130.)
            );
        }
    }

    #[test]
    fn edge_sirens_and_small_groups_stay_inside_the_preview() {
        let view = Rect::from_min_size(Pos2::new(100., 200.), Vec2::new(700., 280.));
        for anchor in [view.left_top(), view.right_bottom(), view.center()] {
            for count in [1, 2, 7, 16, 32] {
                assert_spaced(view, &vec![anchor; count]);
            }
        }
        assert!(label_layout(view, &[]).is_empty());
        let anchor = view.center();
        assert_eq!(
            label_layout(view, &[anchor])[0].center(),
            anchor + Vec2::new(0., -16.)
        );
    }
}
