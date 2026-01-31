use crate::models::ClientConfig;
use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct ClientManager {
    clients: Arc<Mutex<HashMap<String, ClientConfig>>>,
    ping_timeout_secs: u64,
}

impl ClientManager {
    pub fn new(ping_timeout_secs: u64) -> Self {
        info!(
            "Initializing client manager with ping timeout: {}s",
            ping_timeout_secs
        );
        ClientManager {
            clients: Arc::new(Mutex::new(HashMap::new())),
            ping_timeout_secs,
        }
    }

    // Добавление нового клиента
    pub fn add_client(&self, client_id: String, config: ClientConfig) {
        info!(
            "Adding new client: {} -> UDP: {}, Tickers: {}",
            client_id,
            config.udp_addr,
            config.tickers.join(", ")
        );
        let mut clients = self.clients.lock().unwrap();
        let old_count = clients.len();
        clients.insert(client_id, config);
        info!(
            "Client added. Total clients: {} (was: {})",
            clients.len(),
            old_count
        );
    }

    // Удаление клиента
    pub fn remove_client(&self, client_id: &str) -> Option<ClientConfig> {
        let mut clients = self.clients.lock().unwrap();
        if let Some(config) = clients.remove(client_id) {
            info!(
                "Removed client: {}. Active clients: {}",
                client_id,
                clients.len()
            );
            Some(config)
        } else {
            warn!("Attempted to remove non-existent client: {}", client_id);
            None
        }
    }

    // Обновление времени последнего ping
    pub fn update_ping(&self, client_id: &str) -> bool {
        let mut clients = self.clients.lock().unwrap();
        if let Some(config) = clients.get_mut(client_id) {
            config.update_ping();
            debug!("Updated ping for client: {}", client_id);
            true
        } else {
            debug!("Ping update failed: client {} not found", client_id);
            false
        }
    }

    // Запуск обработчика ping сообщений
    pub fn start_ping_handler(&self, udp_port: u16) {
        info!("Starting ping handler on UDP port {}", udp_port);

        let clients = self.clients.clone();
        let ping_timeout = self.ping_timeout_secs;

        thread::spawn(move || {
            let udp_socket = match UdpSocket::bind(format!("127.0.0.1:{}", udp_port)) {
                Ok(socket) => {
                    info!("Ping handler listening on UDP port {}", udp_port);
                    socket
                }
                Err(e) => {
                    error!("Failed to bind UDP socket for ping handler: {}", e);
                    return;
                }
            };

            if let Err(e) = udp_socket.set_read_timeout(Some(Duration::from_millis(500))) {
                error!("Failed to set UDP socket timeout: {}", e);
                return;
            }

            let mut buf = [0; 1024];
            let mut stats_cycles = 0;

            info!("Ping handler thread started");

            loop {
                stats_cycles += 1;

                match udp_socket.recv_from(&mut buf) {
                    Ok((size, addr)) => {
                        let message = String::from_utf8_lossy(&buf[..size]);
                        if message.trim() == "PING" {
                            debug!("Received PING from {}", addr);

                            let client_id = format!("{}", addr);

                            let mut clients_lock = clients.lock().unwrap();
                            if let Some(config) = clients_lock.get_mut(&client_id) {
                                config.update_ping();
                                // Отправляем PONG обратно
                                if let Err(e) = udp_socket.send_to(b"PONG", addr) {
                                    error!("Failed to send PONG to {}: {}", addr, e);
                                } else {
                                    trace!("Sent PONG to {}", addr);
                                }
                            } else {
                                // Если клиент не найден, возможно он только что подключился
                                // с другим ID, ищем по части адреса
                                let addr_ip = addr.ip().to_string();
                                let mut found = false;
                                for (id, config) in clients_lock.iter_mut() {
                                    if id.contains(&addr_ip) {
                                        config.update_ping();
                                        if let Err(e) = udp_socket.send_to(b"PONG", addr) {
                                            error!("Failed to send PONG to {}: {}", addr, e);
                                        }
                                        debug!(
                                            "Matched PING from {} to existing client {}",
                                            addr, id
                                        );
                                        found = true;
                                        break;
                                    }
                                }

                                if !found {
                                    debug!("PING from unknown client: {}", addr);
                                }
                            }
                        } else {
                            debug!("Received non-PING message from {}: {}", addr, message);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // Таймаут - продолжаем проверку
                    }
                    Err(e) => {
                        error!("Error receiving ping: {}", e);
                        thread::sleep(Duration::from_secs(1));
                    }
                }

                // Периодически проверяем устаревших клиентов
                if stats_cycles % 10 == 0 {
                    // Каждую секунду (10 * 100ms)
                    let stale_clients: Vec<String> = {
                        let clients_lock = clients.lock().unwrap();
                        clients_lock
                            .iter()
                            .filter(|(_, config)| config.is_stale(ping_timeout))
                            .map(|(id, _)| id.clone())
                            .collect()
                    };

                    if !stale_clients.is_empty() {
                        warn!(
                            "Found {} stale clients: {:?}",
                            stale_clients.len(),
                            stale_clients
                        );

                        let mut clients_lock = clients.lock().unwrap();
                        for client_id in stale_clients {
                            if let Some(config) = clients_lock.remove(&client_id) {
                                warn!(
                                    "Removed stale client: {} (UDP: {})",
                                    client_id, config.udp_addr
                                );
                            }
                        }
                        info!("Active clients after cleanup: {}", clients_lock.len());
                    } else {
                        debug!("No stale clients found");
                    }

                    // Логируем статистику каждые 10 секунд
                    if stats_cycles % 100 == 0 {
                        let clients_count = clients.lock().unwrap().len();
                        info!("Ping handler status: {} active clients", clients_count);
                    }
                }

                thread::sleep(Duration::from_millis(100));
            }
        });
    }
}
