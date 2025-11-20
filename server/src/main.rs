use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    thread,
    time::{Duration, Instant},
};

fn main() {
    // run TCP + UDP servers at the same time
    let tcp_thread = thread::spawn(|| {
        run_tcp_server();
    });

    let udp_thread = thread::spawn(|| {
        run_udp_server();
    });

    // keep the program alive
    tcp_thread.join().unwrap();
    udp_thread.join().unwrap();
}

//
// ================== TCP SERVER ==================
//

fn run_tcp_server() {
    // TCP listener on port 8080
    let listener = TcpListener::bind("0.0.0.0:8080")
        .expect("tcp bind failed");

    println!("tcp server on 8080");

    // accept connections forever
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                // handle each client on its own thread
                thread::spawn(|| {
                    handle_tcp_connection(stream);
                });
            }
            Err(e) => eprintln!("tcp error: {}", e),
        }
    }
}

fn handle_tcp_connection(mut stream: TcpStream) {
    // read initial command from client
    let mut buf = [0u8; 1024];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };

    let cmd = String::from_utf8_lossy(&buf[..n]).trim().to_string();
    println!("tcp cmd: {}", cmd);

    // run correct mode
    match cmd.as_str() {
        "DL" => handle_tcp_download(stream),
        "UL" => handle_tcp_upload(stream),
        _ => println!("unknown tcp cmd"),
    }
}

// server -> client (download test)
fn handle_tcp_download(mut stream: TcpStream) {
    let payload = [0u8; 8192]; // 8 KB chunk
    let start = Instant::now();

    // send data for 5 seconds
    while start.elapsed() < Duration::from_secs(5) {
        if stream.write_all(&payload).is_err() {
            break; // client probably disconnected
        }
    }

    println!("done tcp dl");
}

// client -> server (upload test)
fn handle_tcp_upload(mut stream: TcpStream) {
    let mut buf = [0u8; 8192];
    let start = Instant::now();
    let mut total = 0u64;

    // read data for 5 seconds
    while start.elapsed() < Duration::from_secs(5) {
        match stream.read(&mut buf) {
            Ok(0) => break, // client closed
            Ok(n) => total += n as u64,
            Err(_) => break,
        }
    }

    println!("done tcp ul: {} bytes", total);
}

//
// ================== UDP SERVER ==================
//

fn run_udp_server() {
    // UDP socket on port 7070
    let socket = UdpSocket::bind("0.0.0.0:7070")
        .expect("udp bind failed");

    println!("udp server on 7070");

    let mut buf = [0u8; 1500];

    loop {
        // wait for any UDP packet
        let (n, addr) = match socket.recv_from(&mut buf) {
            Ok(x) => x,
            Err(_) => continue,
        };

        let cmd = String::from_utf8_lossy(&buf[..n]).trim().to_string();
        println!("udp cmd from {}: {}", addr, cmd);

        // handle mode
        match cmd.as_str() {
            "UDP_DL" => handle_udp_download(&socket, addr),
            "UDP_UL" => handle_udp_upload(&socket, addr),
            _ => println!("unknown udp cmd"),
        }
    }
}

// server -> client (UDP download)
fn handle_udp_download(socket: &UdpSocket, addr: std::net::SocketAddr) {
    let payload = [0u8; 1400]; // safe UDP size
    let start = Instant::now();

    // send packets for 5 seconds
    while start.elapsed() < Duration::from_secs(5) {
        if socket.send_to(&payload, addr).is_err() {
            break;
        }
    }

    println!("done udp dl to {}", addr);
}

// client -> server (UDP upload)
fn handle_udp_upload(socket: &UdpSocket, addr: std::net::SocketAddr) {
    let mut buf = [0u8; 1500];
    let start = Instant::now();
    let mut total = 0u64;

    // receive packets for 5 seconds
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok((n, src)) = socket.recv_from(&mut buf) {
            // only count packets from this client
            if src == addr {
                total += n as u64;
            }
        }
    }

    println!("done udp ul from {}: {} bytes", addr, total);
}
