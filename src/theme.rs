use crate::models::TaskStatus;
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style, TextStyle, Vec2,
    Visuals,
};

// Swiss architectural whiteboard palette (OLED friendly, warm paper).
/// Warm architectural off-white paper canvas.
pub const CANVAS: Color32 = Color32::from_rgb(0xFA, 0xF9, 0xF6);
/// Pure crisp white task cards.
pub const PANE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
/// Deep charcoal grid dividers.
pub const GRID: Color32 = Color32::from_rgb(0x1E, 0x29, 0x3B);
/// Primary ink (titles, strong borders).
pub const INK: Color32 = Color32::from_rgb(0x0F, 0x17, 0x2A);
/// Secondary ink for nested branches.
pub const INK_SOFT: Color32 = Color32::from_rgb(0x33, 0x41, 0x55);
/// Muted slate (secondary text, ink threads).
pub const SLATE: Color32 = Color32::from_rgb(0x64, 0x74, 0x8B);
/// Quiet gray (tertiary text, inline prompts).
pub const MUTED: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);
/// Hairline divider color (#94A3B8 architectural rule).
pub const DIVIDER: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);
pub const INPUT_BG: Color32 = PANE;

pub const TEXT: Color32 = INK;
pub const TEXT_MUTED: Color32 = SLATE;

// Tri-state architectural shadow palette (solid hard offset shadows).
pub const AMBER_INK: Color32 = Color32::from_rgb(0xD9, 0x77, 0x06);
pub const GREEN_BAR: Color32 = Color32::from_rgb(0x16, 0xA3, 0x4A);
pub const RED_BAR: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26);

/// Hard architectural shadow colors.
pub const AMBER_SHADOW: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
pub const GREEN_SHADOW: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
pub const RED_SHADOW: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);

/// Neutral fill for selected rows / widgets.
pub const SEL_FILL: Color32 = Color32::from_rgb(0xF1, 0xF5, 0xF9);

/// Subtle architectural paper fill for embedded note drawers & folded pane rails (#F8FAFC).
pub const DRAWER_BG: Color32 = Color32::from_rgb(0xF8, 0xFA, 0xFC);

/// Faint placeholder/hint color for empty inputs.
pub const HINT: Color32 = Color32::from_rgb(0xCB, 0xD5, 0xE1);

/// Subtle light slate rule (#E2E8F0) for pane column splits and bottom input strip border.
pub const SUBTLE_RULE: Color32 = Color32::from_rgb(0xE2, 0xE8, 0xF0);

pub const CARD_RADIUS: u8 = 0;
/// Thickness of the black tiling grid lines.
pub const GRID_LINE: f32 = 2.0;
/// Thickness of the hairline chrome borders (top bar, bottom bar, cards).
pub const HAIRLINE: f32 = 1.0;
pub const PADDING: i8 = 16;

/// Solid hard-shadow color for the given task status.
pub fn status_shadow_color(status: TaskStatus) -> Color32 {
    match status {
        TaskStatus::Active => AMBER_SHADOW,
        TaskStatus::Completed => GREEN_SHADOW,
        TaskStatus::Cancelled => RED_SHADOW,
    }
}

pub fn install(ctx: &egui::Context) {
    let mut visuals = Visuals::light();

    // Placeholder hints (empty inputs) use a very faint gray.
    visuals.weak_text_color = Some(HINT);
    visuals.panel_fill = CANVAS;
    visuals.window_fill = PANE;
    visuals.window_stroke = Stroke::new(HAIRLINE, GRID);
    visuals.window_corner_radius = CornerRadius::same(CARD_RADIUS);
    visuals.window_shadow = Shadow::NONE;
    visuals.popup_shadow = Shadow::NONE;
    visuals.menu_corner_radius = CornerRadius::same(CARD_RADIUS);
    visuals.faint_bg_color = Color32::from_rgb(0xF4, 0xF4, 0xF2);
    visuals.extreme_bg_color = PANE;
    visuals.text_edit_bg_color = Some(INPUT_BG);
    visuals.code_bg_color = Color32::from_rgb(0xF4, 0xF4, 0xF2);
    visuals.warn_fg_color = AMBER_INK;
    visuals.error_fg_color = RED_BAR;
    visuals.hyperlink_color = GRID;
    visuals.button_frame = true;
    visuals.collapsing_header_frame = false;
    visuals.striped = false;

    visuals.selection.bg_fill = SEL_FILL;
    visuals.selection.stroke = Stroke::new(1.0, GRID);

    let radius = CornerRadius::same(CARD_RADIUS);

    visuals.widgets.noninteractive.bg_fill = PANE;
    visuals.widgets.noninteractive.weak_bg_fill = PANE;
    visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.0, Color32::from_rgb(0xE3, 0xE3, 0xDF));
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.noninteractive.expansion = 0.0;

    visuals.widgets.inactive.bg_fill = PANE;
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xF1, 0xF1, 0xEE);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0xD6, 0xD6, 0xD1));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = radius;

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0xF1, 0xF1, 0xEE);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xF1, 0xF1, 0xEE);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, GRID);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, GRID);
    visuals.widgets.hovered.corner_radius = radius;

    visuals.widgets.active.bg_fill = Color32::from_rgb(0xE7, 0xE7, 0xE3);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(0xE7, 0xE7, 0xE3);
    visuals.widgets.active.bg_stroke = Stroke::new(GRID_LINE, GRID);
    visuals.widgets.active.fg_stroke = Stroke::new(2.0, GRID);
    visuals.widgets.active.corner_radius = radius;

    visuals.widgets.open.bg_fill = Color32::from_rgb(0xF1, 0xF1, 0xEE);
    visuals.widgets.open.weak_bg_fill = Color32::from_rgb(0xF1, 0xF1, 0xEE);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, GRID);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.corner_radius = radius;

    ctx.all_styles_mut(move |style: &mut Style| {
        style.visuals = visuals.clone();
        // Prevent non-editable labels from triggering the I-beam text cursor on hover.
        style.interaction.selectable_labels = false;
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        style.spacing.window_margin = Margin::same(PADDING);
        style.spacing.menu_margin = Margin::same(10);
        style.spacing.indent = 18.0;
        style.text_styles = [
            (TextStyle::Heading, FontId::new(18.0, FontFamily::Monospace)),
            (
                TextStyle::Name("PaneTitle".into()),
                FontId::new(20.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(15.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Monospace)),
            (TextStyle::Small, FontId::new(12.5, FontFamily::Monospace)),
            (
                TextStyle::Monospace,
                FontId::new(14.0, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}
