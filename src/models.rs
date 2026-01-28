use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockQuote {
    pub ticker: String,
    pub price: f64,
    pub volume: u32,
    pub timestamp: u64,
}

impl StockQuote {
    pub fn new(ticker: String, price: f64, volume: u32) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        StockQuote {
            ticker,
            price,
            volume,
            timestamp,
        }
    }

    // Текстовый формат для UDP
    pub fn to_string(&self) -> String {
        format!("{}|{:.2}|{}|{}",
            self.ticker,
            self.price,
            self.volume,
            self.timestamp
        )
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_string().into_bytes()
    }

}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub udp_addr: String,
    pub tickers: Vec<String>,
    pub last_ping: u64,
}

impl ClientConfig {
    pub fn new(udp_addr: String, tickers: Vec<String>) -> Self {
        ClientConfig {
            udp_addr,
            tickers,
            last_ping: Self::current_timestamp(),
        }
    }

    pub fn update_ping(&mut self) {
        self.last_ping = Self::current_timestamp();
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    pub fn is_stale(&self, timeout_secs: u64) -> bool {
        let now = Self::current_timestamp();
        now - self.last_ping > timeout_secs
    }
}

#[derive(Debug)]
pub enum Command {
    Stream {
        udp_addr: String,
        tickers: Vec<String>,
    },
    Ping,
    Stop,
    Help,
}

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Invalid command format: {0}")]
    InvalidFormat(String),
    #[error("Invalid UDP address: {0}")]
    InvalidAddress(String),
    #[error("No tickers specified")]
    NoTickers,
    #[error("Invalid ticker: {0}")]
    InvalidTicker(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl Command {
    pub fn parse(input: &str) -> Result<Self, CommandError> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return Err(CommandError::InvalidFormat("Empty command".to_string()));
        }

        match parts[0].to_uppercase().as_str() {
            "STREAM" => {
                if parts.len() < 2 {
                    return Err(CommandError::InvalidFormat(
                        "STREAM requires UDP address and tickers".to_string()
                    ));
                }

                // Парсим UDP адрес
                let udp_addr = parts[1].to_string();
                if !udp_addr.starts_with("udp://") {
                    return Err(CommandError::InvalidAddress(
                        "Address must start with udp://".to_string()
                    ));
                }

                // Парсим тикеры
                if parts.len() < 3 {
                    return Err(CommandError::NoTickers);
                }

                let tickers: Vec<String> = parts[2]
                    .split(',')
                    .map(|t| t.trim().to_uppercase())
                    .filter(|t| !t.is_empty())
                    .collect();

                if tickers.is_empty() {
                    return Err(CommandError::NoTickers);
                }

                Ok(Command::Stream { udp_addr, tickers })
            }
            "PING" => Ok(Command::Ping),
            "STOP" => Ok(Command::Stop),
            "HELP" => Ok(Command::Help),
            _ => Err(CommandError::InvalidFormat(
                format!("Unknown command: {}", parts[0])
            )),
        }
    }
}