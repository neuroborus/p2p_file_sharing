use std::io;

use p2p_cli::*;

fn main() -> io::Result<()> {
    let mut stream = connect();
    let matches = create_command().get_matches();

    process_actions(&mut stream, &matches)?;
    process_daemon_response(&mut stream)?;

    Ok(())
}
