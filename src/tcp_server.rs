use crate::models::{Command, CommandError, ClientConfig};
use crate::client_manager::ClientManager;
use crate::udp_sender::UdpSender;
use crate::generator::QuoteGenerator;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::io::{Read, Write};
use log::{info, error, warn, debug, trace};

pub struct TcpServer {
    generator: Arc<QuoteGenerator>,
    client_manager: Arc<ClientManager>,
    ping_handler_port: u16,
}

impl TcpServer {
    pub fn new(
        generator: QuoteGenerator,
        ping_timeout_secs: u64,
        ping_handler_port: u16,
    ) -> Self {
        info!("Initializing TCP server with ping timeout: {}s, ping port: {}",
              ping_timeout_secs, ping_handler_port);

        let client_manager = Arc::new(ClientManager::new(ping_timeout_secs));

        TcpServer {
            generator: Arc::new(generator),
            client_manager,
            ping_handler_port,
        }
    }

    pub fn run(&self, port: u16) -> std::io::Result<()> {
        // Запускаем обработчик ping сообщений
        self.client_manager.start_ping_handler(self.ping_handler_port);

        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
        info!("TCP server listening on port {}", port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let server = self.clone();
                    thread::spawn(move || {
                        if let Err(e) = server.handle_client(stream) {
                            warn!("Client handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }

        Ok(())
    }

    fn handle_client(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let peer_addr = match stream.peer_addr() {
            Ok(addr) => {
                debug!("New connection from {}", addr);
                addr
            }
            Err(e) => {
                error!("Failed to get peer address: {}", e);
                return Ok(());
            }
        };

        let client_id = format!("{}", peer_addr);
        info!("Handling client: {}", client_id);

        // Приветственное сообщение
        let welcome_msg = "Welcome to Quote Server!\n\
                          Available commands:\n\
                          STREAM udp://<host>:<port> <ticker1>,<ticker2>,... - Start streaming quotes\n\
                          PING - Send ping to server\n\
                          STOP - Stop current streaming\n\
                          HELP - Show this help\n";

        if let Err(e) = stream.write_all(welcome_msg.as_bytes()) {
            error!("Failed to send welcome message to {}: {}", client_id, e);
            return Err(e);
        }

        debug!("Sent welcome message to {}", client_id);

        loop {
            let mut buf = [0; 1024];
            let n = match stream.read(&mut buf) {
                Ok(0) => {
                    info!("Client {} disconnected", client_id);
                    self.client_manager.remove_client(&client_id);
                    return Ok(());
                }
                Ok(n) => {
                    trace!("Received {} bytes from {}", n, client_id);
                    n
                }
                Err(e) => {
                    error!("Read error from {}: {}", client_id, e);
                    self.client_manager.remove_client(&client_id);
                    return Err(e);
                }
            };

            let input = String::from_utf8_lossy(&buf[..n]).trim().to_string();
            debug!("Command from {}: {}", client_id, input);

            match Command::parse(&input) {
                Ok(command) => {
                    match self.handle_command(command, &client_id, &mut stream) {
                        Ok(should_continue) => {
                            if !should_continue {
                                info!("Client {} requested stop", client_id);
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Command error for {}: {}", client_id, e);
                            let error_msg = format!("{}\n", e);
                            if let Err(e) = stream.write_all(error_msg.as_bytes()) {
                                error!("Failed to write error to client {}: {}", client_id, e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Parse error for command '{}' from {}: {}", input, client_id, e);
                    let error_msg = format!("{}\n", e);
                    if let Err(e) = stream.write_all(error_msg.as_bytes()) {
                        error!("Failed to write error to client {}: {}", client_id, e);
                        break;
                    }

                    let help_msg = "Type HELP for available commands\n";
                    if let Err(e) = stream.write_all(help_msg.as_bytes()) {
                        error!("Failed to send help to client {}: {}", client_id, e);
                        break;
                    }
                }
            }
        }

        self.client_manager.remove_client(&client_id);
        info!("Client {} handler finished", client_id);
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
                info!("Client {} requested stream to {} for tickers: {}",
                      client_id, udp_addr, tickers.join(", "));

                // Проверяем, что все тикеры существуют
                for ticker in &tickers {
                    if !self.generator.has_ticker(ticker) {
                        warn!("Client {} requested invalid ticker: {}", client_id, ticker);
                        return Err(CommandError::InvalidTicker(ticker.clone()));
                    }
                }

                info!("All tickers validated for client {}", client_id);

                // Создаем конфигурацию клиента
                let config = ClientConfig::new(udp_addr.clone(), tickers.clone());

                // Добавляем клиента в менеджер
                self.client_manager.add_client(client_id.to_string(), config.clone());

                // Подписываем клиента на тикеры и получаем ресиверы
                let receivers = self.generator.subscribe_to_tickers(tickers.clone());

                // Создаем UDP отправитель для этого клиента
                let udp_sender = UdpSender::new(
                    client_id.to_string(),
                    config,
                    receivers,
                );

                // Запускаем UDP отправитель
                udp_sender.start();

                info!("Started UDP streaming for client {} to {}", client_id, udp_addr);

                stream.write_all(b"STREAMING_STARTED\n")?;

                Ok(true)
            }
            Command::Ping => {
                debug!("Client {} sent PING", client_id);
                if self.client_manager.update_ping(client_id) {
                    stream.write_all(b"PONG\n")?;
                    trace!("Sent PONG to {}", client_id);
                } else {
                    warn!("Client {} sent PING but is not streaming", client_id);
                    stream.write_all(b"ERROR: Not streaming\n")?;
                }
                Ok(true)
            }
            Command::Stop => {
                info!("Client {} requested STOP", client_id);
                // Отписываем клиента от тикеров
                let config = self.client_manager.remove_client(client_id);
                if let Some(config) = config {
                    self.generator.unsubscribe_from_tickers(config.tickers);
                }
                stream.write_all(b"STREAMING_STOPPED\n")?;
                Ok(false)
            }
            Command::Help => {
                debug!("Client {} requested HELP", client_id);
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
        debug!("Cloning TCP server instance");
        TcpServer {
            generator: self.generator.clone(),
            client_manager: self.client_manager.clone(),
            ping_handler_port: self.ping_handler_port,
        }
    }
}