use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::thread;
use std::time::{Duration, SystemTime};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting test client...");

    // Подключаемся к TCP серверу
    let mut tcp_stream = TcpStream::connect("127.0.0.1:8080")?;
    println!("Connected to TCP server");

    // Создаем UDP сокет для получения данных
    let udp_socket = UdpSocket::bind("127.0.0.1:34255")?;
    udp_socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    println!("UDP socket bound to 127.0.0.1:34255");

    // Создаем UDP сокет для отправки PING
    let ping_socket = UdpSocket::bind("127.0.0.1:0")?;

    // Читаем приветственное сообщение
    let mut buf = [0; 1024];
    let n = tcp_stream.read(&mut buf)?;
    println!("{}", String::from_utf8_lossy(&buf[..n]));

    // Отправляем команду STREAM
    let stream_command = "STREAM udp://127.0.0.1:34255 AAPL,TSLA\n";
    // let stream_command = "STREAM udp://127.0.0.1:34255 AAPL\n";
    tcp_stream.write_all(stream_command.as_bytes())?;
    println!("Sent: {}", stream_command.trim());

    // Читаем ответ
    let n = tcp_stream.read(&mut buf)?;
    println!("Server: {}", String::from_utf8_lossy(&buf[..n]));

    // Запускаем поток для отправки PING сообщений
    let server_ping_addr = "127.0.0.1:34254"; // Сервер слушает на порту ping_port
    let ping_thread = thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            let _ = ping_socket.send_to(b"PING", server_ping_addr);
        }
    });

    // Получаем котировки в течение 10 секунд
    let start_time = SystemTime::now();
    let mut quote_count = 0;

    println!("\nReceiving quotes for 10 seconds...");

    while SystemTime::now().duration_since(start_time).unwrap() < Duration::from_secs(10) {
        let mut buf = [0; 1024];
        match udp_socket.recv_from(&mut buf) {
            Ok((size, addr)) => {
                let message = String::from_utf8_lossy(&buf[..size]);
                println!("Quote {}: [{}] {}", quote_count + 1, addr, message);
                quote_count += 1;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Таймаут - ничего не пришло
                continue;
            }
            Err(e) => {
                println!("UDP receive error: {}", e);
                break;
            }
        }
    }

    // Останавливаем поток PING
    drop(ping_thread);

    // Отправляем команду STOP
    tcp_stream.write_all(b"STOP\n")?;
    println!("Sent: STOP");

    let n = tcp_stream.read(&mut buf)?;
    println!("Server: {}", String::from_utf8_lossy(&buf[..n]));

    println!("\nReceived {} quotes in total", quote_count);
    println!("Test completed successfully!");

    Ok(())
}
