use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
/// If downloadable peer asked file size we answering with size or file not
/// exist
pub enum FileInfo {
    Size(u64),
    NotExist,
}
