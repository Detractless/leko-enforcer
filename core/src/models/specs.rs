//! What the readers return for each settings file: only the parts the cross-file checks and the
//! judge need. Lists keep the order the files were read in (the order of their paths in the zip).

use serde_json::Value;

/// Which fields a reader accepts: exactly what the phone 0.3 reads (`Phone`), or that and the
/// additions Leko Enforcer reads on top of format 2 (`Extended`): sites, session rules, moment seals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMode {
    Phone,
    Extended,
}

/// A partner key as the file writes it: PBKDF2 (`iterations` and `salt` set) or the old unsalted
/// SHA-256 form (both `None`).
#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub alg: String,
    pub iterations: Option<i64>,
    pub salt: Option<String>,
    pub hash: String,
}

/// How hard an item is to loosen. A missing lock is `Open` with no key.
#[derive(Debug, Clone, PartialEq)]
pub enum Lock {
    Open { key: Option<Key> },
    Wait { hours: i64, key: Option<Key> },
    ChangeHours { from: i64, to: i64, hours: i64, key: Option<Key> },
    SealedUntil { until: String, key: Option<Key> },
    /// Not before an exact instant, in milliseconds since the epoch (extended mode only).
    SealedAt { at: i64, key: Option<Key> },
}

impl Lock {
    pub fn open() -> Lock {
        Lock::Open { key: None }
    }

    pub fn key(&self) -> Option<&Key> {
        match self {
            Lock::Open { key } | Lock::Wait { key, .. } | Lock::ChangeHours { key, .. } | Lock::SealedUntil { key, .. } | Lock::SealedAt { key, .. } => key.as_ref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Door {
    Wall,
    Wait { seconds: i64 },
    Sentence,
    Key { key: Key },
    Practice { practice: String },
    Walking { steps_per_minute: i64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pass {
    pub length: String,
    pub minutes: Option<i64>,
    pub scope: String,
    /// `None` is the word "unlimited".
    pub per_day: Option<i64>,
    pub rising_cost: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Earn {
    pub practice: String,
    pub minutes_each: i64,
    pub max_per_day: i64,
}

/// A rule as read: what it is, and the parts the judge at an instant needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub id: String,
    pub kind: String,
    /// The weekdays it applies on ("mon" to "sun", in week order); empty is every day.
    pub days: Vec<String>,
    /// The allowed stretches of an outside-hours rule as (from, to) minutes after midnight; `to` may be 1440.
    pub allowed: Vec<(i64, i64)>,
    /// The end of a session rule, in milliseconds since the epoch.
    pub ends_at: Option<i64>,
}

/// Which apps a block covers: those it names, every app but those it names, or none (it covers sites alone).
#[derive(Debug, Clone, PartialEq)]
pub enum AppCover {
    Nothing,
    Named(Vec<String>),
    AllExcept(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct BlockSpec {
    pub id: String,
    /// The switch: a block with no `on` field is on, as on the phone.
    pub on: bool,
    pub apps: AppCover,
    /// The sites the block covers (extended mode only; empty otherwise).
    pub sites: Vec<String>,
    pub rules: Vec<Rule>,
    pub door: Door,
    pub pass: Option<Pass>,
    pub earn: Option<Earn>,
    pub lock: Lock,
}

#[derive(Debug, Clone)]
pub struct PracticeSpec {
    pub id: String,
    pub deck: Option<String>,
    /// Every deck the practice draws from: its own, or its flashcard parts'.
    pub decks: Vec<String>,
    pub book: Option<String>,
    /// The deck each part names (`None` for a part with no deck).
    pub parts: Vec<Option<String>>,
    pub lock: Lock,
}

#[derive(Debug, Clone)]
pub struct RoutineSpec {
    pub id: String,
    pub practice: String,
    pub hold_blocks: Vec<String>,
    pub lock: Lock,
}

/// A deck or a book: only its id and lock matter after reading.
#[derive(Debug, Clone)]
pub struct ContentSpec {
    pub id: String,
    pub lock: Lock,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Protection {
    Key { key: Key },
    Wait { days: i64 },
}

#[derive(Debug, Clone)]
pub struct Basics {
    pub protection: Option<Protection>,
    pub lock: Lock,
}

/// A list of specs by id, in the order read.
#[derive(Debug, Clone)]
pub struct Table<T> {
    pub items: Vec<(String, T)>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Table { items: Vec::new() }
    }
}

impl<T> Table<T> {
    pub fn insert(&mut self, id: String, item: T) {
        self.items.push((id, item));
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.items.iter().find(|(key, _)| key == id).map(|(_, item)| item)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|(_, item)| item)
    }
}

/// Every settings file of a zip, read. `docs` holds each file's parsed JSON under its zip path.
#[derive(Debug, Clone, Default)]
pub struct ReadSet {
    pub basics: Option<Basics>,
    pub blocks: Table<BlockSpec>,
    pub practices: Table<PracticeSpec>,
    pub routines: Table<RoutineSpec>,
    pub decks: Table<ContentSpec>,
    pub books: Table<ContentSpec>,
    pub docs: Vec<(String, Value)>,
}
