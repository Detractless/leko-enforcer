//! Android adapter stub. It implements no OS behavior yet and must never
//! change core decisions. Future VPN/usage-stats adapters plug in here.

use leko_core::enforcement::{BrowserId, EnforcementAdapter};

pub const STATUS: &str = "not_configured";

pub struct AndroidEnforcementAdapter;

impl EnforcementAdapter for AndroidEnforcementAdapter {
    fn warn(&mut self, _browser: &BrowserId) {}
    fn close(&mut self, _browser: &BrowserId) {}
}

pub fn adapter_status() -> &'static str {
    STATUS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_reports_not_configured() {
        assert_eq!(adapter_status(), "not_configured");
    }
}
