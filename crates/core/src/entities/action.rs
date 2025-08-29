use std::path::PathBuf;

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
/// An action that is serialized in the client and sent to the daemon
pub enum Action {
    // Client -> Daemon
    /// Share a file with a network
    Share { file_path: PathBuf },
    /// Scans the network for files that can be downloaded
    Scan,
    /// Show files that can be downloaded
    Ls,
    /// Download a downloadable file (path is optional)
    Download {
        file_name: String,
        save_path: PathBuf,
        wait: bool, // Block input until file is downloaded
    },
    /// Show distributed files
    Status,
}
