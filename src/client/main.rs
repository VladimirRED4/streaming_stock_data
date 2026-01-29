use clap::Parser;
use std::net::{TcpStream, UdpSocket};
use std::io::{Write, Read, stdin};
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use log::{info, error, warn, debug, trace};

// Константы для конфигурации
const DEFAULT_UDP_PORT: u16 = 55555;
const DEFAULT_SERVER_PING_PORT: u16 = 34254;
const DEFAULT_PING_INTERVAL: u64 = 2;
const DEFAULT_DURATION: u64 = 0;
const LOCALHOST: &str = "127.0.0.1";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// TCP server address
    #[arg(short = 's', long, default_value = "LOCALHOST:8080")]
    server_addr: String,

    /// Local UDP port for receiving quotes
    #[arg(short = 'p', long, default_value_t = DEFAULT_UDP_PORT)]
    udp_port: u16,

    /// Server UDP port for ping messages
    #[arg(long, default_value_t = DEFAULT_SERVER_PING_PORT)]
    server_ping_port: u16,

    /// Ticker file path (alternative to --tickers)
    #[arg(short = 'f', long)]
    ticker_file: Option<String>,

    /// Comma-separated list of tickers (alternative to --ticker-file)
    #[arg(short = 't', long, value_delimiter = ',')]
    tickers: Option<Vec<String>>,

    /// Ping interval in seconds
    #[arg(long, default_value_t = DEFAULT_PING_INTERVAL)]
    ping_interval: u64,

    /// Output format (simple, json, detailed, line)
    #[arg(long, default_value = "line")]
    output_format: String,

    /// Run duration in seconds (0 for unlimited)
    #[arg(short = 'd', long, default_value_t = DEFAULT_DURATION)]
    duration: u64,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Enable colored output
    #[arg(long, default_value_t = true)]
    color: bool,

    /// Show timestamp in output
    #[arg(long, default_value_t = false)]
    show_timestamp: bool,
}

fn setup_logging(level: &str, color: bool) {
    use env_logger::Env;

    // Создаем специальное окружение с нужным уровнем логирования
    let env = Env::default()
        .filter_or("RUST_LOG", format!("quote_client={}", level));

    let mut builder = env_logger::Builder::from_env(env);

    // Настраиваем формат
    builder
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .format_level(true)
        .format_target(false)
        .format_module_path(false);

    if color {
        builder.format_indent(Some(4));
    }

    // Инициализируем логгер
    if let Err(e) = builder.try_init() {
        eprintln!("Failed to initialize logger: {}", e);
        eprintln!("Logging disabled. Using fallback to stdout.");
    }
}

fn load_tickers(args: &Args) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut tickers = Vec::new();

    // Приоритет 1: тикеры из командной строки
    if let Some(cmd_tickers) = &args.tickers {
        for ticker in cmd_tickers {
            let ticker_upper = ticker.trim().to_uppercase();
            if !ticker_upper.is_empty() {
                tickers.push(ticker_upper);
            }
        }

        if !tickers.is_empty() {
            info!("Loaded {} tickers from command line: {}",
                  tickers.len(), tickers.join(", "));
            return Ok(tickers);
        }
    }

    // Приоритет 2: тикеры из файла
    if let Some(filename) = &args.ticker_file {
        info!("Loading tickers from file: {}", filename);
        let content = std::fs::read_to_string(filename)?;
        for line in content.lines() {
            let ticker = line.trim().to_uppercase();
            if !ticker.is_empty() {
                tickers.push(ticker);
            }
        }

        if tickers.is_empty() {
            return Err(format!("ERR No tickers found in file {}", filename).into());
        }

        info!("Loaded {} tickers from file: {}",
              tickers.len(), tickers.join(", "));
        return Ok(tickers);
    }

    // Приоритет 3: файл по умолчанию
    info!("Loading tickers from default file: tickers.txt");
    let content = match std::fs::read_to_string("tickers.txt") {
        Ok(content) => content,
        Err(_) => {
            return Err("ERR No tickers specified. Use --tickers or --ticker-file or create tickers.txt".into());
        }
    };

    for line in content.lines() {
        let ticker = line.trim().to_uppercase();
        if !ticker.is_empty() {
            tickers.push(ticker);
        }
    }

    if tickers.is_empty() {
        return Err("ERR No tickers found in tickers.txt".into());
    }

    info!("Loaded {} tickers from default file: {}",
          tickers.len(), tickers.join(", "));
    Ok(tickers)
}

fn parse_json_quote(json_str: &str) -> Result<(String, f64, u32, u64), Box<dyn std::error::Error>> {
    #[derive(serde::Deserialize)]
    struct Quote {
        ticker: String,
        price: f64,
        volume: u32,
        timestamp: u64,
    }

    let quote: Quote = serde_json::from_str(json_str)?;
    Ok((quote.ticker, quote.price, quote.volume, quote.timestamp))
}

fn format_quote(data: &str, format: &str, show_timestamp: bool) -> String {
    match format {
        "json" => {
            // Уже в JSON формате, просто возвращаем как есть
            data.to_string()
        }
        "simple" => {
            // Пытаемся парсить JSON и конвертировать в простой формат
            match parse_json_quote(data) {
                Ok((ticker, price, volume, timestamp)) => {
                    if show_timestamp {
                        format!("{}|{:.2}|{}|{}", ticker, price, volume, timestamp)
                    } else {
                        format!("{}|{:.2}|{}", ticker, price, volume)
                    }
                }
                Err(_) => {
                    // Если не JSON, возвращаем как есть
                    data.to_string()
                }
            }
        }
        "detailed" => {
            match parse_json_quote(data) {
                Ok((ticker, price, volume, timestamp)) => {
                    // Простой формат без chrono
                    let seconds = timestamp / 1000;
                    let millis = timestamp % 1000;
                    format!("[{}.{:03}] {}: ${:.2} (volume: {})",
                           seconds, millis, ticker, price, volume)
                }
                Err(_) => {
                    format!("[Parse Error] {}", data)
                }
            }
        }
        "line" => {
            match parse_json_quote(data) {
                Ok((ticker, price, volume, timestamp)) => {
                    if show_timestamp {
                        let seconds = timestamp / 1000;
                        let millis = timestamp % 1000;
                        format!("[{}.{:03}] {} ${:.2} ({})",
                               seconds, millis, ticker, price, volume)
                    } else {
                        format!("{} ${:.2} ({})", ticker, price, volume)
                    }
                }
                Err(_) => {
                    format!("[Parse Error] {}", data)
                }
            }
        }
        _ => data.to_string(),
    }
}

fn check_user_input(running: &AtomicBool) {
    let mut input = String::new();
    if stdin().read_line(&mut input).is_ok() {
        let input = input.trim().to_lowercase();
        if input == "quit" || input == "exit" || input == "q" {
            info!("User requested shutdown...");
            running.store(false, Ordering::SeqCst);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Проверка: хотя бы один источник тикеров должен быть указан
    if args.ticker_file.is_none() && args.tickers.is_none() && !std::path::Path::new("tickers.txt").exists() {
        eprintln!("ERROR: No tickers specified!");
        eprintln!("Use one of:");
        eprintln!("  --tickers AAPL,TSLA,MSFT");
        eprintln!("  --ticker-file my_tickers.txt");
        eprintln!("  Or create tickers.txt file");
        std::process::exit(1);
    }

    // Инициализация логирования
    setup_logging(&args.log_level, args.color);

    // Принудительно выводим критически важные сообщения
    println!("=== Quote Client Starting ===");
    println!("Server: {}", args.server_addr);
    println!("UDP Port: {}", args.udp_port);
    println!("Output Format: {}", args.output_format);
    if args.show_timestamp {
        println!("Timestamp: enabled");
    }
    println!("=============================");

    info!("Starting Quote Client...");
    info!("Configuration:");
    info!("  Server address: {}", args.server_addr);
    info!("  UDP receive port: {}", args.udp_port);
    info!("  Server ping port: {}", args.server_ping_port);
    info!("  Ping interval: {}s", args.ping_interval);
    info!("  Output format: {}", args.output_format);
    if args.duration > 0 {
        info!("  Duration: {} seconds", args.duration);
    }
    info!("  Log level: {}", args.log_level);
    info!("  Colored output: {}", args.color);
    info!("Type 'quit' and press Enter to stop");

    // Загрузка тикеров
    let tickers = load_tickers(&args)?;
    println!("Loaded {} tickers: {}", tickers.len(), tickers.join(", "));
    info!("Loaded {} tickers: {}", tickers.len(), tickers.join(", "));

    // Подключаемся к TCP серверу
    println!("Connecting to server {}...", args.server_addr);
    info!("Connecting to server {}...", args.server_addr);
    let mut tcp_stream = TcpStream::connect(&args.server_addr)?;
    println!("Connected successfully to TCP server");
    info!("Connected successfully to TCP server");

    // Читаем приветственное сообщение
    let mut buf = [0; 1024];
    let n = tcp_stream.read(&mut buf)?;
    let greeting = String::from_utf8_lossy(&buf[..n]);
    println!("{}", greeting);
    debug!("Server greeting: {}", greeting);

    // Отправляем команду STREAM
    let stream_command = format!(
        "STREAM udp://{}:{} {}\n",
        LOCALHOST, // Используем константу
        args.udp_port,
        tickers.join(",")
    );

    tcp_stream.write_all(stream_command.as_bytes())?;
    println!("Sent command: {}", stream_command.trim());
    info!("Sent command: {}", stream_command.trim());

    // Читаем ответ
    let n = tcp_stream.read(&mut buf)?;
    let response = String::from_utf8_lossy(&buf[..n]).trim().to_string();
    println!("Server: {}", response);
    info!("Server response: {}", response);

    if !response.contains("STREAMING_STARTED") {
        eprintln!("Failed to start streaming. Server response: {}", response);
        error!("Failed to start streaming. Server response: {}", response);
        return Ok(());
    }

    // Создаем UDP сокет для получения данных
    let udp_socket = UdpSocket::bind(format!("{}:{}", LOCALHOST, args.udp_port))?;
    udp_socket.set_read_timeout(Some(Duration::from_millis(1000)))?;
    println!("UDP socket bound to {}:{}", LOCALHOST, args.udp_port);
    info!("UDP socket bound to {}:{}", LOCALHOST, args.udp_port);

    // Флаг для контроля работы потоков
    let running = Arc::new(AtomicBool::new(true));

    // Запускаем поток для отправки PING сообщений
    let ping_thread = {
        let running = running.clone();
        let server_ping_addr = format!("{}:{}", LOCALHOST, args.server_ping_port);
        let ping_interval = args.ping_interval;

        thread::spawn(move || {
            // Простая реализация ping - пробуем создать сокет, если не получается - выходим
            let ping_socket = match UdpSocket::bind(format!("{}:0", LOCALHOST)) {
                Ok(socket) => {
                    debug!("Ping socket created successfully");
                    socket
                }
                Err(e) => {
                    error!("Failed to create ping socket: {}", e);
                    warn!("PING functionality will be disabled");
                    return;
                }
            };

            let mut ping_count = 0;
            debug!("Starting ping thread, interval: {}s", ping_interval);

            while running.load(Ordering::SeqCst) {
                match ping_socket.send_to(b"PING", &server_ping_addr) {
                    Ok(_) => {
                        ping_count += 1;
                        if ping_count == 1 {
                            debug!("First PING sent successfully");
                        }
                        if ping_count % 10 == 0 {
                            trace!("Sent {} ping messages", ping_count);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to send PING to {}: {}", server_ping_addr, e);
                    }
                }
                thread::sleep(Duration::from_secs(ping_interval));
            }
            info!("Ping thread stopped after {} pings", ping_count);
        })
    };

    // Запускаем поток для проверки пользовательского ввода
    let input_thread = {
        let running = running.clone();
        thread::spawn(move || {
            println!("Type 'quit' and press Enter to stop");
            info!("Input thread started. Type 'quit' to stop.");
            while running.load(Ordering::SeqCst) {
                check_user_input(&running);
                thread::sleep(Duration::from_millis(100));
            }
            info!("Input thread stopped");
        })
    };

    // Главный цикл получения котировок
    println!("\nReceiving quotes (each ticker on new line)...");
    info!("Starting to receive quotes with format: {}", args.output_format);
    let mut quote_count = 0;
    let mut non_quote_messages = 0;
    let start_time = std::time::Instant::now();

    // Для статистики по тикерам
    let mut ticker_stats: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut last_stats_time = start_time;
    const STATS_INTERVAL: Duration = Duration::from_secs(5);

    // Если указана длительность, устанавливаем таймер
    let end_time = if args.duration > 0 {
        Some(start_time + Duration::from_secs(args.duration))
    } else {
        None
    };

    'main_loop: while running.load(Ordering::SeqCst) {
        // Проверяем таймер, если установлен
        if let Some(end) = end_time {
            if std::time::Instant::now() >= end {
                println!("\nDuration limit reached, stopping...");
                info!("Duration limit reached, stopping...");
                running.store(false, Ordering::SeqCst);
                break 'main_loop;
            }
        }

        let mut buf = [0; 4096];
        match udp_socket.recv_from(&mut buf) {
            Ok((size, addr)) => {
                let message = String::from_utf8_lossy(&buf[..size]);

                // ФИЛЬТРАЦИЯ: принимаем только JSON котировки, игнорируем служебные сообщения
                if message.trim() == "PONG" {
                    trace!("Received PONG from {} (ignored)", addr);
                    non_quote_messages += 1;
                    continue;
                }

                if message.trim().is_empty() {
                    trace!("Received empty message from {} (ignored)", addr);
                    non_quote_messages += 1;
                    continue;
                }

                // Пытаемся распарсить как JSON
                match serde_json::from_str::<serde_json::Value>(&message) {
                    Ok(json) => {
                        if json.get("ticker").is_some() &&
                           json.get("price").is_some() &&
                           json.get("volume").is_some() &&
                           json.get("timestamp").is_some() {

                            if let Some(ticker_value) = json.get("ticker") {
                                if let Some(ticker_str) = ticker_value.as_str() {
                                    let ticker_upper = ticker_str.to_uppercase();
                                    if tickers.contains(&ticker_upper) {
                                        // Это валидная котировка для запрошенного тикера
                                        let formatted = format_quote(&message, &args.output_format, args.show_timestamp);
                                        println!("{}", formatted);
                                        quote_count += 1;

                                        // Собираем статистику по тикерам
                                        *ticker_stats.entry(ticker_upper.clone()).or_insert(0) += 1;

                                        // Периодически показываем статистику
                                        if quote_count == 1 {
                                            info!("First quote received: {}", ticker_str);
                                        }
                                        if quote_count % 10 == 0 {
                                            debug!("Received {} quotes from {}", quote_count, addr);
                                        }

                                        // Показываем статистику каждые STATS_INTERVAL
                                        let now = std::time::Instant::now();
                                        if now.duration_since(last_stats_time) >= STATS_INTERVAL {
                                            println!("\n--- Statistics (last {} seconds) ---", STATS_INTERVAL.as_secs());
                                            let mut stats_vec: Vec<(&String, &usize)> = ticker_stats.iter().collect();
                                            stats_vec.sort_by(|a, b| b.1.cmp(a.1)); // Сортировка по убыванию

                                            for (ticker, count) in stats_vec {
                                                println!("  {}: {} quotes", ticker, count);
                                            }
                                            println!("  Total: {} quotes", quote_count);
                                            println!("--------------------------------");

                                            ticker_stats.clear();
                                            last_stats_time = now;
                                        }
                                    } else {
                                        // Это котировка, но не для нашего тикера
                                        // warn!("Received quote for unsubscribed ticker: {} from {}", ticker_str, addr);
                                        non_quote_messages += 1;
                                    }
                                } else {
                                    warn!("Invalid ticker format in JSON from {}: {}", addr, message);
                                    non_quote_messages += 1;
                                }
                            } else {
                                warn!("JSON missing ticker field from {}: {}", addr, message);
                                non_quote_messages += 1;
                            }
                        } else {
                            debug!("Received non-quote JSON from {}: {}", addr, message);
                            non_quote_messages += 1;
                        }
                    }
                    Err(e) => {
                        debug!("Received non-JSON message from {}: {} (error: {})", addr, message, e);
                        non_quote_messages += 1;
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                       || e.kind() == std::io::ErrorKind::TimedOut => {
                // Таймаут - нормально, продолжаем ждать
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                // Другие ошибки - логируем
                error!("UDP receive error: {}", e);
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    // Останавливаем потоки
    info!("Stopping threads...");
    running.store(false, Ordering::SeqCst);

    // Ждем завершения потоков
    let _ = ping_thread.join();
    let _ = input_thread.join();
    info!("All threads stopped");

    // Отправляем команду STOP
    println!("\nSending STOP command to server...");
    info!("Sending STOP command to server...");
    if tcp_stream.write_all(b"STOP\n").is_err() {
        println!("Failed to send STOP (connection may be closed)");
        warn!("Failed to send STOP (connection may be closed)");
    } else {
        let _ = tcp_stream.read(&mut buf);
        println!("STOP command sent successfully");
        info!("STOP command sent successfully");
    }

    // Выводим итоговую статистику
    let elapsed = start_time.elapsed().as_secs_f64();
    let quotes_per_sec = if elapsed > 0.0 {
        quote_count as f64 / elapsed
    } else {
        0.0
    };

    println!("\n=== Session Summary ===");
    println!("Total quotes received: {}", quote_count);
    println!("Non-quote messages filtered: {}", non_quote_messages);
    println!("Total UDP messages: {}", quote_count + non_quote_messages);
    println!("Session duration: {:.1} seconds", elapsed);
    println!("Average rate: {:.1} quotes/sec", quotes_per_sec);

    if non_quote_messages > 0 {
        let filter_percent = (non_quote_messages as f64 / (quote_count + non_quote_messages) as f64) * 100.0;
        println!("Filter efficiency: {:.1}% messages filtered", filter_percent);
    }

    println!("Client stopped successfully!");

    info!("Client shutdown complete. Quotes: {}, Filtered: {}", quote_count, non_quote_messages);
    Ok(())
}