use super::*;
use duck_workshop::updates::{self as service, StagedUpdate, VerifiedRelease};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub(super) struct Settings {
    enabled: bool,
    feed: String,
    skipped: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            feed: service::PUBLIC_FEED.into(),
            skipped: String::new(),
        }
    }
}

enum Event {
    Checked(Result<Option<VerifiedRelease>>),
    Downloaded(Result<StagedUpdate>),
}

pub(super) struct Updater {
    settings: Settings,
    pub show: bool,
    pub show_settings: bool,
    status: String,
    receiver: Option<mpsc::Receiver<Event>>,
    last_check: Option<Instant>,
    manual: bool,
    available: Option<VerifiedRelease>,
    staged: Option<StagedUpdate>,
    confirm_restart: bool,
}

impl Updater {
    pub fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        Self {
            settings: storage
                .and_then(|s| s.get_string("signed-updates-v1"))
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            show: false,
            show_settings: false,
            status: String::new(),
            receiver: None,
            last_check: None,
            manual: false,
            available: None,
            staged: None,
            confirm_restart: false,
        }
    }
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        if let Ok(json) = serde_json::to_string(&self.settings) {
            storage.set_string("signed-updates-v1", json);
        }
    }
    pub fn check(&mut self, ctx: &egui::Context, manual: bool) {
        if self.staged.is_some() {
            self.show |= manual;
            return;
        }
        if self.receiver.is_some() {
            self.show |= manual;
            return;
        }
        self.last_check = Some(Instant::now());
        self.manual = manual;
        self.show |= manual;
        self.status = "Checking for signed updates...".into();
        self.available = None;
        let feed = self.settings.feed.clone();
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(Event::Checked(service::check(&feed)));
            ctx.request_repaint();
        });
    }
    fn poll(&mut self, ctx: &egui::Context) {
        if self.settings.enabled
            && self.receiver.is_none()
            && self.available.is_none()
            && self.staged.is_none()
            && self
                .last_check
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(6 * 60 * 60))
        {
            self.check(ctx, false);
        }
        if self.settings.enabled {
            ctx.request_repaint_after(Duration::from_secs(60));
        }
        if let Some(rx) = &self.receiver {
            match rx.try_recv() {
                Ok(event) => {
                    self.receiver = None;
                    match event {
                        Event::Checked(Ok(Some(verified))) => {
                            let offer =
                                self.manual || verified.release.version != self.settings.skipped;
                            self.show |= offer;
                            self.status =
                                format!("Duck Workshop {} is available.", verified.release.version);
                            if offer {
                                self.available = Some(verified);
                            }
                        }
                        Event::Checked(Ok(None)) => {
                            self.status = "You're using the latest signed release.".into();
                        }
                        Event::Downloaded(Ok(staged)) => {
                            self.status = format!(
                                "Version {} downloaded. Signature, size and SHA256 verified.",
                                staged.release.version
                            );
                            self.staged = Some(staged);
                            self.show = true;
                        }
                        Event::Checked(Err(e)) | Event::Downloaded(Err(e)) => {
                            self.status = e;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.status = "Update task stopped. Try again.".into();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
            }
        }
    }
    fn download(&mut self, ctx: &egui::Context) {
        let Some(verified) = self.available.take() else {
            return;
        };
        let target = match std::env::current_exe() {
            Ok(path) => path,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        self.status = "Downloading and verifying update... You can keep working.".into();
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Downloaded(service::download(verified, &target)));
            ctx.request_repaint();
        });
    }
}

impl Workbench {
    pub(super) fn install_update(&mut self, ctx: &egui::Context) {
        self.update_after_save = false;
        if self.job.is_some() {
            return;
        }
        let Some(staged) = self.updater.staged.take() else {
            return;
        };
        let result = std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|target| staged.launch(&target));
        match result {
            Ok(()) => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(error) => {
                self.updater.status = error;
            }
        }
    }

    pub(super) fn updates_ui(&mut self, ctx: &egui::Context) {
        self.updater.poll(ctx);
        if self.updater.show_settings {
            egui::Window::new("Update settings").open(&mut self.updater.show_settings).default_width(500.).show(ctx, |ui| {
                ui.checkbox(&mut self.updater.settings.enabled, "Check automatically on startup and every six hours");
                ui.add_space(8.);
                ui.label("Signed HTTPS update feed");
                ui.text_edit_singleline(&mut self.updater.settings.feed);
                if ui.button("Use GitHub Releases").clicked() { self.updater.settings.feed = service::PUBLIC_FEED.into(); }
                ui.label(RichText::new("GitHub release downloads must be public. Private-repository access tokens are never stored in the app.").small().color(CAPTION));
                if let Err(error) = service::https_url(&self.updater.settings.feed) { ui.colored_label(ERROR, error); }
                ui.separator();
                ui.label("Only releases signed by Duck Workshop's pinned key are accepted.");
            });
        }
        if !self.updater.show {
            return;
        }
        let mut action = None;
        let mut open = true;
        egui::Window::new("Duck Workshop updates").open(&mut open).resizable(false).default_width(470.).show(ctx, |ui| {
            ui.label(RichText::new(format!("Installed version {}", env!("CARGO_PKG_VERSION"))).color(CAPTION));
            ui.add_space(6.);
            ui.label(&self.updater.status);
            if self.updater.receiver.is_some() { ui.spinner(); }
            let notes = self.updater.staged.as_ref().map(|s| &s.release.notes)
                .or_else(|| self.updater.available.as_ref().map(|r| &r.release.notes));
            if let Some(notes) = notes { egui::ScrollArea::vertical().max_height(170.).show(ui, |ui| { ui.label(notes); }); }
            if self.updater.staged.is_some() {
                ui.separator();
                ui.label("Save your project before restarting. Texture images stay in their existing locations.");
                ui.add_enabled_ui(self.job.is_none() && !self.update_after_save, |ui| {
                    if ui.button("Save project and restart").clicked() { action = Some(0); }
                    if ui.button("Restart without saving").clicked() { action = Some(1); }
                });
                if self.update_after_save { ui.spinner(); ui.label("Saving project before restart..."); }
            } else if self.updater.available.is_some() {
                if primary(ui, "Download update").clicked() { action = Some(2); }
            } else if self.updater.receiver.is_none() && ui.button("Check again").clicked() { action = Some(3); }
            if ui.button("Not now").clicked() { action = Some(4); }
        });
        if !open || action == Some(4) {
            if let Some(release) = self.updater.available.take() {
                self.updater.settings.skipped = release.release.version;
            }
            if let Some(staged) = self.updater.staged.take() {
                self.updater.settings.skipped = staged.release.version.clone();
            }
            self.update_after_save = false;
            self.updater.show = false;
        }
        match action {
            Some(0) => {
                self.save_project(false);
                self.update_after_save = self.job.is_some();
            }
            Some(1) => self.updater.confirm_restart = true,
            Some(2) => self.updater.download(ctx),
            Some(3) => self.updater.check(ctx, true),
            _ => {}
        }
        if self.updater.confirm_restart {
            match workspace::dialog(
                ctx,
                "update-discard",
                "Restart for update?",
                "Restart without saving?",
                "Imported geometry and texture links that are not in a saved project will be lost.",
                &["Restart", "Cancel"],
            ) {
                Some(0) => {
                    self.updater.confirm_restart = false;
                    self.install_update(ctx);
                }
                Some(_) => self.updater.confirm_restart = false,
                None => {}
            }
        }
    }
}
