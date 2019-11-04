    pub use std::{
        env, io,
        thread,
        path::PathBuf,
        sync::{Arc, Mutex, MutexGuard, mpsc, mpsc::{Sender, Receiver},},
        io::{Read, Write},
        net::{Ipv4Addr, UdpSocket, SocketAddr, TcpStream, TcpListener, Shutdown},
        collections::{LinkedList, HashMap},
    };
    //pub enum c_type{share}

    pub const ADDR_DAEMON: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);
    pub const ADDR_DAEMON_MULTICAST: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 124);
    pub const PORT_CLIENT_DAEMON: u16 = 7645;
    pub const PORT_MULTICAST: u16 = 7646;
    pub const SCAN_REQUEST: &[u8; 20] = b"UDP_Scan_Request_P2P";

    use serde_derive::*;

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Command{   //Client -> Daemon
        Share{file_path: PathBuf,},
        Scan,
        Ls,
        Download{file_name: String, save_path: PathBuf,},
        Status,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Answer{    //Daemon -> Client
//      Scan{}, //Just update ls results?
        Ok,
        Ls{available_map: HashMap<String, Vec<SocketAddr>>}, //available
        Status{transferring_map: HashMap<String, Vec<SocketAddr>>,
            shared_map: HashMap<String, PathBuf>},
    }

    #[derive(Debug)]
    pub struct DataTemp{
    //Available to downloading files: name_of_file--shared IP addresses
        pub available: HashMap<String, Vec<SocketAddr>>,
        pub downloading: HashMap<String, Vec<SocketAddr>>,    //Already downloading
        //
    //Your files, that available to transfer
        pub shared: HashMap<String, PathBuf>,    //FileName - Path
        pub transferring: HashMap<String, Vec<SocketAddr>>    //Already transferring
    }
    impl DataTemp{
        pub fn new()->Self{
            DataTemp{
                available: HashMap::new(),
                downloading: HashMap::new(),
                shared: HashMap::new(),
                transferring: HashMap::new(),
            }
        }
    }

    /*#[derive(Serialize, Deserialize, Debug)]
    pub struct File{
        //file_id: u32,   //Inactive in Answer->Status case
        file_name: String,
        peer: String,   //(Or Ipv4Adress) "Remote" peer or client
    }*/
