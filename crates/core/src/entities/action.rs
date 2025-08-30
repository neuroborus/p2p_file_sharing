use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
/// Client-to-daemon actions.
pub enum Action {
    /// Announce a local file for sharing.
    Share { file_path: PathBuf },
    /// Discover peers and their shared files on the local network.
    Scan,
    /// List files currently known as downloadable (from the last scan).
    Ls,
    /// Request to download a file.
    Download {
        /// File name (as advertised by peers).
        file_name: String,
        /// Destination path for the downloaded file.
        save_path: PathBuf,
        /// If `true`, block until the download completes.
        wait: bool,
    },
    /// Show share/download status (shared, downloading, transferring).
    Status,
}
