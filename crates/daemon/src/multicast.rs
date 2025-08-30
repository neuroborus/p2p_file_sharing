use std::io;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::str::FromStr; // because we need to be able to do u128::from_str
use std::sync::{Arc, Mutex};
use rand::{random};

use p2p_config::{
    CHUNK_SIZE, DAEMON_MULTICAST_ADDR, LOCAL_NETWORK, PORT_MULTICAST, PORT_SCAN_TCP, PORT_SELF_IP,
    SCAN_REQUEST,
};
use crate::entities::FileState;
use p2p_core::utils::create_buffer;

use crate::utils::*;

#[cfg(windows)]
pub fn bind_multicast(_addr: &Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
    UdpSocket::bind((LOCAL_NETWORK, port))
}
#[cfg(unix)]
pub fn bind_multicast(addr: &Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
    UdpSocket::bind((*addr, port))
}

/// Getting an IP of the current daemon thread
pub fn get_this_daemon_ip() -> io::Result<IpAddr> {
    let unique_number: u128 = random();
    let self_ip: IpAddr;
    {
        LOGGER.debug(format!(
            "selfip: bind_multicast({}, {})",
            DAEMON_MULTICAST_ADDR, PORT_SELF_IP
        ));
        let listener = bind_multicast(&DAEMON_MULTICAST_ADDR, PORT_SELF_IP)?;
        listener
            .join_multicast_v4(&DAEMON_MULTICAST_ADDR, &LOCAL_NETWORK)
            .unwrap();
        {
            LOGGER.debug("selfip: send probe token");
            let socket = UdpSocket::bind((LOCAL_NETWORK, 0)).unwrap();
            LOGGER.debug(format!(
                "selfip: probe sender local_addr={}",
                socket.local_addr().unwrap()
            ));
            socket
                .send_to(
                    unique_number.to_string().as_bytes(),
                    (DAEMON_MULTICAST_ADDR, PORT_SELF_IP),
                )
                .unwrap();
        }
        let mut buf = vec![0; CHUNK_SIZE];
        loop {
            let (len, remote_addr) = listener.recv_from(&mut buf).unwrap();
            LOGGER.debug(format!("selfip: got {} bytes from {}", len, remote_addr));
            let msg = &buf[..len];
            let rec_num = u128::from_str(str::from_utf8(msg).unwrap()).unwrap();
            if rec_num == unique_number {
                self_ip = remote_addr.ip();
                break;
            }
            continue;
        }
    }

    LOGGER.info(&format!("Daemon IP in the local network is {self_ip}"));
    LOGGER.debug(format!("selfip: resolved {}", self_ip));
    Ok(self_ip)
}

/// Responds to multicast requests from other daemons
pub fn multicast_responder(data: Arc<Mutex<FileState>>) -> io::Result<()> {
    let this_daemon_ip = get_this_daemon_ip().unwrap();

    LOGGER.debug(format!(
        "responder: bind_multicast({}, {})",
        DAEMON_MULTICAST_ADDR, PORT_MULTICAST
    ));
    let listener = bind_multicast(&DAEMON_MULTICAST_ADDR, PORT_MULTICAST)?;
    listener.join_multicast_v4(&DAEMON_MULTICAST_ADDR, &LOCAL_NETWORK)?;
    LOGGER.debug(format!(
        "responder: joined group={}, iface={}",
        DAEMON_MULTICAST_ADDR, LOCAL_NETWORK
    ));

    let mut shared: Vec<String>;
    let mut buf = vec![0; CHUNK_SIZE];
    loop {
        let (len, remote_addr) = listener.recv_from(&mut buf)?;
        LOGGER.debug(format!(
            "responder: recv_from {} ({} bytes)",
            remote_addr, len
        ));
        let remote_addr_ip = remote_addr.ip();
        if remote_addr_ip != this_daemon_ip {
            // Check if that's not our daemon, then we will respond
            let message = &buf[..len];
            LOGGER.debug(format!(
                "responder: msg first20={:?}",
                &message[..message.len().min(20)]
            ));
            let mut stream = match TcpStream::connect((remote_addr_ip, PORT_SCAN_TCP)) {
                Ok(s) => s,
                Err(e) => {
                    LOGGER.error(format!(
                        "responder: connect to {}:{} failed: {}",
                        remote_addr_ip, PORT_SCAN_TCP, e
                    ));
                    continue;
                }
            };

            if message == SCAN_REQUEST {
                LOGGER.debug(format!(
                    "responder: SCAN from {} -> connect {}:{}",
                    remote_addr_ip, remote_addr_ip, PORT_SCAN_TCP
                ));
                LOGGER.info(format!("{remote_addr_ip} asked for scan"));
                let dat = data.lock().unwrap();
                shared = Vec::new();
                for key in dat.shared.keys() {
                    shared.push(key.clone());
                }
                let serialized = serde_json::to_string(&shared)?;
                stream.write_all(serialized.as_bytes()).unwrap(); // Send our "shared" files list
            }
        }
    }
}

// Function that receiving answer from other daemons to refresh our "available
// files to download" list
pub fn multicast_receiver(data: Arc<Mutex<FileState>>) -> io::Result<()> {
    LOGGER.debug(format!(
        "receiver: bind TCP {}:{}",
        LOCAL_NETWORK, PORT_SCAN_TCP
    ));
    let listener = TcpListener::bind((LOCAL_NETWORK, PORT_SCAN_TCP))?;
    LOGGER.debug(format!(
        "receiver: listening on {}",
        listener.local_addr().unwrap()
    ));
    // Get names of files with tcp (his shared - your available)
    let mut buf = create_buffer(CHUNK_SIZE);
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                LOGGER.debug(format!(
                    "receiver: incoming from {}",
                    stream.peer_addr().unwrap()
                ));
                match stream.read(&mut buf) {
                    Ok(size) => {
                        LOGGER.debug(format!("receiver: read {} bytes", size));
                        // Get List of names
                        let names: Vec<String> = serde_json::from_slice(&buf[..size])?;
                        LOGGER.debug(format!("receiver: parsed {} names", names.len()));
                        for name in names.into_iter() {
                            if data.lock().unwrap().available.contains_key(&name) {
                                // If file already exist just update Vec of IP
                                data.lock()
                                    .unwrap()
                                    .available
                                    .get_mut(&name)
                                    .unwrap()
                                    .push(stream.peer_addr().unwrap());
                            } else {
                                // In another case - adding file with first IP that share it
                                let mut v: Vec<SocketAddr> = Vec::new();
                                v.push(stream.peer_addr().unwrap());
                                data.lock().unwrap().available.insert(name, v);
                            }
                        }
                        LOGGER.debug(format!(
                            "receiver: available files now {}",
                            data.lock().unwrap().available.len()
                        ));
                    }
                    Err(e) => {
                        LOGGER.debug(format!("receiver: read error: {}", e));
                        LOGGER.error(e);
                    }
                }
            }
            Err(e) => {
                LOGGER.debug(format!("receiver: accept error: {}", e));
                LOGGER.error(e);
            }
        }
    }
    Ok(())
}
