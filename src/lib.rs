    pub use std::{
        env, io,
        net::{Ipv4Addr, UdpSocket},
        collections::LinkedList,
    };

    //pub enum c_type{share}

    pub const ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);
    pub const PORT: u16 = 7645;

    /*pub extern crate serde_json;
    pub extern crate serde;*/

    //#[macro_use]
    use serde_derive::*;

    #[derive(Serialize, Deserialize)]
    pub enum Command{   //Client -> Daemon
        Share{file_path: String,},
        Scan{},
        LS{},
        Download{file_id: u32, save_path: String,},
        //Download{file_name: String, save_path: String,},
        Status{},
    }

    #[derive(Serialize, Deserialize)]
    pub enum Answer{    //Daemon -> Client
//      Scan{}, //Just update ls results?
        LS{file_list: LinkedList<File>,},
        Status{download_list: LinkedList<File>, shared_list: LinkedList<String>},
    }//shared_list - just names of files, should I use File struct too?

    #[derive(Serialize, Deserialize)]
    pub struct File{
        file_id: u32,   //Inactive in Anwer->Status case
        file_name: String,
        peer: String,   //(Or Ipv4Adress) "Remote" peer or client
    }
