use serde_derive::{Deserialize, Serialize};

use crate::entities::block_info::BlockInfo; // import from the parent layer

#[derive(Serialize, Deserialize, Debug)]
/// Stores an action downloadable peer want to do
pub enum FileSizeOrInfo {
    Info(BlockInfo),
    Size,
}
