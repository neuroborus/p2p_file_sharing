use lib::*;
// On Windows, unlike all Unix variants, it is improper to bind to the multicast address
//
// see https://msdn.microsoft.com/en-us/library/windows/desktop/ms737550(v=vs.85).aspx
#[cfg(windows)]
fn bind_multicast(_addr: &Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
    UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), port))
}

// On unixes we bind to the multicast address, which causes multicast packets to be filtered
#[cfg(unix)]
fn bind_multicast(addr: &Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
    UdpSocket::bind((*addr, port))
}

///
fn command_processor(com: &Command, mut stream: TcpStream, mut data: MutexGuard<DataTemp>) -> io::Result<()> {
    match com{
        Command::Share{file_path: f_path} =>{
            println!("!Share!");

            let name: String = String::from(f_path.file_name().unwrap().to_string_lossy());
            data.shared.insert(name, f_path.clone());   //Name - path

            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        Command::Download{file_name: _file_name, save_path: _save_path} =>{
            println!("!Download!");
            //
            //Empty for now
            //
            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        Command::Scan =>{
            println!("!Scan!");
            let socket = UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), 0))?;
            socket
                .send_to(SCAN_REQUEST, (ADDR_DAEMON_MULTICAST, PORT_MULTICAST))?;
            //get names of files with tcp (his shared - your available)
            let listener = TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), PORT_MULTICAST))?;

            let mut buf = vec![0 as u8; 4096];
            for stream in listener.incoming(){
                match stream{
                    Ok(mut stream)=>{
                        match stream.read(&mut buf) {
                            Ok(size) => {
                                //Get List of names
                                let names: Vec<String> = serde_json::from_slice(&buf[..size])?;
                                for name in names.into_iter() {
                                    if data.available.contains_key(&name){   //If file already exist just update Vec of IP
                                        data.available.get_mut(&name).unwrap().push(stream.peer_addr().unwrap());
                                    }
                                    else{   //In another case - adding file with first IP that share it
                                        let mut v: Vec<SocketAddr> = Vec::new();
                                        v.push(stream.peer_addr().unwrap());
                                        data.available.insert(name, v);
                                    }
                                }
                            },
                            Err(_) => {
                                println!("An error occurred, {}", stream.peer_addr().unwrap());
                            }
                        }
                    }
                    Err(e)=>{
                        println!("Error: {}", e);
                    }
                }
            }
            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        Command::Ls =>{
            println!("!Ls!");
            let answ: Answer = Answer::Ls{available_map: data.available.clone()};
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        Command::Status =>{
            println!("!Status!"); //transferring & shared
            let answ: Answer = Answer::Status{transferring_map: data.transferring.clone(),
                shared_map: data.shared.clone()};
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        //_ => panic!("Undefined behavior!"),
    }

    Ok(())
}

fn multicast_responder(rcvr:  Receiver<Vec<String>>) -> io::Result<()> {
    let listener = bind_multicast(&ADDR_DAEMON_MULTICAST, PORT_MULTICAST)?;
    listener
        .join_multicast_v4(&ADDR_DAEMON_MULTICAST, &Ipv4Addr::new(0, 0, 0, 0))?;

    /*listener
        .join_multicast_v4(&ADDR_DAEMON, &Ipv4Addr::new(0, 0, 0, 0))?;*/

    let mut shared: Vec<String>;
    let mut buf = vec![0; 4096];
    loop {
        let (len, remote_addr) = listener.recv_from(&mut buf)?;
        let message = &buf[..len];
        let mut stream = TcpStream::connect(remote_addr)?;

        if message == SCAN_REQUEST{
            shared = rcvr.recv().unwrap();  //Get vec of names
            let serialized = serde_json::to_string(&shared)?;
            stream.write(serialized.as_bytes()).unwrap();   //Send our "shared"
        }
    }
}

fn main() -> io::Result<()> {
    println!("Daemon: running");
    //Listener for client-daemon connection
    let listener = TcpListener::bind(("localhost", PORT_CLIENT_DAEMON))?;
    //All about files daemon knowledge
    let data: Arc<Mutex<DataTemp>> = Arc::new(Mutex::new(DataTemp::new()));
    //Channel for transfeering info about sharing to multicast_responder
    let (sndr, rcvr): (Sender<Vec<String>>, Receiver<Vec<String>>) = mpsc::channel();

    thread::spawn(move||
        {multicast_responder(rcvr)});
    let mut names: Vec<String>;
    //
    let mut buf = vec![0 as u8; 4096];
    loop {
        for stream in listener.incoming(){
            match stream{
                Ok(mut stream)=>{
                    /////////////
                    match stream.read(&mut buf) {
                        Ok(size) => {
                            let com: Command = serde_json::from_slice(&buf[..size])?;
                            println!{"{:?}", *data};
                            let dat = data.clone();
                            thread::spawn(move||
                                {   command_processor(&com, stream, dat.lock().unwrap()).unwrap();  });   //Unwrap - is it ok?

                            names = Vec::new();
                            for key in data.lock().unwrap().shared.keys() {
                                names.push(key.clone());
                            }
                            sndr.send(names).unwrap();  //Unwrap - is it ok?
                        },
                        Err(_) => {
                            println!("An error occurred, {}", stream.peer_addr().unwrap());
                        }
                    }
                ///////////////
                }
                Err(e)=>{
                    println!("Error: {}", e);
                }
            }
        }

        println!("OK");
    }
}
