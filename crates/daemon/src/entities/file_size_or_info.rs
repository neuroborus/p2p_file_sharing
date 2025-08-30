use serde::{Deserialize, Serialize};

use crate::entities::block_info::BlockInfo; // import from the parent layer

#[derive(Serialize, Deserialize, Debug)]
/// Request from a peer to either get file size or download a specific block
/// range.
pub enum FileSizeOrInfo {
    /// Request to download a specific range of blocks.
    Info(BlockInfo),
    /// Request only the file size.
    Size,
}
