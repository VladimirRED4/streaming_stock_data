use crate::models::{ClientConfig, StockQuote};
use crossbeam_channel::Receiver;
use std::net::UdpSocket;
use std::thread;

pub struct UdpSender {
    client_id: String,
    config: ClientConfig,
    quote_receiver: Receiver<StockQuote>,
}

impl UdpSender {
    pub fn new(
        client_id: String,
        config: ClientConfig,
        quote_receiver: Receiver<StockQuote>,
    ) -> Self {
        UdpSender {
            client_id,
            config,
            quote_receiver,
        }
    }

    pub fn start(self) {
        println!("[INFO] Starting UDP sender for client {}", self.client_id);

        // Парсим UDP адрес
        let target_addr = match self.parse_udp_addr(&self.config.udp_addr) {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("[ERROR] Failed to parse UDP address for {}: {}", self.client_id, e);
                return;
            }
        };

        // Создаем UDP сокет для отправки
        let udp_socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(socket) => socket,
            Err(e) => {
                eprintln!("[ERROR] Failed to create UDP socket for {}: {}", self.client_id, e);
                return;
            }
        };

        // Конвертируем tickers в HashSet для быстрой проверки
        let target_tickers: std::collections::HashSet<String> =
            self.config.tickers.iter().cloned().collect();

        thread::spawn(move || {
            for quote in self.quote_receiver.iter() {
                // Фильтруем по запрошенным тикерам
                if target_tickers.contains(&quote.ticker) {
                    if let Err(e) = udp_socket.send_to(&quote.to_bytes(), &target_addr) {
                        eprintln!("[ERROR] Failed to send quote to {}: {}", self.client_id, e);
                        break;
                    }
                }
            }

            println!("[INFO] UDP sender for client {} stopped", self.client_id);
        });
    }

    fn parse_udp_addr(&self, addr_str: &str) -> Result<String, String> {
        // Убираем "udp://" префикс
        if let Some(addr) = addr_str.strip_prefix("udp://") {
            Ok(addr.to_string())
        } else {
            Err(format!("Invalid UDP address format: {}", addr_str))
        }
    }
}