pub use std::net::Ipv4Addr;

pub const LOCALHOST: &str = "localhost";
pub const LOCAL_NETWORK: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);

pub const DAEMON_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);

pub const PORT_MULTICAST: u16 = 7645;
pub const PORT_CLIENT_DAEMON: u16 = 7646;
pub const PORT_SCAN_TCP: u16 = 7647;
pub const PORT_FILE_SHARE: u16 = 7648;
pub const PORT_SELF_IP: u16 = 60005;

pub const SCAN_REQUEST: &[u8; 20] = b"UDP_Scan_Request_P2P";
