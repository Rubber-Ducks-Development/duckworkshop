//! Duck Studio-matched dark theme: palette, fonts and small styled widgets.
use super::*;
use egui::{CornerRadius, FontFamily, FontId, Margin, Shadow, text::LayoutJob, text::TextFormat};
use std::sync::{Arc, OnceLock};

pub(super) const BG: Color32 = Color32::from_rgb(0x30, 0x30, 0x30);
pub(super) const STATUS_BG: Color32 = Color32::from_rgb(0x30, 0x30, 0x32);
pub(super) const MENU_BG: Color32 = Color32::from_rgb(0x41, 0x41, 0x43);
pub(super) const DIVIDER: Color32 = Color32::from_rgb(0x64, 0x64, 0x67);
pub(super) const LIST_BG: Color32 = Color32::from_rgb(0x3B, 0x3B, 0x3B);
pub(super) const LIST_LINE: Color32 = Color32::from_rgb(0x50, 0x50, 0x50);
pub(super) const ROW_SELECTED: Color32 = Color32::from_rgb(0x62, 0x66, 0x6A);
pub(super) const ROW_HOVER: Color32 = Color32::from_rgb(0x92, 0x92, 0x92);
pub(super) const VIEWPORT: Color32 = Color32::from_rgb(0x24, 0x24, 0x26);
pub(super) const TEXT: Color32 = Color32::from_rgb(0xEE, 0xEE, 0xEE);
pub(super) const CAPTION: Color32 = Color32::from_rgb(0xB9, 0xB9, 0xBA);
pub(super) const ERROR: Color32 = Color32::from_rgb(232, 98, 92);
pub(super) const SUCCESS: Color32 = Color32::from_rgb(120, 200, 130);
const BUTTON: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x4C);
const BUTTON_HOVER: Color32 = Color32::from_rgb(0x56, 0x56, 0x58);
const BUTTON_PRESSED: Color32 = Color32::from_rgb(0x5E, 0x5E, 0x61);
const FIELD: Color32 = Color32::from_rgb(0x37, 0x37, 0x38);
const FOCUS: Color32 = Color32::from_rgb(0xA6, 0xC6, 0xE8);
const TAB: Color32 = Color32::from_rgb(0x50, 0x50, 0x50);
const TAB_SELECTED: Color32 = Color32::from_rgb(0xD1, 0xD1, 0xD1);
const TAB_LINE: Color32 = Color32::from_rgb(0x6B, 0x6B, 0x6B);

/// Segoe MDL2 Assets glyphs, the icon set Duck Studio uses.
pub(super) mod icon {
    pub const UV: char = '\u{E809}';
    pub const LIGHTS: char = '\u{E781}';
    pub const STRIPES: char = '\u{E790}';
    pub const META: char = '\u{E943}';
    pub const COMPARE: char = '\u{E8AB}';
    pub const OPEN: char = '\u{E8E5}';
    pub const SAVE: char = '\u{E74E}';
    pub const HELP: char = '\u{E897}';
    pub const IMPORT: char = '\u{E896}';
    pub const EXPORT: char = '\u{E898}';
    pub const PLAY: char = '\u{E768}';
    pub const PAUSE: char = '\u{E769}';
    pub const STOP: char = '\u{E71A}';
    pub const UNDO: char = '\u{E7A7}';
    pub const REDO: char = '\u{E7A6}';
    pub const ADD: char = '\u{E710}';
    pub const REMOVE: char = '\u{E74D}';
}
pub(super) const TOOL_ICONS: [char; 5] = [
    icon::UV,
    icon::LIGHTS,
    icon::STRIPES,
    icon::META,
    icon::COMPARE,
];

static ICONS: OnceLock<bool> = OnceLock::new();

fn icon_family() -> FontFamily {
    FontFamily::Name("icons".into())
}

pub(super) fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(studio_style);
}

/// Uses the Windows UI fonts when present; the bundled egui fonts remain as fallbacks.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("Fonts");
    let mut load = |file: &str, name: &str, family: FontFamily| {
        let Ok(bytes) = std::fs::read(dir.join(file)) else {
            return false;
        };
        fonts
            .font_data
            .insert(name.into(), Arc::new(egui::FontData::from_owned(bytes)));
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.into());
        true
    };
    load("segoeui.ttf", "segoe-ui", FontFamily::Proportional);
    load("consola.ttf", "consolas", FontFamily::Monospace);
    let icons = load("segmdl2.ttf", "segoe-mdl2", icon_family())
        || load("SegoeIcons.ttf", "segoe-icons", icon_family());
    // The family must exist even without an icon font; icons are then omitted.
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    fonts
        .families
        .entry(icon_family())
        .or_default()
        .extend(fallback);
    let _ = ICONS.set(icons);
    ctx.set_fonts(fonts);
}

fn studio_style(style: &mut egui::Style) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = Color32::from_rgb(0x38, 0x38, 0x3A);
    v.window_stroke = Stroke::new(1.0_f32, DIVIDER);
    v.window_corner_radius = CornerRadius::same(3);
    v.menu_corner_radius = CornerRadius::same(2);
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    v.popup_shadow = Shadow {
        offset: [0, 3],
        blur: 10,
        spread: 0,
        color: Color32::from_black_alpha(90),
    };
    v.extreme_bg_color = FIELD;
    v.faint_bg_color = Color32::from_rgb(0x35, 0x35, 0x36);
    v.code_bg_color = LIST_BG;
    v.selection.bg_fill = Color32::from_rgb(0x53, 0x61, 0x6E);
    v.selection.stroke = Stroke::new(1.0_f32, TEXT);
    v.hyperlink_color = FOCUS;
    v.warn_fg_color = ACCENT;
    v.error_fg_color = ERROR;
    v.slider_trailing_fill = true;
    let radius = CornerRadius::same(2);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = BG;
    w.noninteractive.weak_bg_fill = BG;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, DIVIDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xDD, 0xDD, 0xDD));
    w.noninteractive.corner_radius = radius;
    for (state, fill, line, text) in [
        (&mut w.inactive, BUTTON, DIVIDER, TEXT),
        (
            &mut w.hovered,
            BUTTON_HOVER,
            Color32::from_rgb(0x8A, 0x8D, 0x91),
            Color32::WHITE,
        ),
        (&mut w.active, BUTTON_PRESSED, FOCUS, Color32::WHITE),
        (
            &mut w.open,
            Color32::from_rgb(0x41, 0x43, 0x47),
            Color32::from_rgb(0x8A, 0x8D, 0x91),
            TEXT,
        ),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0_f32, line);
        state.fg_stroke = Stroke::new(1.0_f32, text);
        state.corner_radius = radius;
        state.expansion = 0.;
    }
    style.visuals = v;
    style.spacing.item_spacing = Vec2::new(8., 6.);
    style.spacing.button_padding = Vec2::new(10., 4.);
    style.spacing.interact_size.y = 24.;
    style.spacing.menu_margin = Margin::same(4);
    style.spacing.window_margin = Margin::same(14);
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    for (text, size, family) in [
        (egui::TextStyle::Heading, 20., FontFamily::Proportional),
        (egui::TextStyle::Body, 13., FontFamily::Proportional),
        (egui::TextStyle::Button, 13., FontFamily::Proportional),
        (egui::TextStyle::Small, 11., FontFamily::Proportional),
        (egui::TextStyle::Monospace, 12.5, FontFamily::Monospace),
    ] {
        style.text_styles.insert(text, FontId::new(size, family));
    }
}

/// Icon glyph followed by text; colors follow the widget state.
pub(super) fn icon_text(glyph: char, text: &str) -> egui::WidgetText {
    let mut job = LayoutJob::default();
    let format = |font_id| TextFormat {
        font_id,
        color: Color32::PLACEHOLDER,
        valign: egui::Align::Center,
        ..Default::default()
    };
    let mut leading = 0.;
    if ICONS.get().copied().unwrap_or(false) {
        job.append(
            &glyph.to_string(),
            0.,
            format(FontId::new(14., icon_family())),
        );
        leading = 8.;
    }
    job.append(text, leading, format(FontId::proportional(13.)));
    job.into()
}

pub(super) fn icon_font(size: f32) -> Option<FontId> {
    ICONS
        .get()
        .copied()
        .unwrap_or(false)
        .then(|| FontId::new(size, icon_family()))
}

pub(super) fn icon_button(ui: &mut egui::Ui, glyph: char, text: &str) -> egui::Response {
    ui.add(egui::Button::new(icon_text(glyph, text)))
}

/// The main action of a tool: a standard Studio button with an amber outline.
pub(super) fn primary_button(ui: &mut egui::Ui, text: egui::WidgetText) -> egui::Response {
    ui.scope(|ui| {
        let w = &mut ui.visuals_mut().widgets;
        w.inactive.bg_stroke = Stroke::new(1.0_f32, ACCENT);
        w.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xF0, 0xD6, 0x96));
        w.inactive.weak_bg_fill = Color32::from_rgb(0x52, 0x50, 0x4A);
        ui.add(egui::Button::new(text))
    })
    .inner
}

/// Duck Studio tab: grey when idle, light with dark text when selected.
pub(super) fn tab(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    let (fill, color) = if selected {
        (TAB_SELECTED, Color32::from_rgb(0x25, 0x25, 0x25))
    } else {
        (TAB, TEXT)
    };
    ui.add(
        egui::Button::new(RichText::new(text).color(color))
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, TAB_LINE))
            .corner_radius(CornerRadius {
                nw: 2,
                ne: 2,
                sw: 0,
                se: 0,
            }),
    )
}

/// Row of tabs sitting on a divider line.
pub(super) fn tabs<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    current: &mut T,
    items: &[(T, &str)],
) -> bool {
    let mut changed = false;
    let response = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.;
        for (value, label) in items {
            if tab(ui, *current == *value, label).clicked() && *current != *value {
                *current = *value;
                changed = true;
            }
        }
    });
    let rect = response.response.rect;
    ui.painter().hline(
        rect.left()..=ui.max_rect().right(),
        rect.bottom(),
        Stroke::new(1.0_f32, TAB_LINE),
    );
    changed
}
