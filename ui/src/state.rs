use crate::card::CardState;
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
    Stopped,
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
    pub card_active: bool,
    pub card_text: String,
    pub card_columns: Vec<String>,
    pub card_rows: Vec<Vec<String>>,
    pub card_width: u32,
    pub card_height: u32,
}

pub struct Machine {
    pub state: PillState,
    pub timer: String,
    pub blocked_until: f64,
    pub stopped_until: f64,
    pub quiet_timer: bool,
    pub hovering: bool,
    pub card: CardState,
}

impl Machine {
    pub fn new(quiet_timer: bool) -> Self {
        Machine {
            state: PillState::Idle,
            timer: String::new(),
            blocked_until: 0.0,
            stopped_until: 0.0,
            quiet_timer,
            hovering: false,
            card: CardState::default(),
        }
    }

    pub fn apply_value(&mut self, value: &serde_json::Value, now: f64, duration: f64) -> Snapshot {
        let event = value.get("event").and_then(|v| v.as_str()).unwrap_or("");
        match event {
            "blocked" => {
                self.state = PillState::Blocked;
                self.blocked_until = now + 2.2;
                self.card.clear();
            }
            "ptt_start" => self.state = PillState::Held,
            "ptt_end" | "grace" => self.state = PillState::Listening,
            "speaking" => {
                self.state = PillState::Speaking;
                if self.card.active {
                    self.card.speaking = true;
                    self.card.collapse_at = None;
                }
            }
            "thinking" => {
                self.state = PillState::Thinking;
                if let Some(q) = value.get("query").and_then(|v| v.as_str()) {
                    if !q.is_empty() {
                        self.card.open(q.to_string(), vec![], vec![]);
                    }
                }
            }
            "stop" | "stopped" => {
                self.state = PillState::Stopped;
                self.stopped_until = now + 1.2;
                self.card.clear();
            }
            "error" => {
                self.state = PillState::Error;
                self.card.clear();
            }
            "idle" => {
                self.state = self.rest();
                if self.card.active && self.card.speaking {
                    self.card.speaking = false;
                    if !self.card.hovered {
                        self.card.collapse_at = Some(now + 4.0);
                    }
                }
            }
            "timer_start" => {
                self.timer = clock(duration);
                if self.quiet_timer && matches!(self.state, PillState::Idle | PillState::Timer) {
                    self.state = PillState::Timer;
                }
            }
            "timer_stop" => {
                self.timer.clear();
                self.state = PillState::Idle;
            }
            "hover_on" => self.hovering = true,
            "hover_off" => self.hovering = false,
            "reply_card" => {
                self.card.apply_reply(value);
                if matches!(self.state, PillState::Held | PillState::Listening | PillState::Thinking) {
                    self.state = self.rest();
                }
            }
            "force_release" => {
                if matches!(self.state, PillState::Held | PillState::Listening) {
                    self.state = self.rest();
                }
            }
            "card_hover_on" => {
                self.card.hovered = true;
                self.card.collapse_at = None;
            }
            "card_hover_off" => {
                self.card.hovered = false;
                if self.card.active && !self.card.speaking {
                    self.card.collapse_at = Some(now + 2.0);
                }
            }
            "card_dismiss" | "escape" | "outside_click" => self.card.clear(),
            _ => {}
        }
        if self.state == PillState::Blocked && now >= self.blocked_until {
            self.state = self.rest();
        }
        if self.state == PillState::Stopped && now >= self.stopped_until {
            self.state = self.rest();
        }
        if self.card.active && !self.card.hovered {
            if let Some(at) = self.card.collapse_at {
                if now >= at {
                    self.card.clear();
                }
            }
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
            PillState::Stopped => ("", 112, 30, 7, false, "stopped"),
            PillState::Timer => (self.timer.as_str(), 68, 24, 0, false, "timer"),
            PillState::Listening | PillState::Held => ("", 112, 30, 7, false,
                if self.state == PillState::Held { "held" } else { "listening" }),
            PillState::Thinking => ("", 82, 28, 0, false, "thinking"),
            PillState::Speaking => ("", 112, 30, 7, false, "speaking"),
            PillState::Idle => ("", if self.hovering { 72 } else { 54 },
                if self.hovering { 24 } else { 16 }, 0, true, "idle"),
        };
        let (card_width, card_height) = self.card.desired_size();
        Snapshot {
            state: state.to_string(),
            text: text.to_string(),
            width,
            height,
            timer: self.timer.clone(),
            bars,
            dash,
            card_active: self.card.active,
            card_text: self.card.text.clone(),
            card_columns: self.card.columns.clone(),
            card_rows: self.card.rows.clone(),
            card_width,
            card_height,
        }
    }
}

pub fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}
