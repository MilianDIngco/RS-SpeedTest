use std::{
    collections::HashSet, io::{Read, Write}, net::{TcpListener, TcpStream, UdpSocket}, sync::{atomic::{AtomicU64, Ordering}, Arc}, thread, time::{Duration, Instant}
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
    socket
        .set_read_timeout(Some(Duration::from_millis(800)))
        .expect("Client timed out");

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
                    Err(e) => println!("Error when handling UDP Upload: {:?}", e),
                }

                handle_udp_upload(&socket, addr)
            }
            "UDST" => {
                socket.send_to(b"UDAH", addr).unwrap();
                // wait until client sends back triple handshake
                match socket.recv_from(&mut buf) {
                    Ok((len, pack_addr)) if (pack_addr == addr && &buf[..len] == b"UDAA") => {
                        println!("Starting UDP Download test");
                    }
                    Ok(_) => {}
                    Err(e) => println!("Error when handling UDP Download: {:?}", e),
                }
                handle_udp_download(&socket, addr)
            }
            _ => println!("unknown udp cmd"),
        }
    }
}

// server -> client (UDP download)
fn handle_udp_download(socket: &UdpSocket, addr: std::net::SocketAddr) {
    // Shared send rate (packets/sec)
    let test_duration: f32 = 5.0;
    let rate = Arc::new(AtomicU64::new(100_00)); 

    let r_recv = rate.clone();
    let socket_recv = socket.try_clone().unwrap();
    socket_recv.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
    let client_addr = addr.clone();
    let listener_thread = thread::spawn(move || {
        let mut buf = [0u8; 8]; 
        loop {
            match socket_recv.recv_from(&mut buf) {
                Ok((len, src)) if src == client_addr && len == 16 => {
                    let drop_ratio = f64::from_be_bytes(buf[..8].try_into().unwrap());
                    if drop_ratio < 0.0 {
                        // Client signals test complete
                        println!("UDP Download test complete (client finished)");
                        break;
                    }

                    // Simple dynamic rate adjustment
                    let mut curr_rate = r_recv.load(Ordering::SeqCst) as f64;
                    if drop_ratio > 0.05 * 10_000.0 {
                        curr_rate *= 0.8; // decrease rate by 20%
                    } else {
                        curr_rate *= 1.1; // increase rate by 10%
                    }

                    r_recv.store(curr_rate.round() as u64, Ordering::SeqCst);

                    let n_recv = u64::from_be_bytes(buf[8..16].try_into().unwrap());
                    println!(
                        "Client received {} packets, drop ratio {:.2}%",
                        n_recv,
                        drop_ratio / 100.0
                    );
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("Error receiving from client: {:?}", e),
            }
        }
    });

    let r_send = rate.clone();
    let socket_send = socket.try_clone().unwrap();
    let client_addr_send = addr.clone();
    let sender_thread = thread::spawn(move || {
        let start = Instant::now();
        let mut seq: i64 = 0;

        while start.elapsed() < Duration::from_secs_f32(test_duration) {
            let last_sent = Instant::now();
            let curr_rate = r_send.load(Ordering::SeqCst);
            let interval = if curr_rate > 0 {
                Duration::from_micros(1_000_000 / curr_rate)
            } else {
                Duration::from_micros(1_000_000) // default 1 packet/sec
            };

            // Build packet: {i64 seq_num, [0u8; 1016]}
            let mut packet = [0u8; 1024];
            packet[..8].copy_from_slice(&seq.to_be_bytes());
            seq += 1;

            socket_send.send_to(&packet, client_addr_send).ok();

            if last_sent.elapsed() < interval {
                std::thread::sleep(interval - last_sent.elapsed());
            }
        }

        let mut packet = [0u8; 1024];
        packet[..8].copy_from_slice(&(-1i64).to_be_bytes());
        socket_send.send_to(&packet, client_addr_send).ok();
    });

    sender_thread.join().unwrap();
    listener_thread.join().unwrap();

    println!("UDP Download test completed for client {}", addr);
}

// client -> server (UDP upload)
fn handle_udp_upload(socket: &UdpSocket, addr: std::net::SocketAddr) {
    let mut buf = [0u8; 1024];
    let mut last_updated = Instant::now();
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
                match i64::from_be_bytes(buf[..8].try_into().unwrap()) {
                    -1 => {
                        println!("UDP Upload test complete!");
                        // create buf
                        let end: f64 = -1.0;
                        let payload: u64 = 1;
                        let end_bytes = end.to_be_bytes();
                        let payload_bytes = payload.to_be_bytes();
                        let mut packet = Vec::with_capacity(end_bytes.len() + payload_bytes.len());
                        packet.extend_from_slice(&end_bytes);
                        packet.extend_from_slice(&payload_bytes);
                        socket.send_to(&packet, addr).unwrap();
                        break;
                    }
                    n => {
                        if first_seq == -1 {
                            first_seq = n;
                            max_seq = n;
                            total += 1;
                        }

                        if !seen.contains(&n) {
                            seen.insert(n);
                            n_recv += 1;
                            total += 1;
                        }
                        if n > max_seq {
                            max_seq = n;
                        }

                        // Update ratio then send to the client
                        let expected = max_seq - first_seq + 1;
                        let n_lost = expected - n_recv;
                        let drop_ratio: u64 =
                            ((n_lost as f64 / expected as f64) * 100_00.0).round() as u64;
                        // Print out the current speed
                        let speed =
                            (n_recv as f64 * len as f64) / last_updated.elapsed().as_secs_f64();

                        let ratio_bytes = drop_ratio.to_be_bytes();
                        let speed_bytes = speed.to_be_bytes();

                        // create buf
                        if last_updated.elapsed() >= Duration::from_millis(500) {
                            let mut packet =
                                Vec::with_capacity(ratio_bytes.len() + speed_bytes.len());
                            packet.extend_from_slice(&ratio_bytes);
                            packet.extend_from_slice(&speed_bytes);
                            socket.send_to(&packet, addr).unwrap();
                            last_updated = Instant::now();
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }

    println!("done udp ul from {}: {} bytes", addr, total);
}
