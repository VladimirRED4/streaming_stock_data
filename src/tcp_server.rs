use crate::models::{Command, CommandError, ClientConfig};
use crate::client_manager::ClientManager;
use crate::udp_sender::UdpSender;
use crate::generator::QuoteGenerator;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::io::{Read, Write};

pub struct TcpServer {
    generator: QuoteGenerator,
    client_manager: Arc<ClientManager>,
    ping_handler_port: u16,
}

impl TcpServer {
    pub fn new(
        generator: QuoteGenerator,
        ping_timeout_secs: u64,
        ping_handler_port: u16,
    ) -> Self {
        let client_manager = Arc::new(ClientManager::new(ping_timeout_secs));

        TcpServer {
            generator,
            client_manager,
            ping_handler_port,
        }
    }

    pub fn run(&self, port: u16) -> std::io::Result<()> {
        // Запускаем обработчик ping сообщений
        self.client_manager.start_ping_handler(self.ping_handler_port);

        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
        println!("[INFO] TCP server listening on port {}", port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let server = self.clone();
                    thread::spawn(move || {
                        if let Err(e) = server.handle_client(stream) {
                            eprintln!("[ERROR] Client handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[ERROR] Failed to accept connection: {}", e);
                }
            }
        }

        Ok(())
    }

    fn handle_client(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let peer_addr = match stream.peer_addr() {
            Ok(addr) => addr,
            Err(_) => {
                eprintln!("[ERROR] Failed to get peer address");
                return Ok(());
            }
        };

        let client_id = format!("{}", peer_addr);
        println!("[INFO] New connection from {}", peer_addr);

        // Приветственное сообщение
        let welcome_msg = "Welcome to Quote Server!\n\
                          Available commands:\n\
                          STREAM udp://<host>:<port> <ticker1>,<ticker2>,... - Start streaming quotes\n\
                          PING - Send ping to server\n\
                          STOP - Stop current streaming\n\
                          HELP - Show this help\n";

        stream.write_all(welcome_msg.as_bytes())?;

        loop {
            let mut buf = [0; 1024];
            let n = match stream.read(&mut buf) {
                Ok(0) => {
                    println!("[INFO] Client {} disconnected", peer_addr);
                    self.client_manager.remove_client(&client_id);
                    return Ok(());
                }
                Ok(n) => n,
                Err(e) => {
                    eprintln!("[ERROR] Read error from {}: {}", peer_addr, e);
                    self.client_manager.remove_client(&client_id);
                    return Err(e);
                }
            };

            let input = String::from_utf8_lossy(&buf[..n]).trim().to_string();
            println!("[DEBUG] Received from {}: {}", peer_addr, input);

            match Command::parse(&input) {
                Ok(command) => {
                    match self.handle_command(command, &client_id, &mut stream) {
                        Ok(should_continue) => {
                            if !should_continue {
                                break;
                            }
                        }
                        Err(e) => {
                            // Используем fmt::Display для CommandError
                            let error_msg = format!("ERROR: {}\n", e);
                            if let Err(e) = stream.write_all(error_msg.as_bytes()) {
                                eprintln!("[ERROR] Failed to write error to client: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("ERROR: {}\n", e);
                    stream.write_all(error_msg.as_bytes())?;
                    stream.write_all(b"Type HELP for available commands\n")?;
                }
            }
        }

        self.client_manager.remove_client(&client_id);
        Ok(())
    }

    fn handle_command(
    &self,
    command: Command,
    client_id: &str,
    stream: &mut TcpStream,
) -> Result<bool, CommandError> {
    match command {
        Command::Stream { udp_addr, tickers } => {
            // Проверяем, что все тикеры существуют
            for ticker in &tickers {
                if !self.generator.has_ticker(ticker) {
                    return Err(CommandError::InvalidTicker(ticker.clone()));
                }
            }

            // Создаем конфигурацию клиента (теперь без client_id)
            let config = ClientConfig::new(udp_addr, tickers);

            // Добавляем клиента в менеджер
            self.client_manager.add_client(client_id.to_string(), config.clone());

            // Создаем ресивер котировок для этого клиента
            let quote_receiver = self.generator.create_client_receiver();

            // Создаем UDP отправитель для этого клиента
            let udp_sender = UdpSender::new(
                client_id.to_string(),
                config,
                quote_receiver,
            );

            // Запускаем UDP отправитель
            udp_sender.start();

            stream.write_all(b"STREAMING_STARTED\n")?;

            Ok(true)
        }
        Command::Ping => {
            if self.client_manager.update_ping(client_id) {
                stream.write_all(b"PONG\n")?;
            } else {
                stream.write_all(b"ERROR: Not streaming\n")?;
            }
            Ok(true)
        }
        Command::Stop => {
            self.client_manager.remove_client(client_id);
            stream.write_all(b"STREAMING_STOPPED\n")?;
            Ok(false)
        }
        Command::Help => {
            let help_msg = "Available commands:\n\
                          STREAM udp://<host>:<port> <ticker1>,<ticker2>,... - Start streaming quotes to UDP address\n\
                          PING - Send ping to keep connection alive\n\
                          STOP - Stop current streaming\n\
                          HELP - Show this help\n\n\
                          Example:\n\
                          STREAM udp://127.0.0.1:34254 AAPL,TSLA,GOOGL\n";
            stream.write_all(help_msg.as_bytes())?;
            Ok(true)
        }
    }
}

}

impl Clone for TcpServer {
    fn clone(&self) -> Self {
        TcpServer {
            generator: self.generator.clone(),
            client_manager: self.client_manager.clone(),
            ping_handler_port: self.ping_handler_port,
        }
    }
}