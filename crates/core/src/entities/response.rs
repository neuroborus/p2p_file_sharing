use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
/// Daemon-to-client responses.
pub enum Response {
    /// Operation succeeded.
    Ok,
    /// Operation failed with an error message.
    Err(String),
    /// Files currently available for download.
    Ls {
        /// Map: file name → list of peer addresses.
        available_map: HashMap<String, Vec<SocketAddr>>,
    },
    /// Current sharing and transfer state.
    Status {
        /// Files being transferred to peers.
        transferring_map: HashMap<String, Vec<SocketAddr>>,
        /// Files shared by this daemon.
        shared_map: HashMap<String, PathBuf>,
        /// Files currently being downloaded.
        downloading_map: Vec<String>,
    },
}
