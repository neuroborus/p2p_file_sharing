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

fn main() -> io::Result<()> {
    println!("Daemon: running");
    //Just output command from client
    let listener = TcpListener::bind(("localhost", PORT))?;
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
                        },
                        Err(_) => {
                            println!("An error occurred, terminating connection with {}", stream.peer_addr().unwrap());
                            stream.shutdown(Shutdown::Both).unwrap();
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
