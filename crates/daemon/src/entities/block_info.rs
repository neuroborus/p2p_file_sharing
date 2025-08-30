use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
/// Range of file blocks available for download (start inclusive, end
/// exclusive).
pub struct BlockInfo {
    /// First block index in the range (inclusive).
    pub from_block: u32,
    /// Block index at which the range ends (exclusive).
    pub to_block: u32,
}
