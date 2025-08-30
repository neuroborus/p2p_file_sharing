use serde::{Deserialize, Serialize};

use crate::entities::file_info::FileInfo;
use crate::entities::file_size_or_info::FileSizeOrInfo;

#[derive(Serialize, Deserialize, Debug)]
/// Request sent by a peer to either obtain a file size or start a download.
pub struct HandshakeRequest {
    /// Name of the file being requested.
    pub filename: String,
    /// Requested action: get size or download a block range.
    pub action: FileSizeOrInfo,
}

#[derive(Serialize, Deserialize, Debug)]
/// Response to a handshake request, containing the file name and the result.
pub struct HandshakeResponse {
    /// Name of the file the response refers to.
    pub filename: String,
    /// Result of the request: file size or "not exist".
    pub answer: FileInfo,
}
