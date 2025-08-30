use std::io;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command};
use p2p_config::{CHUNK_SIZE, LOCALHOST, PORT_CLIENT_DAEMON};
use p2p_core::entities::{Action, Response};
use p2p_core::utils::{Logger, create_buffer};
use serde_json::{from_slice, to_string};

pub static LOGGER: Logger = Logger::compact("cli");

pub fn connect() -> TcpStream {
    LOGGER.debug(format!("connect {}:{}", LOCALHOST, PORT_CLIENT_DAEMON));
    let stream = TcpStream::connect((LOCALHOST, PORT_CLIENT_DAEMON)).unwrap();
    LOGGER.debug(format!("connected, local={}", stream.local_addr().unwrap()));
    stream
}

/// Builds the CLI interface definition for the `p2p-cli` client.
///
/// # Overview
/// This command-line tool is used to interact with a running P2P daemon
/// over TCP. It supports sharing files, scanning the LAN for available
/// files, listing them, downloading, and checking current transfer status.
///
/// # Subcommands
///
/// - **share** `<PATH>`   Share a file with other peers in the LAN.
///   - `PATH`: Path to the file to share (required).
///
///   Example:
///   ```bash
///   p2p-cli share ./myfile.txt
///   ```
///
/// - **scan**   Broadcast a scan request to discover all files shared in the
///   LAN.   Example: ```bash p2p-cli scan ```
///
/// - **ls**   List all files currently available to download (after a scan).
///   Example: ```bash p2p-cli ls ```
///
/// - **download** `-f <NAME>` [`-o <OUT_DIR>`] [`-w`]   Download a file by name
///   from available peers.
///   - `-f, --file <NAME>`: Name of the file to download (required).
///   - `-o, --out <OUT_DIR>`: Optional output directory.
///   - `-w, --wait`: Wait (block) until the download finishes.
///
///   Examples:
///   ```bash
///   # Download file into current directory without waiting
///   p2p-cli download -f myfile.txt
///
///   # Download file into specific directory and wait until finished
///   p2p-cli download -f myfile.txt -o ./downloads -w
///   ```
///
/// - **status**   Show the current daemon status: files being transferred,
///   shared files, and ongoing downloads.   Example: ```bash p2p-cli status ```
///
/// # Usage
/// 1. Start the daemon on your machine.
/// 2. Run commands from this CLI to interact with it.
///
/// This function only **defines** the CLI structure; to parse user input,
/// call `.get_matches()` on the returned `Command` in `main()`.
pub fn create_command() -> Command {
    Command::new("p2p-cli")
        .about("Interaction with daemon")
        .subcommand_required(true) // should be at least 1 action
        .arg_required_else_help(true)
        .subcommand(
            Command::new("share").about("Share a file").arg(
                Arg::new("FILE_PATH")
                    .help("Path to file to share")
                    .required(true) // path arg
                    .value_name("PATH"),
            ),
        )
        .subcommand(Command::new("scan").about("Scan LAN for shared files"))
        .subcommand(Command::new("ls").about("List available files"))
        .subcommand(
            Command::new("download")
                .about("Download a file")
                .arg(
                    Arg::new("FILE_NAME")
                        .help("Name of file to download")
                        .required(true)
                        .short('f')
                        .long("file")
                        .value_name("NAME"),
                )
                .arg(
                    Arg::new("FILE_PATH")
                        .help("Optional save directory")
                        .value_name("OUT_DIR")
                        .short('o')
                        .long("out"),
                )
                .arg(
                    Arg::new("WAIT")
                        .help("Wait (block) until download finishes")
                        .short('w')
                        .long("wait")
                        .action(ArgAction::SetTrue), // bool-flag
                ),
        )
        .subcommand(Command::new("status").about("Show current status"))
}

pub fn process_actions(stream: &mut TcpStream, matches: &ArgMatches) -> io::Result<()> {
    match matches.subcommand() {
        Some(("share", sub)) => {
            if !sub.args_present() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "No path for sharing!",
                ));
            }
            let f_path = PathBuf::from(sub.get_one::<String>("FILE_PATH").unwrap());

            if f_path.is_file() {
                // Checking the file
                let share = Action::Share { file_path: f_path };
                //
                LOGGER.debug("send Action::Share");
                let serialized = to_string(&share)?;
                stream.write_all(serialized.as_bytes()).unwrap();
                LOGGER.debug("request written, waiting for reply...");
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "File does not exists!",
                ));
            }
        }

        Some(("scan", _)) => {
            let scan: Action = Action::Scan;
            //
            LOGGER.debug("send Action::Scan");
            let serialized = to_string(&scan)?;
            stream.write_all(serialized.as_bytes()).unwrap();
            LOGGER.debug("request written, waiting for reply...");
        }

        Some(("ls", _)) => {
            let ls: Action = Action::Ls;

            LOGGER.debug("send Action::Ls");
            let serialized = to_string(&ls)?;
            stream.write_all(serialized.as_bytes()).unwrap();
            LOGGER.debug("request written, waiting for reply...");
        }

        Some(("download", sub)) => {
            if !sub.args_present() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "No file name to download!",
                ));
            }

            let s_path: PathBuf = sub
                .get_one::<String>("FILE_PATH")
                .map(|s| PathBuf::from(s))
                .unwrap_or_default();

            let f_name: String = String::from(sub.get_one::<String>("FILE_NAME").unwrap());

            let wait = sub.get_flag("WAIT");
            let download: Action = Action::Download {
                file_name: f_name,
                save_path: s_path,
                wait,
            };

            LOGGER.debug("send Action::Download");
            let serialized = to_string(&download)?;
            stream.write_all(serialized.as_bytes()).unwrap();
            LOGGER.debug("request written, waiting for reply...");
        }

        Some(("status", _)) => {
            let status: Action = Action::Status;

            LOGGER.debug("send Action::Status");
            let serialized = to_string(&status)?;
            stream.write_all(serialized.as_bytes()).unwrap();
            LOGGER.debug("request written, waiting for reply...");
        }

        _ => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Wrong action!"));
        }
    }

    Ok(())
}

pub fn process_daemon_response(stream: &mut TcpStream) -> io::Result<()> {
    let mut buf = create_buffer(CHUNK_SIZE);
    match stream.read(&mut buf) {
        Ok(size) => {
            let answ: Response = from_slice(&buf[..size])?;
            LOGGER.debug(format!("got reply {} bytes", size));

            match answ {
                Response::Ls { available_map: map } => {
                    LOGGER.debug(format!("Ls -> {} files", map.len()));
                    LOGGER.info("Files available to download:");

                    for file in map.keys() {
                        println!("\t{}", file);
                    }
                }

                Response::Status {
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

                    LOGGER.info("Downloading:");

                    for file in d_map {
                        println!("\t{}", file);
                    }
                }

                Response::Err(e) => {
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
    Ok(())
}
