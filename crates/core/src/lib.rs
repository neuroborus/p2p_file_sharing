// TODO: remove pub and re-import
pub use std::collections::{HashMap, LinkedList};
pub use std::io::prelude::*;
pub use std::io::{Read, SeekFrom, Write};
pub use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
pub use std::path::PathBuf;
pub use std::str::FromStr;
pub use std::sync::mpsc::{Receiver, Sender};
pub use std::sync::{Arc, Mutex, MutexGuard, mpsc};
pub use std::time::Duration;
pub use std::{fs, io, str, thread};

pub use rand::Rng;
use serde_derive::*;
pub use threadpool::ThreadPool;

pub mod helpers;

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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
/// The response to the action is serialized in the daemon and sent to the
/// client
pub enum Answer {
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

#[derive(Debug)]
/// Contain HashMaps of available for transfer and download files
pub struct DataTemp {
    // Available to downloading files: name_of_file--shared IP addresses
    pub available: HashMap<String, Vec<SocketAddr>>,
    // Your files, that available to transfer
    pub shared: HashMap<String, PathBuf>, // FileName - Path
}

impl DataTemp {
    pub fn new() -> Self {
        DataTemp {
            available: HashMap::new(),
            shared: HashMap::new(),
        }
    }
}
#[derive(Debug)]
/// Adding peer to transferring vector when created, and removing peer from
/// vector while destroying
pub struct TransferGuard {
    pub transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
    pub filename: String,
    pub peer: SocketAddr,
}

impl TransferGuard {
    pub fn new(
        _transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
        _filename: String,
        _peer: SocketAddr,
    ) -> Self {
        let guard = TransferGuard {
            transferring: _transferring,
            filename: _filename,
            peer: _peer,
        };
        // Pushing the peer to vector
        {
            let mut transfer_map = guard.transferring.lock().unwrap();
            match transfer_map.get_mut(&guard.filename) {
                Some(addr_vec) => {
                    addr_vec.push(guard.peer.clone());
                }
                None => {
                    let mut v: Vec<SocketAddr> = Vec::new();
                    v.push(guard.peer);
                    transfer_map.insert(guard.filename.clone(), v);
                }
            }
        }
        guard
    }
}

impl Drop for TransferGuard {
    fn drop(&mut self) {
        // Removing the peer from vector
        {
            let mut transfer_map = self.transferring.lock().unwrap();
            if transfer_map.get(&self.filename).unwrap().len() == 1 {
                transfer_map.remove(&self.filename).unwrap();
            } else {
                let peer_vec: &mut Vec<SocketAddr> = transfer_map.get_mut(&self.filename).unwrap();
                let pos: usize = peer_vec.iter().position(|&peer| peer == self.peer).unwrap();
                peer_vec.remove(pos);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// Serialized request from daemon which want to get file size or start download
/// a file
pub struct FirstRequest {
    pub filename: String,
    pub action: FileSizeorInfo,
}

#[derive(Serialize, Deserialize, Debug)]
/// Stores an action downloadable peer want to do
pub enum FileSizeorInfo {
    Info(BlockInfo),
    Size,
}

#[derive(Serialize, Deserialize, Debug)]
/// Store which blocks are downloadable
pub struct BlockInfo {
    pub from_block: u32,
    pub to_block: u32,
}

#[derive(Serialize, Deserialize, Debug)]
/// Stores filename and answer to size request
pub struct AnswerToFirstRequest {
    pub filename: String,
    pub answer: FileInfo,
}

#[derive(Serialize, Deserialize, Debug)]
/// If downloadable peer asked file size we answering with size or file not
/// exist
pub enum FileInfo {
    Size(u64),
    NotExist,
}
