//! Textures shown on the model. Files are watched and reloaded whenever they are saved.
use super::*;
use duck_workshop::textures::{self, Stamp};
use image::RgbaImage;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(super) struct TextureFile {
    pub path: PathBuf,
    pub image: Option<Arc<RgbaImage>>,
    pub version: u64,
    pub error: Option<String>,
    pub loading: bool,
    pub updated: Option<Instant>,
    reload: bool,
}
impl TextureFile {
    pub fn name(&self) -> String {
        file_name(&self.path)
    }
}

enum Event {
    Changed(PathBuf),
    Loaded(PathBuf, Result<RgbaImage>),
}

pub(super) struct Live {
    pub files: Vec<TextureFile>,
    /// Material name to texture file.
    pub assigned: BTreeMap<String, PathBuf>,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
    watched: Arc<Mutex<HashMap<PathBuf, Option<Stamp>>>>,
    next_version: u64,
    preview: Option<(PathBuf, u64, egui::TextureHandle)>,
}

impl Live {
    pub fn new(ctx: &egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let watched: Arc<Mutex<HashMap<PathBuf, Option<Stamp>>>> = Arc::default();
        let (shared, sender, ctx) = (watched.clone(), tx.clone(), ctx.clone());
        // Polls the watched files. A change is reported once the size and time have
        // stayed the same for one poll, so files are not read while still being written.
        std::thread::spawn(move || {
            let mut observed: HashMap<PathBuf, Option<Stamp>> = HashMap::new();
            loop {
                std::thread::sleep(Duration::from_millis(400));
                let list: Vec<_> = match shared.lock() {
                    Ok(map) => map.iter().map(|(p, s)| (p.clone(), *s)).collect(),
                    Err(_) => return,
                };
                observed.retain(|p, _| list.iter().any(|(q, _)| q == p));
                for (path, known) in list {
                    let now = textures::stamp(&path);
                    if now.is_some() && now != known && observed.get(&path) == Some(&now) {
                        if let Ok(mut map) = shared.lock()
                            && let Some(entry) = map.get_mut(&path)
                        {
                            *entry = now;
                        }
                        if sender.send(Event::Changed(path.clone())).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                    }
                    observed.insert(path, now);
                }
            }
        });
        Self {
            files: vec![],
            assigned: BTreeMap::new(),
            tx,
            rx,
            watched,
            next_version: 1,
            preview: None,
        }
    }

    /// Shows `path` on the given materials, loading it if it is not open yet.
    pub fn assign(&mut self, ctx: &egui::Context, materials: Vec<String>, path: PathBuf) {
        for material in materials {
            self.assigned.insert(material, path.clone());
        }
        if !self.files.iter().any(|f| f.path == path) {
            self.files.push(TextureFile {
                path: path.clone(),
                image: None,
                version: 0,
                error: None,
                loading: false,
                updated: None,
                reload: false,
            });
            self.load(ctx, &path);
        }
        self.prune();
    }

    pub fn unassign(&mut self, material: &str) {
        self.assigned.remove(material);
        self.prune();
    }

    /// Keeps assignments for materials that still exist.
    pub fn retain_materials(&mut self, materials: &[String]) {
        self.assigned.retain(|m, _| materials.contains(m));
        self.prune();
    }

    pub fn clear(&mut self) {
        self.assigned.clear();
        self.prune();
    }

    fn prune(&mut self) {
        let assigned = &self.assigned;
        self.files
            .retain(|f| assigned.values().any(|p| *p == f.path));
        if let Ok(mut map) = self.watched.lock() {
            map.retain(|p, _| assigned.values().any(|q| q == p));
        }
    }

    pub fn reload(&mut self, ctx: &egui::Context, path: &PathBuf) {
        self.load(ctx, path);
    }

    fn load(&mut self, ctx: &egui::Context, path: &PathBuf) {
        let Some(file) = self.files.iter_mut().find(|f| f.path == *path) else {
            return;
        };
        if file.loading {
            file.reload = true;
            return;
        }
        file.loading = true;
        if let Ok(mut map) = self.watched.lock() {
            map.insert(path.clone(), textures::stamp(path));
        }
        let (tx, path, ctx) = (self.tx.clone(), path.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| textures::load(&path))
                .unwrap_or_else(|_| Err("This image could not be read safely.".into()));
            let _ = tx.send(Event::Loaded(path, result));
            ctx.request_repaint();
        });
    }

    /// Applies finished loads and file changes. Returns messages for the status bar.
    pub fn poll(&mut self, ctx: &egui::Context) -> Vec<Result<String>> {
        let mut messages = vec![];
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Changed(path) => self.load(ctx, &path),
                Event::Loaded(path, result) => {
                    let Some(file) = self.files.iter_mut().find(|f| f.path == path) else {
                        continue;
                    };
                    file.loading = false;
                    let reload = file.image.is_some();
                    match result {
                        Ok(image) => {
                            messages.push(Ok(if reload {
                                format!("Updated {} on the model", file.name())
                            } else {
                                format!(
                                    "Loaded {} ({} x {})",
                                    file.name(),
                                    image.width(),
                                    image.height()
                                )
                            }));
                            file.image = Some(Arc::new(image));
                            file.version = self.next_version;
                            file.error = None;
                            if reload {
                                file.updated = Some(Instant::now());
                            }
                            self.next_version += 1;
                        }
                        Err(e) => {
                            messages.push(Err(format!("Couldn't load {}: {e}", file.name())));
                            file.error = Some(e);
                        }
                    }
                    if std::mem::take(&mut file.reload) {
                        self.load(ctx, &path);
                    }
                }
            }
        }
        messages
    }

    pub fn file(&self, path: &PathBuf) -> Option<&TextureFile> {
        self.files.iter().find(|f| f.path == *path)
    }

    pub fn for_material(&self, material: &str) -> Option<&TextureFile> {
        self.assigned.get(material).and_then(|p| self.file(p))
    }

    /// The texture as an egui image, for drawing behind the UV template.
    pub fn preview(&mut self, ctx: &egui::Context, path: &PathBuf) -> Option<egui::TextureId> {
        let file = self.files.iter().find(|f| f.path == *path)?;
        let image = file.image.as_ref()?;
        if let Some((p, v, handle)) = &self.preview
            && p == path
            && *v == file.version
        {
            return Some(handle.id());
        }
        let side = ctx.input(|i| i.max_texture_side).min(4096) as u32;
        let scaled;
        let mut source: &RgbaImage = image;
        if image.width().max(image.height()) > side {
            scaled = image::imageops::thumbnail(
                image.as_ref(),
                (image.width() * side / image.width().max(image.height())).max(1),
                (image.height() * side / image.width().max(image.height())).max(1),
            );
            source = &scaled;
        }
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [source.width() as usize, source.height() as usize],
            source.as_raw(),
        );
        let handle = ctx.load_texture("uv-texture", color, egui::TextureOptions::LINEAR);
        let id = handle.id();
        self.preview = Some((path.clone(), file.version, handle));
        Some(id)
    }
}

pub(super) fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
