pub use rand::Rng;
use serde_derive::*;
pub use std::{
    collections::{HashMap, LinkedList},
    fs, io,
    io::{prelude::*, Read, SeekFrom, Write},
    net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket},
    path::PathBuf,
    str,
    str::FromStr,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::Duration,
};
pub use threadpool::ThreadPool;

pub const ADDR_DAEMON_MULTICAST: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);
pub const PORT_MULTICAST: u16 = 7645;
pub const PORT_CLIENT_DAEMON: u16 = 7646;
pub const PORT_SCAN_TCP: u16 = 7647;
pub const PORT_FILE_SHARE: u16 = 7648;
pub const GET_SELF_IP_PORT: u16 = 60005;
pub const SCAN_REQUEST: &[u8; 20] = b"UDP_Scan_Request_P2P";

#[derive(Serialize, Deserialize, Debug)]
///A command that is serialized in the client and sent to the daemon
pub enum Command {
    //Client -> Daemon
    ///Share a file with a network
    Share { file_path: PathBuf },
    ///Scans the network for files that can be downloaded
    Scan,
    ///Show files that can be downloaded
    Ls,
    ///Download a downloadable file (path is optional)
    Download {
        file_name: String,
        save_path: PathBuf,
        wait: bool, //Block input until file is downloaded
    },
    ///Show distributed files
    Status,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
///The response to the command is serialized in the daemon and sent to the client
pub enum Answer {
    //Daemon -> Client
    Ok,
    Err(String),
    ///Available to download
    Ls {
        available_map: HashMap<String, Vec<SocketAddr>>,
    },
    ///Distributed files
    Status {
        transferring_map: HashMap<String, Vec<SocketAddr>>,
        shared_map: HashMap<String, PathBuf>,
        downloading_map: Vec<String>,
    },
}

#[derive(Debug)]
///Contain HashMaps of available for transfer and download files
pub struct DataTemp {
    //Available to downloading files: name_of_file--shared IP addresses
    pub available: HashMap<String, Vec<SocketAddr>>,
    //Your files, that available to transfer
    pub shared: HashMap<String, PathBuf>, //FileName - Path
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
///Adding peer to transferring vector when created, and removing peer from vector while destroying
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
        //Pushing the peer to vector
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
    //Removing the peer from vector
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
///Serialized request from daemon which want to get file size or start download a file
pub struct FirstRequest {
    pub filename: String,
    pub action: FileSizeorInfo,
}

#[derive(Serialize, Deserialize, Debug)]
///Stores an action downloadable peer want to do
pub enum FileSizeorInfo {
    Info(BlockInfo),
    Size,
}

#[derive(Serialize, Deserialize, Debug)]
///Store which blocks are downloadable
pub struct BlockInfo {
    pub from_block: u32,
    pub to_block: u32,
}

#[derive(Serialize, Deserialize, Debug)]
///Stores filename and answer to size request
pub struct AnswerToFirstRequest {
    pub filename: String,
    pub answer: FileInfo,
}

#[derive(Serialize, Deserialize, Debug)]
///If downloadable peer asked file size we answering with size or file not exist
pub enum FileInfo {
    Size(u64),
    NotExist,
}
