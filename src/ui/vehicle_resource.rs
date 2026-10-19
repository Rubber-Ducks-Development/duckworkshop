use super::*;
use duck_workshop::vehicle_resource::{self, Imported};
impl Workbench {
    pub(super) fn import_vehicle_folder(&mut self) {
        if self.job.is_some() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Choose a vehicle folder containing stream and data")
            .pick_folder()
        {
            self.start_job(move || {
                Ok(JobResult::VehicleCatalog(vehicle_resource::discover(
                    &path,
                )?))
            });
        }
    }
    pub(super) fn accept_vehicle(&mut self, imported: Imported) {
        self.blink_snapshot();
        self.session.model_name = imported.name.clone();
        self.session.handling_name = imported.handling.clone();
        self.live = live::Live::new(&self.ctx);
        let matched = imported.pattern.is_some();
        self.session.pattern = imported.pattern.unwrap_or_else(|| {
            let count = imported
                .model
                .sirens
                .iter()
                .map(|a| a.id as usize)
                .max()
                .unwrap_or(1);
            Pattern {
                name: format!("{} - blank sequence", imported.model.name),
                channels: (1..=count)
                    .map(|i| patterns::Channel::new(format!("Siren {i}"), [255, 255, 255]))
                    .collect(),
                ..Default::default()
            }
        });
        self.blink_source_settings = self
            .session
            .pattern
            .source_xml
            .as_ref()
            .and_then(|s| Pattern::settings(s).ok())
            .unwrap_or_default();
        self.blink_play = false;
        self.blink_step = 0;
        self.blink_selected = 0;
        self.blink_brush = 0;
        self.session.siren_preview.scale = 1.;
        for (slot, source) in self.session.metas.iter_mut().zip(imported.metas) {
            *slot = source.unwrap_or_default();
        }
        self.meta_index = 0;
        self.refresh_meta();
        self.sync_decimal();
        self.accept_model(imported.model);
        self.tool = 1;
        self.siren_popout = true;
        self.status.push_str(if matched {
            " · matching siren pattern loaded"
        } else {
            " · no matching pattern; see Import notes"
        });
    }
    pub(super) fn vehicle_resource_dialog(&mut self, ctx: &egui::Context) {
        let Some(catalog) = self.vehicle_catalog.as_ref() else {
            return;
        };
        let mut open = true;
        let mut import = None;
        egui::Window::new("Import vehicle folder").open(&mut open).collapsible(false).default_width(580.).show(ctx,|ui|{
            ui.label("Choose the vehicle. Its model, textures and matching metadata will load together.");
            egui::ComboBox::from_id_salt("resource-vehicle").selected_text(&catalog.vehicles[self.vehicle_choice].name).width(300.).show_ui(ui,|ui|{
                for(i,v)in catalog.vehicles.iter().enumerate(){ui.selectable_value(&mut self.vehicle_choice,i,&v.name).on_hover_text(v.model.display().to_string());}
            });
            let v=&catalog.vehicles[self.vehicle_choice];
            ui.label(RichText::new(v.root.display().to_string().trim_start_matches(r"\\?\")).small().color(MUTED));
            ui.add_enabled(v.high.is_some(),egui::Checkbox::new(&mut self.vehicle_high,"Use high-detail model (_hi.yft)"));
            if !catalog.warnings.is_empty(){egui::CollapsingHeader::new("Folder scan notes").show(ui,|ui|{for w in &catalog.warnings{ui.label(w);}});}
            ui.label(RichText::new("The current light sequence is replaced. Undo restores it.").small().color(MUTED));
            if ui.add_enabled(self.job.is_none(),egui::Button::new("Import vehicle")).clicked(){import=Some((v.clone(),self.vehicle_high));}
        });
        if !open {
            self.vehicle_catalog = None;
        }
        if let Some((v, hi)) = import {
            self.vehicle_catalog = None;
            self.start_job(move || {
                Ok(JobResult::Vehicle(Box::new(vehicle_resource::import(
                    &v, hi,
                )?)))
            })
        }
    }
}
