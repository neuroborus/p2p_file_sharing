use std::collections::HashMap;
use std::io::prelude::*;
use std::io::{Read, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use std::{fs, io, thread};

use p2p_config::{
    CHUNK_SIZE, DAEMON_MULTICAST_ADDR, LOCAL_NETWORK, PORT_FILE_SHARE, PORT_MULTICAST, SCAN_REQUEST,
};
use p2p_core::entities::{Action, Response};
use serde_json::{from_slice, to_string};
use threadpool::ThreadPool;

mod entities;
pub use entities::{
    BlockInfo, FileInfo, FileSize, FileSizeOrInfo, FileState, HandshakeRequest, HandshakeResponse,
    TransferGuard,
};

mod utils;
pub use utils::{LOGGER, blocks_count};

mod multicast;
pub use multicast::*;

/// Handle a client action on the daemon side.
pub fn action_processor(
    action: &Action,
    mut stream: TcpStream,
    // Shared state: local `shared` and remote `available` files.
    mut data: MutexGuard<FileState>,
    // Tracks peers currently receiving each file.
    transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
    // Tracks files this daemon is downloading.
    downloading: Arc<Mutex<Vec<String>>>,
) -> io::Result<()> {
    match action {
        Action::Share { file_path: f_path } => {
            // Register the file under its basename.
            let name: String = String::from(f_path.file_name().unwrap().to_string_lossy());
            data.shared.insert(name, f_path.clone());

            let answ = Response::Ok;
            let serialized = to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap();

            LOGGER.debug(format!("responder: sent {} names", data.shared.len()));
        }
        Action::Download {
            file_name: f_name,
            save_path: s_path,
            wait: wat,
        } => {
            // Validate that the file is available and not already shared by us.
            let answ = if !data.available.contains_key(f_name) {
                Response::Err(String::from("File is not available to download!"))
            } else if data.shared.contains_key(f_name) {
                Response::Err(String::from("You are already sharing this file!"))
            } else {
                Response::Ok
            };

            if let Response::Ok = answ {
                // Launch a background download; optionally wait for completion.
                let available_list = data.available.get(f_name).unwrap().clone();
                let filename = f_name.clone();
                let savepath = s_path.clone();
                let t_handle = thread::spawn(move || {
                    if let Err(e) =
                        download_request(filename.clone(), savepath, available_list, downloading)
                    {
                        LOGGER.error(&format!("Failed to download {filename}: {e}"));
                    }
                });
                if *wat {
                    t_handle.join().unwrap();
                }
            }

            // Reply to client.
            let serialized = to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap(); // Answer to client
        }
        Action::Scan => {
            // Clear local cache; multicast_receiver will repopulate it.
            let socket = UdpSocket::bind((LOCAL_NETWORK, 0))?;
            data.available.clear();
            socket.send_to(SCAN_REQUEST, (DAEMON_MULTICAST_ADDR, PORT_MULTICAST))?;

            let answ = Response::Ok;
            let serialized = to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
        Action::Ls => {
            let answ: Response = Response::Ls {
                available_map: data.available.clone(),
            };
            let serialized = to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
        Action::Status => {
            let answ: Response = Response::Status {
                transferring_map: transferring.lock().unwrap().clone(),
                shared_map: data.shared.clone(),
                downloading_map: downloading.lock().unwrap().clone(),
            };
            let serialized = to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
    }

    Ok(())
}

/// Accept and serve incoming file-transfer requests.
pub fn share_responder(
    transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
    data: Arc<Mutex<FileState>>,
) -> io::Result<()> {
    let listener = TcpListener::bind((LOCAL_NETWORK, PORT_FILE_SHARE))?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                let peer_transferring = transferring.clone();
                let shared = data.lock().unwrap().shared.clone();
                thread::spawn(move || {
                    // Giving a response for sharing a file with the other daemon
                    let peer = _stream.peer_addr().unwrap().ip();
                    match transfer_to_peer(_stream, peer_transferring, shared) {
                        Ok(_) => (),
                        Err(e) => {
                            LOGGER.error(&format!(
                                "Interrupted sharing to {} -> an error occured: {}",
                                peer, e
                            ));
                        }
                    }
                });
            }
            Err(e) => {
                LOGGER.error(e);
            }
        }
    }
    Ok(())
}

/// Stream requested file blocks to a peer.
pub fn transfer_to_peer(
    mut stream: TcpStream,
    transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
    shared: HashMap<String, PathBuf>,
) -> io::Result<()> {
    LOGGER.debug("transfer: waiting first request...");
    let mut buf = vec![0; CHUNK_SIZE];

    // I/O timeouts for the transfer session.
    stream.set_read_timeout(Some(Duration::new(45, 0)))?;
    stream.set_write_timeout(Some(Duration::new(45, 0)))?; // setting the timeouts

    let file_info: BlockInfo;
    let file_name: String;
    let file_size: FileSize;

    // First message is a handshake: either size query or download request.
    match stream.read(&mut buf) {
        Ok(size) => {
            LOGGER.debug(format!(
                "transfer: first req {} bytes from {}",
                size,
                stream.peer_addr().unwrap()
            ));
            let request: HandshakeRequest = from_slice(&buf[..size])?;
            match handle_handshake(shared, request, stream) {
                // processing the first request
                Some((info, name, size, s)) => {
                    file_info = info;
                    file_name = name;
                    file_size = size;
                    stream = s;
                }
                None => {
                    // Size-only request answered; nothing more to do.
                    return Ok(());
                }
            }
            LOGGER.debug(format!(
                "transfer: start file='{}' size={} peer={} range=[{}, {})",
                file_name,
                file_size,
                stream.peer_addr().unwrap(),
                file_info.from_block,
                file_info.to_block
            ));
        }
        Err(e) => {
            LOGGER.error(&e);
            return Err(e);
        }
    }

    let blocks: u32;
    // !: Register this transfer; the guard removes the peer on drop.
    // While TransferGuard is creating he's adding a peer to the vector,
    // that stores a daemons, which download a file from us
    let _transfer_guard = TransferGuard::new(
        transferring.clone(),
        file_name.clone(),
        stream.peer_addr().unwrap(),
    );
    // and when something is wrong (like we cannot open the file
    // or the stream write timeout is ended),
    // guard will automatically remove the daemon address from vector

    blocks = blocks_count(file_size, CHUNK_SIZE);
    let last_block_size = (file_size as usize) % CHUNK_SIZE;

    LOGGER.debug(format!(
        "transfer: blocks={} last_block_size={}",
        blocks, last_block_size
    ));

    // Send the requested block range.
    let mut file = fs::File::open(&file_name)?;
    file.seek(SeekFrom::Start(
        CHUNK_SIZE as u64 * file_info.from_block as u64,
    ))?;
    for i in file_info.from_block..file_info.to_block {
        if i == blocks - 1 {
            // The last block may be shorter than CHUNK_SIZE; read only the remainder.
            if last_block_size == 0 {
                break;
            }
            buf.resize(last_block_size, 0u8);
        }
        file.read_exact(&mut buf)?;
        stream.write_all(&buf).unwrap();
    }

    Ok(())
}

/// Handle the first request: file-size query or a download start.
pub fn handle_handshake(
    shared: HashMap<String, PathBuf>,
    request: HandshakeRequest,
    mut stream: TcpStream,
) -> Option<(BlockInfo, String, u64, TcpStream)> {
    let asked_filename: String = request.filename;
    match request.action {
        FileSizeOrInfo::Size => {
            // Return file size (or NotExist).
            let answ = if !shared.contains_key(&asked_filename) {
                LOGGER.info(&format!(
                    "{} asked for non-existing file",
                    stream.peer_addr().unwrap().ip()
                ));
                HandshakeResponse {
                    filename: asked_filename.clone(),
                    answer: FileInfo::NotExist,
                }
            } else {
                let size_of_file = std::fs::metadata(shared.get(&asked_filename).unwrap())
                    .unwrap()
                    .len();
                LOGGER.info(&format!(
                    "{} asked size of {}",
                    stream.peer_addr().unwrap().ip(),
                    &asked_filename
                ));
                HandshakeResponse {
                    filename: asked_filename.clone(),
                    answer: FileInfo::Size(size_of_file),
                }
            };

            let serialized = to_string(&answ).unwrap();
            stream.write_all(serialized.as_bytes()).unwrap(); //  Sending the answer
            return None;
        }
        FileSizeOrInfo::Info(info) => {
            // If the other daemon wants to start download
            LOGGER.info(&format!(
                "Starting sharing a {} to {}",
                &asked_filename,
                stream.peer_addr().unwrap().ip()
            ));

            return Some((
                info,
                asked_filename.clone(),
                std::fs::metadata(shared.get(&asked_filename).unwrap())
                    .unwrap()
                    .len(),
                stream,
            ));
        }
    }
}

/// Query peers for the file size and return those reporting a size.
pub fn get_fsizes(
    peer_list: Vec<SocketAddr>,
    file_name: String,
) -> io::Result<Vec<(SocketAddr, u64)>> {
    LOGGER.debug(format!(
        "fsizes: peers_in={} file='{}'",
        peer_list.len(),
        file_name
    ));
    let mut buf = vec![0; CHUNK_SIZE];
    let mut peers: Vec<(SocketAddr, u64)> = Vec::new();
    let request_to_get_size = to_string(&HandshakeRequest {
        filename: file_name.clone(),
        action: FileSizeOrInfo::Size,
    })?;
    let mut refresh = true;

    for peer in peer_list.iter() {
        let mut stream: TcpStream;
        LOGGER.debug(format!("fsizes: connect {}:{}", peer.ip(), PORT_FILE_SHARE));
        match TcpStream::connect((peer.ip(), PORT_FILE_SHARE)) {
            Ok(_stream) => {
                stream = _stream;
            }
            Err(e) => {
                LOGGER.debug(format!("fsizes: connect error to {} -> {}", peer.ip(), e));
                LOGGER.error(&format!(
                    "Error while connecting to {} to download a file {}",
                    peer.ip(),
                    e
                ));

                continue;
            }
        }
        stream.set_read_timeout(Some(Duration::new(30, 0)))?;
        stream.set_write_timeout(Some(Duration::new(30, 0)))?;
        stream.write_all(request_to_get_size.as_bytes())?; //sending the request to other daemons

        LOGGER.debug("fsizes: send size request");
        match stream.read(&mut buf) {
            Ok(size) => {
                LOGGER.debug(format!(
                    "fsizes: read {} bytes from {}",
                    size,
                    stream.peer_addr()?
                ));
                let answer: HandshakeResponse = from_slice(&buf[..size])?;
                match answer.answer {
                    FileInfo::Size(file_size) => {
                        // If daemon sharing a file, we pushing his ip and size to vec
                        LOGGER.debug(format!(
                            "fsizes: {} reports size={}",
                            stream.peer_addr()?,
                            file_size
                        ));
                        peers.push((stream.peer_addr()?, file_size));
                    }
                    FileInfo::NotExist => {
                        // The peer does not share the file; advise to rescan once.
                        if refresh {
                            LOGGER.info(
                                "That peer doesn't share the file. Please rescan the network.",
                            );
                            LOGGER.debug(format!("fsizes: {} -> NotExist", stream.peer_addr()?));

                            refresh = false;
                        }
                    }
                }
            }
            Err(e) => {
                LOGGER.error(&format!(
                    "Error {} while interracting with {:?}",
                    e,
                    stream.peer_addr()
                ));
            }
        }
    }
    LOGGER.debug(format!("fsizes: valid_peers={}", peers.len()));
    // Return (peer address, file size) pairs.
    Ok(peers)
}

/// Keep only peers reporting the most common file size.
pub fn clear_fsizes(mut peers: Vec<(SocketAddr, u64)>) -> io::Result<Vec<(SocketAddr, u64)>> {
    let mut _max_peers: u16 = 1;
    let mut fsize_count: Vec<(u64, u16)> = Vec::new();
    peers.iter().for_each(|(_, fsize)| {
        if let Some((_, cnt)) = fsize_count.iter_mut().find(|(size, _)| *size == *fsize) {
            *cnt += 1
        } else {
            fsize_count.push((*fsize, 1));
        }
    });

    // Choosing the most used. And if they are equal taking the last one
    let file_size = fsize_count
        .iter()
        .max_by(|(_, fcount), (_, scount)| fcount.cmp(scount))
        .unwrap()
        .0;

    // Remove peers with the other file sizes
    peers.retain(|(_, size)| *size == file_size);

    Ok(peers)
}

/// Split the file into [start, end) block ranges and assign them across peers.
pub fn fill_block_watcher(
    blocks: u32,
    peers_count: u32,
) -> io::Result<Arc<Mutex<HashMap<(u32, u32), bool>>>> {
    let blocks_watcher = Arc::new(Mutex::new(HashMap::new()));
    if blocks < peers_count {
        let mut b_watch = blocks_watcher.lock().unwrap();
        for i in 0..blocks {
            if i == blocks - 1 {
                b_watch.insert((i, i + 2), false);
            } else {
                b_watch.insert((i, i + 1), false);
            }
        }
    } else {
        let blocks_per_peer = blocks / peers_count;
        let mut b_watch = blocks_watcher.lock().unwrap();
        for i in 0..peers_count {
            let fblock = i * blocks_per_peer;
            let mut lblock = (i + 1) * blocks_per_peer;
            if i == peers_count - 1 {
                lblock = blocks; // end-exclusive
            }
            b_watch.insert((fblock, lblock), false);
        }
    }
    Ok(blocks_watcher)
}

/// Schedule and perform a download, tracking progress in `blocks_watcher`.
pub fn download_request(
    file_name: String,
    file_path: PathBuf,
    available: Vec<SocketAddr>,
    downloading: Arc<Mutex<Vec<String>>>,
) -> io::Result<()> {
    // Query peers and keep those with the majority file size.
    let peers = get_fsizes(available, file_name.clone()).unwrap();
    let mut clean_peers = clear_fsizes(peers).unwrap();

    LOGGER.debug(format!("download: peers_after_clear={}", clean_peers.len()));
    let file_size = clean_peers.get(0).unwrap().1;
    LOGGER.debug(format!("download: file_size={}", file_size));
    let peers_count = clean_peers.len() as u32;

    let blocks = blocks_count(file_size, CHUNK_SIZE);
    LOGGER.debug(format!(
        "download: blocks={} peers_count={}",
        blocks, peers_count
    ));

    // Track the file as "downloading".
    downloading.lock().unwrap().push(file_name.clone());

    // Track completion of each [start, end) segment.
    let blocks_watcher: Arc<Mutex<HashMap<(u32, u32), bool>>> =
        fill_block_watcher(blocks, peers_count).unwrap();
    LOGGER.debug(format!(
        "download: segments={}",
        blocks_watcher.lock().unwrap().len()
    ));

    let pool = ThreadPool::new(blocks_watcher.lock().unwrap().len());

    let mut interrupted = false;

    //
    while blocks_watcher
        .lock()
        .unwrap()
        .values()
        .any(|done| *done == false)
        && !clean_peers.is_empty()
    {
        let block_lines = blocks_watcher.lock().unwrap().clone();
        let mut i = 0;
        let mut del_i: usize = 0; // index of a failed peer to remove
        for ((fblock, lblock), done) in block_lines {
            if done == false {
                if interrupted {
                    clean_peers.remove(del_i);
                    if clean_peers.len() == 0 {
                        break;
                    }
                }
                let file_info = HandshakeRequest {
                    filename: file_name.clone(),
                    action: FileSizeOrInfo::Info(BlockInfo {
                        from_block: fblock,
                        to_block: lblock,
                    }),
                };
                let fpath = file_path.clone();
                let block_watcher = blocks_watcher.clone();
                let _fsize = file_size;
                let peer = clean_peers[i].0.clone();
                let peers_left = clean_peers.len();

                // Starting download a certain block from peer
                LOGGER.debug(format!(
                    "download: schedule peer={} range=[{}, {})",
                    peer, fblock, lblock
                ));

                pool.execute(move || {
                    let epeer = peer.clone();
                    let efname = file_info.filename.clone();
                    let res = download_from_peer(peer, file_info, fpath, _fsize, block_watcher);
                    match res {
                        Ok(_) => (),
                        Err(e) => {
                            LOGGER
                                .debug(format!("download: interrupted; peers_left={}", peers_left));
                            LOGGER.error(&format!(
                                "Downloading {} from {} was interrupted, an error occured {}",
                                efname,
                                epeer.ip(),
                                e
                            ));
                        }
                    }
                });
                i += 1;
            }
            del_i += 1;
        }

        pool.join(); // Waiting till every thread will end
        LOGGER.debug("download: batch finished, checking remaining segments...");

        if blocks_watcher
            .lock()
            .unwrap()
            .values()
            .any(|done| *done == false)
            && interrupted == false
        {
            interrupted = true;
        }
    }

    // Remove from the "downloading" list.
    downloading
        .lock()
        .unwrap()
        .retain(|line| *line != file_name); // Delete from the "downloading" downloaded file

    // If some segments are unfinished, remove the partial file.
    if blocks_watcher
        .lock()
        .unwrap()
        .values()
        .any(|done| *done == false)
    {
        LOGGER.debug(format!(
            "download: failed; removing partial '{}'",
            file_name
        ));
        // If file does not downloaded completely
        fs::remove_file(&file_name)?; // Delete an already downloaded part
        LOGGER.error(&format!("Failed to download a {file_name}"));
    }

    Ok(())
}

/// Download a specific [start, end) block range from a peer.
pub fn download_from_peer(
    peer: SocketAddr,
    file_info: HandshakeRequest,
    _file_path: PathBuf,
    file_size: u64,
    block_watcher: Arc<Mutex<HashMap<(u32, u32), bool>>>,
) -> io::Result<()> {
    let mut stream = TcpStream::connect((peer.ip(), PORT_FILE_SHARE))?;

    let mut buf = vec![0u8; CHUNK_SIZE];
    let file_blocks = (file_size / CHUNK_SIZE as u64) as u32;

    stream.set_read_timeout(Some(Duration::new(45, 0)))?;
    stream.set_write_timeout(Some(Duration::new(45, 0)))?;

    let ser = to_string(&file_info)?;
    stream.write_all(ser.as_bytes())?;

    let fblock: u32;
    let lblock: u32;

    match file_info.action {
        FileSizeOrInfo::Info(obj) => {
            fblock = obj.from_block;
            lblock = obj.to_block;
        }
        _ => {
            fblock = 0;
            lblock = 0;
        }
    }

    let last_block_size = file_size as usize % CHUNK_SIZE;


    // Decide destination path:
    // - empty -> "./<filename>"
    // - existing dir -> "<dir>/<filename>"
    // - otherwise -> treat as full file path (allow renaming)
    let dest_path = if _file_path.as_os_str().is_empty() {
        PathBuf::from(&file_info.filename)
    } else if _file_path.is_dir() {
        let mut p = _file_path.clone();
        p.push(&file_info.filename);
        p
    } else {
        _file_path.clone()
    };
    if let Some(parent) = dest_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let mut file = fs::File::create(&dest_path)?;

    file.seek(SeekFrom::Start(CHUNK_SIZE as u64 * fblock as u64))?;
    for i in fblock..lblock {
        if i == file_blocks {
            if last_block_size == 0 {
                break;
            }
            buf.resize(last_block_size, 0u8);
        }
        stream.read_exact(&mut buf)?;
        file.write_all(&buf)?;
    }
    *block_watcher
        .lock()
        .unwrap()
        .get_mut(&(fblock, lblock))
        .unwrap() = true;
    Ok(())
}

/////////////
/// TESTS ///
/////////////

#[cfg(test)] //Unit-tests
mod unit_tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::*;

    #[test]
    fn test_get_fsizes() {
        let v: Vec<SocketAddr> = Vec::new();
        let name: String = String::from("Name");

        match get_fsizes(v, name) {
            Ok(o) => LOGGER.info(&format!("{:?}", o)),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[test]
    fn test_get_fsizes_contain() {
        let mut v: Vec<SocketAddr> = Vec::new();
        v.push(SocketAddr::new(
            IpAddr::from(Ipv4Addr::new(224, 0, 0, 123)),
            1222,
        ));
        let name: String = String::from("Name");

        match get_fsizes(v, name) {
            Ok(o) => LOGGER.info(&format!("{:?}", o)),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[test]
    fn test_clear_fsizes_contain() {
        let mut v: Vec<(SocketAddr, u64)> = Vec::new();
        let sa: SocketAddr = SocketAddr::new(IpAddr::from(Ipv4Addr::new(224, 0, 0, 123)), 1222);

        v.push((sa, 64));

        match clear_fsizes(v) {
            Ok(o) => LOGGER.info(&format!("{:?}", o)),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[test]
    fn test_clear_fsizes_two_16_one_32() {
        let _sock = SocketAddr::new(IpAddr::from(LOCAL_NETWORK), 80);
        let v: Vec<(SocketAddr, u64)> = vec![
            (_sock.clone(), 16),
            (_sock.clone(), 32),
            (_sock.clone(), 16),
        ];
        let res: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 16), (_sock.clone(), 16)];
        assert_eq!(clear_fsizes(v).unwrap(), res);
    }

    #[test]
    fn test_clear_fsizes_two_16_three_32() {
        let _sock = SocketAddr::new(IpAddr::from(LOCAL_NETWORK), 80);
        let v: Vec<(SocketAddr, u64)> = vec![
            (_sock.clone(), 16),
            (_sock.clone(), 16),
            (_sock.clone(), 32),
            (_sock.clone(), 32),
            (_sock.clone(), 32),
        ];
        let res: Vec<(SocketAddr, u64)> = vec![
            (_sock.clone(), 32),
            (_sock.clone(), 32),
            (_sock.clone(), 32),
        ];
        assert_eq!(clear_fsizes(v).unwrap(), res);
    }

    #[test]
    fn test_clear_fsizes_one_16() {
        let _sock = SocketAddr::new(IpAddr::from(LOCAL_NETWORK), 80);
        let v: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 16)];
        let res: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 16)];
        assert_eq!(clear_fsizes(v).unwrap(), res);
    }

    #[test]
    fn test_clear_fsizes_two_16_two_32() {
        let _sock = SocketAddr::new(IpAddr::from(LOCAL_NETWORK), 80);
        let v: Vec<(SocketAddr, u64)> = vec![
            (_sock.clone(), 16),
            (_sock.clone(), 16),
            (_sock.clone(), 32),
            (_sock.clone(), 32),
        ];
        let res: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 32), (_sock.clone(), 32)];
        assert_eq!(clear_fsizes(v).unwrap(), res);
    }

    #[test]
    fn test_clear_fsizes_one_16_one_32() {
        let _sock = SocketAddr::new(IpAddr::from(LOCAL_NETWORK), 80);
        let v: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 16), (_sock.clone(), 32)];
        let res: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 32)];
        assert_eq!(clear_fsizes(v).unwrap(), res);
    }

    #[test]
    fn test_fill_block_watcher_50_blocks_1_peers() {
        let blocks: u32 = 50;
        let peers: u32 = 1;
        let mut res: HashMap<(u32, u32), bool> = HashMap::new();
        res.insert((0, 50), false);
        assert_eq!(
            res,
            fill_block_watcher(blocks, peers)
                .unwrap()
                .lock()
                .unwrap()
                .clone()
        );
    }

    #[test]
    fn test_fill_block_watcher_100_blocks_2_peers() {
        let blocks: u32 = 100;
        let peers: u32 = 2;
        let mut res: HashMap<(u32, u32), bool> = HashMap::new();
        res.insert((0, 50), false);
        res.insert((50, 100), false);
        assert_eq!(
            res,
            fill_block_watcher(blocks, peers)
                .unwrap()
                .lock()
                .unwrap()
                .clone()
        );
    }

    #[test]
    fn test_fill_block_watcher_3_blocks_5_peers() {
        let blocks: u32 = 3;
        let peers: u32 = 5;
        let mut res: HashMap<(u32, u32), bool> = HashMap::new();
        res.insert((0, 1), false);
        res.insert((1, 2), false);
        res.insert((2, 4), false);
        assert_eq!(
            res,
            fill_block_watcher(blocks, peers)
                .unwrap()
                .lock()
                .unwrap()
                .clone()
        );
    }

    #[test]
    fn test_fill_block_watcher_1_blocks_1_peers() {
        let blocks: u32 = 1;
        let peers: u32 = 1;
        let mut res: HashMap<(u32, u32), bool> = HashMap::new();
        res.insert((0, 1), false);
        assert_eq!(
            res,
            fill_block_watcher(blocks, peers)
                .unwrap()
                .lock()
                .unwrap()
                .clone()
        );
    }

    #[test]
    fn test_fill_block_watcher_2_blocks_1_peers() {
        let blocks: u32 = 2;
        let peers: u32 = 1;
        let mut res: HashMap<(u32, u32), bool> = HashMap::new();
        res.insert((0, 2), false);
        assert_eq!(
            res,
            fill_block_watcher(blocks, peers)
                .unwrap()
                .lock()
                .unwrap()
                .clone()
        );
    }

    #[test]
    fn test_fill_block_watcher_2_blocks_2_peers() {
        let blocks: u32 = 2;
        let peers: u32 = 2;
        let mut res: HashMap<(u32, u32), bool> = HashMap::new();
        res.insert((0, 1), false);
        res.insert((1, 2), false);
        assert_eq!(
            res,
            fill_block_watcher(blocks, peers)
                .unwrap()
                .lock()
                .unwrap()
                .clone()
        );
    }
}

// Functional tests
#[cfg(test)]
mod func_tests {
    use p2p_config::{LOCALHOST, PORT_CLIENT_DAEMON};
    use p2p_core::utils::create_buffer;

    use super::*;

    #[test]
    fn test_share() {
        let _listener = TcpListener::bind((LOCALHOST, PORT_CLIENT_DAEMON + 1)).unwrap();
        //
        let mut stream = TcpStream::connect((LOCALHOST, PORT_CLIENT_DAEMON + 1)).unwrap();
        let share = Action::Share {
            file_path: PathBuf::from("crates/daemon/src/lib.rs"),
        };
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<FileState>> = Arc::new(Mutex::new(FileState::new()));

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        action_processor(&share, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = create_buffer(CHUNK_SIZE);
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Response = from_slice(&buf[..size]).unwrap();
                assert_eq!(answ, Response::Ok);
                // Checking dat after changes
                assert_eq!(
                    (dat.lock().unwrap())
                        .shared
                        .get(&String::from("lib.rs"))
                        .unwrap(),
                    &PathBuf::from("crates/daemon/src/lib.rs")
                );
            }
            Err(_) => {
                panic!("An error occurred, {}", stream.peer_addr().unwrap());
            }
        }
    }

    #[test]
    fn test_download() {
        let _listener = TcpListener::bind((LOCALHOST, PORT_CLIENT_DAEMON + 2)).unwrap();
        //
        let mut stream = TcpStream::connect((LOCALHOST, PORT_CLIENT_DAEMON + 2)).unwrap();
        let download = Action::Download {
            file_name: String::from("FILE.dat"),
            save_path: PathBuf::from("fake\\path\\"),
            wait: false,
        };
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<FileState>> = Arc::new(Mutex::new(FileState::new()));

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        action_processor(&download, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = create_buffer(CHUNK_SIZE);
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Response = from_slice(&buf[..size]).unwrap();
                assert_eq!(
                    answ,
                    Response::Err(String::from("File is not available to download!"))
                );
            }
            Err(e) => panic!("An error occurred, {}", e),
        }
    }

    #[test]
    fn test_scan() {
        let _listener = TcpListener::bind((LOCALHOST, PORT_CLIENT_DAEMON + 3)).unwrap();
        //
        let mut stream = TcpStream::connect((LOCALHOST, PORT_CLIENT_DAEMON + 3)).unwrap();
        let scan = Action::Scan;
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<FileState>> = Arc::new(Mutex::new(FileState::new()));

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        action_processor(&scan, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = create_buffer(CHUNK_SIZE);
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Response = from_slice(&buf[..size]).unwrap();
                assert_eq!(answ, Response::Ok);
            }
            Err(_) => {
                panic!("An error occurred, {}", stream.peer_addr().unwrap());
            }
        }
    }

    #[test]
    fn test_ls() {
        let _listener = TcpListener::bind((LOCALHOST, PORT_CLIENT_DAEMON + 4)).unwrap();
        //
        let mut stream = TcpStream::connect((LOCALHOST, PORT_CLIENT_DAEMON + 4)).unwrap();
        let ls = Action::Ls;
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<FileState>> = Arc::new(Mutex::new(FileState::new()));

        dat.lock()
            .unwrap()
            .available
            .insert(String::from("FILE.dat"), Vec::new());

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        action_processor(&ls, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = create_buffer(CHUNK_SIZE);
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Response = from_slice(&buf[..size]).unwrap();
                //
                let mut t: HashMap<String, Vec<SocketAddr>> = HashMap::new();
                t.insert(String::from("FILE.dat"), Vec::new());

                let tmp: Response = Response::Ls { available_map: t };
                assert_eq!(answ, tmp);
            }
            Err(_) => {
                panic!("An error occurred, {}", stream.peer_addr().unwrap());
            }
        }
    }

    #[test]
    fn test_status() {
        let _listener = TcpListener::bind((LOCALHOST, PORT_CLIENT_DAEMON + 5)).unwrap();
        //
        let mut stream = TcpStream::connect((LOCALHOST, PORT_CLIENT_DAEMON + 5)).unwrap();
        let status = Action::Status;
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<FileState>> = Arc::new(Mutex::new(FileState::new()));

        dat.lock().unwrap().shared.insert(
            String::from("FILE1.dat"),
            PathBuf::from("fake\\path\\FILE1.dat"),
        );

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        tr.lock()
            .unwrap()
            .insert(String::from("FILE.dat"), Vec::new());

        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        action_processor(&status, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = create_buffer(CHUNK_SIZE);
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Response = from_slice(&buf[..size]).unwrap();
                //
                let mut tr: HashMap<String, Vec<SocketAddr>> = HashMap::new();
                tr.insert(String::from("FILE.dat"), Vec::new());

                let mut sh: HashMap<String, PathBuf> = HashMap::new();
                sh.insert(
                    String::from("FILE1.dat"),
                    PathBuf::from("fake\\path\\FILE1.dat"),
                );

                let dow_t: Vec<String> = Vec::new();

                let tmp: Response = Response::Status {
                    transferring_map: tr,
                    shared_map: sh,
                    downloading_map: dow_t,
                };
                assert_eq!(answ, tmp);
            }
            Err(_) => {
                panic!("An error occurred, {}", stream.peer_addr().unwrap());
            }
        }
    }
}
