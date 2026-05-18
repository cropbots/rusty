import re

with open("src/notebook.rs", "r") as f:
    src = f.read()

# 1. Update layouter in draw_programming_ui
layouter = """
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
                            job.append("\\n", 0.0, egui::TextFormat::default());
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
"""
src = re.sub(
    r"ui\.add_sized\([\s\S]*?egui::TextEdit::multiline.*?\n.*\.font\(egui::TextStyle::Monospace\)\n.*\.desired_width\(f32::INFINITY\),\n\s*\);",
    layouter,
    src
)

# 2. Add speed options ComboBox
speed_options = """
                    egui::ComboBox::from_id_source("speed_option")
                        .selected_text(format!("{:?}", self.speed_option))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Turtle, "Turtle");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Slow, "Slow");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Regular, "Regular");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Fast, "Fast");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Speedster, "Speedster");
                            ui.selectable_value(&mut self.speed_option, SpeedOption::Cheetah, "Cheetah");
                        });
                    ui.separator();
                    if ui
"""
src = src.replace("ui.separator();\n                    if ui\n                        .selectable_label(", speed_options + "                        .selectable_label(")

# 3. Update current_viz_config logic
viz_config_logic = """
        let config_result = if self.is_playing {
            self.current_viz_config.clone().ok_or_else(|| "No config".to_string())
        } else {
            self.parse_robot_viz_config()
        };
"""
src = src.replace("let config_result = self.parse_robot_viz_config();", viz_config_logic)

# 4. Update cell types in draw_robot_viz
cell_colors = """
                let mut fill = match cell_kind {
                    "obstacle" => egui::Color32::from_rgb(164, 74, 74),
                    "target" => egui::Color32::from_rgb(94, 180, 130),
                    "path" => egui::Color32::from_rgb(80, 108, 180),
                    "pushable" => egui::Color32::from_rgb(50, 150, 250),
                    "immovable" => egui::Color32::from_rgb(20, 50, 100),
                    "enemy" => egui::Color32::from_rgb(250, 50, 50),
                    "flag" => egui::Color32::from_rgb(50, 250, 50),
                    "key" | "lock" => egui::Color32::from_rgb(250, 150, 50),
                    "coin" => egui::Color32::from_rgb(250, 250, 50),
                    _ => egui::Color32::from_rgb(30, 38, 56),
                };
"""
src = re.sub(r"let mut fill = match cell_kind \{[\s\S]*?_ => egui::Color32::from_rgb\(30, 38, 56\),\n\s*\};", cell_colors, src)

icons = """
                if cell_kind != "empty" {
                    let text = match cell_kind {
                        "target" => "★",
                        "pushable" => "P",
                        "immovable" => "I",
                        "enemy" => "E",
                        "flag" => "F",
                        "key" => "K",
                        "lock" => "L",
                        "coin" => "C",
                        _ => "",
                    };
                    if !text.is_empty() {
                        painter.text(
                            tile_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            text,
                            egui::FontId::monospace((tile_rect.width() * 0.6).max(10.0)),
                            egui::Color32::WHITE,
                        );
                    }
                }
"""
src = re.sub(r"if cell_kind == \"target\" \{[\s\S]*?\}\n\s*\}\n\s*\}", icons + "\n            }\n        }", src)

with open("src/notebook.rs", "w") as f:
    f.write(src)

print("done")
