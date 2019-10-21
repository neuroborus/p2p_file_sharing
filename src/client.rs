use lib::*;
//socket == "channel"
use clap::{Arg, App};

fn main() -> io::Result<()> {
    assert!(ADDR.is_multicast());
    let socket = UdpSocket::bind((Ipv4Addr::new(0,0,0,0), 0))?;
    //Parsing arguments
    let matches = App::new("ClientP2P")
                          .about("Interaction with daemon")
                          .arg(Arg::with_name("COMMAND")
                          .required(true)
                      )
                          .arg(Arg::with_name("FLG_BLOCK_INPUT")
                          .short("w")
                      )
                          .arg(Arg::with_name("FILE_NAME")    //Filename (download) - is option now
                          .short("f")
                          .takes_value(true)
                      )
                          .arg(Arg::with_name("SHARE_PATH")
                          .short("o")
                          .takes_value(true)
                      )
                          .arg(Arg::with_name("FILE_PATH")
                      )
                          .get_matches();

    match matches.value_of("COMMAND").unwrap(){
        "share" => {
            if !matches.is_present("FILE_PATH"){
                panic!("No path for sharing!")
            }
            //println!("\n\n\tshare\n");
            //////////
            let f_path = String::from(matches.value_of("FILE_PATH").unwrap());
            let com = Command::Share{file_path: f_path};
            //  NEED TCP HERE, NOT A UDP. NEED TO REDO!!!
            let serialized = serde_json::to_string(&com)?;
            socket.send_to(serialized.as_bytes(), (ADDR, PORT))?;
        },
        "scan" => {
            //println!("\n\n\tscan\n");
            //////////
            let com = Command::Scan;
            //  NEED TCP HERE, NOT A UDP. NEED TO REDO!!!
            let serialized = serde_json::to_string(&com)?;
            socket.send_to(serialized.as_bytes(), (ADDR, PORT))?;
        },
        "ls" => {
            //println!("\n\n\tls\n");
            //////////
            let com = Command::LS;
            //  NEED TCP HERE, NOT A UDP. NEED TO REDO!!!
            let serialized = serde_json::to_string(&com)?;
            socket.send_to(serialized.as_bytes(), (ADDR, PORT))?;
        },
        "download" => {
            if !matches.is_present("FILE_NAME"){
                panic!("No file name to download!")
            }
            //
            let s_path: String;
            if matches.is_present("FILE_PATH"){
                s_path = String::from(matches.value_of("FILE_PATH").unwrap());
            }
            else{
                s_path = String::from("");
            }
            //
            let f_name: String = String::from(matches.value_of("FILE_NAME").unwrap());
            //println!("\n\n\tls\n");
            //////////
            let com = Command::Download{file_name: f_name, save_path: s_path};
            //  NEED TCP HERE, NOT A UDP. NEED TO REDO!!!
            let serialized = serde_json::to_string(&com)?;
            socket.send_to(serialized.as_bytes(), (ADDR, PORT))?;
        },
        "status" => {
            //println!("\n\n\tstatus\n");
            //////////
            let com = Command::Status;
            //  NEED TCP HERE, NOT A UDP. NEED TO REDO!!!
            let serialized = serde_json::to_string(&com)?;
            socket.send_to(serialized.as_bytes(), (ADDR, PORT))?;
        },
        _ => {
            panic!("Wrong command!");
        }

    }
    Ok(())
}
