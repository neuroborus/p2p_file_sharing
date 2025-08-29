use std::io;
use p2p_cli::*;

fn main() -> io::Result<()> {
    let mut stream = connect();
    let matches = create_command().get_matches();

    let mut health: bool = true;

    process_actions(&mut stream, &mut health, &matches)?;
    process_daemon_response(&mut stream, &health)?;

    Ok(())
}