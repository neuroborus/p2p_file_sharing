use lib::*;
//socket == "channel"
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
        .arg(Arg::with_name("FLG_BLOCK_INPUT").short("w"))
        .arg(
            Arg::with_name("FILE_NAME") //Filename (download) - is option now
                .short("f")
                .takes_value(true),
        )
        /*  .arg(Arg::with_name("SAVE_PATH")    //Commented until I understood why
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
            //println!("\n\n\tshare\n");
            //////////
            if flag {
                let f_path = PathBuf::from(matches.value_of("FILE_PATH").unwrap());

                if f_path.is_file() {
                    //Checking the file
                    let com = Command::Share { file_path: f_path };
                    //
                    let serialized = serde_json::to_string(&com)?;
                    stream.write(serialized.as_bytes()).unwrap();
                } else {
                    println!("File does not exists!");
                    flag = false;
                }
            }
        }
        "scan" => {
            //println!("\n\n\tscan\n");
            //////////
            let com = Command::Scan;
            //
            let serialized = serde_json::to_string(&com)?;
            stream.write(serialized.as_bytes()).unwrap();
        }
        "ls" => {
            //println!("\n\n\tls\n");
            //////////
            let com = Command::Ls;
            //
            let serialized = serde_json::to_string(&com)?;
            stream.write(serialized.as_bytes()).unwrap();
        }
        "download" => {
            if !matches.is_present("FILE_NAME") {
                println!("No file name to download!");
                flag = false;
            }
            //
            if flag == true {
                let s_path: PathBuf;
                if matches.is_present("FILE_PATH") {
                    s_path = PathBuf::from(matches.value_of("FILE_PATH").unwrap());
                } else {
                    s_path = PathBuf::from("");
                }
                //
                let f_name: String = String::from(matches.value_of("FILE_NAME").unwrap());
                //println!("\n\n\tls\n");
                //////////
                let com = Command::Download {
                    file_name: f_name,
                    save_path: s_path,
                };
                //
                let serialized = serde_json::to_string(&com)?;
                stream.write(serialized.as_bytes()).unwrap();
            }
        }
        "status" => {
            //println!("\n\n\tstatus\n");
            //////////
            let com = Command::Status;
            //
            let serialized = serde_json::to_string(&com)?;
            stream.write(serialized.as_bytes()).unwrap();
        }
        _ => {
            println!("Wrong command!");
            flag = false;
        }
    }

    if flag {
        let mut buf = vec![0 as u8; 4096];
        match stream.read(&mut buf) {
            Ok(size) => {
                let answ: Answer = serde_json::from_slice(&buf[..size])?;
                println!("{:?}", answ);
            }
            Err(_) => {
                println!("An error occurred, {}", stream.peer_addr().unwrap());
            }
        }
    }

    Ok(())
}
