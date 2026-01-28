use clap::Parser;
use std::net::{TcpStream, UdpSocket};
use std::io::{Write, Read};
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use ctrlc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// TCP server address
    #[arg(short = 's', long, default_value = "127.0.0.1:8080")]
    server_addr: String,

    /// Local UDP port for receiving quotes
    #[arg(short = 'p', long, default_value_t = 55555)]
    udp_port: u16,

    /// Server UDP port for ping messages
    #[arg(long, default_value_t = 34254)]
    server_ping_port: u16,

    /// Ticker file path
    #[arg(short = 'f', long, default_value = "tickers.txt")]
    ticker_file: String,

    /// Ping interval in seconds
    #[arg(long, default_value_t = 2)]
    ping_interval: u64,

    /// Output format (simple, json, detailed)
    #[arg(long, default_value = "simple")]
    output_format: String,
}

fn load_tickers(filename: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(filename)?;
    let tickers: Vec<String> = content
        .lines()
        .map(|line| line.trim().to_uppercase())
        .filter(|line| !line.is_empty())
        .collect();

    if tickers.is_empty() {
        return Err("No tickers found in file".into());
    }

    Ok(tickers)
}

fn format_ticker_list(tickers: &[String]) -> String {
    tickers.join(",")
}

fn format_quote(data: &str, format: &str) -> String {
    match format {
        "json" => {
            // Парсим и форматируем как JSON
            let parts: Vec<&str> = data.split('|').collect();
            if parts.len() == 4 {
                format!(
                    "{{\"ticker\":\"{}\",\"price\":{},\"volume\":{},\"timestamp\":{}}}",
                    parts[0], parts[1], parts[2], parts[3]
                )
            } else {
                data.to_string()
            }
        }
        "detailed" => {
            let parts: Vec<&str> = data.split('|').collect();
            if parts.len() == 4 {
                format!(
                    "[{}] {}: ${} (vol: {})",
                    parts[3], parts[0], parts[1], parts[2]
                )
            } else {
                data.to_string()
            }
        }
        _ => data.to_string(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("Starting Quote Client...");
    println!("Server: {}", args.server_addr);
    println!("UDP receive port: {}", args.udp_port);
    println!("Ticker file: {}", args.ticker_file);

    // Загрузка тикеров из файла
    let tickers = load_tickers(&args.ticker_file)?;
    println!("Loaded {} tickers: {}", tickers.len(), tickers.join(", "));

    // Подключаемся к TCP серверу
    println!("Connecting to server...");
    let mut tcp_stream = TcpStream::connect(&args.server_addr)?;
    println!("Connected successfully!");

    // Читаем приветственное сообщение
    let mut buf = [0; 1024];
    let n = tcp_stream.read(&mut buf)?;
    println!("{}", String::from_utf8_lossy(&buf[..n]));

    // Отправляем команду STREAM
    let stream_command = format!(
        "STREAM udp://127.0.0.1:{} {}\n",
        args.udp_port,
        format_ticker_list(&tickers)
    );

    tcp_stream.write_all(stream_command.as_bytes())?;
    println!("Sent: {}", stream_command.trim());

    // Читаем ответ
    let n = tcp_stream.read(&mut buf)?;
    let response = String::from_utf8_lossy(&buf[..n]).trim().to_string();
    println!("Server: {}", response);

    if !response.contains("STREAMING_STARTED") {
        eprintln!("Failed to start streaming");
        return Ok(());
    }

    // Создаем UDP сокет для получения данных
    let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", args.udp_port))?;
    udp_socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    println!("UDP socket bound to port {}", args.udp_port);

    // Флаг для контроля работы потоков
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Обработка Ctrl+C
    ctrlc::set_handler(move || {
        println!("\nReceived Ctrl+C, shutting down...");
        running_clone.store(false, Ordering::SeqCst);
    })?;

    // Запускаем поток для отправки PING сообщений
    let ping_thread = {
        let running = running.clone();
        let server_addr = format!("{}:{}",
            args.server_addr.split(':').next().unwrap_or("127.0.0.1"),
            args.server_ping_port
        );
        let ping_interval = args.ping_interval;

        thread::spawn(move || {
            let ping_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
            let mut ping_count = 0;

            while running.load(Ordering::SeqCst) {
                if ping_socket.send_to(b"PING", &server_addr).is_ok() {
                    ping_count += 1;
                    if ping_count % 10 == 0 { // Логируем каждые 10 пингов
                        println!("[PING] Sent {} ping messages", ping_count);
                    }
                }
                thread::sleep(Duration::from_secs(ping_interval));
            }
            println!("[PING] Ping thread stopped");
        })
    };

    // Главный цикл получения котировок
    println!("\nReceiving quotes (Press Ctrl+C to stop)...\n");
    let mut quote_count = 0;
    let start_time = std::time::Instant::now();

    while running.load(Ordering::SeqCst) {
        let mut buf = [0; 1024];
        match udp_socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                let message = String::from_utf8_lossy(&buf[..size]);
                let formatted = format_quote(&message, &args.output_format);
                println!("{}", formatted);
                quote_count += 1;

                // Периодически показываем статистику
                if quote_count % 10 == 0 {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let quotes_per_sec = quote_count as f64 / elapsed;
                    println!("[STATS] Received {} quotes ({:.1} quotes/sec)",
                             quote_count, quotes_per_sec);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Таймаут - проверяем нужно ли продолжать
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                eprintln!("UDP receive error: {}", e);
                break;
            }
        }
    }

    // Останавливаем поток ping
    running.store(false, Ordering::SeqCst);
    let _ = ping_thread.join();

    // Отправляем команду STOP
    println!("\nSending STOP command...");
    tcp_stream.write_all(b"STOP\n")?;

    let n = tcp_stream.read(&mut buf)?;
    println!("Server: {}", String::from_utf8_lossy(&buf[..n]));

    // Выводим итоговую статистику
    let elapsed = start_time.elapsed().as_secs_f64();
    let quotes_per_sec = quote_count as f64 / elapsed;

    println!("\n=== Session Summary ===");
    println!("Total quotes received: {}", quote_count);
    println!("Session duration: {:.1} seconds", elapsed);
    println!("Average rate: {:.1} quotes/sec", quotes_per_sec);
    println!("Client stopped successfully!");

    Ok(())
}