//! Leko native core: canonical 5-pillar rules with deterministic evaluators.
//! Pure logic takes an injected clock; only io::SystemClock reads the real time.

pub mod clock;
pub mod domain;
pub mod countdowns {
    pub use crate::domain::{
        compute_countdowns, rule_countdowns, sort_countdowns, CountdownItem, CountdownsReport,
    };
}

#[path = "domain/enforcement_bridge.rs"]
pub mod enforcement;
pub mod error;
pub mod io;
pub mod models;
pub mod services;
pub mod store;

pub use clock::{Clock, FixedClock, Moment};
pub use error::{CoreError, CoreResult};
pub use models::TechnologyManagementRule;
