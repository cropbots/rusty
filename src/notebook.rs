use egui_macroquad::egui;
use khoron::Runtime as KhoronRuntime;
use macroquad::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::helpers::resolution_ui_scale;

pub const NOTEBOOK_TOGGLE_KEY: KeyCode = KeyCode::N;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotebookTab {
    Programming,
    PuzzleProgramming,
    Almanac,
}

pub struct ProgrammingNotebook {
    pub open: bool,
    pub tab: NotebookTab,
    pub right_panel_mode: RightPanelMode,
    pub program_text: String,
    pub puzzle_text: String,
    pub console_text: String,
    pub robot_viz_config_text: String,
    pub ui_captures_pointer: bool,
    runtime_paused: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RightPanelMode {
    Console,
    Robot,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RobotVizCell {
    pub x: usize,
    pub y: usize,
    #[serde(default = "default_cell_kind")]
    pub kind: String,
}

pub fn default_cell_kind() -> String {
    "empty".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
pub struct VizPos {
    pub x: usize,
    pub y: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RobotVizConfig {
    pub width: usize,
    pub height: usize,
    pub robot_start: VizPos,
    #[serde(default)]
    pub robot_direction: Option<String>,
    #[serde(default)]
    pub cells: Vec<RobotVizCell>,
}

impl Default for RobotVizConfig {
    fn default() -> Self {
        Self {
            width: 5,
            height: 5,
            robot_start: VizPos { x: 2, y: 2 },
            robot_direction: Some("up".to_owned()),
            cells: vec![
                RobotVizCell {
                    x: 1,
                    y: 1,
                    kind: "obstacle".to_owned(),
                },
                RobotVizCell {
                    x: 3,
                    y: 2,
                    kind: "target".to_owned(),
                },
            ],
        }
    }
}

impl ProgrammingNotebook {
    pub fn new() -> Self {
        Self {
            open: false,
            tab: NotebookTab::Programming,
            right_panel_mode: RightPanelMode::Robot,
            program_text:
                "function patrol()\n  move.forward(3)\n  turn.left()\n  print(\"ready\")\nend"
                    .to_string(),
            puzzle_text: "function solve()\n  move.forward(2)\n  scan()\n  turn.right()\nend"
                .to_string(),
            console_text: "> ready\n> waiting for block execution\n> no runtime errors".to_string(),
            robot_viz_config_text: r#"{
  "width": 5,
  "height": 5,
  "robot_start": { "x": 2, "y": 2 },
  "robot_direction": "up",
  "cells": [
    { "x": 1, "y": 1, "kind": "obstacle" },
    { "x": 3, "y": 2, "kind": "target" }
  ]
}"#
            .to_string(),
            ui_captures_pointer: false,
            runtime_paused: false,
        }
    }

    pub fn set_right_panel_mode(&mut self, mode: RightPanelMode) {
        self.right_panel_mode = mode;
    }

    pub fn bounds(&self) -> Rect {
        let scale = resolution_ui_scale();
        let margin = 14.0 * scale;
        let w = (screen_width() - margin * 2.0)
            .min(920.0 * scale)
            .max(320.0);
        let h = (screen_height() - margin * 2.0)
            .min(500.0 * scale)
            .max(260.0);
        Rect::new((screen_width() - w) * 0.5, margin, w, h)
    }

    pub fn captures_pointer(&self) -> bool {
        self.ui_captures_pointer
    }

    pub fn handle_input(&mut self) {
        if !self.open && is_key_pressed(NOTEBOOK_TOGGLE_KEY) {
            self.open = true;
        }
    }

    pub fn draw(&mut self) {
        if !self.open {
            let margin = 16.0 * resolution_ui_scale();
            egui_macroquad::ui(|ctx| {
                style_notebook_egui(ctx);
                egui::Area::new(egui::Id::new("notebook_open_button"))
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-margin, -margin))
                    .show(ctx, |ui| {
                        if ui
                            .add_sized([150.0, 38.0], egui::Button::new("Notebook  [N]"))
                            .clicked()
                        {
                            self.open = true;
                        }
                    });
                self.ui_captures_pointer = ctx.is_pointer_over_area() || ctx.wants_pointer_input();
            });
            return;
        }

        let rect = self.bounds();
        egui_macroquad::ui(|ctx| {
            style_notebook_egui(ctx);
            egui::Window::new("Notebook")
                .title_bar(false)
                .fixed_pos(egui::pos2(rect.x, rect.y))
                .fixed_size(egui::vec2(rect.w, rect.h))
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        tab_button(ui, &mut self.tab, NotebookTab::Programming, "Programming");
                        tab_button(
                            ui,
                            &mut self.tab,
                            NotebookTab::PuzzleProgramming,
                            "Puzzle Programming",
                        );
                        tab_button(ui, &mut self.tab, NotebookTab::Almanac, "Almanac");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                self.open = false;
                            }
                        });
                    });
                    ui.separator();

                    match self.tab {
                        NotebookTab::Programming => self.draw_programming_ui(ui, false),
                        NotebookTab::PuzzleProgramming => self.draw_programming_ui(ui, true),
                        NotebookTab::Almanac => self.draw_almanac_ui(ui),
                    }
                });
            self.ui_captures_pointer = ctx.is_pointer_over_area() || ctx.wants_pointer_input();
        });
    }

    fn parse_robot_viz_config(&self) -> Result<RobotVizConfig, String> {
        serde_json::from_str(&self.robot_viz_config_text)
            .map_err(|err| format!("Robot visualization JSON parse error: {}", err))
    }

    fn draw_robot_viz(ui: &mut egui::Ui, config: &RobotVizConfig) {
        let viz_height = 130.0;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), viz_height),
            egui::Sense::hover(),
        );
        let painter = ui.painter();
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(12),
            egui::Color32::from_rgb(18, 22, 32),
        );

        let inner = rect.shrink(8.0);
        let cols = config.width.max(1) as f32;
        let rows = config.height.max(1) as f32;
        let tile_size = (inner.width() / cols).min(inner.height() / rows).max(16.0);
        let board_w = tile_size * cols;
        let board_h = tile_size * rows;
        let origin = inner.left_top()
            + egui::vec2(
                (inner.width() - board_w) * 0.5,
                (inner.height() - board_h) * 0.5,
            );

        let mut cell_map: HashMap<(usize, usize), &RobotVizCell> = HashMap::new();
        for cell in &config.cells {
            if cell.x < config.width && cell.y < config.height {
                cell_map.insert((cell.x, cell.y), cell);
            }
        }

        for y in 0..config.height {
            for x in 0..config.width {
                let tile_origin = origin + egui::vec2(x as f32 * tile_size, y as f32 * tile_size);
                let tile_size = tile_size - 6.0;
                let tile_origin = tile_origin + egui::vec2(3.0, 3.0);
                let tile_rect =
                    egui::Rect::from_min_size(tile_origin, egui::vec2(tile_size, tile_size));

                let cell_kind = cell_map
                    .get(&(x, y))
                    .map(|cell| cell.kind.as_str())
                    .unwrap_or("empty");
                let mut fill = match cell_kind {
                    "obstacle" => egui::Color32::from_rgb(164, 74, 74),
                    "target" => egui::Color32::from_rgb(94, 180, 130),
                    "path" => egui::Color32::from_rgb(80, 108, 180),
                    _ => egui::Color32::from_rgb(30, 38, 56),
                };

                let robot_on_tile = x == config.robot_start.x && y == config.robot_start.y;
                if robot_on_tile {
                    fill = egui::Color32::from_rgb(70, 132, 220);
                }

                painter.rect_filled(tile_rect, egui::CornerRadius::same(10), fill);
                painter.rect_stroke(
                    tile_rect,
                    egui::CornerRadius::same(10),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 88, 110)),
                    egui::StrokeKind::Outside,
                );

                if robot_on_tile {
                    let robot_rect = tile_rect.shrink(6.0);
                    painter.rect_filled(
                        robot_rect,
                        egui::CornerRadius::same(8),
                        egui::Color32::from_rgb(198, 228, 255),
                    );
                    painter.text(
                        robot_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "R",
                        egui::FontId::monospace((robot_rect.width() * 0.55).max(10.0)),
                        egui::Color32::from_rgb(12, 20, 34),
                    );
                }

                if cell_kind == "target" {
                    painter.text(
                        tile_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "★",
                        egui::FontId::monospace((tile_rect.width() * 0.6).max(10.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
        }

        let info = format!(
            "board {}x{}  start=({}, {})  dir={}",
            config.width,
            config.height,
            config.robot_start.x,
            config.robot_start.y,
            config.robot_direction.as_deref().unwrap_or("none"),
        );
        painter.text(
            rect.left_bottom() + egui::vec2(6.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            info,
            egui::FontId::monospace(12.0),
            egui::Color32::from_rgb(190, 210, 235),
        );
    }

    fn draw_programming_ui(&mut self, ui: &mut egui::Ui, puzzle: bool) {
        ui.label(egui::RichText::new("Programming Space").strong());
        if puzzle {
            ui.colored_label(egui::Color32::from_rgb(250, 203, 128), "No puzzle active");
        }
        let config_result = self.parse_robot_viz_config();
        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Play").clicked() {
                        self.runtime_paused = false;
                        let source = if puzzle {
                            self.puzzle_text.clone()
                        } else {
                            self.program_text.clone()
                        };
                        self.execute_khoron(&source);
                    }
                    if ui.button("Pause").clicked() {
                        self.runtime_paused = true;
                        self.console_text.push_str("\n> runtime paused");
                    }
                    if ui.button("Stop").clicked() {
                        self.runtime_paused = false;
                        self.console_text.push_str("\n> runtime stopped");
                    }
                    if ui.button("Clear Console").clicked() {
                        self.console_text.clear();
                    }
                    ui.separator();
                    if ui
                        .selectable_label(
                            self.right_panel_mode == RightPanelMode::Console,
                            "Console",
                        )
                        .clicked()
                    {
                        self.set_right_panel_mode(RightPanelMode::Console);
                    }
                    if ui
                        .selectable_label(self.right_panel_mode == RightPanelMode::Robot, "Robot")
                        .clicked()
                    {
                        self.set_right_panel_mode(RightPanelMode::Robot);
                    }
                });
                ui.separator();
                ui.add_sized(
                    [
                        ui.available_width(),
                        (ui.available_height() - 8.0).max(170.0),
                    ],
                    egui::TextEdit::multiline(if puzzle {
                        &mut self.puzzle_text
                    } else {
                        &mut self.program_text
                    })
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
                );
            });

            cols[1].vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Right Panel").strong());
                    ui.separator();
                    if ui
                        .selectable_label(
                            self.right_panel_mode == RightPanelMode::Console,
                            "Console",
                        )
                        .clicked()
                    {
                        self.set_right_panel_mode(RightPanelMode::Console);
                    }
                    if ui
                        .selectable_label(self.right_panel_mode == RightPanelMode::Robot, "Robot")
                        .clicked()
                    {
                        self.set_right_panel_mode(RightPanelMode::Robot);
                    }
                });
                ui.separator();

                if self.right_panel_mode == RightPanelMode::Robot {
                    ui.label(egui::RichText::new("Robot Movement Visualization").strong());

                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        if let Ok(config) = &config_result {
                            Self::draw_robot_viz(ui, config);
                        } else if let Err(err) = &config_result {
                            ui.colored_label(egui::Color32::from_rgb(255, 120, 120), err);
                        }

                        ui.separator();
                        ui.label(egui::RichText::new("Visualization JSON configuration").small());
                        ui.add_sized(
                            [ui.available_width(), 95.0],
                            egui::TextEdit::multiline(&mut self.robot_viz_config_text)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.separator();
                }

                ui.label(egui::RichText::new("Console").strong());
                ui.add_sized(
                    [
                        ui.available_width(),
                        (ui.available_height() - 8.0).max(120.0),
                    ],
                    egui::TextEdit::multiline(&mut self.console_text)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
            });
        });
    }

    fn execute_khoron(&mut self, source: &str) {
        if self.runtime_paused {
            self.console_text.push_str("\n> runtime paused");
            return;
        }
        let mut runtime = KhoronRuntime::default();
        match runtime.run(source) {
            Ok(lines) => {
                if lines.is_empty() {
                    self.console_text.push_str("\n> ok");
                } else {
                    for line in lines {
                        self.console_text.push('\n');
                        self.console_text.push_str(line);
                    }
                }
            }
            Err(err) => {
                self.console_text.push_str("\n[khoron] ");
                self.console_text.push_str(&err);
            }
        }
    }

    fn draw_almanac_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Known Programming Functions").strong());
        ui.separator();

        let entries = [
            (
                "print(value)",
                "Writes a value to the console.",
                "print(\"hello\")",
            ),
            (
                "move.forward(n)",
                "Moves the robot forward n tiles.",
                "move.forward(3)",
            ),
            (
                "move.back(n)",
                "Moves the robot backward n tiles.",
                "move.back(1)",
            ),
            (
                "turn.left()",
                "Rotates the robot left by one quarter turn.",
                "turn.left()",
            ),
            (
                "turn.right()",
                "Rotates the robot right by one quarter turn.",
                "turn.right()",
            ),
            (
                "scan()",
                "Reads the tile or object in front of the robot.",
                "scan()",
            ),
            ("wait(t)", "Pauses execution for t seconds.", "wait(0.5)"),
            (
                "harvest()",
                "Uses the robot tool on the current tile.",
                "harvest()",
            ),
        ];

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (name, meaning, usage) in entries {
                ui.label(egui::RichText::new(name).monospace().strong());
                ui.label(meaning);
                ui.colored_label(egui::Color32::from_rgb(140, 222, 255), usage);
                ui.separator();
            }
        });
    }
}

fn tab_button(ui: &mut egui::Ui, current: &mut NotebookTab, tab: NotebookTab, label: &str) {
    let selected = *current == tab;
    if ui.selectable_label(selected, label).clicked() {
        *current = tab;
    }
}

fn style_notebook_egui(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = egui::vec2(6.0, 4.0);
    style.spacing.item_spacing = egui::vec2(4.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.scroll = egui::style::ScrollStyle {
        floating: false,
        bar_width: 10.0,
        handle_min_length: 40.0,
        bar_inner_margin: 4.0,
        bar_outer_margin: 0.0,
        floating_width: 10.0,
        floating_allocated_width: 0.0,
        foreground_color: true,
        dormant_background_opacity: 1.0,
        active_background_opacity: 1.0,
        interact_background_opacity: 1.0,
        dormant_handle_opacity: 1.0,
        active_handle_opacity: 1.0,
        interact_handle_opacity: 1.0,
    };

    style.visuals = egui::Visuals::dark();
    style.visuals.window_fill = egui::Color32::from_rgb(14, 18, 26);
    style.visuals.panel_fill = egui::Color32::from_rgb(14, 18, 26);
    style.visuals.faint_bg_color = egui::Color32::from_rgb(22, 28, 38);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(10, 12, 18);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(80, 160, 255);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(68, 130, 220);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(20, 28, 44);
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(14, 18, 26);
    style.visuals.widgets.inactive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(190, 210, 255));
    style.visuals.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(190, 210, 255));
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(80, 160, 255);
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(220, 230, 255));

    style.visuals.window_corner_radius = 0.0.into();
    style.visuals.menu_corner_radius = 0.0.into();
    style.visuals.widgets.active.corner_radius = 0.0.into();
    style.visuals.widgets.hovered.corner_radius = 0.0.into();
    style.visuals.widgets.inactive.corner_radius = 0.0.into();
    style.visuals.widgets.noninteractive.corner_radius = 0.0.into();
    style.visuals.widgets.open.corner_radius = 0.0.into();
    style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 210, 255));

    ctx.set_style(style);
}
