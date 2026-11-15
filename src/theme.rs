use crate::models::TaskStatus;
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style, TextStyle, Vec2,
    Visuals,
};

// Modern obsidian palette.
pub const CANVAS: Color32 = Color32::from_rgb(0x0F, 0x11, 0x17);
pub const CARD: Color32 = Color32::from_rgb(0x18, 0x1B, 0x24);
pub const CARD_ALT: Color32 = Color32::from_rgb(0x1E, 0x22, 0x2D);
pub const BORDER: Color32 = Color32::from_rgb(0x2A, 0x2F, 0x3D);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(0x22, 0x26, 0x31);
pub const INPUT_BG: Color32 = Color32::from_rgb(0x12, 0x15, 0x1D);

pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEE);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x94, 0x9B, 0xAA);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x5C, 0x63, 0x74);

pub const AMBER: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
pub const EMERALD: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
pub const CORAL: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);
pub const SLATE: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);

pub const CARD_RADIUS: u8 = 10;
pub const PADDING: i8 = 14;

pub fn blend(base: Color32, overlay: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t).round() as u8;
    Color32::from_rgb(
        mix(base.r(), overlay.r()),
        mix(base.g(), overlay.g()),
        mix(base.b(), overlay.b()),
    )
}

pub fn status_color(status: TaskStatus) -> Color32 {
    match status {
        TaskStatus::Default => SLATE,
        TaskStatus::Yellow => AMBER,
        TaskStatus::Green => EMERALD,
        TaskStatus::Red => CORAL,
    }
}

/// Accent used for text of the given status.
pub fn status_text_color(status: TaskStatus) -> Color32 {
    match status {
        TaskStatus::Default => TEXT,
        _ => status_color(status),
    }
}

/// Vivid but subtle tinted card surface.
pub fn card_fill(status: TaskStatus) -> Color32 {
    match status {
        TaskStatus::Default => CARD,
        TaskStatus::Yellow => blend(CARD, AMBER, 0.07),
        TaskStatus::Green => blend(CARD, EMERALD, 0.06),
        TaskStatus::Red => blend(CARD, CORAL, 0.07),
    }
}

pub fn card_border(status: TaskStatus, selected: bool) -> Color32 {
    if selected {
        AMBER
    } else {
        match status {
            TaskStatus::Default => BORDER,
            _ => blend(BORDER, status_color(status), 0.55),
        }
    }
}

/// Tinted background for a tree row.
pub fn row_fill(status: TaskStatus, selected: bool) -> Color32 {
    if selected {
        return blend(CARD_ALT, AMBER, 0.16);
    }
    match status {
        TaskStatus::Default => Color32::from_rgb(0x15, 0x18, 0x21),
        TaskStatus::Yellow => blend(CARD, AMBER, 0.12),
        TaskStatus::Green => blend(CARD, EMERALD, 0.11),
        TaskStatus::Red => blend(CARD, CORAL, 0.12),
    }
}

pub fn row_border(status: TaskStatus, selected: bool) -> Color32 {
    if selected {
        AMBER
    } else {
        let strength = if status == TaskStatus::Default {
            0.20
        } else {
            0.45
        };
        blend(row_fill(status, false), status_color(status), strength)
    }
}

pub fn chip_fill(status: TaskStatus) -> Color32 {
    match status {
        TaskStatus::Default => Color32::from_rgb(0x22, 0x26, 0x31),
        _ => blend(CARD, status_color(status), 0.22),
    }
}

pub fn install(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = CANVAS;
    visuals.window_fill = CARD;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.window_shadow = Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(160),
    };
    visuals.popup_shadow = visuals.window_shadow;
    visuals.menu_corner_radius = CornerRadius::same(CARD_RADIUS);
    visuals.faint_bg_color = Color32::from_rgb(0x13, 0x16, 0x1E);
    visuals.extreme_bg_color = INPUT_BG;
    visuals.text_edit_bg_color = Some(INPUT_BG);
    visuals.code_bg_color = INPUT_BG;
    visuals.warn_fg_color = AMBER;
    visuals.error_fg_color = CORAL;
    visuals.hyperlink_color = AMBER;
    visuals.button_frame = true;
    visuals.collapsing_header_frame = false;
    visuals.striped = false;

    visuals.selection.bg_fill = blend(CARD, AMBER, 0.30);
    visuals.selection.stroke = Stroke::new(1.0, AMBER);

    let widget_radius = CornerRadius::same(8);

    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.weak_bg_fill = CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.corner_radius = widget_radius;
    visuals.widgets.noninteractive.expansion = 0.0;

    visuals.widgets.inactive.bg_fill = CARD_ALT;
    visuals.widgets.inactive.weak_bg_fill = CARD_ALT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_SOFT);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = widget_radius;

    visuals.widgets.hovered.bg_fill = blend(CARD_ALT, AMBER, 0.12);
    visuals.widgets.hovered.weak_bg_fill = blend(CARD_ALT, AMBER, 0.12);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, blend(BORDER, AMBER, 0.45));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, AMBER);
    visuals.widgets.hovered.corner_radius = widget_radius;

    visuals.widgets.active.bg_fill = blend(CARD_ALT, AMBER, 0.20);
    visuals.widgets.active.weak_bg_fill = blend(CARD_ALT, AMBER, 0.20);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, AMBER);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, AMBER);
    visuals.widgets.active.corner_radius = widget_radius;

    visuals.widgets.open.bg_fill = CARD_ALT;
    visuals.widgets.open.weak_bg_fill = CARD_ALT;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, AMBER);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.corner_radius = widget_radius;

    ctx.all_styles_mut(move |style: &mut Style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        style.spacing.window_margin = Margin::same(PADDING);
        style.spacing.menu_margin = Margin::same(10);
        style.spacing.indent = 18.0;
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(22.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Name("PaneTitle".into()),
                FontId::new(18.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(14.0, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}
