use lib::*;

#[cfg(windows)]
pub fn bind_multicast(_addr: &Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
    UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), port))
}
#[cfg(unix)]
pub fn bind_multicast(addr: &Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
    UdpSocket::bind((*addr, port))
}

///Getting IP of current daemon thread
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

///Processing command from client
fn command_processor(
    com: &Command,//the request itself
    mut stream: TcpStream,
    mut data: MutexGuard<DataTemp>,
    transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,//HashMap to refresh status of transferring files
    downloading: Arc<Mutex<Vec<String>>>,// Vector to refresh status of downloading files
) -> io::Result<()> {
    match com {
        Command::Share { file_path: f_path } => {

            let name: String = String::from(f_path.file_name().unwrap().to_string_lossy());
            data.shared.insert(name, f_path.clone()); //Name - path

            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
        Command::Download {
            file_name: f_name,
            save_path: s_path,
            wait: wat,
        } => {
            let answ: Answer;

            if data.available.contains_key(f_name) == false {// If no one share this file
                answ = Answer::Err(String::from("File is not available to download!"));
            } else if data.shared.contains_key(f_name) == true {//If we already share this file
                answ = Answer::Err(String::from("You already own this file, and even sharing!"));
            } else {
                answ = Answer::Ok;
            }

            match answ {
                Answer::Ok => {//If everything is ok we starting a thread with download request to other daemons
                    let available_list = data.available.get(f_name).unwrap().clone();
                    let filename = f_name.clone();
                    let savepath = s_path.clone();
                    let file_thread = thread::spawn(move || {
                        match download_request(
                            filename.clone(),
                            savepath,
                            available_list,
                            downloading,
                        ) {
                            Ok(_) => (),
                            Err(e) => {
                                eprintln!(
                                    "Failed to download a {}, an error occured {}",
                                    filename, e
                                );
                            }
                        }
                        ()
                    });
                    if *wat == true {//Waiting for ending of file downloading
                        file_thread.join().unwrap();
                    }
                }
                _ => {}
            }

            let serialized = serde_json::to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap();//answer to client
        }
        Command::Scan => {
            let socket = UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), 0))?;
            socket.send_to(SCAN_REQUEST, (ADDR_DAEMON_MULTICAST, PORT_MULTICAST))?;
            data.available.clear(); // clear list of available files to download
                                    //the list gonna be refreshed in multicast_receiver

            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
        Command::Ls => {
            let answ: Answer = Answer::Ls {
                available_map: data.available.clone(),
            };
            let serialized = serde_json::to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
        Command::Status => {
            //transferring & shared
            let answ: Answer = Answer::Status {
                transferring_map: transferring.lock().unwrap().clone(),
                shared_map: data.shared.clone(),
                downloading_map: downloading.lock().unwrap().clone(),
            };
            let serialized = serde_json::to_string(&answ)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
    }

    Ok(())
}

///Responds to multicast requests from other daemons
fn multicast_responder(data: Arc<Mutex<DataTemp>>) -> io::Result<()> {
    let this_daemon_ip = get_this_daemon_ip().unwrap();

    let listener = bind_multicast(&ADDR_DAEMON_MULTICAST, PORT_MULTICAST)?;
    listener.join_multicast_v4(&ADDR_DAEMON_MULTICAST, &Ipv4Addr::new(0, 0, 0, 0))?;

    let mut shared: Vec<String>;
    let mut buf = vec![0; 4096];
    loop {
        let (len, remote_addr) = listener.recv_from(&mut buf)?;
        if remote_addr.ip() != this_daemon_ip {
            //check if that's not our daemon, then we will respond
            let message = &buf[..len];
            let mut stream = TcpStream::connect((remote_addr.ip(), PORT_SCAN_TCP))?;

            if message == SCAN_REQUEST {
                let dat = data.lock().unwrap();
                shared = Vec::new();
                for key in dat.shared.keys() {
                    shared.push(key.clone());
                }
                let serialized = serde_json::to_string(&shared)?;
                stream.write_all(serialized.as_bytes()).unwrap(); //Send our "shared" files list
            }
        }
    }
}

//Function that receiving answer from other daemons to refresh our "available files to download" list
fn multicast_receiver(data: Arc<Mutex<DataTemp>>) -> io::Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), PORT_SCAN_TCP))?;
    //get names of files with tcp (his shared - your available)
    let mut buf = vec![0 as u8; 4096];
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                match stream.read(&mut buf) {
                    Ok(size) => {
                        //Get List of names
                        let names: Vec<String> = serde_json::from_slice(&buf[..size])?;
                        for name in names.into_iter() {
                            if data.lock().unwrap().available.contains_key(&name) {
                                //If file already exist just update Vec of IP
                                data.lock()
                                    .unwrap()
                                    .available
                                    .get_mut(&name)
                                    .unwrap()
                                    .push(stream.peer_addr().unwrap());
                            } else {
                                //In another case - adding file with first IP that share it
                                let mut v: Vec<SocketAddr> = Vec::new();
                                v.push(stream.peer_addr().unwrap());
                                data.lock().unwrap().available.insert(name, v);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("An error occurred, {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
    Ok(())
}

//Function to process the other daemon "download file" requests
fn share_responder(
    transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
    data: Arc<Mutex<DataTemp>>,
) -> io::Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), PORT_FILE_SHARE))?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                let peer_transferring = transferring.clone();
                let shared = data.lock().unwrap().shared.clone();
                thread::spawn(move || {//giving a response for sharing a file other daemon to thread with func 
                    let peer = _stream.peer_addr().unwrap().ip();
                    match share_to_peer(_stream, peer_transferring, shared) {
                        Ok(_) => (),
                        Err(e) => {
                            eprintln!("Interrupted sharing to {}, an error occured {}", peer, e);
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
    Ok(())
}

///Process first query which is get file size or start download a file
fn handle_first_share_request(
    shared: HashMap<String, PathBuf>,
    request: FirstRequest,
    mut stream: TcpStream,
) -> Option<(FileInfo, String, u64, TcpStream)> {
    let asked_filename: String = request.filename;
    match request.action { // Process the request
        FileSizeorInfo::Size => {// The other guy wants a file size
            let answ: AnswerToFirstRequest;
            if shared.contains_key(&asked_filename) == false {//If we do not sharing this file
                answ = AnswerToFirstRequest {
                    filename: asked_filename.clone(),
                    answer: EnumAnswer::NotExist,
                };// We setting the answer to file does not exist
                println!(
                    "{} asked not existing file",
                    stream.peer_addr().unwrap().ip()
                );
            } else {
                let size_of_file: u64 = std::fs::metadata(shared.get(&asked_filename).unwrap())
                    .unwrap()
                    .len(); //get file size
                answ = AnswerToFirstRequest {
                    filename: asked_filename.clone(),
                    answer: EnumAnswer::Size(size_of_file),
                };//In the other case, we setting the answer to file size
                println!(
                    "{} asked size of {}",
                    stream.peer_addr().unwrap().ip(),
                    &asked_filename
                );
            }
            let serialized = serde_json::to_string(&answ).unwrap();
            stream.write_all(serialized.as_bytes()).unwrap();//Sending the answer
            return None;
        }
        FileSizeorInfo::Info(info) => {//The other daemon wants to start downloading a file
            println!(
                "Starting sharing a {} to {}",
                &asked_filename,
                stream.peer_addr().unwrap().ip()
            );
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

///Start sharing the file to other daemon
fn share_to_peer(
    mut stream: TcpStream,
    transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
    shared: HashMap<String, PathBuf>,
) -> io::Result<()> {
    let mut buf = vec![0; 4096];

    stream.set_read_timeout(Some(Duration::new(45, 0)))?;
    stream.set_write_timeout(Some(Duration::new(45, 0)))?;// setting the timeouts

    let file_info: FileInfo;
    let file_name: String;
    let file_size: u64;

    match stream.read(&mut buf) {
        Ok(size) => {
            let request: FirstRequest = serde_json::from_slice(&buf[..size])?;
            match handle_first_share_request(shared, request, stream) {//processing the first request
                Some((info, name, size, s)) => {
                    file_info = info;
                    file_name = name;
                    file_size = size;
                    stream = s;
                }
                None => {
                    return Ok(());// if other daemon wanted a file size we ended here
                }
            }
        }
        Err(e) => {
            eprintln!("An error occurred, {}", e);
            return Err(e);
        }
    }

    let blocks: u32;
    // that thing is interesting one
    // While TransferGuard is creating he's adding a peer to a vector,
    // that stores a daemons, which downloading a file from us
    let _transfer_guard = TransferGuard::new(
        transferring.clone(),
        file_name.clone(),
        stream.peer_addr().unwrap(),
    );
    // and when something is wrong like we cannot open the file,
    // or the stream write timeout is ended,
    // guard will automatically remove the daemon address from vector...

    blocks = (file_size / 4096) as u32;
    let last_block_size = (file_size % 4096) as usize;

    let mut file = fs::File::open(&file_name)?;
    file.seek(SeekFrom::Start(4096 * file_info.from_block as u64))?;
    for i in file_info.from_block..file_info.to_block {
        if i == blocks {//last block is not always 4096 so we resizing the vector and reading exactly as much as left
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

///Fill the HashMap of peer and his file size
fn get_fsize_on_each_peer(
    peer_list: Vec<SocketAddr>,
    file_name: String,
) -> io::Result<Vec<(SocketAddr, u64)>> {
    let mut buf = vec![0; 4096];
    let mut peers: Vec<(SocketAddr, u64)> = Vec::new();
    let request_to_get_size = serde_json::to_string(&FirstRequest {
        filename: file_name.clone(),
        action: FileSizeorInfo::Size,
    })?;// request to get a file size
    let mut refresh = true;

    for peer in peer_list.iter() {
        let mut stream: TcpStream;
        match TcpStream::connect((peer.ip(), PORT_FILE_SHARE)) {
            Ok(_stream) => {
                stream = _stream;
            }
            Err(e) => {
                eprintln!(
                    "Error while connecting to {} to download a file {}",
                    peer.ip(),
                    e
                );
                continue;
            }
        }
        stream.set_read_timeout(Some(Duration::new(30, 0)))?;
        stream.set_write_timeout(Some(Duration::new(30, 0)))?;
        stream.write_all(request_to_get_size.as_bytes())?;//sending the request to other daemons
        match stream.read(&mut buf) {
            Ok(size) => {
                let answer: AnswerToFirstRequest = serde_json::from_slice(&buf[..size])?;
                match answer.answer {
                    EnumAnswer::Size(file_size) => {//if daemon sharing a file, we pushing his ip and size to vec
                        peers.push((stream.peer_addr()?, file_size));
                    }
                    EnumAnswer::NotExist => {//in other case our daemon says to scan the network again
                        if refresh {
                            println!("That peer doesn't share a file! Please refresh list of files with scan!");
                            refresh = false;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "Error {} while interracting with {:?}",
                    e,
                    stream.peer_addr()
                );
            }
        }
    }
    Ok(peers)// returning the vector with daemons addresses and there's file sizes
}

///Leave the most used file size and peer
fn remove_other_fsizes_in_vec(
    mut peers: Vec<(SocketAddr, u64)>,
) -> io::Result<Vec<(SocketAddr, u64)>> {
    let mut _max_peers: u16 = 1;
    //Create the vector with a (FILE_SIZE, HOW_MANY_PEERS_SHARING_THIS_FILE_SIZE)
    let mut fsize_count: Vec<(u64, u16)> = Vec::new();
    peers.iter().for_each(|(_, fsize)| {//counting them all
        let buffer_var = fsize_count.iter_mut().find(|(size, _)| *size == *fsize);
        if buffer_var == Option::None {
            fsize_count.push((*fsize, 1));
        } else {
            buffer_var.unwrap().1 += 1;
        }
    });
    //Choosing the most used, if they equal, selecting the last one
    let file_size = fsize_count
        .iter()
        .max_by(|(_, fcount), (_, scount)| fcount.cmp(scount))
        .unwrap()
        .0;

    peers.retain(|(_, size)| *size == file_size); // removes peers with other file size
    //This is the power iterators of RUST
    Ok(peers)
}

//Split up the file in blocks and peers
fn fill_block_watcher(
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
                lblock = blocks + 1;
            }
            b_watch.insert((fblock, lblock), false);
        }
    }
    Ok(blocks_watcher)
}

//Still need to refactor this func
//Send the download file request to other daemons
fn download_request(
    file_name: String,
    file_path: PathBuf,
    available: Vec<SocketAddr>,
    downloading: Arc<Mutex<Vec<String>>>,
) -> io::Result<()> {
    //Getting the available peer which sharing file
    let peers = get_fsize_on_each_peer(available, file_name.clone()).unwrap();
    //Leaving peers which sharing the moust popular file
    let mut peers = remove_other_fsizes_in_vec(peers).unwrap();

    let file_size = peers.get(0).unwrap().1;
    let peers_count = peers.len() as u32;

    let blocks = (file_size / 4096) as u32;
    let file_size: u64 = file_size;
    //Oh damn, DownloadGuard would be useful in here
    downloading.lock().unwrap().push(file_name.clone());

    let pool: ThreadPool;
    //We tracking the downloading file status in watcher
    let blocks_watcher: Arc<Mutex<HashMap<(u32, u32), bool>>> =
        fill_block_watcher(blocks, peers_count).unwrap();

    pool = ThreadPool::new(blocks_watcher.lock().unwrap().len());

    let mut interrupted = false;

    while blocks_watcher
        .lock()
        .unwrap()
        .values()
        .any(|done| *done == false)
        && peers.len() > 0
    {
        let block_lines = blocks_watcher.lock().unwrap().clone();
        let mut i = 0;
        let mut del_i: usize = 0;
        for ((fblock, lblock), done) in block_lines {
            if done == false {
                if interrupted {
                    peers.remove(del_i);
                    if peers.len() == 0 {
                        break;
                    }
                }
                let file_info = FirstRequest {
                    filename: file_name.clone(),
                    action: FileSizeorInfo::Info(FileInfo {
                        from_block: fblock,
                        to_block: lblock,
                    }),
                };
                let fpath = file_path.clone();
                let block_watcher = blocks_watcher.clone();
                let _fsize = file_size;
                let peer = peers[i].0.clone();
                //Starting download a certain block from peer
                pool.execute(move || {
                    let epeer = peer.clone();
                    let efname = file_info.filename.clone();
                    let res = download_from_peer(peer, file_info, fpath, _fsize, block_watcher);
                    match res {
                        Ok(_) => (),
                        Err(e) => {
                            eprintln!(
                                "Downloading {} from {} was interrupted, an error occured {}",
                                efname,
                                epeer.ip(),
                                e
                            );
                        }
                    }
                });
                i += 1;
            }
            del_i += 1;
        }
        pool.join();//Waiting till every thread is ended
        if blocks_watcher
            .lock()
            .unwrap()
            .values()
            .any(|done| *done == false)
            && interrupted == false
        {
            // eprintln!("The connection was interrupted while downloading a {}", &file_name);
            interrupted = true;
        }
    }
    downloading
        .lock()
        .unwrap()
        .retain(|line| *line != file_name);
    if blocks_watcher
        .lock()
        .unwrap()
        .values()
        .any(|done| *done == false)
    {// If file is does not downloaded completely
        fs::remove_file(&file_name)?;//Removing downloaded
        eprintln!("Failed to download a {}", file_name);
    }

    Ok(())
}

///Download a specific file blocks from peer
fn download_from_peer(
    peer: SocketAddr,
    file_info: FirstRequest,
    _file_path: PathBuf,
    file_size: u64,
    block_watcher: Arc<Mutex<HashMap<(u32, u32), bool>>>,
) -> io::Result<()> {
    let mut stream = TcpStream::connect((peer.ip(), PORT_FILE_SHARE))?;

    let mut buf = vec![0u8; 4096];
    let file_blocks = (file_size / 4096) as u32;

    stream.set_read_timeout(Some(Duration::new(45, 0)))?;
    stream.set_write_timeout(Some(Duration::new(45, 0)))?;

    let ser = serde_json::to_string(&file_info)?;
    stream.write_all(ser.as_bytes())?;

    let fblock: u32;
    let lblock: u32;

    match file_info.action {
        FileSizeorInfo::Info(obj) => {
            fblock = obj.from_block;
            lblock = obj.to_block;
        }
        _ => {
            fblock = 0;
            lblock = 0;
        }
    }

    let last_block_size = file_size as usize % 4096;
    let mut file = fs::File::create(&file_info.filename)?;
    file.seek(SeekFrom::Start(4096 * fblock as u64))?;
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

fn main() -> io::Result<()> {
    println!("Daemon: running");
    //Listener for client-daemon connection
    let listener = TcpListener::bind(("localhost", PORT_CLIENT_DAEMON))?;
    //All about files daemon knowledge
    let data: Arc<Mutex<DataTemp>> = Arc::new(Mutex::new(DataTemp::new()));
    //HashMap which contains peers, that currently downloading a file from this daemon
    let transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    //Which files we downloading right now
    let downloading: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let mult_resp_data = data.clone();
    thread::spawn(move || {
        multicast_responder(mult_resp_data).unwrap();
    });

    let mult_recv_data = data.clone();
    thread::spawn(move || {
        multicast_receiver(mult_recv_data).unwrap();
    });

    let share_transfer = transferring.clone();
    let share_data = data.clone();
    thread::spawn(move || {
        share_responder(share_transfer, share_data).unwrap();
    });

    //
    let mut buf = vec![0 as u8; 4096];
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                /////////////
                match stream.read(&mut buf) {
                    Ok(size) => {
                        //Now the daemon does not crash when the command is entered incorrectly
                        let com: Command;
                        match serde_json::from_slice(&buf[..size]) {
                            Ok(c) => {
                                com = c;
                            }
                            Err(_) => {
                                println!("Client made a mistake!");
                                continue;
                            }
                        }

                        //println!("{:?}", *data);
                        let dat = data.clone();
                        let com_transfer = transferring.clone();
                        let com_download = downloading.clone();
                        thread::spawn(move || {
                            command_processor(
                                &com,
                                stream,
                                dat.lock().unwrap(),
                                com_transfer,
                                com_download,
                            )
                            .unwrap();
                        });
                    }
                    Err(e) => {
                        eprintln!("An error occurred, {}", e);
                    }
                }
                ///////////////
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    Ok(())
}

#[cfg(test)] //Unit-tests
mod unit_tests {
    use super::*;

    #[test]
    fn test_get_fsize_on_each_peer() {
        let v: Vec<SocketAddr> = Vec::new();
        let name: String = String::from("Name");

        match get_fsize_on_each_peer(v, name) {
            Ok(o) => println!("{:?}", o),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[test]
    fn test_get_fsize_on_each_peer_contain() {
        let mut v: Vec<SocketAddr> = Vec::new();
        v.push(SocketAddr::new(
            IpAddr::from(Ipv4Addr::new(224, 0, 0, 123)),
            1222,
        ));
        let name: String = String::from("Name");

        match get_fsize_on_each_peer(v, name) {
            Ok(o) => println!("{:?}", o),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[test]
    fn test_remove_other_fsizes_in_vec_contain() {
        let mut v: Vec<(SocketAddr, u64)> = Vec::new();
        let sa: SocketAddr = SocketAddr::new(IpAddr::from(Ipv4Addr::new(224, 0, 0, 123)), 1222);

        v.push((sa, 64));

        match remove_other_fsizes_in_vec(v) {
            Ok(o) => println!("{:?}", o),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[test]
    fn test_remove_other_fsizes_in_vec_two_16_one_32() {
        let _sock = SocketAddr::new(IpAddr::from(Ipv4Addr::new(0, 0, 0, 0)), 80);
        let v: Vec<(SocketAddr, u64)> = vec![
            (_sock.clone(), 16),
            (_sock.clone(), 32),
            (_sock.clone(), 16),
        ];
        let res: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 16), (_sock.clone(), 16)];
        assert_eq!(remove_other_fsizes_in_vec(v).unwrap(), res);
    }

    #[test]
    fn test_remove_other_fsizes_in_vec_two_16_three_32() {
        let _sock = SocketAddr::new(IpAddr::from(Ipv4Addr::new(0, 0, 0, 0)), 80);
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
        assert_eq!(remove_other_fsizes_in_vec(v).unwrap(), res);
    }

    #[test]
    fn test_remove_other_fsizes_in_vec_one_16() {
        let _sock = SocketAddr::new(IpAddr::from(Ipv4Addr::new(0, 0, 0, 0)), 80);
        let v: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 16)];
        let res: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 16)];
        assert_eq!(remove_other_fsizes_in_vec(v).unwrap(), res);
    }

    #[test]
    fn test_remove_other_fsizes_in_vec_two_16_two_32() {
        let _sock = SocketAddr::new(IpAddr::from(Ipv4Addr::new(0, 0, 0, 0)), 80);
        let v: Vec<(SocketAddr, u64)> = vec![
            (_sock.clone(), 16),
            (_sock.clone(), 16),
            (_sock.clone(), 32),
            (_sock.clone(), 32),
        ];
        let res: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 32), (_sock.clone(), 32)];
        assert_eq!(remove_other_fsizes_in_vec(v).unwrap(), res);
    }

    #[test]
    fn test_remove_other_fsizes_in_vec_one_16_one_32() {
        let _sock = SocketAddr::new(IpAddr::from(Ipv4Addr::new(0, 0, 0, 0)), 80);
        let v: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 16), (_sock.clone(), 32)];
        let res: Vec<(SocketAddr, u64)> = vec![(_sock.clone(), 32)];
        assert_eq!(remove_other_fsizes_in_vec(v).unwrap(), res);
    }

    #[test]
    fn test_fill_block_watcher_50_blocks_1_peers() {
        let blocks: u32 = 50;
        let peers: u32 = 1;
        let mut res: HashMap<(u32, u32), bool> = HashMap::new();
        res.insert((0, 51), false);
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
        res.insert((50, 101), false);
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
    fn test_fill_block_watcher_2_blocks_1_peers() {
        let blocks: u32 = 2;
        let peers: u32 = 1;
        let mut res: HashMap<(u32, u32), bool> = HashMap::new();
        res.insert((0, 3), false);
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
        res.insert((1, 3), false);
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

#[cfg(test)] //Functional tests
mod func_tests {
    use super::*;

    #[test]
    fn test_share() {
        let _listener = TcpListener::bind(("localhost", PORT_CLIENT_DAEMON + 1)).unwrap();
        //
        let mut stream = TcpStream::connect(("localhost", PORT_CLIENT_DAEMON + 1)).unwrap();
        let com = Command::Share {
            file_path: PathBuf::from("fake\\path\\FILE.dat"),
        };
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<DataTemp>> = Arc::new(Mutex::new(DataTemp::new()));

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        command_processor(&com, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = vec![0 as u8; 4096];
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Answer = serde_json::from_slice(&buf[..size]).unwrap();
                assert_eq!(answ, Answer::Ok);
                //Checking dat after changes
                assert_eq!(
                    (dat.lock().unwrap())
                        .shared
                        .get(&String::from("FILE.dat"))
                        .unwrap(),
                    &PathBuf::from("fake\\path\\FILE.dat")
                );
            }
            Err(_) => {
                panic!("An error occurred, {}", stream.peer_addr().unwrap());
            }
        }
    }

    #[test]
    fn test_download() {
        let _listener = TcpListener::bind(("localhost", PORT_CLIENT_DAEMON + 2)).unwrap();
        //
        let mut stream = TcpStream::connect(("localhost", PORT_CLIENT_DAEMON + 2)).unwrap();
        let com = Command::Download {
            file_name: String::from("FILE.dat"),
            save_path: PathBuf::from("fake\\path\\"),
            wait: false,
        };
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<DataTemp>> = Arc::new(Mutex::new(DataTemp::new()));

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        command_processor(&com, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = vec![0 as u8; 4096];
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Answer = serde_json::from_slice(&buf[..size]).unwrap();
                assert_eq!(
                    answ,
                    Answer::Err(String::from("File is not available to download!"))
                );
            }
            Err(e) => panic!("An error occurred, {}", e),
        }
    }

    #[test]
    fn test_scan() {
        let _listener = TcpListener::bind(("localhost", PORT_CLIENT_DAEMON + 3)).unwrap();
        //
        let mut stream = TcpStream::connect(("localhost", PORT_CLIENT_DAEMON + 3)).unwrap();
        let com = Command::Scan;
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<DataTemp>> = Arc::new(Mutex::new(DataTemp::new()));

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        command_processor(&com, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = vec![0 as u8; 4096];
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Answer = serde_json::from_slice(&buf[..size]).unwrap();
                assert_eq!(answ, Answer::Ok);
            }
            Err(_) => {
                panic!("An error occurred, {}", stream.peer_addr().unwrap());
            }
        }
    }

    #[test]
    fn test_ls() {
        let _listener = TcpListener::bind(("localhost", PORT_CLIENT_DAEMON + 4)).unwrap();
        //
        let mut stream = TcpStream::connect(("localhost", PORT_CLIENT_DAEMON + 4)).unwrap();
        let com = Command::Ls;
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<DataTemp>> = Arc::new(Mutex::new(DataTemp::new()));

        dat.lock()
            .unwrap()
            .available
            .insert(String::from("FILE.dat"), Vec::new());

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        command_processor(&com, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = vec![0 as u8; 4096];
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Answer = serde_json::from_slice(&buf[..size]).unwrap();
                //
                let mut t: HashMap<String, Vec<SocketAddr>> = HashMap::new();
                t.insert(String::from("FILE.dat"), Vec::new());

                let tmp: Answer = Answer::Ls { available_map: t };
                assert_eq!(answ, tmp);
            }
            Err(_) => {
                panic!("An error occurred, {}", stream.peer_addr().unwrap());
            }
        }
    }

    #[test]
    fn test_status() {
        let _listener = TcpListener::bind(("localhost", PORT_CLIENT_DAEMON + 5)).unwrap();
        //
        let mut stream = TcpStream::connect(("localhost", PORT_CLIENT_DAEMON + 5)).unwrap();
        let com = Command::Status;
        let stream_tmp = _listener.accept().unwrap().0;
        let dat: Arc<Mutex<DataTemp>> = Arc::new(Mutex::new(DataTemp::new()));

        dat.lock().unwrap().shared.insert(
            String::from("FILE1.dat"),
            PathBuf::from("fake\\path\\FILE1.dat"),
        );

        let tr: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> = Arc::new(Mutex::new(HashMap::new()));
        tr.lock()
            .unwrap()
            .insert(String::from("FILE.dat"), Vec::new());

        let dow: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        command_processor(&com, stream_tmp, dat.lock().unwrap(), tr, dow).unwrap();

        let mut buf = vec![0 as u8; 4096];
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Answer = serde_json::from_slice(&buf[..size]).unwrap();
                //
                let mut tr: HashMap<String, Vec<SocketAddr>> = HashMap::new();
                tr.insert(String::from("FILE.dat"), Vec::new());

                let mut sh: HashMap<String, PathBuf> = HashMap::new();
                sh.insert(
                    String::from("FILE1.dat"),
                    PathBuf::from("fake\\path\\FILE1.dat"),
                );

                let dow_t: Vec<String> = Vec::new();

                let tmp: Answer = Answer::Status {
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
