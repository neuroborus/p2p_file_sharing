pub fn blocks_count(file_size: u64, chunk_size: usize) -> u32 {
    std::cmp::max(
        1u32,
        ((file_size + chunk_size as u64 - 1) / chunk_size as u64) as u32,
    )
}

pub fn create_buffer(chunk_size: usize) -> Vec<u8> {
    vec![0 as u8; chunk_size]
}
