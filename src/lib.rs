    pub use std::{
        env, io,
        thread,
        sync::{Arc, Mutex, mpsc},
        io::{Read, Write},
        net::{Ipv4Addr, UdpSocket, TcpStream, TcpListener, Shutdown},
        collections::{LinkedList, HashMap},
    };
    //pub enum c_type{share}

    pub const ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);
    pub const PORT: u16 = 7645;

    use serde_derive::*;

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Command{   //Client -> Daemon
        Share{file_path: String,},
        Scan,
        LS,
        Download{file_name: String, save_path: String,},
        Status,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Answer{    //Daemon -> Client
//      Scan{}, //Just update ls results?
        Ok,
        LS{file_list: LinkedList<File>,},
        Status{download_list: LinkedList<File>, shared_list: LinkedList<String>},
    }//shared_list - just names of files, should I use File struct too?

    #[derive(Serialize, Deserialize, Debug)]
    pub struct File{
        //file_id: u32,   //Inactive in Answer->Status case
        file_name: String,
        peer: String,   //(Or Ipv4Adress) "Remote" peer or client
    }
