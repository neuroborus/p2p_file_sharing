use lib::*;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use std::thread;
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
fn command_processor(com: &Command, mut stream: TcpStream) -> io::Result<()> {

    //let data: Arc<Mutex<HashMap<String, Vec<Ipv4Addr>>>> = Arc::new(Mutex::new(HashMap::new()));
    match com{
        Command::Share{file_path: f_path} =>{
            println!("!Share!");
            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        Command::Download{file_name: f_name, save_path: s_path} =>{
            println!("!Download!");
            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        Command::Scan =>{
            println!("!Scan!");
            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        Command::LS =>{
            println!("!LS!");
            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        Command::Status =>{
            println!("!Status!");
            let answ = Answer::Ok;
            let serialized = serde_json::to_string(&answ)?;
            stream.write(serialized.as_bytes()).unwrap();
        },
        _ => panic!("Undefined behavior!"),
    }

    Ok(())
}


fn main() -> io::Result<()> {
    println!("Daemon: running");
    //Just output command from client
    let listener = TcpListener::bind(("localhost", PORT))?;

    //Data will be contain key as file_name and Vec of IPs of share file
    let data: Arc<Mutex<HashMap<String, Vec<Ipv4Addr>>>> = Arc::new(Mutex::new(HashMap::new()));
    let (sender, receiver): (Sender<Arc<Mutex<HashMap<String, Vec<Ipv4Addr>>>>>,
        Receiver<Arc<Mutex<HashMap<String, Vec<Ipv4Addr>>>>>) = mpsc::channel();

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
                            println!("{:?}", com);
                            thread::spawn(move||
                                {command_processor(&com, stream)});
                        },
                        Err(_) => {
                            println!("An error occurred, {}", stream.peer_addr().unwrap());
                            //stream.shutdown(Shutdown::Both).unwrap();
                        }
                    }
                ///////////////
                }
                Err(e)=>{
                    println!("Error: {}", e);
                }
            }
        }
    }
}
