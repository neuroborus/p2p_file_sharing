    pub use std::{
        env, io,
        net::{Ipv4Addr, UdpSocket},
        collections::HashMap,
    };

    //pub enum c_type{share}

    pub const ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);
    pub const PORT: u16 = 7645;

    /*pub extern crate serde_json;
    pub extern crate serde;*/



    #[macro_use]
    use serde_derive::*;
    //pub extern crate serde_derive;


    #[derive(Serialize, Deserialize)]
    enum Command{
    Share{file_path: String,},
    Scan{},
    LS{file_list: HashMap<u32, String>,},
    Download{file_index: u32,save_path: String,},
    CmdStatus{},
}
