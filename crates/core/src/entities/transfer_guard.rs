use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
/// Adding peer to transferring vector when created, and removing peer from
/// vector while destroying
pub struct TransferGuard {
    pub transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
    pub filename: String,
    pub peer: SocketAddr,
}

impl TransferGuard {
    pub fn new(
        _transferring: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
        _filename: String,
        _peer: SocketAddr,
    ) -> Self {
        let guard = TransferGuard {
            transferring: _transferring,
            filename: _filename,
            peer: _peer,
        };
        // Pushing the peer to vector
        {
            let mut transfer_map = guard.transferring.lock().unwrap();
            match transfer_map.get_mut(&guard.filename) {
                Some(addr_vec) => {
                    addr_vec.push(guard.peer.clone());
                }
                None => {
                    let mut v: Vec<SocketAddr> = Vec::new();
                    v.push(guard.peer);
                    transfer_map.insert(guard.filename.clone(), v);
                }
            }
        }
        guard
    }
}

impl Drop for TransferGuard {
    fn drop(&mut self) {
        // Removing the peer from vector
        {
            let mut transfer_map = self.transferring.lock().unwrap();
            if transfer_map.get(&self.filename).unwrap().len() == 1 {
                transfer_map.remove(&self.filename).unwrap();
            } else {
                let peer_vec: &mut Vec<SocketAddr> = transfer_map.get_mut(&self.filename).unwrap();
                let pos: usize = peer_vec.iter().position(|&peer| peer == self.peer).unwrap();
                peer_vec.remove(pos);
            }
        }
    }
}
