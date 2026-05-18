use egui_macroquad::egui;
use khoron::Runtime as KhoronRuntime;
use macroquad::prelude::*;
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;

use crate::helpers::resolution_ui_scale;

pub const NOTEBOOK_TOGGLE_KEY: KeyCode = KeyCode::N;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotebookTab {
    Programming,
    PuzzleProgramming,
    Almanac,
}

#[derive(Debug, Clone)]
pub enum TraceEvent {
    Line(usize),
    RobotState(RobotVizConfig),
    Print(String),
    RepeatCount(usize, i64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeedOption {
    Turtle,
    Slow,
    Regular,
    Fast,
    Speedster,
    Cheetah,
}

impl SpeedOption {
    fn delay(&self) -> f32 {
        match self {
            SpeedOption::Turtle => 1.5,
            SpeedOption::Slow => 1.0,
            SpeedOption::Regular => 0.5,
            SpeedOption::Fast => 0.25,
            SpeedOption::Speedster => 0.1,
            SpeedOption::Cheetah => 0.05,
        }
    }
}

thread_local! {
    static VIZ_TRACE: std::cell::RefCell<Vec<TraceEvent>> = std::cell::RefCell::new(Vec::new());
    static VIZ_STATE: std::cell::RefCell<RobotVizConfig> = std::cell::RefCell::new(RobotVizConfig::default());
}

pub struct ProgrammingNotebook {
    pub puzzle_completed: bool,
    pub trace: Vec<TraceEvent>,
    pub trace_index: usize,
    pub trace_timer: f32,
    pub is_playing: bool,
    pub active_line: Option<usize>,
    pub current_viz_config: Option<RobotVizConfig>,
    pub speed_option: SpeedOption,
    pub open: bool,
    pub tab: NotebookTab,
    pub right_panel_mode: RightPanelMode,
    pub program_text: String,
    pub puzzle_text: String,
    pub puzzle_prompt: String,
    pub validator_contains: String,
    pub console_text: String,
    pub robot_viz_config_text: String,
    pub ui_captures_pointer: bool,
    pub puzzle_panel_locked: bool,
    runtime_paused: bool,
    pending_puzzle_check: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RightPanelMode {
    Console,
    Robot,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct RobotVizCell {
    pub x: usize,
    pub y: usize,
    #[serde(default = "default_cell_kind")]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct VizPos {
    pub x: usize,
    pub y: usize,
}

pub fn default_cell_kind() -> String {
    "empty".to_owned()
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
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
            puzzle_prompt: String::new(),
            validator_contains: String::new(),
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
            puzzle_completed: false,
            trace: Vec::new(),
            trace_index: 0,
            trace_timer: 0.0,
            is_playing: false,
            active_line: None,
            current_viz_config: None,
            speed_option: SpeedOption::Regular,
            runtime_paused: false,
            puzzle_panel_locked: false,
            pending_puzzle_check: false,
        }
    }

    pub fn discovered_functions() -> &'static [(&'static str, &'static str, &'static str)] {
        ALMANAC_ENTRIES
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

    pub fn activate_puzzle(
        &mut self,
        prompt: &str,
        starter_code: &str,
        validator_contains: &str,
        viz_config: Option<&serde_json::Value>,
    ) {
        self.tab = NotebookTab::PuzzleProgramming;
        self.puzzle_text = starter_code.to_string();
        self.puzzle_prompt = prompt.to_string();
        self.validator_contains = validator_contains.to_string();
        self.puzzle_completed = false;
        self.open = true;
        self.puzzle_panel_locked = true;
        self.right_panel_mode = RightPanelMode::Robot;
        self.pending_puzzle_check = false;

        if let Some(config) = viz_config {
            if let Ok(parsed) = serde_json::from_value::<RobotVizConfig>(config.clone()) {
                self.robot_viz_config_text = serde_json::to_string_pretty(&parsed).unwrap_or_default();
            }
        }
    }

    pub fn take_puzzle_completed(&mut self) -> bool {
        let completed = self.puzzle_completed;
        self.puzzle_completed = false;
        completed
    }

    pub fn handle_input(&mut self) {
        if !self.open && is_key_pressed(NOTEBOOK_TOGGLE_KEY) {
            self.open = true;
        }
    }

    pub fn captures_pointer(&self) -> bool {
        self.ui_captures_pointer
    }

    pub fn draw(&mut self) {
        if self.is_playing && !self.runtime_paused {
            self.trace_timer += get_frame_time();
            if self.trace_timer >= self.speed_option.delay() {
                self.trace_timer = 0.0;
                if self.trace_index < self.trace.len() {
                    let mut advanced_line = false;
                    while self.trace_index < self.trace.len() && !advanced_line {
                        match &self.trace[self.trace_index] {
                            TraceEvent::Line(l) => {
                                self.active_line = Some(*l);
                                self.trace_index += 1;
                                advanced_line = true;
                            }
                            TraceEvent::RobotState(config) => {
                                self.current_viz_config = Some(config.clone());
                                self.trace_index += 1;
                            }
                            TraceEvent::Print(s) => {
                                self.console_text.push_str(&format!("\n> {}", s));
                                self.trace_index += 1;
                            }
                            TraceEvent::RepeatCount(line, remaining) => {
                                self.active_line = Some(*line);
                                self.console_text
                                    .push_str(&format!("\n> repeat {}", remaining));
                                self.trace_index += 1;
                                advanced_line = true;
                            }
                        }
                    }
                } else {
                    self.is_playing = false;
                    self.active_line = None;
                    self.console_text.push_str("\n> execution finished");
                    self.pending_puzzle_check = self.puzzle_panel_locked;
                }
            }
        } else if self.pending_puzzle_check {
            self.pending_puzzle_check = false;
            self.check_puzzle_completion();
        }

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
        let viz_height = 150.0;
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
        let tile_size = (inner.width() / cols).min(inner.height() / rows).max(18.0);
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
                let tile_sz = tile_size - 6.0;
                let tile_origin = tile_origin + egui::vec2(3.0, 3.0);
                let tile_rect =
                    egui::Rect::from_min_size(tile_origin, egui::vec2(tile_sz, tile_sz));

                let cell_kind = cell_map
                    .get(&(x, y))
                    .map(|cell| cell.kind.as_str())
                    .unwrap_or("empty");
                let mut fill = match cell_kind {
                    "obstacle" => egui::Color32::from_rgb(120, 70, 70),
                    "target" => egui::Color32::from_rgb(80, 180, 120),
                    "path" => egui::Color32::from_rgb(70, 100, 160),
                    "pushable" => egui::Color32::from_rgb(50, 150, 250),
                    "immovable" => egui::Color32::from_rgb(20, 80, 150),
                    "enemy" => egui::Color32::from_rgb(250, 60, 60),
                    "enemy_block" => egui::Color32::from_rgb(200, 40, 40),
                    "flag" => egui::Color32::from_rgb(50, 250, 50),
                    "key" => egui::Color32::from_rgb(255, 180, 60),
                    "lock" => egui::Color32::from_rgb(220, 140, 50),
                    "coin" => egui::Color32::from_rgb(255, 220, 60),
                    _ => egui::Color32::from_rgb(30, 38, 56),
                };

                let robot_on_tile = x == config.robot_start.x && y == config.robot_start.y;
                if robot_on_tile {
                    fill = egui::Color32::from_rgb(80, 140, 240);
                }

                painter.rect_filled(tile_rect, egui::CornerRadius::same(8), fill);
                painter.rect_stroke(
                    tile_rect,
                    egui::CornerRadius::same(8),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 75, 95)),
                    egui::StrokeKind::Outside,
                );

                if robot_on_tile {
                    let robot_rect = tile_rect.shrink(5.0);
                    painter.rect_filled(
                        robot_rect,
                        egui::CornerRadius::same(6),
                        egui::Color32::from_rgb(210, 235, 255),
                    );
                    painter.text(
                        robot_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "R",
                        egui::FontId::monospace((robot_rect.width() * 0.5).max(9.0)),
                        egui::Color32::from_rgb(12, 20, 34),
                    );
                }

if cell_kind != "empty" && cell_kind != "path" {
                     let text = match cell_kind {
                         "target" => "★",
                         "pushable" => "P",
                         "immovable" => "I",
                         "enemy" | "enemy_block" => "E",
                         "flag" => "F",
                         "key" => "K",
                         "lock" => "L",
                         "coin" => "C",
                         "obstacle" => "█",
                         _ => "?",
                     };
                     if !text.is_empty() {
                         painter.text(
                             tile_rect.center(),
                             egui::Align2::CENTER_CENTER,
                             text,
                             egui::FontId::monospace((tile_rect.width() * 0.55).max(9.0)),
                             egui::Color32::WHITE,
                         );
                     }
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
        if puzzle && !self.puzzle_prompt.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(250, 203, 128), &self.puzzle_prompt);
        }
        let config_result = if self.is_playing {
            self.current_viz_config.clone().ok_or_else(|| "No config".to_string())
        } else {
            self.parse_robot_viz_config()
        };
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
                    egui::ComboBox::from_id_salt("speed_option")
                        .selected_text(format!("{:?}", self.speed_option))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Turtle, "Turtle");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Slow, "Slow");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Regular, "Regular");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Fast, "Fast");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Speedster, "Speedster");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Cheetah, "Cheetah");
                        });
                    if !puzzle {
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
                            .selectable_label(
                                self.right_panel_mode == RightPanelMode::Robot,
                                "Robot",
                            )
                            .clicked()
                        {
                            self.set_right_panel_mode(RightPanelMode::Robot);
                        }
                    }
                });
                ui.separator();
                let is_playing = self.is_playing;
                let active_line = self.active_line;
                let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                    let mut job = egui::text::LayoutJob::default();
                    for (i, line) in string.lines().enumerate() {
                        let line_num = i + 1;
                        let color = if is_playing && active_line == Some(line_num) {
                            egui::Color32::WHITE
                        } else if is_playing && active_line.map_or(false, |a| line_num < a) {
                            egui::Color32::from_gray(100)
                        } else {
                            egui::Color32::from_gray(180)
                        };
                        job.append(line, 0.0, egui::TextFormat {
                            font_id: egui::FontId::monospace(14.0),
                            color,
                            ..Default::default()
                        });
                        job.append("\n", 0.0, egui::TextFormat::default());
                    }
                    ui.fonts(|f| f.layout_job(job))
                };

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
                    .layouter(&mut layouter)
                    .desired_width(f32::INFINITY),
                );
            });

            cols[1].vertical(|ui| {
                let show_robot = puzzle || self.right_panel_mode == RightPanelMode::Robot;
                if !puzzle {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Right Panel").strong());
                    });
                    ui.separator();
                }

                if show_robot {
                    ui.label(egui::RichText::new("Robot Movement Visualization").strong());
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        if let Ok(config) = &config_result {
                            Self::draw_robot_viz(ui, config);
                        } else if let Err(err) = &config_result {
                            ui.colored_label(egui::Color32::from_rgb(255, 120, 120), err);
                        }
                        if !puzzle {
                            ui.separator();
                            ui.label(
                                egui::RichText::new("Visualization JSON configuration").small(),
                            );
                            ui.add_sized(
                                [ui.available_width(), 95.0],
                                egui::TextEdit::multiline(&mut self.robot_viz_config_text)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY),
                            );
                        }
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
        let starting_config = self.parse_robot_viz_config().unwrap_or_default();
        VIZ_STATE.with(|s| *s.borrow_mut() = starting_config.clone());
        VIZ_TRACE.with(|t| t.borrow_mut().clear());
        
        let mut runtime = KhoronRuntime::default();
        runtime.on_step = Some(Box::new(|line| {
            VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Line(line)));
        }));
        
        fn make_dict(methods: Vec<(&str, khoron::Value)>) -> khoron::Value {
            let mut map = std::collections::HashMap::new();
            for (k, v) in methods {
                map.insert(k.to_string(), v);
            }
            khoron::Value::Dict(std::rc::Rc::new(std::cell::RefCell::new(map)))
        }

        let move_fwd = khoron::Value::NativeFunction(|args| {
            let mut steps = 1isize;
            if let Some(khoron::Value::Number(n)) = args.first() {
                steps = *n as isize;
            }
            VIZ_STATE.with(|s| {
                let mut c = s.borrow_mut();
                for _ in 0..steps.abs() {
                    let (dx, dy) = match c.robot_direction.as_deref().unwrap_or("up") {
                        "up" => (0, -1),
                        "down" => (0, 1),
                        "left" => (-1, 0),
                        "right" => (1, 0),
                        _ => (0, 0),
                    };
                    if steps < 0 {
                        c.robot_start.x = (c.robot_start.x as isize - dx).max(0) as usize;
                        c.robot_start.y = (c.robot_start.y as isize - dy).max(0) as usize;
                    } else {
                        c.robot_start.x = (c.robot_start.x as isize + dx).max(0) as usize;
                        c.robot_start.y = (c.robot_start.y as isize + dy).max(0) as usize;
                    }
                    VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::RobotState(c.clone())));
                }
            });
            Ok(khoron::Value::Nil)
        });

        let move_back = khoron::Value::NativeFunction(|args| {
            let mut steps = 1isize;
            if let Some(khoron::Value::Number(n)) = args.first() {
                steps = *n as isize;
            }
            VIZ_STATE.with(|s| {
                let mut c = s.borrow_mut();
                for _ in 0..steps.abs() {
                    let (dx, dy) = match c.robot_direction.as_deref().unwrap_or("up") {
                        "up" => (0, -1),
                        "down" => (0, 1),
                        "left" => (-1, 0),
                        "right" => (1, 0),
                        _ => (0, 0),
                    };
                    // Move backward (opposite direction)
                    c.robot_start.x = (c.robot_start.x as isize - dx).max(0) as usize;
                    c.robot_start.y = (c.robot_start.y as isize - dy).max(0) as usize;
                    VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::RobotState(c.clone())));
                }
            });
            Ok(khoron::Value::Nil)
        });

        let turn_left = khoron::Value::NativeFunction(|_| {
            VIZ_STATE.with(|s| {
                let mut c = s.borrow_mut();
                c.robot_direction = Some(match c.robot_direction.as_deref().unwrap_or("up") {
                    "up" => "left",
                    "left" => "down",
                    "down" => "right",
                    "right" => "up",
                    _ => "up",
                }.to_string());
                VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::RobotState(c.clone())));
            });
            Ok(khoron::Value::Nil)
        });

        let turn_right = khoron::Value::NativeFunction(|_| {
            VIZ_STATE.with(|s| {
                let mut c = s.borrow_mut();
                c.robot_direction = Some(match c.robot_direction.as_deref().unwrap_or("up") {
                    "up" => "right",
                    "right" => "down",
                    "down" => "left",
                    "left" => "up",
                    _ => "up",
                }.to_string());
                VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::RobotState(c.clone())));
            });
            Ok(khoron::Value::Nil)
        });

        let collect = khoron::Value::NativeFunction(|_| {
            VIZ_STATE.with(|s| {
                let mut c = s.borrow_mut();
                let rx = c.robot_start.x;
                let ry = c.robot_start.y;
                c.cells
                    .retain(|cell| !(cell.x == rx && cell.y == ry && cell.kind == "coin"));
                VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::RobotState(c.clone())));
            });
            VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Print("collect()".to_string())));
            Ok(khoron::Value::Nil)
        });

        let attack = khoron::Value::NativeFunction(|_| {
            VIZ_STATE.with(|s| {
                let mut c = s.borrow_mut();
                let (dx, dy) = robot_front_delta(&c);
                let tx = (c.robot_start.x as isize + dx).max(0) as usize;
                let ty = (c.robot_start.y as isize + dy).max(0) as usize;
                c.cells.retain(|cell| {
                    !((cell.x == tx && cell.y == ty)
                        && (cell.kind == "enemy" || cell.kind == "enemy_block"))
                });
                VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::RobotState(c.clone())));
            });
            VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Print("attack()".to_string())));
            Ok(khoron::Value::Nil)
        });

        let flag = khoron::Value::NativeFunction(|_| {
            VIZ_STATE.with(|s| {
                let mut c = s.borrow_mut();
                let (dx, dy) = robot_front_delta(&c);
                let tx = (c.robot_start.x as isize + dx).max(0) as usize;
                let ty = (c.robot_start.y as isize + dy).max(0) as usize;
                let on_flag = c.cells.iter().any(|cell| {
                    cell.kind == "flag"
                        && ((cell.x == c.robot_start.x && cell.y == c.robot_start.y)
                            || (cell.x == tx && cell.y == ty))
                });
                if on_flag {
                    c.cells.retain(|cell| cell.kind != "flag");
                }
                VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::RobotState(c.clone())));
            });
            VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Print("flag()".to_string())));
            Ok(khoron::Value::Nil)
        });

        let harvest = khoron::Value::NativeFunction(|_| {
            VIZ_STATE.with(|s| {
                let mut c = s.borrow_mut();
                let rx = c.robot_start.x;
                let ry = c.robot_start.y;
                c.cells.retain(|cell| {
                    !(cell.x == rx
                        && cell.y == ry
                        && (cell.kind == "target" || cell.kind == "path"))
                });
                VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::RobotState(c.clone())));
            });
            VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Print("harvest()".to_string())));
            Ok(khoron::Value::Nil)
        });

        let unlock = khoron::Value::NativeFunction(|_| {
            VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Print("unlock() called".to_string())));
            Ok(khoron::Value::Nil)
        });

        let scan = khoron::Value::NativeFunction(|_| {
            Ok(khoron::Value::String("empty".to_string()))
        });

        let input_fn = khoron::Value::NativeFunction(|_| {
            VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Print("[input: not implemented in viz]".to_string())));
            Ok(khoron::Value::String("".to_string()))
        });
        
        let wait_fn = khoron::Value::NativeFunction(|args| {
            let _seconds = args.first().and_then(|v| v.as_number()).unwrap_or(1.0);
            VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Print("wait() called".to_string())));
            Ok(khoron::Value::Nil)
        });
        
        runtime.env.borrow_mut().define("move".to_string(), make_dict(vec![("forward", move_fwd.clone()), ("back", move_back.clone())]));
        runtime.env.borrow_mut().define("turn".to_string(), make_dict(vec![("left", turn_left.clone()), ("right", turn_right.clone())]));
        
        runtime.env.borrow_mut().define("move_forward".to_string(), move_fwd);
        runtime.env.borrow_mut().define("move_back".to_string(), move_back);
        runtime.env.borrow_mut().define("turn_left".to_string(), turn_left);
        runtime.env.borrow_mut().define("turn_right".to_string(), turn_right);
        runtime.env.borrow_mut().define("collect".to_string(), collect);
        runtime.env.borrow_mut().define("attack".to_string(), attack);
        runtime.env.borrow_mut().define("flag".to_string(), flag);
        runtime.env.borrow_mut().define("unlock".to_string(), unlock);
        runtime.env.borrow_mut().define("scan".to_string(), scan);
        runtime.env.borrow_mut().define("wait".to_string(), wait_fn);
        runtime.env.borrow_mut().define("input".to_string(), input_fn);
        runtime.env.borrow_mut().define("harvest".to_string(), harvest);

        runtime.on_repeat = Some(Box::new(|line, remaining| {
            VIZ_TRACE.with(|t| {
                t.borrow_mut()
                    .push(TraceEvent::RepeatCount(line, remaining));
            });
        }));

        let puzzle_mode = self.puzzle_panel_locked;

        match runtime.run(source) {
            Ok(lines) => {
                for line in lines {
                    VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Print(line.clone())));
                }
            }
            Err(e) => {
                VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Print(format!("[error] {}", e))));
            }
        }
        
        self.trace = VIZ_TRACE.with(|t| t.borrow().clone());
        self.trace_index = 0;
        self.trace_timer = 0.0;
        self.is_playing = true;
        self.active_line = None;
        self.current_viz_config = Some(starting_config);
        self.console_text.clear();
        self.console_text.push_str("> execution started");
        self.pending_puzzle_check = puzzle_mode;
    }

    fn check_puzzle_completion(&mut self) {
        if !self.puzzle_panel_locked {
            return;
        }
        if self.validator_contains.is_empty() {
            return;
        }
        if self.puzzle_text.contains(&self.validator_contains) {
            self.puzzle_completed = true;
            self.console_text.push_str("\n> puzzle complete!");
        }
    }

    fn draw_almanac_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Function Gallery").strong());
        ui.label("Discovered Khoron functions for Basics Bay.");
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("almanac_grid")
                .num_columns(2)
                .spacing([12.0, 12.0])
                .show(ui, |ui| {
                    for (name, meaning, usage) in Self::discovered_functions() {
                        egui::Frame::group(ui.style())
                            .fill(egui::Color32::from_rgb(22, 28, 40))
                            .show(ui, |ui| {
                                ui.set_min_width(200.0);
                                ui.label(egui::RichText::new(*name).monospace().strong());
                                ui.label(*meaning);
                                ui.colored_label(
                                    egui::Color32::from_rgb(140, 222, 255),
                                    *usage,
                                );
                            });
                    }
                });
        });
    }
}

const ALMANAC_ENTRIES: &[(&str, &str, &str)] = &[
    ("print(value)", "Writes a value to the console.", "print(\"hello\")"),
    ("input()", "Prompts for input (returns string).", "name = input()"),
    (
        "move.forward(n)",
        "Moves the robot forward n tiles.",
        "move.forward(3)",
    ),
    ("move.back(n)", "Moves the robot backward n tiles.", "move.back(1)"),
    ("turn.left()", "Rotates the robot left.", "turn.left()"),
    ("turn.right()", "Rotates the robot right.", "turn.right()"),
    ("scan()", "Reads the tile in front.", "scan()"),
    ("collect()", "Collects a coin on this tile.", "collect()"),
    ("attack()", "Attacks an enemy in front.", "attack()"),
    ("flag()", "Raises the flag tile.", "flag()"),
    ("unlock()", "Unlocks a lock with a key.", "unlock()"),
    ("harvest()", "Harvests the crop on this tile.", "harvest()"),
    ("wait(t)", "Pauses execution.", "wait(0.5)"),
    (
        "repeat n",
        "Repeats a block (counts down).",
        "repeat 3\n  move.forward(1)\nend",
    ),
];

fn robot_front_delta(config: &RobotVizConfig) -> (isize, isize) {
    match config.robot_direction.as_deref().unwrap_or("up") {
        "up" => (0, -1),
        "down" => (0, 1),
        "left" => (-1, 0),
        "right" => (1, 0),
        _ => (0, -1),
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
