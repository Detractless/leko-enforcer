use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PillState {
    Idle,
    Timer,
    Blocked,
    Listening,
    Held,
    Thinking,
    Speaking,
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub state: String,
    pub text: String,
    pub width: u32,
    pub height: u32,
    pub timer: String,
    pub bars: u8,
    pub dash: bool,
}

pub struct Machine {
    pub state: PillState,
    pub timer: String,
    pub blocked_until: f64,
    pub quiet_timer: bool,
    pub hovering: bool,
}

impl Machine {
    pub fn new(quiet_timer: bool) -> Self {
        Machine {
            state: PillState::Idle,
            timer: String::new(),
            blocked_until: 0.0,
            quiet_timer,
            hovering: false,
        }
    }

    pub fn apply(&mut self, event: &str, now: f64, duration: f64) -> Snapshot {
        match event {
            "blocked" => {
                self.state = PillState::Blocked;
                self.blocked_until = now + 2.2;
            }
            "ptt_start" => self.state = PillState::Held,
            "ptt_end" => self.state = PillState::Listening,
            "speaking" => self.state = PillState::Speaking,
            "thinking" => self.state = PillState::Thinking,
            "error" => self.state = PillState::Error,
            "idle" => self.state = self.rest(),
            "timer_start" => {
                self.timer = clock(duration);
                if self.quiet_timer {
                    self.state = PillState::Timer;
                }
            }
            "timer_stop" => {
                self.timer.clear();
                self.state = PillState::Idle;
            }
            "hover_on" => self.hovering = true,
            "hover_off" => self.hovering = false,
            _ => {}
        }
        if self.state == PillState::Blocked && now >= self.blocked_until {
            self.state = self.rest();
        }
        self.snapshot()
    }

    fn rest(&self) -> PillState {
        if self.quiet_timer && !self.timer.is_empty() {
            PillState::Timer
        } else {
            PillState::Idle
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let (text, width, height, bars, dash, state) = match self.state {
            PillState::Blocked => ("Blocked", 92, 28, 0, false, "blocked"),
            PillState::Error => ("Error", 92, 28, 0, false, "error"),
            PillState::Timer => (self.timer.as_str(), 68, 24, 0, false, "timer"),
            PillState::Listening | PillState::Held => ("", 112, 30, 5, false,
                if self.state == PillState::Held { "held" } else { "listening" }),
            PillState::Thinking | PillState::Speaking => ("MAEVE", 112, 30, 3, false,
                if self.state == PillState::Speaking { "speaking" } else { "thinking" }),
            PillState::Idle => ("", if self.hovering { 72 } else { 54 },
                if self.hovering { 24 } else { 16 }, 0, true, "idle"),
        };
        Snapshot {
            state: state.to_string(),
            text: text.to_string(),
            width,
            height,
            timer: self.timer.clone(),
            bars,
            dash,
        }
    }
}

pub fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}
