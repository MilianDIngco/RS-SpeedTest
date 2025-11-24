use std::{
    collections::HashSet, io::{Read, Write}, net::{TcpListener, TcpStream, UdpSocket}, thread, time::{Duration, Instant}
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
    let listener = TcpListener::bind("0.0.0.0:8080").expect("tcp bind failed");

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
        "TDST" => {
            // Send TUAH acknowledgement
            stream.write_all(b"TDAH").unwrap();
            handle_tcp_download(stream)
        }
        "TUST" => {
            stream.write_all(b"TUAH").unwrap();
            handle_tcp_upload(stream)
        }
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
    let mut last_updated = Instant::now();
    let mut total_bytes = 0u64;
    let mut average_bytes = 0u64;

    // read data for 5 seconds
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break, // client closed
            Ok(n) => {
                total_bytes += n as u64;
                average_bytes += n as u64;
            }
            Err(_) => break,
        }

        // Every 0.5 seconds, send the number of bytes received
        if last_updated.elapsed() >= Duration::from_secs_f64(0.5) {
            let speed = average_bytes as f64 / 0.5;
            //reset
            average_bytes = 0;
            last_updated = Instant::now();

            stream.write_all(&speed.to_be_bytes()).unwrap();
            stream.flush().unwrap();
        }
    }

    // When done, send -1 as speed
    let end_sig: f64 = -1.0;
    stream.write_all(&end_sig.to_be_bytes()).unwrap();
    stream.flush().unwrap();

    // Send final total speed
    let total_speed = total_bytes as f64 / start.elapsed().as_secs_f64();
    stream.write_all(&total_speed.to_be_bytes()).unwrap();
    stream.flush().unwrap();

    println!("done tcp ul: {} bytes", total_bytes);
}

//
// ================== UDP SERVER ==================
//

fn run_udp_server() {
    // UDP socket on port 7070
    let socket = UdpSocket::bind("0.0.0.0:7070").expect("udp bind failed");
    socket.set_read_timeout(Some(Duration::from_millis(800))).expect("Client timed out");

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
            "UUST" => {
                socket.send_to(b"UUAH", addr).unwrap();
                // wait until client sends back triple handshake
                match socket.recv_from(&mut buf) {
                    Ok((len, pack_addr)) if (pack_addr == addr && &buf[..len] == b"UUAA") => {
                        println!("Starting UDP Upload test");
                    }
                    Ok(_) => {}
                    Err(e) => println!("Error when handling UDP Upload: {:?}", e)
                }
                
                handle_udp_upload(&socket, addr)
            },
            "UDST" => {
                socket.send_to(b"UDAH", addr).unwrap();
                // wait until client sends back triple handshake
                match socket.recv_from(&mut buf) {
                    Ok((len, pack_addr)) if (pack_addr == addr && &buf[..len] == b"UDAA") => {
                        println!("Starting UDP Download test");
                    }
                    Ok(_) => {}
                    Err(e) => println!("Error when handling UDP Download: {:?}", e)
                }
                handle_udp_download(&socket, addr)
            },
            _ => println!("unknown udp cmd"),
        }
    }
}

// server -> client (UDP download)
fn handle_udp_download(socket: &UdpSocket, addr: std::net::SocketAddr) {
    let payload = [0u8; 1024]; // safe UDP size
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
    let mut buf = [0u8; 1024];
    let start = Instant::now();
    let mut total = 0u64;

    // start one thread for receiving packets
    // loop until we get a seq num with -1
    let mut seen = HashSet::new();
    let mut first_seq = -1;
    let mut n_recv: i64 = 0;
    let mut max_seq: i64 = 0;

    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, pack_addr)) if (pack_addr == addr) => {
                match f64::from_be_bytes(buf[..8].try_into().unwrap()) {
                    -1.0 => {
                        println!("UDP Upload test complete!");
                        break;
                    }
                    n => {
                        if first_seq == -1 {
                            first_seq = n;
                            max_seq = n;
                        }

                        if !seen.contains(&n) {
                            
                        }
                    }
                }
            }
            Err(_, _) => {}
        }
        if let Ok((n, src)) = socket.recv_from(&mut buf) {
            // only count packets from this client
            if src == addr {
                total += n as u64;
            }
        }
    }

    // start another thread to send the packet drop rate

    println!("done udp ul from {}: {} bytes", addr, total);
}
