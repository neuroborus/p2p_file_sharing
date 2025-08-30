use serde_derive::{Deserialize, Serialize};

use crate::entities::file_info::FileInfo;
use crate::entities::file_size_or_info::FileSizeOrInfo;

#[derive(Serialize, Deserialize, Debug)]
/// Serialized request from daemon which want to get file size or start download
/// a file
pub struct HandshakeRequest {
    pub filename: String,
    pub action: FileSizeOrInfo,
}

#[derive(Serialize, Deserialize, Debug)]
/// Stores filename and answer to size request
pub struct HandshakeResponse {
    pub filename: String,
    pub answer: FileInfo,
}
