//! The bytes of a settings zip with one block switched off: the block's file gets `"on": false`,
//! every other field of the block and every other file of the zip keep their names, contents and
//! order. The zip is read first and refused if the core refuses it, and the new zip is read again
//! before it is returned, so a zip that the core would refuse is never handed back. The input is
//! never changed.

use crate::blocks::block_flag_zip::set_block_flag;

/// `bytes` with the block `id` switched off, or the sentence saying why that cannot be done.
pub fn switch_block_off(bytes: &[u8], id: &str) -> Result<Vec<u8>, String> {
    set_block_flag(bytes, id, false)
}
