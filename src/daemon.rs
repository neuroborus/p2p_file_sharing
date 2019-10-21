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

    let listener = bind_multicast(&ADDR, PORT)?;
    listener
        .join_multicast_v4(&ADDR, &Ipv4Addr::new(0, 0, 0, 0))?;

    let mut buf = vec![0; 4096];
    loop {
        let (len, remote_addr) = listener.recv_from(&mut buf)?;
        let com: Command = serde_json::from_slice(&buf[..len])?;
        println!("{:?}", com);

        let responder = UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), 0))?;
        responder
            .send_to(b"hello, client!", &remote_addr)?;

        println!("server: sent response to {}", remote_addr);
    }
}
