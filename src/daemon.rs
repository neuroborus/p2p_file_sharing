use lib::*;

///
fn command_processor(
    com: &Command,
    mut stream: TcpStream,
    mut data: MutexGuard<DataTemp>,
) -> io::Result<()> {
    match com {
        Command::Share { file_path: f_path } => {
            println!("!Share!");

            let name: String = String::from(f_path.file_name().unwrap().to_string_lossy());
            data.shared.insert(name, f_path.clone()); //Name - path

            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        }
        Command::Download {
            file_name: _file_name,
            save_path: _save_path,
            wait: _wait,
        } => {
            println!("!Download!");
            //
            //Empty for now
            //
            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        }
        Command::Scan => {
            println!("!Scan!");
            let socket = UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), 0))?;
            socket.send_to(SCAN_REQUEST, (ADDR_DAEMON_MULTICAST, PORT_MULTICAST))?;
            data.available.clear();// clear list of available files to download
            //the list gonna be refreshed in multicast_receiver

            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        }
        Command::Ls => {
            println!("!Ls!");
            let answ: Answer = Answer::Ls {
                available_map: data.available.clone(),
            };
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        }
        Command::Status => {
            println!("!Status!"); //transferring & shared
            let answ: Answer = Answer::Status {
                transferring_map: data.transferring.clone(),
                shared_map: data.shared.clone(),
            };
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        }
    }
    
    println!("{:?}", data);

    Ok(())
}

fn multicast_responder(data: Arc<Mutex<DataTemp>>) -> io::Result<()> {

    let this_daemon_ip = get_this_daemon_ip().unwrap();

    let listener = bind_multicast(&ADDR_DAEMON_MULTICAST, PORT_MULTICAST)?;
    listener.join_multicast_v4(&ADDR_DAEMON_MULTICAST, &Ipv4Addr::new(0, 0, 0, 0))?;

    let mut shared: Vec<String>;
    let mut buf = vec![0; 4096];
    loop {
        //println!("MLTCST_RESPOND PART 1");
        let (len, remote_addr) = listener.recv_from(&mut buf)?;
        //println!("MLTCST_RESPOND PART 2 {}", remote_addr);
        if remote_addr.ip() != this_daemon_ip {
            //check if that's not our daemon, then we will respond
            let message = &buf[..len];
            let mut stream = TcpStream::connect((remote_addr.ip(), PORT_SCAN_TCP))?;
            println!("MULTICAST RESPONDING TO {}", remote_addr.ip());

            if message == SCAN_REQUEST {
                let dat = data.lock().unwrap();
                shared = Vec::new();
                for key in dat.shared.keys() {
                    shared.push(key.clone());
                }
                let serialized = serde_json::to_string(&shared)?;
                stream.write(serialized.as_bytes()).unwrap(); //Send our "shared"
            }
        }
    }
}

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
                                data.lock().unwrap().available
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
                    Err(_) => {
                        println!("An error occurred, {}", stream.peer_addr().unwrap());
                    }
                }
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    println!("Daemon: running");
    //Listener for client-daemon connection
    let listener = TcpListener::bind(("localhost", PORT_CLIENT_DAEMON))?;
    //All about files daemon knowledge
    let data: Arc<Mutex<DataTemp>> = Arc::new(Mutex::new(DataTemp::new()));
    //Channel for transfeering info about sharing to multicast_responder

    let mult_resp_data = data.clone();
    thread::spawn(move || {
        multicast_responder(mult_resp_data).unwrap();
    });

    let mult_recv_data = data.clone();
    thread::spawn(move || {
        multicast_receiver(mult_recv_data).unwrap();
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

                        println!("{:?}", *data);
                        let dat = data.clone();
                        thread::spawn(move || {
                            command_processor(&com, stream, dat.lock().unwrap()).unwrap();
                        });
                        
                    }
                    Err(_) => {
                        println!("An error occurred, {}", stream.peer_addr().unwrap());
                    }
                }
                ///////////////
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }

    Ok(())
}
