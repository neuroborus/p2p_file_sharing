    pub use std::{
        env, io,
        net::{Ipv4Addr, UdpSocket},
    };

    //pub enum c_type{share}

    pub const ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);
    pub const PORT: u16 = 7645;

    /*pub extern crate serde_json;
    pub extern crate serde;*/
