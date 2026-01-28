use quote_common::{QuoteGenerator, TcpServer};
use clap::Parser;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// TCP server port
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// UDP port for ping handler
    #[arg(long, default_value_t = 34254)]
    ping_port: u16,

    /// Volatility for price generation (0.0 to 1.0)
    #[arg(short = 'v', long, default_value_t = 0.01)]
    volatility: f64,

    /// Generation interval in milliseconds
    #[arg(short = 'i', long, default_value_t = 500)]
    interval_ms: u64,

    /// Ping timeout in seconds
    #[arg(short = 't', long, default_value_t = 5)]
    ping_timeout: u64,

    /// Ticker file path
    #[arg(short = 'f', long, default_value = "tickers.txt")]
    ticker_file: String,

    /// Log level (quiet, normal, verbose)
    #[arg(long, default_value = "normal")]
    log_level: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Настройка логирования
    match args.log_level.as_str() {
        "quiet" => {},
        "verbose" => {
            println!("[VERBOSE] Starting quote server...");
            println!("[VERBOSE] Loading tickers from {}...", args.ticker_file);
        }
        _ => {
            println!("Starting Quote Server...");
            println!("TCP Server port: {}", args.port);
            println!("Ping handler port: {}", args.ping_port);
        }
    }

    // Загрузка тикеров из файла
    let generator = QuoteGenerator::from_file(&args.ticker_file, args.volatility)?;

    // Запуск генератора котировок
    generator.clone().start(args.interval_ms);

    if args.log_level != "quiet" {
        println!("Volatility: {}", args.volatility);
        println!("Generation interval: {}ms", args.interval_ms);
        println!("Ping timeout: {}s", args.ping_timeout);
        println!("Server is running!");
    }

    // Создание TCP сервера
    let tcp_server = TcpServer::new(
        generator,
        args.ping_timeout,
        args.ping_port,
    );

    // Запуск TCP сервера
    match tcp_server.run(args.port) {
        Ok(_) => {
            // Бесконечный цикл для главного потока
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        Err(e) => {
            eprintln!("Failed to start TCP server: {}", e);
            std::process::exit(1);
        }
    }
}