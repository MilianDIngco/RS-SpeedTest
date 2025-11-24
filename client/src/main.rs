use std::{
    collections::HashSet, io::{Read, Write}, net::{SocketAddr, TcpStream, UdpSocket}, sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    }, thread, time::{Duration, Instant}
};

use iced::{
    alignment,
    widget::{button, column, container, image, row, text, text_input, toggler, Stack},
    Background, Border, Color, Element, Length, Size, Task, Theme,
};

fn main() -> iced::Result {
    iced::application(
        "Speed Testing",
        NetworkConfigApp::update,
        NetworkConfigApp::view,
    )
    .window_size(Size::new(800.0, 600.0))
    .theme(|_| Theme::Dark)
    .run_with(|| NetworkConfigApp::new())
}

#[derive(Debug, Clone)]
pub enum Message {
    IpAddressChanged(String),
    PortChanged(String),
    ProtocolToggled(bool),
    Connect,
    SpeedUpdated(f32, f32),
}

#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    UDP,
    TCP,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::UDP => write!(f, "UDP"),
            Protocol::TCP => write!(f, "TCP"),
        }
    }
}

#[derive(Debug)]
pub struct NetworkConfigApp {
    ip_address: String,
    port: String,
    is_tcp: bool,
    background_image: iced::widget::image::Handle,
    upload_speed: f32,
    download_speed: f32,
    test_duration: f32,
    is_running: Arc<AtomicBool>,
}

impl NetworkConfigApp {
    fn new() -> (Self, Task<Message>) {
        let background_image = image::Handle::from_path("TCNJ_Speed.jpg");

        (
            Self {
                ip_address: String::from("127.0.0.1"),
                port: String::from("8080"),
                is_tcp: true,
                background_image,
                upload_speed: 0_f32,
                download_speed: 0_f32,
                test_duration: 5_f32,
                is_running: Arc::new(AtomicBool::new(false)),
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::IpAddressChanged(value) => {
                self.ip_address = value;
            }
            Message::PortChanged(value) => {
                self.port = value;
            }
            Message::ProtocolToggled(is_tcp) => {
                self.is_tcp = is_tcp;
            }
            Message::Connect => {
                // Check if already running
                if self.is_running.load(Ordering::SeqCst) {
                    println!("Test is already running");
                    return Task::none();
                }

                // Mark as running
                self.is_running.store(true, Ordering::SeqCst);
                let is_running_clone = Arc::clone(&self.is_running);

                let mut protocol = Protocol::TCP;
                if self.is_tcp {
                    let ip = format!("{}:{}", self.ip_address, self.port);
                    let test_duration = self.test_duration;

                    thread::spawn(move || {
                        // TCP UPLOAD TEST
                        println!("Starting TCP Upload test");
                        {
                            // TCP Upload Block (avoids name conflicts)
                            let upload_stream = TcpStream::connect(ip.clone()).unwrap();

                            // Split stream to send packets and listen for speed from server
                            let mut send_stream = upload_stream.try_clone().unwrap();
                            let mut recv_stream = upload_stream;

                            // send msg to server indicating we want to upload packets
                            send_stream.write_all(b"TUST").unwrap(); // TCp Upload STart

                            // listen for acknowledging "TUAH" message
                            let mut buf = [0u8; 4]; // Size of expected message
                            recv_stream.read_exact(&mut buf).unwrap();
                            if &buf != b"TUAH" {
                                // Tcp Upload Acknowledged H
                                is_running_clone.store(false, Ordering::SeqCst);
                                panic!("Invalid handshake message from server");
                            }

                            // thread sends packets to server
                            let tcp_upload_send = thread::spawn({
                                let duration = test_duration;
                                move || {
                                    let buffer = vec![0u8; 8192]; // 8kb packet
                                    let start = Instant::now();

                                    while start.elapsed() <= Duration::from_secs_f32(duration) {
                                        // send packet to server
                                        send_stream.write_all(&buffer).unwrap();
                                    }

                                    send_stream.shutdown(std::net::Shutdown::Write).unwrap();
                                }
                            });

                            // thread listens to server's speed updates
                            // listens until it receives an "End" message from the server.
                            let tcp_upload_recv = thread::spawn(move || {
                                let mut buf = [0u8; 8];
                                loop {
                                    recv_stream.read_exact(&mut buf).unwrap();

                                    let speed = f64::from_be_bytes(buf);

                                    if speed == -1.0 {
                                        println!("Test finished");
                                        // get final total speed
                                        recv_stream.read_exact(&mut buf).unwrap();
                                        let total_speed = f64::from_be_bytes(buf);
                                        println!("Final TCP Upload speed: {:.2}", total_speed);
                                        break;
                                    }

                                    println!("Current speed: {}", speed);
                                }
                            });

                            // Wait until upload test is finished
                            tcp_upload_send.join().unwrap();
                            tcp_upload_recv.join().unwrap();
                        } // TCP Upload block

                        // TCP Download test
                        println!("TCP Upload test is finished\nStarting TCP Download test");

                        {
                            // TCP Download block
                            let mut download_stream = TcpStream::connect(ip).unwrap();

                            // Start handshake, indicate starting TCP download test
                            // Indicate we wanna start a TCP download test
                            download_stream.write_all(b"TDST").unwrap(); // Tcp Download STart

                            // Wait until the server says allg
                            let mut buf = [0u8; 4];
                            download_stream.read_exact(&mut buf).unwrap();
                            if &buf != b"TDAH" {
                                is_running_clone.store(false, Ordering::SeqCst);
                                panic!("Invalid handshake message sent by server");
                            }

                            // Start TCP receiving thread
                            let tcp_download_recv = thread::spawn({
                                let duration = test_duration as f64;
                                move || {
                                    let mut buf = [0u8; 8192];
                                    let mut total_bytes = 0;
                                    let mut average_bytes = 0;
                                    let mut last_updated = Instant::now();

                                    let start = Instant::now();
                                    let update_interval_secs: f64 = 0.5; // seconds
                                    loop {
                                        // Match number of bytes
                                        match download_stream.read(&mut buf) {
                                            Ok(0) => break, // connection is closed by server
                                            Ok(n) => {
                                                total_bytes += n;
                                                average_bytes += n;
                                            }
                                            Err(e) => {
                                                panic!("Read error happened: {:?}", e);
                                            }
                                        }

                                        // Update speed every 0.5 seconds
                                        if last_updated.elapsed()
                                            >= Duration::from_secs_f64(update_interval_secs)
                                        {
                                            let total_speed =
                                                total_bytes as f64 / start.elapsed().as_secs_f64();
                                            let speed = average_bytes as f64 / update_interval_secs; // bytes per sec;
                                            println!("Average speed: {:.2} B/s", total_speed);
                                            println!("Current speed: {:.2} B/s", speed);
                                            average_bytes = 0;
                                            last_updated = Instant::now();
                                        }

                                        // Quit loop once 5 seconds passed
                                        if start.elapsed() >= Duration::from_secs_f64(duration) {
                                            let total_speed = total_bytes as f64 / duration;
                                            println!("Overall speed: {:.2} B/s", total_speed);
                                            println!("TCP Download test completed");
                                            break;
                                        }
                                    }
                                }
                            });

                            // Wait until download test is finished
                            tcp_download_recv.join().unwrap();
                        } // TCP Download block

                        is_running_clone.store(false, Ordering::SeqCst);
                    }); // End of TCP speed test controller thread
                } else {
                    protocol = Protocol::UDP;

                    // TODO: update the is running thingy!!!

                    // Controller thread
                    let ip: SocketAddr = format!("{}:{}", self.ip_address, self.port)
                        .parse()
                        .unwrap();
                    let test_duration = self.test_duration;
                    thread::spawn(move || {
                        {
                            // UDP Upload test
                            const MAX_RETRY: u32 = 10;
                            let mut retry: u32 = 0;
                            
                            // Create UdpSocket
                            let upload_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
                            upload_socket
                                .set_read_timeout(Some(Duration::from_millis(500)))
                                .expect("UDP Read timed out");

                            let server_addr: SocketAddr = ip.clone();
                            // Send UUST
                            while retry < MAX_RETRY {
                                upload_socket.send_to(b"UUST", server_addr.clone()).unwrap();

                                let mut buf = [0u8; 4];
                                match upload_socket.recv_from(&mut buf) {
                                    Ok((len, addr)) => {
                                        // check if from the correct server and
                                        // matches the expected handshake message
                                        if addr == server_addr && &buf[..len] == b"UUAH" {
                                            // Send final ack "UUAA"
                                            upload_socket.send_to(b"UUAA", server_addr.clone()).unwrap();
                                            println!("UDP Upload test handshake complete");
                                            break;
                                        }
                                    } // v indicates timeout, assume packet was dropped
                                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                        retry += 1;
                                        println!("No response, retrying..");
                                        continue;
                                    }
                                    Err(e) => panic!("Error during TCP Upload handshake: {:?}", e),
                                }
                            }

                            // Create shared variable "rate" that will be adjusted by the listener
                            // thread. If the server indicates there aren't any drops, increase
                            // rate by 10%, otherwise, decrease by 30%
                            let rate = Arc::new(AtomicU64::new(1000)); // 1000 packets per second

                            // Spawn UDP sender thread
                            let r_send = rate.clone();
                            let upload_test_duration = test_duration;
                            let send_server_addr = server_addr.clone();
                            let upload_socket_send = upload_socket.try_clone().unwrap();
                            let udp_upload_send = thread::spawn(move || {
                                // uhhhh ngl im basing the speed at which i send packets based off
                                // of how i remember video games do delta time updates. so... it
                                // could be not only incorrect for this application, but completely
                                // wrong for delta time in a video game too... OH WELL

                                let start = Instant::now();
                                let mut seq: i64 = 0;

                                while start.elapsed()
                                    < Duration::from_secs_f32(upload_test_duration)
                                {
                                    let last_sent = Instant::now();
                                    // update interval
                                    let rate = r_send.load(Ordering::SeqCst);
                                    let interval = if rate > 0 {
                                        Duration::from_micros(1_000_000 / rate)
                                    } else {
                                        Duration::from_micros(1_000_000) // default 1 packet/sec if rate is 0
                                    };

                                    // send packet
                                    let mut packet = [0u8; 1024];
                                    packet[..8].copy_from_slice(&seq.to_be_bytes());
                                    seq += 1;

                                    upload_socket_send.send_to(&packet, send_server_addr).ok();

                                    // check how much time elapsed, if < interval, sleep
                                    if last_sent.elapsed() < interval {
                                        let sleep_amt = interval - last_sent.elapsed();
                                        std::thread::sleep(sleep_amt);
                                    }
                                }
                            });

                            // Spawn UDP listener thread (listens to server's packet loss rate)
                            let r_recv = rate.clone();
                            let recv_server_addr = server_addr.clone();
                            let upload_socket_recv = upload_socket;
                            let udp_upload_recv = thread::spawn(move || {
                                let mut buf = [0u8; 64];

                                loop {
                                    let mut curr_rate: f64 = r_recv.load(Ordering::SeqCst) as f64;
                                    match upload_socket_recv.recv_from(&mut buf) {
                                        // checks that it comes from the right address and that the
                                        // length of the packet is correct for an f64
                                        // UDP Upload on the server side should send a packet
                                        // with this form 
                                        // {
                                        // f64: ratio 
                                        // u64: n_recv
                                        // }
                                        // then we can print out the speed in this thread
                                        Ok((len, addr)) if (addr == recv_server_addr && len == 16) => {
                                            match f64::from_be_bytes(buf[..8].try_into().unwrap())
                                            {
                                                -1.0 => {
                                                    // Server sends -1 to indicate test
                                                    // complete
                                                    println!("UDP Upload test complete!");
                                                    break;
                                                }
                                                // very very super duper simple dynamic rate stuff
                                                ref n if *n >= 0.05f64 => {
                                                    // Dropping packets
                                                    curr_rate *= 0.8;
                                                }
                                                _ => {
                                                    // Healthy rate? increase
                                                    curr_rate *= 1.1;
                                                }
                                            }

                                            // Update rate
                                            r_recv.store(curr_rate.round() as u64, Ordering::SeqCst);
                                            // Print speed 
                                            let n_recv = u64::from_be_bytes(buf[8..16].try_into().unwrap());
                                            let speed = n_recv as f64 / 0.5;
                                            println!("Current speed: {:.2}", speed);
                                        }
                                        Ok((_, _)) => {
                                        }
                                        Err(ref e)
                                            if e.kind() == std::io::ErrorKind::WouldBlock =>
                                        {
                                            println!("No response, retrying..");
                                            continue;
                                        }
                                        Err(e) => {
                                            panic!("Error during UDP Upload: {:?}", e)
                                        }
                                    }
                                }
                            });

                            udp_upload_send.join().unwrap();
                            udp_upload_recv.join().unwrap();
                        } // UDP Upload test block

                        println!("UDP Upload test is finished\nStarting UDP Download test");

                        { // UDP Download test block
                            const MAX_RETRY: u32 = 10;
                            let mut retry: u32 = 0;
                            
                            // Create UdpSocket
                            let download_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
                            download_socket
                                .set_read_timeout(Some(Duration::from_millis(500)))
                                .expect("UDP Read timed out");

                            let server_addr: SocketAddr = ip.clone();
                            // Send UUST
                            while retry < MAX_RETRY {
                                download_socket.send_to(b"UDST", server_addr.clone()).unwrap();

                                let mut buf = [0u8; 4];
                                match download_socket.recv_from(&mut buf) {
                                    Ok((len, addr)) => {
                                        // check if from the correct server and
                                        // matches the expected handshake message
                                        if addr == server_addr && &buf[..len] == b"UDAH" {
                                            // send final triple ack UDAA
                                            upload_socket.send_to(b"UDAA", server_addr.clone()).unwrap();
                                            println!("UDP Download test handshake complete");
                                            break;
                                        }
                                    } // v indicates timeout, assume packet was dropped
                                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                        retry += 1;
                                        println!("No response, retrying..");
                                        continue;
                                    }
                                    Err(e) => panic!("Error during TCP Download handshake: {:?}", e),
                                }
                            }

                            let rate = Arc::new(AtomicU64::new(100_00)); // %100.00 loss rate 

                            // Spawn UDP listener thread (listens to server's packet loss rate)
                            let r_recv = rate.clone();
                            let recv_server_addr = server_addr.clone();
                            let download_socket_recv = download_socket.try_clone().unwrap();
                            let udp_download_recv = thread::spawn(move || {
                                let mut buf = [0u8; 64];
                                let mut seen = HashSet::new();
                                let mut first_seq = -1;
                                let mut n_recv: i64 = 0;
                                let mut max_seq: i64 = 0;
                                let start = Instant::now();

                                loop {
                                    match download_socket_recv.recv_from(&mut buf) {
                                        // checks that it comes from the right address and that the
                                        // length of the packet is correct for an f64
                                        Ok((len, addr)) if (addr == recv_server_addr) => {
                                            // Parse packet, grab sequence number, grab
                                            // packet contents
                                            let seq_num = i64::from_be_bytes(buf[..8].try_into().unwrap());
                                            if seq_num == -1 {
                                                println!("UDP Download test DONEEEE");
                                                break;
                                            }

                                            // Update hashset, n_recv, max_seq
                                            if first_seq == -1 {
                                                first_seq = seq_num;
                                                max_seq = seq_num;
                                            }
                                            if !seen.contains(&seq_num) {
                                                seen.insert(seq_num);
                                                n_recv += 1;
                                            }
                                            if seq_num > max_seq {
                                                max_seq = seq_num;
                                            }
                                            // Update r_recv ratio so the sender can send it
                                            let expected = max_seq - first_seq + 1;
                                            let n_lost = expected - n_recv;
                                            let drop_ratio:u64 = ((n_lost as f64 / n_recv as f64) * 100_00.0).round() as u64;
                                            r_recv.store(drop_ratio, Ordering::SeqCst);
                                            // Print out the current speed
                                            let speed = (n_recv as f64 * len as f64) / start.elapsed().as_secs_f64();
                                            println!("Current speed: {:.2}", speed);
                                        }
                                        Ok((_, _)) => {
                                        }
                                        Err(ref e)
                                            if e.kind() == std::io::ErrorKind::WouldBlock =>
                                        {
                                            println!("No response, retrying..");
                                            continue;
                                        }
                                        Err(e) => {
                                            panic!("Error during TCP Upload handshake: {:?}", e)
                                        }
                                    }
                                }
                            });

                            // Spawn UDP sender thread
                            let r_send = rate.clone();
                            let download_test_duration = test_duration;
                            let send_server_addr = server_addr.clone();
                            let download_socket_send = download_socket;
                            let udp_download_send = thread::spawn(move || {

                                let start = Instant::now();
                                let interval = Duration::from_millis(200);

                                while start.elapsed()
                                    < Duration::from_secs_f32(download_test_duration)
                                {
                                    // Every 0.2 seconds, send the current rate 
                                    let last_sent = Instant::now();
                                    // update interval
                                    let rate = r_send.load(Ordering::SeqCst);

                                    // send packet
                                    download_socket_send.send_to(&rate.to_be_bytes(), send_server_addr).ok();

                                    // check how much time elapsed, if < interval, sleep
                                    if last_sent.elapsed() < interval {
                                        let sleep_amt = interval - last_sent.elapsed();
                                        std::thread::sleep(sleep_amt);
                                    }
                                }
                            });

                            udp_download_send.join().unwrap();
                            udp_download_recv.join().unwrap();
                            
                        } // UDP Download test block
                    });
                }
                println!(
                    "Connecting to {}:{} using {}",
                    self.ip_address, self.port, protocol
                );
            }
            Message::SpeedUpdated(upload_value, download_value) => {
                self.upload_speed = upload_value;
                self.download_speed = download_value;
            }
        }
        Task::none()
    }

    //fn tcp_upload() {}

    //fn tcp_download() {}

    //fn udp_upload() {}

    //fn udp_download() {}

    fn view(&self) -> Element<Message> {
        let protocol = if self.is_tcp {
            Protocol::TCP
        } else {
            Protocol::UDP
        };

        // Background image
        let background = image(self.background_image.clone())
            .width(Length::Fill)
            .height(Length::Fill);

        // Main content container with semi-transparent background
        let content = container(
            column![
                // Title
                text("Speed Testing").size(32).color(Color::WHITE),
                // Increased spacing to push inputs lower
                text("").size(100),
                // IP Address input
                column![
                    text("IP Address:").size(16).color(Color::WHITE),
                    text_input("Enter IP address", &self.ip_address)
                        .on_input(Message::IpAddressChanged)
                        .padding(10)
                        .size(16)
                        .width(Length::Fixed(300.0))
                ]
                .spacing(5),
                // Port input
                column![
                    text("Port:").size(16).color(Color::WHITE),
                    text_input("Enter port", &self.port)
                        .on_input(Message::PortChanged)
                        .padding(10)
                        .size(16)
                        .width(Length::Fixed(300.0))
                ]
                .spacing(5),
                // Protocol toggle
                row![
                    text("Protocol:").size(16).color(Color::WHITE),
                    text("UDP").size(16).color(Color::WHITE),
                    toggler(self.is_tcp)
                        .on_toggle(Message::ProtocolToggled)
                        .size(25),
                    text("TCP").size(16).color(Color::WHITE),
                ]
                .spacing(10)
                .align_y(alignment::Vertical::Center),
                // Current selection display
                text(format!(
                    "Selected: {}:{} ({})",
                    self.ip_address, self.port, protocol
                ))
                .size(14)
                .color(Color::from_rgb(0.8, 0.8, 1.0)),
                // Connect button
                button(text("Connect").size(18))
                    .on_press(Message::Connect)
                    .padding(15)
                    .width(Length::Fixed(150.0))
            ]
            .spacing(20)
            .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fixed(400.0))
        .height(Length::Shrink)
        .padding(30)
        .style(|_theme| -> container::Style {
            container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.7))),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }
        });

        // Stack background and content
        container(
            Stack::new().push(background).push(
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            ),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
