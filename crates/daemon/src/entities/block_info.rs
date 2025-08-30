use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
/// Store which blocks are downloadable
pub struct BlockInfo {
    pub from_block: u32,
    pub to_block: u32,
}
