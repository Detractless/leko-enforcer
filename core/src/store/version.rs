use crate::error::{CoreError, CoreResult};

pub const CURRENT_VERSION: u32 = 1;

pub fn check_version(version: u32) -> CoreResult<()> {
    if version == CURRENT_VERSION {
        Ok(())
    } else {
        Err(CoreError::UnknownVersion(version))
    }
}
