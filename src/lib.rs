pub use rand::Rng;
use serde_derive::*;
pub use std::{
    collections::{HashMap, LinkedList},
    env, fs, io,
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
//pub enum c_type{share}

pub const ADDR_DAEMON_MULTICAST: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);
pub const PORT_MULTICAST: u16 = 7645;
pub const PORT_CLIENT_DAEMON: u16 = 7646;
pub const PORT_SCAN_TCP: u16 = 7647;
pub const PORT_FILE_SHARE: u16 = 7648;
pub const GET_SELF_IP_PORT: u16 = 60005;
pub const SCAN_REQUEST: &[u8; 20] = b"UDP_Scan_Request_P2P";

#[cfg(windows)]
pub fn bind_multicast(_addr: &Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
    UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), port))
}

#[cfg(unix)]
pub fn bind_multicast(addr: &Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
    UdpSocket::bind((*addr, port))
}

pub fn get_this_daemon_ip() -> io::Result<IpAddr> {
    let unique_number = rand::thread_rng().gen::<u128>();
    let self_ip: IpAddr;
    {
        let local_network: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
        let listener = bind_multicast(&ADDR_DAEMON_MULTICAST, GET_SELF_IP_PORT)?;
        listener
            .join_multicast_v4(&ADDR_DAEMON_MULTICAST, &local_network)
            .unwrap();
        {
            let socket = UdpSocket::bind((local_network, 0)).unwrap();
            socket
                .send_to(
                    unique_number.to_string().as_bytes(),
                    (ADDR_DAEMON_MULTICAST, GET_SELF_IP_PORT),
                )
                .unwrap();
        }
        let mut buf = vec![0; 4096];
        loop {
            let (len, remote_addr) = listener.recv_from(&mut buf).unwrap();
            let msg = &buf[..len];
            let rec_num = u128::from_str(str::from_utf8(msg).unwrap()).unwrap();
            if rec_num == unique_number {
                self_ip = remote_addr.ip();
                break;
            }
            continue;
        }
    }
    println!("Daemon IP in local network is {}", self_ip);
    Ok(self_ip)
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    //Client -> Daemon
    Share {
        file_path: PathBuf,
    },
    Scan,
    Ls,
    Download {
        file_name: String,
        save_path: PathBuf,
        wait: bool,
    },
    Status,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Answer {
    //Daemon -> Client
    Ok,
    Err(String),
    Ls {
        available_map: HashMap<String, Vec<SocketAddr>>,
    }, //available
    Status {
        transferring_map: HashMap<String, Vec<SocketAddr>>,
        shared_map: HashMap<String, PathBuf>,
        downloading_map: Vec<String>,
    },
}

#[derive(Debug)]
pub struct DataTemp {
    //Available to downloading files: name_of_file--shared IP addresses
    pub available: HashMap<String, Vec<SocketAddr>>,
    //pub downloading: HashMap<String, Vec<SocketAddr>>, //Already downloading
    //
    //Your files, that available to transfer
    pub shared: HashMap<String, PathBuf>, //FileName - Path
                                          //pub transferring: HashMap<String, Vec<SocketAddr>>, //Already transferring
}

impl DataTemp {
    pub fn new() -> Self {
        DataTemp {
            available: HashMap::new(),
            //downloading: HashMap::new(),
            shared: HashMap::new(),
            //transferring: HashMap::new(),
        }
    }
}
#[derive(Debug)]
pub struct TransferGuard {
    pub transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
    pub filename: String,
    pub peer: SocketAddr
}

impl TransferGuard {
    pub fn new(_transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>, _filename: String, _peer: SocketAddr) -> Self {
        let guard = TransferGuard {
            transferring: _transferring,
            filename: _filename,
            peer: _peer
        };

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

    {
        let mut transfer_map = self.transferring.lock().unwrap();
        if transfer_map.get(&self.filename).unwrap().len() == 1 {
            transfer_map.remove(&self.filename).unwrap();
        } else {
            let peer_vec: &mut Vec<SocketAddr> = transfer_map.get_mut(&self.filename).unwrap();
            let pos: usize = peer_vec
                .iter()
                .position(|&peer| peer == self.peer)
                .unwrap();
            peer_vec.remove(pos);
        }
    }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FirstRequest {
    pub filename: String,
    pub action: FileSizeorInfo,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum FileSizeorInfo {
    Info(FileInfo),
    Size,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileInfo {
    pub from_block: u32,
    pub to_block: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AnswerToFirstRequest {
    pub filename: String,
    pub answer: EnumAnswer,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum EnumAnswer {
    Size(u64),
    NotExist,
}
