use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
/// The response to the action is serialized in the daemon and sent to the
/// client
pub enum Response {
    // Daemon -> Client
    Ok,
    Err(String),
    /// Available to download
    Ls {
        available_map: HashMap<String, Vec<SocketAddr>>,
    },
    /// Distributed files
    Status {
        transferring_map: HashMap<String, Vec<SocketAddr>>,
        shared_map: HashMap<String, PathBuf>,
        downloading_map: Vec<String>,
    },
}
