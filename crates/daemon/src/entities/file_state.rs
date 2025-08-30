use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug)]
/// Tracks files available from other peers and files shared by this daemon.
pub struct FileState {
    /// Files available for download: file name → list of peer addresses.
    pub available: HashMap<String, Vec<SocketAddr>>,
    /// Local files currently shared: file name → full file path.
    pub shared: HashMap<String, PathBuf>, // FileName - Path
}

impl FileState {
    /// Create an empty `FileState`.
    pub fn new() -> Self {
        FileState {
            available: HashMap::new(),
            shared: HashMap::new(),
        }
    }
}
