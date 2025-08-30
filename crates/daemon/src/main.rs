use std::collections::HashMap;
use std::io::Read;
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::{io, thread};

use p2p_config::{CHUNK_SIZE, LOCALHOST, PORT_CLIENT_DAEMON};
use p2p_core::entities::Action;
use p2p_core::utils::create_buffer;
use p2p_daemon::*;

fn main() -> io::Result<()> {
    LOGGER.info("Running...");

    // Listen for client connections.
    let listener = TcpListener::bind((LOCALHOST, PORT_CLIENT_DAEMON))?;

    // Shared state: known remote files and locally shared files.
    let data: Arc<Mutex<FileState>> = Arc::new(Mutex::new(FileState::new()));

    // Peers currently downloading each file from this daemon.
    let transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Files this daemon is currently downloading.
    let downloading: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Answer multicast discovery requests.
    let mult_resp_data = data.clone();
    thread::spawn(move || {
        multicast_responder(mult_resp_data).unwrap();
    });

    // Receive multicast announcements from peers.
    let mult_recv_data = data.clone();
    thread::spawn(move || {
        multicast_receiver(mult_recv_data).unwrap();
    });

    // Serve file-share requests (TCP).
    let share_transfer = transferring.clone();
    let share_data = data.clone();
    thread::spawn(move || {
        share_responder(share_transfer, share_data).unwrap();
    });

    let mut buf = create_buffer(CHUNK_SIZE);
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                match stream.read(&mut buf) {
                    Ok(size) => {
                        // Parse incoming action; ignore malformed JSON.
                        let action: Action;
                        match serde_json::from_slice(&buf[..size]) {
                            Ok(c) => {
                                action = c;
                            }
                            Err(_) => {
                                LOGGER.debug("Client made a mistake!");

                                continue;
                            }
                        }

                        let dat = data.clone();
                        let transfer = transferring.clone();
                        let download = downloading.clone();
                        thread::spawn(move || {
                            action_processor(
                                &action,
                                stream,
                                dat.lock().unwrap(),
                                transfer,
                                download,
                            )
                            .unwrap();
                        });
                    }
                    Err(e) => {
                        LOGGER.error(e);
                    }
                }
            }
            Err(e) => {
                LOGGER.error(e);
            }
        }
    }

    Ok(())
}
