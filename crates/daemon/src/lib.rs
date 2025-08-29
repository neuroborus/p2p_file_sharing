use std::io;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::str::FromStr; // because we need to be able to do u128::from_str

use p2p_config::{CHUNK_SIZE, DAEMON_MULTICAST_ADDR, LOCAL_NETWORK, PORT_SELF_IP};
use p2p_utils::logger::Logger;

pub static LOGGER: Logger = Logger::verbose("Daemon");

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
    let unique_number: u128 = rand::random();
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
