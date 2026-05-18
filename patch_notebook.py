import re

with open("src/notebook.rs", "r") as f:
    src = f.read()

# Add TraceEvent, VIZ_TRACE, VIZ_STATE
trace_structs = """
#[derive(Debug, Clone)]
pub enum TraceEvent {
    Line(usize),
    RobotState(RobotVizConfig),
    Print(String),
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
"""
src = src.replace("pub const NOTEBOOK_TOGGLE_KEY", trace_structs + "\npub const NOTEBOOK_TOGGLE_KEY")

# Modify ProgrammingNotebook fields
fields = """
    pub puzzle_completed: bool,
    pub trace: Vec<TraceEvent>,
    pub trace_index: usize,
    pub trace_timer: f32,
    pub is_playing: bool,
    pub active_line: Option<usize>,
    pub current_viz_config: Option<RobotVizConfig>,
    pub speed_option: SpeedOption,
"""
src = src.replace("pub puzzle_completed: bool,", fields)

# Modify new()
new_fields = """
            puzzle_completed: false,
            trace: Vec::new(),
            trace_index: 0,
            trace_timer: 0.0,
            is_playing: false,
            active_line: None,
            current_viz_config: None,
            speed_option: SpeedOption::Regular,
"""
src = src.replace("puzzle_completed: false,", new_fields)

# Modify play loop in draw
draw_start = """
    pub fn draw(&mut self) {
        if self.is_playing && !self.runtime_paused {
            self.trace_timer += get_frame_time();
            if self.trace_timer >= self.speed_option.delay() {
                self.trace_timer = 0.0;
                if self.trace_index < self.trace.len() {
                    let event = &self.trace[self.trace_index];
                    match event {
                        TraceEvent::Line(l) => self.active_line = Some(*l),
                        TraceEvent::RobotState(config) => self.current_viz_config = Some(config.clone()),
                        TraceEvent::Print(s) => self.console_text.push_str(&format!("\\n> {}", s)),
                    }
                    self.trace_index += 1;
                } else {
                    self.is_playing = false;
                    self.active_line = None;
                    self.console_text.push_str("\\n> execution finished");
                }
            }
        }
"""
src = src.replace("pub fn draw(&mut self) {", draw_start)

# Modify execute_khoron
execute_khoron = """
    fn execute_khoron(&mut self, source: &str) {
        let starting_config = self.parse_robot_viz_config().unwrap_or_default();
        VIZ_STATE.with(|s| *s.borrow_mut() = starting_config.clone());
        VIZ_TRACE.with(|t| t.borrow_mut().clear());
        
        let mut runtime = KhoronRuntime::default();
        runtime.on_step = Some(Box::new(|line| {
            VIZ_TRACE.with(|t| t.borrow_mut().push(TraceEvent::Line(line)));
        }));
        
        fn make_dict(methods: Vec<(&str, khoron::Value)>) -> khoron::Value {
            let mut map = HashMap::new();
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

        let scan = khoron::Value::NativeFunction(|_| {
            Ok(khoron::Value::String("empty".to_string())) // Mock
        });

        runtime.env.borrow_mut().define("move".to_string(), make_dict(vec![("forward", move_fwd.clone())]));
        runtime.env.borrow_mut().define("turn".to_string(), make_dict(vec![("left", turn_left.clone()), ("right", turn_right.clone())]));
        
        // global aliases
        runtime.env.borrow_mut().define("move_forward".to_string(), move_fwd);
        runtime.env.borrow_mut().define("turn_left".to_string(), turn_left);
        runtime.env.borrow_mut().define("turn_right".to_string(), turn_right);
        runtime.env.borrow_mut().define("scan".to_string(), scan);
        
        match runtime.run(source) {
            Ok(_) => {}
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
    }

"""
src = src.replace("    fn draw_programming_ui(&mut self, ui: &mut egui::Ui, puzzle: bool) {", execute_khoron + "\n    fn draw_programming_ui(&mut self, ui: &mut egui::Ui, puzzle: bool) {")

with open("src/notebook.rs", "w") as f:
    f.write(src)

print("done")
