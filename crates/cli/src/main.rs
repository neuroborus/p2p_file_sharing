use clap::{App, Arg};
use p2p_config::*;
use p2p_core::*;
use p2p_utils::logger::Logger;

static LOGGER: Logger = Logger::compact("cli");

fn main() -> io::Result<()> {
    LOGGER.debug(format!("connect {}:{}", LOCALHOST, PORT_CLIENT_DAEMON));
    let mut stream = TcpStream::connect((LOCALHOST, PORT_CLIENT_DAEMON)).unwrap();
    LOGGER.debug(format!("connected, local={}", stream.local_addr().unwrap()));

    // Parsing arguments
    // share "file_path"
    // download "save_path" -fFileName (flag and save path in any order)
    // scan //ls //status
    let matches = App::new("ClientP2P")
        .about("Interaction with daemon")
        .arg(Arg::with_name("COMMAND").required(true))
        .arg(Arg::with_name("FLG_WAIT").short("w"))
        .arg(
            Arg::with_name("FILE_NAME") // Filename (download) - is option now
                .short("f")
                .takes_value(true),
        )
        //  .arg(Arg::with_name("SAVE_PATH")    //Relative path
        // .short("o")
        // .takes_value(true)
        // )
        .arg(Arg::with_name("FILE_PATH"))
        .get_matches();

    let mut flag: bool = true; // Now we don't need panic!
    match matches.value_of("COMMAND").unwrap() {
        "share" => {
            if !matches.is_present("FILE_PATH") {
                LOGGER.error("No path for sharing!");

                flag = false;
            }
            if flag {
                let f_path = PathBuf::from(matches.value_of("FILE_PATH").unwrap());

                if f_path.is_file() {
                    // Checking the file
                    let share = Action::Share { file_path: f_path };
                    //
                    LOGGER.debug("send Action::Share");
                    let serialized = serde_json::to_string(&share)?;
                    stream.write_all(serialized.as_bytes()).unwrap();
                    LOGGER.debug("request written, waiting for reply...");
                } else {
                    LOGGER.error("File does not exists!");

                    flag = false;
                }
            }
        }
        "scan" => {
            let scan: Action = Action::Scan;
            //
            LOGGER.debug("send Action::Scan");
            let serialized = serde_json::to_string(&scan)?;
            stream.write_all(serialized.as_bytes()).unwrap();
            LOGGER.debug("request written, waiting for reply...");
        }
        "ls" => {
            let ls: Action = Action::Ls;
            //
            LOGGER.debug("send Action::Ls");
            let serialized = serde_json::to_string(&ls)?;
            stream.write_all(serialized.as_bytes()).unwrap();
            LOGGER.debug("request written, waiting for reply...");
        }
        "download" => {
            if !matches.is_present("FILE_NAME") {
                LOGGER.error("No file name to download!");

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
                let download: Action;
                if matches.is_present("FLG_WAIT") {
                    // If the user wants to block the input until the file is downloaded
                    download = Action::Download {
                        file_name: f_name,
                        save_path: s_path,
                        wait: true,
                    };
                } else {
                    download = Action::Download {
                        file_name: f_name,
                        save_path: s_path,
                        wait: false,
                    };
                }
                //
                LOGGER.debug("send Action::Download");
                let serialized = serde_json::to_string(&download)?;
                stream.write_all(serialized.as_bytes()).unwrap();
                LOGGER.debug("request written, waiting for reply...");
            }
        }
        "status" => {
            let status: Action = Action::Status;
            //
            LOGGER.debug("send Action::Status");
            let serialized = serde_json::to_string(&status)?;
            stream.write_all(serialized.as_bytes()).unwrap();
            LOGGER.debug("request written, waiting for reply...");
        }
        _ => {
            LOGGER.error("Wrong action!");

            flag = false;
        }
    }

    if flag {
        let mut buf = vec![0 as u8; CHUNK_SIZE];
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Answer = serde_json::from_slice(&buf[..size])?;
                LOGGER.debug(format!("got reply {} bytes", size));
                // println!("{:?}", answ);
                match answ {
                    Answer::Ls { available_map: map } => {
                        LOGGER.debug(format!("Ls -> {} files", map.len()));
                        LOGGER.info("Files available to download:");

                        for file in map.keys() {
                            println!("\t{}", file);
                        }
                    }
                    Answer::Status {
                        transferring_map: t_map,
                        shared_map: s_map,
                        downloading_map: d_map,
                    } => {
                        LOGGER.debug(format!(
                            "Status -> shared={} transferring={} downloading={}",
                            s_map.len(),
                            t_map.len(),
                            d_map.len()
                        ));
                        LOGGER.info("Sharing:");

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

                        // ?:
                        LOGGER.info("Downloading:");

                        for file in d_map {
                            println!("\t{}", file);
                        }
                    }
                    Answer::Err(e) => {
                        LOGGER.debug(format!("read error from {}", stream.peer_addr().unwrap()));
                        LOGGER.error(e);
                    }
                    _ => (),
                }
            }
            Err(_) => {
                LOGGER.error(stream.peer_addr().unwrap());
            }
        }
    }

    Ok(())
}
