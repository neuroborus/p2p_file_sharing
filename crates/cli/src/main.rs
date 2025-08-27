use p2p_core::*;

use clap::{App, Arg};

fn main() -> io::Result<()> {
    let mut stream = TcpStream::connect(("localhost", PORT_CLIENT_DAEMON)).unwrap();
    //Parsing arguments
    //share "file_path"
    //download "save_path" -fFileName (flag and save path in any order)
    //scan //ls //status
    let matches = App::new("ClientP2P")
        .about("Interaction with daemon")
        .arg(Arg::with_name("COMMAND").required(true))
        .arg(Arg::with_name("FLG_WAIT").short("w"))
        .arg(
            Arg::with_name("FILE_NAME") //Filename (download) - is option now
                .short("f")
                .takes_value(true),
        )
        /*  .arg(Arg::with_name("SAVE_PATH")    //Relative path
            .short("o")
            .takes_value(true)
        )*/
        .arg(Arg::with_name("FILE_PATH"))
        .get_matches();

    let mut flag: bool = true; //Now we don't need panic!
    match matches.value_of("COMMAND").unwrap() {
        "share" => {
            if !matches.is_present("FILE_PATH") {
                println!("No path for sharing!");
                flag = false;
            }
            if flag {
                let f_path = PathBuf::from(matches.value_of("FILE_PATH").unwrap());

                if f_path.is_file() {
                    //Checking the file
                    let com = Command::Share { file_path: f_path };
                    //
                    let serialized = serde_json::to_string(&com)?;
                    stream.write_all(serialized.as_bytes()).unwrap();
                } else {
                    eprintln!("File does not exists!");
                    flag = false;
                }
            }
        }
        "scan" => {
            let com = Command::Scan;
            //
            let serialized = serde_json::to_string(&com)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
        "ls" => {
            let com = Command::Ls;
            //
            let serialized = serde_json::to_string(&com)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
        "download" => {
            if !matches.is_present("FILE_NAME") {
                println!("No file name to download!");
                flag = false;
            }
            //
            if flag {
                let s_path: PathBuf;
                if matches.is_present("FILE_PATH") {
                    s_path = PathBuf::from(matches.value_of("FILE_PATH").unwrap());
                } else {
                    s_path = PathBuf::from("");
                }
                //
                let f_name: String = String::from(matches.value_of("FILE_NAME").unwrap());
                //
                let com: Command;
                if matches.is_present("FLG_WAIT") {
                    //If the user wants to block the input until the file is downloaded
                    com = Command::Download {
                        file_name: f_name,
                        save_path: s_path,
                        wait: true,
                    };
                } else {
                    com = Command::Download {
                        file_name: f_name,
                        save_path: s_path,
                        wait: false,
                    };
                }
                //
                let serialized = serde_json::to_string(&com)?;
                stream.write_all(serialized.as_bytes()).unwrap();
            }
        }
        "status" => {
            let com = Command::Status;
            //
            let serialized = serde_json::to_string(&com)?;
            stream.write_all(serialized.as_bytes()).unwrap();
        }
        _ => {
            eprintln!("Wrong command!");
            flag = false;
        }
    }

    if flag {
        let mut buf = vec![0 as u8; 4096];
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Answer = serde_json::from_slice(&buf[..size])?;
                //println!("{:?}", answ);
                match answ {
                    Answer::Ls { available_map: map } => {
                        println!("Files available to download:");
                        for file in map.keys() {
                            println!("\t{}", file);
                        }
                    }
                    Answer::Status {
                        transferring_map: t_map,
                        shared_map: s_map,
                        downloading_map: d_map,
                    } => {
                        println!("Sharing:");
                        for file in s_map.keys() {
                            println!("\t{}", file);
                            let f_vec = t_map.get(file);
                            match f_vec {
                                Some(vec) => {
                                    for peer in vec.iter() {
                                        println!("\t\t{}", peer.ip());
                                    }
                                }
                                None => (),
                            }
                        }
                        println!("Downloading:");
                        for file in d_map {
                            println!("\t{}", file);
                        }
                    }
                    Answer::Err(e) => {
                        println!("{}", e);
                    }
                    _ => (),
                }
            }
            Err(_) => {
                eprintln!("An error occurred, {}", stream.peer_addr().unwrap());
            }
        }
    }

    Ok(())
}
