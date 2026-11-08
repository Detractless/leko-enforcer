//! A request to add a block, as plain values: what to call it, what it closes, when, and how a person
//! gets past it. The command line fills this in from flags; a voice layer or a model can fill in the
//! same thing. It holds values and checks nothing: the drafter and the readers judge them.

use crate::models::specs::DeviceAction;

/// When the block closes what it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleChoice {
    /// All day, on the request's weekdays (every day when none are named).
    Always,
    /// Open only inside these (from, to) stretches written HH:MM (`24:00` allowed as an end), closed at all other times.
    OpenHours(Vec<(String, String)>),
    /// Closed for this many minutes from the moment of the request, with a seal that ends with it.
    ForMinutes(i64),
}

/// How a person gets in while the block is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorChoice {
    /// A wait of 30 seconds for a block with weekdays or hours, no way in for a timed block.
    Default,
    /// No way in.
    Wall,
    /// A wait of this many seconds.
    Wait(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRequest {
    pub name: String,
    /// Apps by package name or bundle id.
    pub apps: Vec<String>,
    /// Sites by host name.
    pub sites: Vec<String>,
    /// What the block does to the whole computer instead of covering apps or sites.
    pub device: Option<DeviceAction>,
    pub rule: RuleChoice,
    /// Weekdays `mon` to `sun`; none means every day. A timed block has none.
    pub days: Vec<String>,
    pub door: DoorChoice,
    pub message: Option<String>,
}
