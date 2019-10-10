use lib::*;
//socket == "channel"
use clap::*;

fn main() -> io::Result<()> {   //
    assert!(ADDR.is_multicast());  //


    let message = env::args().skip(1).collect::<Vec<_>>().join(" ");    //+
    //let message: Vec<String> = env::args().skip(1).collect();
    //String s;
    let exit_flag = true;
    while(exit_flag){



    }

    let socket = UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), 0))?;  //
    socket
        .send_to(message.as_bytes(), (ADDR, PORT))?;
    println!("client: sent data to {}", ADDR);

    let mut buf = vec![0; 4096];    //zero init 4096els in vec
    let (len, remote_addr) = socket.recv_from(&mut buf)?;
    let response = String::from_utf8_lossy(&buf[..len]);

    println!("client: got data {:?} from {}", response, remote_addr);

    Ok(())
}
