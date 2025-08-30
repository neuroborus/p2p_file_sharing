use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug)]
/// Contain HashMaps of available for transfer and download files
pub struct FileState {
    // Available to downloading files: name_of_file--shared IP addresses
    pub available: HashMap<String, Vec<SocketAddr>>,
    // Your files, that available to transfer
    pub shared: HashMap<String, PathBuf>, // FileName - Path
}

impl FileState {
    pub fn new() -> Self {
        FileState {
            available: HashMap::new(),
            shared: HashMap::new(),
        }
    }
}
