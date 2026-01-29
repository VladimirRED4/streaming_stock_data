use quote_common::{QuoteGenerator, TcpServer};
use clap::Parser;
use std::time::Duration;
use log::{error, info};

// Константы для конфигурации
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_PING_PORT: u16 = 34254;
const DEFAULT_PING_TIMEOUT: u64 = 5;
const DEFAULT_GENERATION_INTERVAL: u64 = 500;
const DEFAULT_VOLATILITY: f64 = 0.01;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// TCP server port
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// UDP port for ping handler
    #[arg(long, default_value_t = DEFAULT_PING_PORT)]
    ping_port: u16,

    /// Volatility for price generation (0.0 to 1.0)
    #[arg(short = 'v', long, default_value_t = DEFAULT_VOLATILITY)]
    volatility: f64,

    /// Generation interval in milliseconds
    #[arg(short = 'i', long, default_value_t = DEFAULT_GENERATION_INTERVAL)]
    interval_ms: u64,

    /// Ping timeout in seconds
    #[arg(short = 't', long, default_value_t = DEFAULT_PING_TIMEOUT)]
    ping_timeout: u64,

    /// Ticker file path
    #[arg(short = 'f', long, default_value = "tickers.txt")]
    ticker_file: String,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Enable colored output
    #[arg(long, default_value_t = true)]
    color: bool,
}

fn setup_logging(level: &str, color: bool) {
    use env_logger::Env;

    // Создаем специальное окружение с нужным уровнем логирования
    let env = Env::default()
        .filter_or("RUST_LOG", format!("quote_system={}", level));

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
        // Если не удалось инициализировать логгер, выводим хотя бы это сообщение
        eprintln!("Logging disabled. Using fallback to stdout.");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Инициализация логирования
    setup_logging(&args.log_level, args.color);

    println!("=== Quote Server Starting ===");
    println!("TCP Port: {}", args.port);
    println!("Ping Port: {}", args.ping_port);
    println!("Log Level: {}", args.log_level);
    println!("=============================");

    info!("Starting Quote Server...");
    info!("Configuration:");
    info!("  TCP Server port: {}", args.port);
    info!("  Ping handler port: {}", args.ping_port);
    info!("  Volatility: {}", args.volatility);
    info!("  Generation interval: {}ms", args.interval_ms);
    info!("  Ping timeout: {}s", args.ping_timeout);
    info!("  Ticker file: {}", args.ticker_file);
    info!("  Log level: {}", args.log_level);
    info!("  Colored output: {}", args.color);

    // Загрузка тикеров из файла
    println!("Loading tickers from {}...", args.ticker_file);
    info!("Loading tickers from {}...", args.ticker_file);
    let generator = QuoteGenerator::from_file(&args.ticker_file, args.volatility)?;
    println!("Loaded tickers successfully");
    info!("Loaded tickers successfully");

    // Запуск генератора котировок
    generator.clone().start(args.interval_ms);
    info!("Quote generator started with interval {}ms", args.interval_ms);

    // Создание TCP сервера
    info!("Initializing TCP server...");
    let tcp_server = TcpServer::new(
        generator,
        args.ping_timeout,
        args.ping_port,
    );

    // Запуск TCP сервера
    println!("Starting TCP server on port {}...", args.port);
    info!("Starting TCP server on port {}...", args.port);
    match tcp_server.run(args.port) {
        Ok(_) => {
            println!("Server is running and ready for connections");
            println!("Press Ctrl+C to stop the server");
            info!("Server is running and ready for connections");
            info!("Press Ctrl+C to stop the server");

            // Бесконечный цикл для главного потока
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        Err(e) => {
            eprintln!("Failed to start TCP server: {}", e);
            eprintln!("Please check if port {} is available", args.port);
            error!("Failed to start TCP server: {}", e);
            error!("Please check if port {} is available", args.port);
            std::process::exit(1);
        }
    }
}