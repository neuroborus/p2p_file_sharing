use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
/// Response to a peer's file-size request.
pub enum FileInfo {
    /// File exists — return its size in bytes.
    Size(u64),
    /// File does not exist.
    NotExist,
}
