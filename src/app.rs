use crate::models::{AppData, MainTaskPane, TaskNode, TaskStatus};
use crate::rtl_text::shape_arabic;
use crate::storage::save_app_data;
use crate::theme;
use egui::{
    pos2, vec2, Align, Button, Color32, CornerRadius, Frame, Galley, Id, Key, Label, Layout, Margin,
    Pos2, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, Vec2,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct WhiteboardApp {
    pub data: AppData,
    pub show_quick_entry_modal: bool,
    pub quick_entry_input: String,

    pub selected_node_ids: HashSet<String>,
    pub selected_pane_ids: HashSet<String>,

    pub focused_node_id: Option<String>,
    pub editing_node_id: Option<String>,

    /// Node that should receive keyboard focus on the next frame (newly created).
    pub next_focus_node_id: Option<String>,

    /// Per-pane inline "add sub-task" draft text.
    pub pane_drafts: HashMap<String, String>,
    /// Pane whose inline add-sub-task input should grab focus on the next frame.
    pub pane_draft_focus: Option<String>,

    /// When set, the canvas shows a single maximized pane (focus mode).
    pub focused_pane_id: Option<String>,

    pub is_lassoing: bool,
    pub lasso_start: Option<Pos2>,
    pub lasso_current: Option<Pos2>,

    pub active_mode: AppMode,
    pub dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    LiveBoard,
    Harbor,
}

impl WhiteboardApp {
    pub fn new(cc: &eframe::CreationContext<'_>, data: AppData) -> Self {
        crate::rtl_text::install_arabic_fonts(&cc.egui_ctx);
        theme::install(&cc.egui_ctx);
        Self::from_data(data)
    }

    pub fn from_data(data: AppData) -> Self {
        let show_quick_entry_modal = data.current_day.panes.is_empty();

        Self {
            data,
            show_quick_entry_modal,
            quick_entry_input: String::new(),
            selected_node_ids: HashSet::new(),
            selected_pane_ids: HashSet::new(),
            focused_node_id: None,
            editing_node_id: None,
            next_focus_node_id: None,
            pane_drafts: HashMap::new(),
            pane_draft_focus: None,
            focused_pane_id: None,
            is_lassoing: false,
            lasso_start: None,
            lasso_current: None,
            active_mode: AppMode::LiveBoard,
            dirty: false,
        }
    }

    pub fn auto_save(&mut self) {
        if self.dirty {
            if let Err(e) = save_app_data(&self.data) {
                eprintln!("Error saving data: {}", e);
            } else {
                self.dirty = false;
            }
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn set_selected_nodes_status(&mut self, status: TaskStatus) {
        if self.selected_node_ids.is_empty() && self.selected_pane_ids.is_empty() {
            return;
        }

        for pane in &mut self.data.current_day.panes {
            if self.selected_pane_ids.contains(&pane.id) {
                pane.status = status;
            }
            set_status_in_nodes(&mut pane.subtasks, &self.selected_node_ids, status);
        }
        self.mark_dirty();
    }

    pub fn process_quick_entry(&mut self) {
        let input = self.quick_entry_input.trim();
        if input.is_empty() {
            self.show_quick_entry_modal = false;
            return;
        }

        let raw_tasks: Vec<&str> = input
            .split(&['/', '\n'][..])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for title in raw_tasks {
            self.data.current_day.panes.push(MainTaskPane::new(title));
        }

        self.quick_entry_input.clear();
        self.show_quick_entry_modal = false;
        self.mark_dirty();
    }

    // ---------------------------------------------------------------------
    // Keyboard outlining
    // ---------------------------------------------------------------------

    fn handle_keyboard_navigation(&mut self, ctx: &egui::Context) {
        if self.show_quick_entry_modal || ctx.wants_keyboard_input() {
            return;
        }
        let Some(focused) = self.focused_node_id.clone() else {
            return;
        };

        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        let tab = ctx.input(|i| i.key_pressed(Key::Tab));
        let shift = ctx.input(|i| i.modifiers.shift);
        let delete = ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace));

        if enter {
            self.add_sibling_after(&focused);
        } else if tab && !shift {
            self.add_child_under(&focused);
        } else if tab && shift {
            self.outdent_node(&focused);
        } else if delete {
            self.delete_node(&focused);
        }
    }

    fn add_sibling_after(&mut self, target_id: &str) -> Option<String> {
        let new_node = TaskNode::new("");
        let new_id = new_node.id.clone();
        for pane in &mut self.data.current_day.panes {
            if insert_sibling_in_nodes(&mut pane.subtasks, target_id, new_node.clone()) {
                return self.begin_editing(new_id);
            }
        }
        None
    }

    fn add_child_under(&mut self, parent_id: &str) -> Option<String> {
        let new_node = TaskNode::new("");
        let new_id = new_node.id.clone();
        for pane in &mut self.data.current_day.panes {
            if insert_child_recursive(&mut pane.subtasks, parent_id, new_node.clone()) {
                return self.begin_editing(new_id);
            }
        }
        None
    }

    fn outdent_node(&mut self, target_id: &str) -> Option<String> {
        for pane in &mut self.data.current_day.panes {
            if outdent_recursive(&mut pane.subtasks, target_id) {
                self.mark_dirty();
                self.focused_node_id = Some(target_id.to_string());
                return Some(target_id.to_string());
            }
        }
        None
    }

    fn delete_node(&mut self, target_id: &str) -> Option<String> {
        for pane in &mut self.data.current_day.panes {
            if delete_node_in_nodes(&mut pane.subtasks, target_id) {
                self.selected_node_ids.remove(target_id);
                if self.focused_node_id.as_deref() == Some(target_id) {
                    self.focused_node_id = None;
                }
                if self.editing_node_id.as_deref() == Some(target_id) {
                    self.editing_node_id = None;
                }
                self.mark_dirty();
                return Some(target_id.to_string());
            }
        }
        None
    }

    fn begin_editing(&mut self, node_id: String) -> Option<String> {
        self.focused_node_id = Some(node_id.clone());
        self.editing_node_id = Some(node_id.clone());
        self.next_focus_node_id = Some(node_id.clone());
        self.mark_dirty();
        Some(node_id)
    }

    fn apply_lasso(
        &mut self,
        lasso: Rect,
        node_rects: &[(String, Rect)],
        pane_header_rects: &[(String, Rect)],
    ) {
        if lasso.width() < 4.0 && lasso.height() < 4.0 {
            return;
        }

        self.selected_node_ids.clear();
        self.selected_pane_ids.clear();

        for (id, rect) in node_rects {
            if lasso.intersects(*rect) {
                self.selected_node_ids.insert(id.clone());
            }
        }
        for (id, rect) in pane_header_rects {
            if lasso.intersects(*rect) {
                self.selected_pane_ids.insert(id.clone());
            }
        }
    }
}

impl eframe::App for WhiteboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

impl WhiteboardApp {
    /// Draws the whole application. Separated from [`eframe::App::update`] so it
    /// can be exercised headlessly in tests.
    pub fn draw(&mut self, ctx: &egui::Context) {
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(Key::Num1)) {
                self.set_selected_nodes_status(TaskStatus::Yellow);
            } else if ctx.input(|i| i.key_pressed(Key::Num2)) {
                self.set_selected_nodes_status(TaskStatus::Green);
            } else if ctx.input(|i| i.key_pressed(Key::Num3)) {
                self.set_selected_nodes_status(TaskStatus::Red);
            } else if ctx.input(|i| i.key_pressed(Key::Num0)) {
                self.set_selected_nodes_status(TaskStatus::Default);
            } else if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.selected_node_ids.clear();
                self.selected_pane_ids.clear();
                self.focused_pane_id = None;
            }
        }

        self.handle_keyboard_navigation(ctx);

        egui::TopBottomPanel::top("top_bar")
            .frame(
                Frame::new()
                    .fill(theme::CARD)
                    .inner_margin(Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| {
                self.render_top_bar(ui);
            });

        egui::TopBottomPanel::bottom("bottom_bar")
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(0x12, 0x15, 0x1D))
                    .inner_margin(Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                self.render_bottom_bar(ui);
            });

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(theme::CANVAS)
                    .inner_margin(Margin::same(18)),
            )
            .show(ctx, |ui| match self.active_mode {
                AppMode::LiveBoard => {
                    if self.show_quick_entry_modal {
                        self.render_quick_entry_modal(ctx);
                    } else {
                        self.render_daily_canvas(ui);
                    }
                }
                AppMode::Harbor => self.render_harbor_placeholder(ui),
            });

        if self.dirty {
            self.auto_save();
        }
    }
}

impl WhiteboardApp {
    fn render_top_bar(&mut self, ui: &mut Ui) {
        rtl_row(ui, 30.0, |ui| {
            ui.label(
                RichText::new(shape_arabic("السبورة الذكية"))
                    .size(20.0)
                    .strong()
                    .color(theme::TEXT),
            );
            ui.label(
                RichText::new("WHITEBOARD TRACKER")
                    .size(10.0)
                    .color(theme::AMBER)
                    .strong(),
            );

            ui.add_space(18.0);

            let live = mode_tab(ui, self.active_mode == AppMode::LiveBoard, "السبورة الحية");
            if live.clicked() {
                self.active_mode = AppMode::LiveBoard;
            }
            let harbor = mode_tab(ui, self.active_mode == AppMode::Harbor, "ميناء التقارير");
            if harbor.clicked() {
                self.active_mode = AppMode::Harbor;
            }

            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let quick = pill_button(ui, "+ مهمة سريعة", theme::AMBER, false);
                if quick.clicked() {
                    self.show_quick_entry_modal = true;
                }

                let selected_count = self.selected_node_ids.len() + self.selected_pane_ids.len();
                if selected_count > 0 {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(shape_arabic(&format!("محدد: {}", selected_count)))
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                    color_chip(ui, "1", TaskStatus::Yellow);
                    color_chip(ui, "2", TaskStatus::Green);
                    color_chip(ui, "3", TaskStatus::Red);
                    color_chip(ui, "0", TaskStatus::Default);

                    if ui
                        .add(
                            Button::new(
                                RichText::new(shape_arabic("إلغاء"))
                                    .size(12.0)
                                    .color(theme::TEXT_MUTED),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.selected_node_ids.clear();
                        self.selected_pane_ids.clear();
                    }
                }
            });
        });
    }

    fn render_bottom_bar(&mut self, ui: &mut Ui) {
        let counts = self.data.current_day.calculate_counts();
        let rate = counts.completion_rate();

        rtl_row(ui, 30.0, |ui| {
            ui.label(
                RichText::new(shape_arabic(&format!("نسبة الإنجاز {:.1}%", rate)))
                    .size(13.0)
                    .strong()
                    .color(theme::EMERALD),
            );

            let bar = egui::ProgressBar::new((rate / 100.0).clamp(0.0, 1.0))
                .desired_width(140.0)
                .fill(theme::EMERALD)
                .text("");
            ui.add(bar);

            ui.add_space(18.0);

            count_chip(ui, counts.green_count, TaskStatus::Green, "مكتمل");
            count_chip(ui, counts.yellow_count, TaskStatus::Yellow, "قيد التنفيذ");
            count_chip(ui, counts.red_count, TaskStatus::Red, "ملغى");
            count_chip(ui, counts.default_count, TaskStatus::Default, "مخطط");
            count_chip(ui, counts.total, TaskStatus::Default, "إجمالي");

            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                if self.dirty {
                    ui.label(
                        RichText::new(shape_arabic("جاري الحفظ..."))
                            .size(11.0)
                            .color(theme::AMBER),
                    );
                } else {
                    ui.label(
                        RichText::new(shape_arabic("محفوظ تلقائياً"))
                            .size(11.0)
                            .color(theme::TEXT_FAINT),
                    );
                }
            });
        });
    }

    fn render_quick_entry_modal(&mut self, ctx: &egui::Context) {
        egui::Window::new(shape_arabic("إدخال المهام السريع"))
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                    ui.label(
                        RichText::new(shape_arabic(&self.data.current_day.display_title))
                            .size(19.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.label(
                        RichText::new(shape_arabic(
                            "أدخل المهام الرئيسية مقسمة بشرطة مائلة (/) أو بأسطر جديدة",
                        ))
                        .size(13.0)
                        .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(10.0);

                    let mut layouter = rtl_layouter(15.0, true);
                    let edit = TextEdit::multiline(&mut self.quick_entry_input)
                        .id(Id::new("quick_entry_input"))
                        .desired_rows(4)
                        .hint_text(shape_arabic("علاج بابا / كتابة اللائحة للمرافعة"))
                        .desired_width(f32::INFINITY)
                        .margin(Margin::same(10))
                        .background_color(theme::INPUT_BG)
                        .text_color(theme::TEXT)
                        .horizontal_align(Align::Max)
                        .layouter(&mut layouter);
                    let response = ui.add(edit);
                    if self.quick_entry_input.is_empty() {
                        response.request_focus();
                    }

                    ui.add_space(12.0);

                    let raw_tasks: Vec<&str> = self
                        .quick_entry_input
                        .split(&['/', '\n'][..])
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();

                    if !raw_tasks.is_empty() {
                        ui.label(
                            RichText::new(shape_arabic(&format!(
                                "معاينة الألواح ({})",
                                raw_tasks.len()
                            )))
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(4.0);
                        ScrollArea::vertical().max_height(130.0).show(ui, |ui| {
                            for task in &raw_tasks {
                                rtl_row(ui, 20.0, |ui| {
                                    ui.label(
                                        RichText::new(shape_arabic(task))
                                            .size(14.0)
                                            .color(theme::TEXT),
                                    );
                                    let (rect, _) =
                                        ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 3.5, theme::SLATE);
                                });
                            }
                        });
                        ui.add_space(8.0);
                    }

                    ui.add_space(6.0);
                    rtl_row(ui, 34.0, |ui| {
                        let submit = pill_button(ui, "إنشاء السبورة", theme::AMBER, true);
                        if submit.clicked() {
                            self.process_quick_entry();
                        }
                        if !self.data.current_day.panes.is_empty()
                            && ui
                                .add(
                                    Button::new(
                                        RichText::new(shape_arabic("إلغاء"))
                                            .color(theme::TEXT_MUTED),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                        {
                            self.show_quick_entry_modal = false;
                        }
                    });
                });
            });
    }

    fn render_harbor_placeholder(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            Frame::new()
                .fill(theme::CARD)
                .stroke(Stroke::new(1.0, theme::BORDER))
                .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
                .inner_margin(Margin::same(28))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(shape_arabic("ميناء التقارير"))
                            .size(24.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(shape_arabic(
                            "سيتم تفعيل التقارير والحفظ الدائم في المرحلة الرابعة.",
                        ))
                        .color(theme::TEXT_MUTED),
                    );
                });
        });
    }

    fn render_daily_canvas(&mut self, ui: &mut Ui) {
        let panes_count = self.data.current_day.panes.len();
        if panes_count == 0 {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                Frame::new()
                    .fill(theme::CARD)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
                    .inner_margin(Margin::same(28))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(shape_arabic(&self.data.current_day.display_title))
                                .size(20.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(shape_arabic("لا توجد مهام لليوم بعد"))
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(14.0);
                        if pill_button(ui, "+ إضافة مهام جديدة", theme::AMBER, true).clicked()
                        {
                            self.show_quick_entry_modal = true;
                        }
                    });
            });
            return;
        }

        rtl_row(ui, 28.0, |ui| {
            ui.label(
                RichText::new(shape_arabic(&self.data.current_day.display_title))
                    .size(20.0)
                    .strong()
                    .color(theme::TEXT),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new(shape_arabic(&format!("{} ألواح", panes_count)))
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        });
        ui.add_space(8.0);

        let mut actions = PaneActions::default();
        let mut request_focus_id = self.next_focus_node_id.take();
        let mut draft_focus = self.pane_draft_focus.take();

        let mut finish_lasso = false;
        let viewport_height = ui.available_height();

        // Focus-mode switcher pill bar.
        let mut focus_switch: Option<Option<String>> = None;
        if let Some(active_id) = self.focused_pane_id.clone() {
            rtl_row(ui, 28.0, |ui| {
                if pill_button(ui, "عرض الكل", theme::TEXT_MUTED, false).clicked() {
                    focus_switch = Some(None);
                }
                ui.add_space(6.0);
                for pane in self.data.current_day.panes.iter() {
                    let active = pane.id == active_id;
                    if focus_pill(ui, &pane.title, active).clicked() {
                        focus_switch = Some(if active { None } else { Some(pane.id.clone()) });
                    }
                }
            });
            ui.add_space(8.0);
        }
        if let Some(new_focus) = focus_switch {
            self.focused_pane_id = new_focus;
        }

        let focused_idx = self
            .focused_pane_id
            .as_ref()
            .and_then(|id| self.data.current_day.panes.iter().position(|p| &p.id == id));

        ScrollArea::vertical()
            .id_salt("canvas_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = ui.available_width();
                let (placements, content_height) = if let Some(idx) = focused_idx {
                    let pane = &self.data.current_day.panes[idx];
                    let height =
                        cluster_natural_height(pane.subtasks.len()).max(viewport_height);
                    (
                        vec![(
                            idx,
                            TilePlacement {
                                pos: Pos2::ZERO,
                                size: Vec2::new(width, height),
                            },
                        )],
                        height,
                    )
                } else {
                    let mut tiles = compute_masonry_layout(&self.data.current_day.panes, width);
                    for tile in &mut tiles {
                        tile.size.y = tile.size.y.max(viewport_height);
                    }
                    let height = tiles
                        .iter()
                        .map(|tile| tile.pos.y + tile.size.y)
                        .fold(0.0, f32::max);
                    let placements = tiles.into_iter().enumerate().collect();
                    (placements, height)
                };

                let canvas_height = content_height.max(viewport_height);
                let (canvas_rect, canvas_response) =
                    ui.allocate_exact_size(Vec2::new(width, canvas_height), Sense::drag());

                if canvas_response.drag_started() && !self.is_lassoing {
                    if let Some(pos) = canvas_response.interact_pointer_pos() {
                        self.is_lassoing = true;
                        self.lasso_start = Some(pos);
                        self.lasso_current = Some(pos);
                    }
                }

                if self.is_lassoing {
                    if let Some(pos) = ctx_pointer_pos(ui) {
                        self.lasso_current = Some(pos);
                    }
                    if canvas_response.drag_stopped()
                        || ui.input(|i| i.pointer.any_released())
                    {
                        finish_lasso = true;
                    }
                }

                let origin = canvas_rect.min;
                for (pane_idx, tile) in placements {
                    let pane_rect = Rect::from_min_size(origin + tile.pos.to_vec2(), tile.size);
                    let pane = &mut self.data.current_day.panes[pane_idx];
                    let is_selected = self.selected_pane_ids.contains(&pane.id);
                    let draft = self.pane_drafts.entry(pane.id.clone()).or_default();
                    let focus_here = draft_focus.as_deref() == Some(pane.id.as_str());
                    render_mind_map(
                        ui,
                        pane_rect,
                        pane,
                        pane_idx,
                        is_selected,
                        draft,
                        focus_here,
                        &mut self.focused_pane_id,
                        &mut self.focused_node_id,
                        &mut self.editing_node_id,
                        &mut request_focus_id,
                        &mut self.selected_node_ids,
                        &mut self.selected_pane_ids,
                        &mut actions,
                    );
                    if focus_here {
                        draft_focus = None;
                    }
                }

                if self.is_lassoing {
                    if let (Some(start), Some(curr)) = (self.lasso_start, self.lasso_current) {
                        let rect = Rect::from_two_pos(start, curr);
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(2),
                            Color32::from_rgba_unmultiplied(0xF5, 0x9E, 0x0B, 34),
                        );
                        ui.painter().rect_stroke(
                            rect,
                            CornerRadius::same(2),
                            Stroke::new(1.2, theme::AMBER),
                            StrokeKind::Outside,
                        );

                        if finish_lasso {
                            self.apply_lasso(rect, &actions.node_rects, &actions.pane_header_rects);
                        }
                    }
                }
            });

        if actions.dirty {
            self.dirty = true;
        }

        if let Some(idx) = actions.delete_pane {
            if idx < self.data.current_day.panes.len() {
                let removed = self.data.current_day.panes.remove(idx);
                self.pane_drafts.remove(&removed.id);
                self.selected_pane_ids.remove(&removed.id);
                if self.focused_pane_id.as_deref() == Some(removed.id.as_str()) {
                    self.focused_pane_id = None;
                }
            }
            self.mark_dirty();
        }

        if let Some(target_id) = actions.add_sibling {
            self.add_sibling_after(&target_id);
        }
        if let Some(target_id) = actions.add_child {
            self.add_child_under(&target_id);
        }
        if let Some(target_id) = actions.outdent {
            self.outdent_node(&target_id);
        }
        if let Some(target_id) = actions.delete_node {
            self.delete_node(&target_id);
        }
        if actions.draft_focus.is_some() {
            self.pane_draft_focus = actions.draft_focus;
        }

        if finish_lasso {
            self.is_lassoing = false;
            self.lasso_start = None;
            self.lasso_current = None;
        }
    }
}

#[derive(Default)]
struct PaneActions {
    delete_pane: Option<usize>,
    add_sibling: Option<String>,
    add_child: Option<String>,
    outdent: Option<String>,
    delete_node: Option<String>,
    node_rects: Vec<(String, Rect)>,
    pane_header_rects: Vec<(String, Rect)>,
    dirty: bool,
    draft_focus: Option<String>,
}

/// Placement of a pane on the tiled canvas, in canvas-local coordinates.
#[derive(Debug, Clone, Copy)]
struct TilePlacement {
    pos: Pos2,
    size: Vec2,
}

const TILE_GAP: f32 = 12.0;
const MIN_TILE_WIDTH: f32 = 260.0;

/// Approximate height needed to show a pane's header, inline input, and all of
/// its top-level sub-tasks without scrolling.
fn cluster_natural_height(subtask_count: usize) -> f32 {
    96.0 + subtask_count as f32 * 36.0
}

/// Packs panes into a balanced masonry of side-by-side columns, choosing the
/// number of columns from the available width (capped by the pane count).
fn compute_masonry_layout(panes: &[MainTaskPane], width: f32) -> Vec<TilePlacement> {
    if panes.is_empty() {
        return Vec::new();
    }

    let max_columns = panes.len().min(4).max(1);
    let columns = (((width + TILE_GAP) / MIN_TILE_WIDTH).floor() as usize).clamp(1, max_columns);
    let column_width =
        ((width - TILE_GAP * (columns.saturating_sub(1)) as f32) / columns as f32).max(60.0);

    let mut column_heights = vec![0.0_f32; columns];
    let mut placements = Vec::with_capacity(panes.len());
    for pane in panes {
        let column = (0..columns)
            .min_by(|&a, &b| {
                column_heights[a]
                    .partial_cmp(&column_heights[b])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0);
        let y = column_heights[column];
        let height = cluster_natural_height(pane.subtasks.len());
        placements.push(TilePlacement {
            pos: pos2(column as f32 * (column_width + TILE_GAP), y),
            size: vec2(column_width, height),
        });
        column_heights[column] = y + height + TILE_GAP;
    }
    placements
}

/// Draws one pane inside its slot on the tiled canvas.
#[allow(clippy::too_many_arguments)]
fn render_mind_map(
    ui: &mut Ui,
    rect: Rect,
    pane: &mut MainTaskPane,
    pane_idx: usize,
    is_selected: bool,
    draft: &mut String,
    draft_focus: bool,
    focused_pane_id: &mut Option<String>,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    selected_pane_ids: &mut HashSet<String>,
    actions: &mut PaneActions,
) {
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
    child.set_clip_rect(rect);
    render_pane(
        &mut child,
        pane,
        pane_idx,
        is_selected,
        draft,
        draft_focus,
        focused_pane_id,
        focused_node_id,
        editing_node_id,
        request_focus_id,
        selected_node_ids,
        selected_pane_ids,
        actions,
    );
}

/// Renders a single task pane. Shared by the tiled grid and focus mode.
#[allow(clippy::too_many_arguments)]
fn render_pane(
    ui: &mut Ui,
    pane: &mut MainTaskPane,
    pane_idx: usize,
    is_selected: bool,
    draft: &mut String,
    draft_focus: bool,
    focused_pane_id: &mut Option<String>,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    selected_pane_ids: &mut HashSet<String>,
    actions: &mut PaneActions,
) {
    let accent = theme::status_color(pane.status);
    let frame = Frame::new()
        .fill(theme::card_fill(pane.status))
        .stroke(Stroke::new(
            if is_selected { 2.0 } else { 1.0 },
            theme::card_border(pane.status, is_selected),
        ))
        .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
        .inner_margin(Margin::same(theme::PADDING));

    let frame_response = frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.set_min_height(ui.available_height());

        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            let mut focus_toggle: Option<bool> = None;

            rtl_row(ui, 30.0, |ui| {
                let title_width = (ui.available_width() - 132.0).max(60.0);
                let title_response = ui.add_sized(
                    Vec2::new(title_width, 28.0),
                    Label::new(
                        RichText::new(shape_arabic(&pane.title))
                            .size(18.0)
                            .strong()
                            .color(theme::TEXT),
                    )
                    .sense(Sense::click())
                    .truncate(),
                );
                actions
                    .pane_header_rects
                    .push((pane.id.clone(), title_response.rect));

                if title_response.double_clicked() {
                    focus_toggle = Some(true);
                } else if title_response.clicked() {
                    let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    if ctrl {
                        if selected_pane_ids.contains(&pane.id) {
                            selected_pane_ids.remove(&pane.id);
                        } else {
                            selected_pane_ids.insert(pane.id.clone());
                        }
                    } else {
                        focus_toggle = Some(focused_pane_id.as_deref() != Some(pane.id.as_str()));
                    }
                }
                title_response.on_hover_text(shape_arabic(
                    "نقرة: وضع التركيز، Ctrl+نقرة: تحديد اللوح، نقرة مزدوجة: تكبير",
                ));

                let status = status_chip(ui, pane.status);
                if status.clicked() {
                    pane.status = pane.status.next();
                    actions.dirty = true;
                }
                status.on_hover_text(shape_arabic("تغيير حالة اللوح"));

                let add = ui
                    .add(
                        Button::new(RichText::new("+").size(17.0).color(theme::AMBER))
                            .frame(false)
                            .min_size(Vec2::splat(24.0)),
                    )
                    .on_hover_text(shape_arabic("إضافة مهمة فرعية"));
                if add.clicked() {
                    actions.draft_focus = Some(pane.id.clone());
                }

                let delete = ui
                    .add(
                        Button::new(RichText::new("×").size(14.0).color(theme::TEXT_MUTED))
                            .frame(false)
                            .min_size(Vec2::splat(24.0)),
                    )
                    .on_hover_text(shape_arabic("حذف اللوح"));
                if delete.clicked() {
                    actions.delete_pane = Some(pane_idx);
                }
            });

            if let Some(focus) = focus_toggle {
                *focused_pane_id = if focus { Some(pane.id.clone()) } else { None };
            }

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // Scrollable tree above a pinned inline add-input.
            let scroll_height = (ui.available_height() - 46.0).max(40.0);
            ScrollArea::vertical()
                .id_salt(("pane_scroll", pane.id.as_str()))
                .auto_shrink([false, false])
                .max_height(scroll_height)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    render_task_tree(
                        ui,
                        &mut pane.subtasks,
                        0,
                        focused_node_id,
                        editing_node_id,
                        request_focus_id,
                        selected_node_ids,
                        actions,
                    );
                });
            ui.add_space(4.0);

            let submitted = subtask_input(ui, draft, &pane.id, draft_focus);
            if submitted {
                let title = draft.trim().to_string();
                if !title.is_empty() {
                    pane.subtasks.push(TaskNode::new(&title));
                    draft.clear();
                    actions.dirty = true;
                }
                actions.draft_focus = Some(pane.id.clone());
            }
        });
    });

    let rect = frame_response.response.rect;
    let bar = Rect::from_min_max(
        egui::pos2(rect.right() - 3.0, rect.top() + 12.0),
        egui::pos2(rect.right() - 0.5, rect.bottom() - 12.0),
    );
    ui.painter().rect_filled(bar, CornerRadius::same(2), accent);
}

/// Layouter that shapes Arabic but keeps the logical string untouched, so
/// `TextEdit` cursor indices remain character-aligned with the original text.
fn rtl_layouter(
    font_size: f32,
    multiline: bool,
) -> impl FnMut(&Ui, &dyn egui::TextBuffer, f32) -> Arc<Galley> {
    move |ui, text, wrap_width| {
        let mut job = egui::text::LayoutJob::default();
        if multiline {
            job.wrap.max_width = wrap_width;
        }
        job.append(
            &shape_arabic(text.as_str()),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(font_size),
                color: theme::TEXT,
                ..Default::default()
            },
        );
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    }
}

fn focus_pill(ui: &mut Ui, text: &str, active: bool) -> egui::Response {
    let (fill, fg, stroke) = if active {
        (
            theme::blend(theme::CARD_ALT, theme::AMBER, 0.25),
            theme::AMBER,
            Stroke::new(1.0, theme::AMBER),
        )
    } else {
        (Color32::TRANSPARENT, theme::TEXT_MUTED, Stroke::NONE)
    };
    Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.add(
                Label::new(
                    RichText::new(shape_arabic(text))
                        .size(12.0)
                        .color(fg)
                        .strong(),
                )
                .sense(Sense::click()),
            )
        })
        .inner
}

fn ctx_pointer_pos(ui: &Ui) -> Option<Pos2> {
    ui.input(|i| i.pointer.hover_pos())
}

fn mode_tab(ui: &mut Ui, active: bool, text: &str) -> egui::Response {
    let (fill, fg) = if active {
        (
            theme::blend(theme::CARD_ALT, theme::AMBER, 0.20),
            theme::AMBER,
        )
    } else {
        (Color32::TRANSPARENT, theme::TEXT_MUTED)
    };
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.add(
                Label::new(
                    RichText::new(shape_arabic(text))
                        .size(13.0)
                        .color(fg)
                        .strong(),
                )
                .sense(Sense::click()),
            )
        })
        .inner
}

/// A bounded-height right-to-left row.
///
/// Using `ui.with_layout(Layout::right_to_left(...))` directly inside a
/// vertical layout makes the row's cross-axis span the full available height
/// (the elements get vertically centered), which can swallow the whole canvas.
/// Giving the child UI an explicit height keeps rows content-sized.
fn rtl_row<R>(
    ui: &mut Ui,
    height: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let size = Vec2::new(ui.available_width(), height);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), add_contents)
}

fn pill_button(ui: &mut Ui, text: &str, color: Color32, filled: bool) -> egui::Response {
    let (fill, fg) = if filled {
        (color, Color32::from_rgb(0x14, 0x14, 0x14))
    } else {
        (Color32::TRANSPARENT, color)
    };
    let stroke = if filled {
        Stroke::NONE
    } else {
        Stroke::new(1.0, theme::blend(theme::BORDER, color, 0.55))
    };
    Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.add(
                Label::new(
                    RichText::new(shape_arabic(text))
                        .size(13.0)
                        .strong()
                        .color(fg),
                )
                .sense(Sense::click()),
            )
        })
        .inner
}

fn color_chip(ui: &mut Ui, key: &str, status: TaskStatus) {
    let color = theme::status_color(status);
    let fill = theme::chip_fill(status);
    let response = Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.add(
                Label::new(RichText::new(key).size(12.0).strong().color(color))
                    .sense(Sense::click()),
            )
        })
        .inner;
    let hint = format!("{} {}", status.label_ar(), status.label_en());
    response.on_hover_text(shape_arabic(&hint));
}

fn count_chip(ui: &mut Ui, value: usize, status: TaskStatus, label: &str) {
    let color = theme::status_color(status);
    Frame::new()
        .fill(theme::chip_fill(status))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(value.to_string())
                        .size(12.0)
                        .strong()
                        .color(color),
                );
                ui.label(
                    RichText::new(shape_arabic(label))
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
            });
        });
}

fn status_chip(ui: &mut Ui, status: TaskStatus) -> egui::Response {
    let color = theme::status_color(status);
    Frame::new()
        .fill(theme::chip_fill(status))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.add(
                Label::new(
                    RichText::new(shape_arabic(status.label_ar()))
                        .size(10.5)
                        .strong()
                        .color(color),
                )
                .sense(Sense::click()),
            )
        })
        .inner
}

/// Sticky inline input at the bottom of a pane. Returns true when submitted
/// (Enter pressed while focused, or the "+" button clicked) with non-empty text.
fn subtask_input(ui: &mut Ui, draft: &mut String, pane_id: &str, focus: bool) -> bool {
    let mut submitted = false;

    rtl_row(ui, 38.0, |ui| {
        let plus = ui
            .add(
                Button::new(RichText::new("+").size(17.0).color(theme::AMBER))
                    .frame(false)
                    .min_size(Vec2::splat(26.0)),
            )
            .on_hover_text(shape_arabic("إضافة مهمة فرعية"));

        let width = (ui.available_width() - 4.0).max(60.0);
        let mut layouter = rtl_layouter(15.0, false);
        let response = ui.add(
            TextEdit::singleline(draft)
                .id(Id::new(("pane_add", pane_id)))
                .hint_text(shape_arabic("+ إضافة مهمة فرعية"))
                .desired_width(width)
                .margin(Margin::symmetric(10, 7))
                .background_color(theme::INPUT_BG)
                .text_color(theme::TEXT)
                .horizontal_align(Align::Max)
                .layouter(&mut layouter),
        );

        if focus {
            response.request_focus();
        }

        let enter = (response.has_focus() || response.lost_focus())
            && ui.input(|i| i.key_pressed(Key::Enter));
        if (plus.clicked() || enter) && !draft.trim().is_empty() {
            submitted = true;
        }
    });

    submitted
}

#[allow(clippy::too_many_arguments)]
fn render_task_tree(
    ui: &mut Ui,
    nodes: &mut [TaskNode],
    depth: usize,
    focused_node_id: &mut Option<String>,
    editing_node_id: &mut Option<String>,
    request_focus_id: &mut Option<String>,
    selected_node_ids: &mut HashSet<String>,
    actions: &mut PaneActions,
) {
    let count = nodes.len();
    for (idx, node) in nodes.iter_mut().enumerate() {
        let is_last = idx + 1 == count;
        let connector = if is_last { "└─" } else { "├─" };

        let is_selected = selected_node_ids.contains(&node.id);
        let accent = theme::status_color(node.status);
        let fill = theme::row_fill(node.status, is_selected);
        let stroke = Stroke::new(
            if is_selected { 1.5 } else { 1.0 },
            theme::row_border(node.status, is_selected),
        );

        let frame = Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(7))
            .inner_margin(Margin::symmetric(8, 5));

        let inner = frame.show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(0.0));
            rtl_row(ui, 30.0, |ui| {
                if depth > 0 {
                    ui.add_space(depth as f32 * 16.0);
                    ui.label(RichText::new(connector).size(12.0).color(theme::TEXT_FAINT));
                    ui.add_space(2.0);
                }

                let (dot_rect, dot_response) =
                    ui.allocate_exact_size(Vec2::splat(10.0), Sense::click());
                ui.painter().circle_filled(dot_rect.center(), 4.0, accent);
                let dot_response = dot_response.on_hover_text(shape_arabic(&format!(
                    "{} / {} — انقر للتبديل",
                    node.status.label_ar(),
                    node.status.label_en()
                )));
                if dot_response.clicked() {
                    node.status = node.status.next();
                    actions.dirty = true;
                    *focused_node_id = Some(node.id.clone());
                }

                let is_editing = editing_node_id.as_deref() == Some(node.id.as_str());

                if is_editing {
                    let node_id = Id::new(&node.id);
                    let mut layouter = rtl_layouter(14.0, false);
                    let response = ui.add(
                        TextEdit::singleline(&mut node.title)
                            .id(node_id)
                            .desired_width(170.0)
                            .margin(Margin::symmetric(8, 5))
                            .background_color(theme::INPUT_BG)
                            .text_color(theme::TEXT)
                            .hint_text(shape_arabic("عنوان المهمة..."))
                            .horizontal_align(Align::Max)
                            .layouter(&mut layouter),
                    );

                    if response.changed() {
                        actions.dirty = true;
                    }

                    if request_focus_id.as_deref() == Some(node.id.as_str()) {
                        response.request_focus();
                        *request_focus_id = None;
                    }

                    let enter = (response.has_focus() || response.lost_focus())
                        && ui.input(|i| i.key_pressed(Key::Enter));

                    if response.lost_focus() || enter {
                        *editing_node_id = None;
                    }
                    if response.has_focus() && ui.input(|i| i.key_pressed(Key::Tab)) {
                        *editing_node_id = None;
                        if ui.input(|i| i.modifiers.shift) {
                            actions.outdent = Some(node.id.clone());
                            *focused_node_id = Some(node.id.clone());
                        } else {
                            actions.add_child = Some(node.id.clone());
                        }
                    }
                    if enter {
                        actions.add_sibling = Some(node.id.clone());
                    }
                } else {
                    let mut label = RichText::new(shape_arabic(&node.title))
                        .size(14.0)
                        .color(theme::status_text_color(node.status));
                    if node.status == TaskStatus::Red {
                        label = label.strikethrough();
                    }

                    let response = ui.selectable_label(is_selected, label);

                    if response.clicked() {
                        if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
                            if is_selected {
                                selected_node_ids.remove(&node.id);
                            } else {
                                selected_node_ids.insert(node.id.clone());
                            }
                        } else {
                            selected_node_ids.clear();
                            selected_node_ids.insert(node.id.clone());
                        }
                        *focused_node_id = Some(node.id.clone());
                    }

                    if response.double_clicked() {
                        *editing_node_id = Some(node.id.clone());
                        *request_focus_id = Some(node.id.clone());
                    }
                }

                let delete_response = ui
                    .add(
                        Button::new(RichText::new("×").size(13.0).color(theme::TEXT_MUTED))
                            .frame(false)
                            .min_size(Vec2::splat(22.0)),
                    )
                    .on_hover_text(shape_arabic("حذف"));
                if delete_response.clicked() {
                    actions.delete_node = Some(node.id.clone());
                }

                let add_response = ui
                    .add(
                        Button::new(RichText::new("+").size(14.0).color(theme::AMBER))
                            .frame(false)
                            .min_size(Vec2::splat(22.0)),
                    )
                    .on_hover_text(shape_arabic("إضافة مهمة فرعية"));
                if add_response.clicked() {
                    actions.add_child = Some(node.id.clone());
                }
            });
        });

        let row_rect = inner.response.rect;
        actions.node_rects.push((node.id.clone(), row_rect));

        let bar = Rect::from_min_max(
            egui::pos2(row_rect.right() - 3.0, row_rect.top() + 6.0),
            egui::pos2(row_rect.right() - 0.5, row_rect.bottom() - 6.0),
        );
        ui.painter().rect_filled(bar, CornerRadius::same(2), accent);

        ui.add_space(2.0);

        if node.is_expanded && !node.children.is_empty() {
            render_task_tree(
                ui,
                &mut node.children,
                depth + 1,
                focused_node_id,
                editing_node_id,
                request_focus_id,
                selected_node_ids,
                actions,
            );
        }
    }
}

fn set_status_in_nodes(nodes: &mut [TaskNode], target_ids: &HashSet<String>, status: TaskStatus) {
    for node in nodes {
        if target_ids.contains(&node.id) {
            node.status = status;
        }
        set_status_in_nodes(&mut node.children, target_ids, status);
    }
}

fn insert_child_recursive(nodes: &mut [TaskNode], parent_id: &str, new_node: TaskNode) -> bool {
    for node in nodes {
        if node.id == parent_id {
            node.children.push(new_node);
            node.is_expanded = true;
            return true;
        }
        if insert_child_recursive(&mut node.children, parent_id, new_node.clone()) {
            return true;
        }
    }
    false
}

fn insert_sibling_in_nodes(
    nodes: &mut Vec<TaskNode>,
    target_id: &str,
    new_node: TaskNode,
) -> bool {
    if let Some(pos) = nodes.iter().position(|n| n.id == target_id) {
        nodes.insert(pos + 1, new_node);
        return true;
    }
    for node in nodes.iter_mut() {
        if insert_sibling_in_nodes(&mut node.children, target_id, new_node.clone()) {
            return true;
        }
    }
    false
}

fn delete_node_in_nodes(nodes: &mut Vec<TaskNode>, target_id: &str) -> bool {
    if let Some(pos) = nodes.iter().position(|n| n.id == target_id) {
        nodes.remove(pos);
        return true;
    }
    for node in nodes.iter_mut() {
        if delete_node_in_nodes(&mut node.children, target_id) {
            return true;
        }
    }
    false
}

fn outdent_recursive(nodes: &mut Vec<TaskNode>, target_id: &str) -> bool {
    let mut found = None;
    for (i, node) in nodes.iter().enumerate() {
        if let Some(pos) = node.children.iter().position(|n| n.id == target_id) {
            found = Some((i, pos));
            break;
        }
    }
    if let Some((parent_idx, child_idx)) = found {
        let child = nodes[parent_idx].children.remove(child_idx);
        nodes.insert(parent_idx + 1, child);
        return true;
    }
    for node in nodes.iter_mut() {
        if outdent_recursive(&mut node.children, target_id) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> AppData {
        let mut data = AppData::default();
        let mut pane = MainTaskPane::new("علاج بابا");
        pane.status = TaskStatus::Yellow;

        let mut child = TaskNode::new("كتابة اللائحة للمرافعة");
        child.status = TaskStatus::Green;
        child.children.push(TaskNode::new("مراجعة مع المحامي"));

        let mut cancelled = TaskNode::new("التقديم على المناقصة");
        cancelled.status = TaskStatus::Red;
        pane.subtasks.push(child);
        pane.subtasks.push(cancelled);

        data.current_day.panes.push(pane);
        data.current_day.panes.push(MainTaskPane::new("مهمة ثانية"));
        data
    }

    #[test]
    fn renders_daily_tiled_ui_headless() {
        let ctx = egui::Context::default();
        crate::rtl_text::install_arabic_fonts(&ctx);
        crate::theme::install(&ctx);

        let mut app = WhiteboardApp::from_data(sample_data());
        app.show_quick_entry_modal = false;

        // Two frames so fonts and layout settle.
        let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
        let output = ctx.run(Default::default(), |ctx| app.draw(ctx));
        let shapes = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        assert!(!shapes.is_empty(), "expected the board to draw geometry");

        // Regression: pane and subtask text must actually be laid out.
        let text = rendered_text(&output);
        for expected in ["علاج بابا", "كتابة اللائحة للمرافعة", "مهمة ثانية"]
        {
            let shaped = crate::rtl_text::shape_arabic(expected);
            assert!(
                text.contains(&shaped),
                "missing rendered text {expected:?}; got {text:?}"
            );
        }
    }

    #[test]
    fn renders_focused_pane_headless() {
        let ctx = egui::Context::default();
        crate::rtl_text::install_arabic_fonts(&ctx);
        crate::theme::install(&ctx);

        let mut app = WhiteboardApp::from_data(sample_data());
        app.show_quick_entry_modal = false;
        app.focused_pane_id = Some(app.data.current_day.panes[0].id.clone());

        let _ = ctx.run(Default::default(), |ctx| app.draw(ctx));
        let output = ctx.run(Default::default(), |ctx| app.draw(ctx));
        let text = rendered_text(&output);
        assert!(
            text.contains(&crate::rtl_text::shape_arabic("علاج بابا")),
            "focused pane title not rendered"
        );
    }

    fn rendered_text(output: &egui::FullOutput) -> String {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn tree_helpers_insert_outdent_delete() {
        let mut nodes = vec![TaskNode::new("a")];
        let root_id = nodes[0].id.clone();

        let child = TaskNode::new("b");
        let child_id = child.id.clone();
        assert!(insert_child_recursive(&mut nodes, &root_id, child));
        assert_eq!(nodes[0].children.len(), 1);

        assert!(insert_sibling_in_nodes(
            &mut nodes,
            &child_id,
            TaskNode::new("c")
        ));
        assert_eq!(nodes[0].children.len(), 2);

        assert!(outdent_recursive(&mut nodes, &child_id));
        assert_eq!(nodes.len(), 2);

        assert!(delete_node_in_nodes(&mut nodes, &child_id));
        assert_eq!(nodes.len(), 1);
    }
}
