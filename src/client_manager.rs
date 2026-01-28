use crate::models::ClientConfig;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::net::UdpSocket;

pub struct ClientManager {
    clients: Arc<Mutex<HashMap<String, ClientConfig>>>,
    ping_timeout_secs: u64,
}

impl ClientManager {
    pub fn new(ping_timeout_secs: u64) -> Self {
        ClientManager {
            clients: Arc::new(Mutex::new(HashMap::new())),
            ping_timeout_secs,
        }
    }

    // Добавление нового клиента
    pub fn add_client(&self, client_id: String, config: ClientConfig) {
        println!("[INFO] Adding new client: {} -> {}", client_id, config.udp_addr);
        let mut clients = self.clients.lock().unwrap();
        clients.insert(client_id, config);
    }

    // Удаление клиента
    pub fn remove_client(&self, client_id: &str) -> Option<ClientConfig> {
        println!("[INFO] Removing client: {}", client_id);
        let mut clients = self.clients.lock().unwrap();
        clients.remove(client_id)
    }

    // Обновление времени последнего ping
    pub fn update_ping(&self, client_id: &str) -> bool {
        let mut clients = self.clients.lock().unwrap();
        if let Some(config) = clients.get_mut(client_id) {
            config.update_ping();
            true
        } else {
            false
        }
    }

    // Запуск обработчика ping сообщений
    pub fn start_ping_handler(&self, udp_port: u16) {
        let clients = self.clients.clone();
        let ping_timeout = self.ping_timeout_secs;

        thread::spawn(move || {
            let udp_socket = match UdpSocket::bind(format!("0.0.0.0:{}", udp_port)) {
                Ok(socket) => {
                    println!("[INFO] Ping handler listening on UDP port {}", udp_port);
                    socket
                }
                Err(e) => {
                    eprintln!("[ERROR] Failed to bind UDP socket for ping handler: {}", e);
                    return;
                }
            };

            udp_socket.set_read_timeout(Some(Duration::from_millis(100))).unwrap();

            let mut buf = [0; 1024];

            loop {
                match udp_socket.recv_from(&mut buf) {
                    Ok((size, addr)) => {
                        let message = String::from_utf8_lossy(&buf[..size]);
                        if message.trim() == "PING" {
                            // Формируем client_id на основе адреса
                            let client_id = format!("{}", addr);

                            let mut clients_lock = clients.lock().unwrap();
                            if let Some(config) = clients_lock.get_mut(&client_id) {
                                config.update_ping();
                                // Отправляем PONG обратно
                                let _ = udp_socket.send_to(b"PONG", addr);
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // Таймаут - продолжаем проверку
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Error receiving ping: {}", e);
                        thread::sleep(Duration::from_secs(1));
                    }
                }

                // Проверяем устаревших клиентов
                let stale_clients: Vec<String> = {
                    let clients_lock = clients.lock().unwrap();
                    clients_lock.iter()
                        .filter(|(_, config)| config.is_stale(ping_timeout))
                        .map(|(id, _)| id.clone())
                        .collect()
                };

                for client_id in stale_clients {
                    println!("[WARN] Client {} is stale, removing", client_id);
                    clients.lock().unwrap().remove(&client_id);
                }

                thread::sleep(Duration::from_millis(100));
            }
        });
    }
}