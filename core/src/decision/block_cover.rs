//! Whether a block covers one app or one website host. Apps match by the exact name written in the
//! block; hosts match the block's sites (a listed name or a subdomain of one). An app is never
//! matched against sites, a host never against apps, and an app that is never blocked is covered by
//! no block, not even an all-except one.

use crate::models::specs::{AppCover, BlockSpec};
use crate::sites::site_match::any_site_covers;
use std::collections::HashSet;

/// What is asked about: an app by its name, or a website by its host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    App(&'a str),
    Host(&'a str),
}

/// True when `block` covers `target`. The switch of the block is not looked at here.
pub fn covers(block: &BlockSpec, target: Target, never_blocked: &HashSet<String>) -> bool {
    match target {
        Target::Host(host) => any_site_covers(&block.sites, host),
        Target::App(app) if app.is_empty() || never_blocked.contains(app) => false,
        Target::App(app) => match &block.apps {
            AppCover::Nothing => false,
            AppCover::Named(names) => names.iter().any(|name| name == app),
            AppCover::AllExcept(except) => !except.iter().any(|name| name == app),
        },
    }
}
