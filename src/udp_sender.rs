use crate::models::{ClientConfig, StockQuote};
use crossbeam_channel::Receiver;
use log::{debug, error, info, trace};
use std::net::UdpSocket;
use std::thread;

pub struct UdpSender {
    client_id: String,
    config: ClientConfig,
    quote_receivers: Vec<Receiver<StockQuote>>,
}

impl UdpSender {
    pub fn new(
        client_id: String,
        config: ClientConfig,
        quote_receivers: Vec<Receiver<StockQuote>>,
    ) -> Self {
        debug!("Creating UDP sender for client: {}", client_id);
        UdpSender {
            client_id,
            config,
            quote_receivers,
        }
    }

    pub fn start(self) {
        info!(
            "Starting UDP sender for client {} to {}",
            self.client_id, self.config.udp_addr
        );
        info!(
            "Subscribed to {} tickers: {:?}",
            self.quote_receivers.len(),
            self.config.tickers
        );

        let target_addr = match self.parse_udp_addr(&self.config.udp_addr) {
            Ok(addr) => {
                debug!("Parsed UDP address for {}: {}", self.client_id, addr);
                addr
            }
            Err(e) => {
                error!("Failed to parse UDP address for {}: {}", self.client_id, e);
                return;
            }
        };

        let udp_socket = match UdpSocket::bind("127.0.0.1:0") {
            Ok(socket) => {
                debug!("UDP socket created for client {}", self.client_id);
                socket
            }
            Err(e) => {
                error!("Failed to create UDP socket for {}: {}", self.client_id, e);
                return;
            }
        };

        thread::spawn(move || {
            let mut sent_count = 0;
            let mut errors_count = 0;

            info!("UDP sender thread started for client {}", self.client_id);

            // Запускаем отдельный поток для каждого ресивера
            let mut handles = Vec::new();

            for (i, receiver) in self.quote_receivers.into_iter().enumerate() {
                let udp_socket = udp_socket.try_clone().expect("Failed to clone UDP socket");
                let target_addr = target_addr.clone();
                let client_id = self.client_id.clone();

                let handle = thread::spawn(move || {
                    let mut thread_sent_count = 0;
                    let mut thread_errors_count = 0;

                    debug!("Started receiver thread {} for client {}", i, client_id);

                    for quote in receiver.iter() {
                        let json_data = quote.to_json();

                        if let Err(e) = udp_socket.send_to(json_data.as_bytes(), &target_addr) {
                            error!(
                                "Failed to send quote in thread {} for client {}: {}",
                                i, client_id, e
                            );
                            thread_errors_count += 1;

                            if thread_errors_count > 5 {
                                break;
                            }
                        } else {
                            thread_sent_count += 1;

                            if thread_sent_count % 50 == 0 {
                                trace!(
                                    "Thread {} for client {} sent {} quotes",
                                    i, client_id, thread_sent_count
                                );
                            }
                        }
                    }

                    (thread_sent_count, thread_errors_count)
                });

                handles.push(handle);
            }

            // Ждем завершения всех потоков
            for (i, handle) in handles.into_iter().enumerate() {
                match handle.join() {
                    Ok((thread_sent, thread_errors)) => {
                        sent_count += thread_sent;
                        errors_count += thread_errors;
                        debug!(
                            "Receiver thread {} finished: sent={}, errors={}",
                            i, thread_sent, thread_errors
                        );
                    }
                    Err(e) => {
                        error!("Receiver thread {} panicked: {:?}", i, e);
                    }
                }
            }

            info!(
                "UDP sender for client {} stopped. Sent {} quotes, errors: {}",
                self.client_id, sent_count, errors_count
            );
        });
    }

    fn parse_udp_addr(&self, addr_str: &str) -> Result<String, String> {
        if let Some(addr) = addr_str.strip_prefix("udp://") {
            Ok(addr.to_string())
        } else {
            Err(format!("Invalid UDP address format: {}", addr_str))
        }
    }
}
