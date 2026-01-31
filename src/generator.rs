use crate::models::StockQuote;
use crossbeam_channel::{Receiver, Sender, unbounded};
use log::{debug, info, trace, warn};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct QuoteGenerator {
    ticker_prices: Arc<Mutex<HashMap<String, f64>>>,
    base_volumes: Arc<Mutex<HashMap<String, u32>>>,
    volatility: f64,
    // Храним senders для каждого тикера отдельно
    ticker_senders: Arc<Mutex<HashMap<String, Vec<Sender<StockQuote>>>>>,
}

impl QuoteGenerator {
    pub fn new(tickers: Vec<String>, volatility: f64) -> Self {
        let mut ticker_prices = HashMap::new();
        let mut base_volumes = HashMap::new();
        let mut ticker_senders = HashMap::new();

        let mut rng = rand::thread_rng();

        // Инициализируем начальные цены и senders для каждого тикера
        for ticker in tickers {
            let ticker_upper = ticker.to_uppercase();
            let initial_price = rng.gen_range(50.0..1000.0);
            ticker_prices.insert(ticker_upper.clone(), initial_price);
            ticker_senders.insert(ticker_upper.clone(), Vec::new());

            let base_volume = match ticker_upper.as_str() {
                "AAPL" | "MSFT" | "GOOGL" => 5000,
                "TSLA" | "AMZN" | "NVDA" => 3000,
                "META" | "JPM" | "JNJ" => 2000,
                _ => 1000,
            };
            base_volumes.insert(ticker_upper, base_volume);
        }

        debug!(
            "Initialized quote generator with {} tickers",
            ticker_prices.len()
        );

        QuoteGenerator {
            ticker_prices: Arc::new(Mutex::new(ticker_prices)),
            base_volumes: Arc::new(Mutex::new(base_volumes)),
            volatility,
            ticker_senders: Arc::new(Mutex::new(ticker_senders)),
        }
    }

    // Создание нового ресивера для клиента для конкретных тикеров
    // Возвращает Vec<Receiver<StockQuote>> - по одному ресиверу на каждый тикер
    pub fn subscribe_to_tickers(&self, tickers: Vec<String>) -> Vec<Receiver<StockQuote>> {
        let mut receivers = Vec::new();

        {
            let mut ticker_senders = self.ticker_senders.lock().unwrap();

            for ticker in tickers {
                let ticker_upper = ticker.to_uppercase();

                if let Some(sender_list) = ticker_senders.get_mut(&ticker_upper) {
                    let (tx, rx) = unbounded();
                    sender_list.push(tx);
                    receivers.push(rx);
                    debug!("Client subscribed to ticker: {}", ticker_upper);
                } else {
                    warn!(
                        "Client tried to subscribe to non-existent ticker: {}",
                        ticker_upper
                    );
                }
            }
        }

        receivers
    }

    // Отписка клиента от тикеров
    pub fn unsubscribe_from_tickers(&self, tickers: Vec<String>) {
        let ticker_senders = self.ticker_senders.lock().unwrap();

        for ticker in tickers {
            let ticker_upper = ticker.to_uppercase();

            if let Some(sender_list) = ticker_senders.get(&ticker_upper) {
                debug!(
                    "Cleaning up subscriptions for ticker: {} ({} senders)",
                    ticker_upper,
                    sender_list.len()
                );
            }
        }
    }

    // Запуск генерации котировок в отдельном потоке
    pub fn start(self, interval_ms: u64) {
        info!("Starting quote generator with interval {}ms", interval_ms);

        thread::spawn(move || {
            let tickers: Vec<String> = {
                let prices = self.ticker_prices.lock().unwrap();
                prices.keys().cloned().collect()
            };

            let mut iteration = 0;
            info!(
                "Quote generator thread started for {} tickers",
                tickers.len()
            );

            loop {
                iteration += 1;
                trace!("Generation iteration {} started", iteration);

                // Генерируем котировки для ВСЕХ тикеров
                for ticker in &tickers {
                    let (price, volume) = {
                        let mut prices = self.ticker_prices.lock().unwrap();
                        let base_volumes = self.base_volumes.lock().unwrap();

                        let last_price = prices.get_mut(ticker).unwrap();
                        let mut rng = rand::thread_rng();
                        let change = rng.gen_range(-self.volatility..self.volatility);
                        *last_price *= 1.0 + change;

                        if *last_price < 1.0 {
                            *last_price = 1.0;
                        }

                        let base_volume = base_volumes.get(ticker).copied().unwrap_or(1000);
                        let std_dev = (base_volume as f64 * 0.3) as u32;
                        let normal_sample = rng.gen_range(-2.0..2.0);
                        let volume_f64 = base_volume as f64 + normal_sample * std_dev as f64;

                        let volume = if rng.gen_bool(0.05) {
                            (volume_f64.max(100.0) as u32) * 3
                        } else {
                            volume_f64.max(100.0) as u32
                        };

                        (*last_price, volume)
                    };

                    let quote = StockQuote::new(ticker.clone(), price, volume);

                    // Отправляем котировку только подписанным клиентам для этого тикера
                    {
                        let mut ticker_senders = self.ticker_senders.lock().unwrap();

                        if let Some(senders) = ticker_senders.get_mut(ticker) {
                            // Удаляем отключившихся клиентов
                            senders.retain(|sender| {
                                if sender.send(quote.clone()).is_err() {
                                    trace!("Removing disconnected sender for ticker {}", ticker);
                                    false
                                } else {
                                    true
                                }
                            });

                            trace!(
                                "Generated quote for {}: price={:.2}, volume={} (sent to {} clients)",
                                ticker,
                                price,
                                volume,
                                senders.len()
                            );
                        }
                    }
                }

                if iteration % 100 == 0 {
                    // Статистика по подпискам
                    let ticker_senders = self.ticker_senders.lock().unwrap();
                    let mut total_clients = 0;
                    let mut active_tickers = 0;

                    for (_ticker, senders) in ticker_senders.iter() {
                        if !senders.is_empty() {
                            active_tickers += 1;
                            total_clients += senders.len();
                        }
                    }

                    info!(
                        "Completed {} cycles, {} active tickers, total active clients: {}",
                        iteration, active_tickers, total_clients
                    );
                }

                thread::sleep(Duration::from_millis(interval_ms));
            }
        });
    }

    // Проверка существования тикера
    pub fn has_ticker(&self, ticker: &str) -> bool {
        let ticker_upper = ticker.to_uppercase();
        let prices = self.ticker_prices.lock().unwrap();
        prices.contains_key(&ticker_upper)
    }

    // Загрузка тикеров из файла
    pub fn from_file(filename: &str, volatility: f64) -> std::io::Result<Self> {
        info!("Loading tickers from file: {}", filename);
        let content = std::fs::read_to_string(filename)?;
        let tickers: Vec<String> = content
            .lines()
            .map(|line| line.trim().to_uppercase())
            .filter(|line| !line.is_empty())
            .collect();

        info!("Loaded {} tickers from {}", tickers.len(), filename);
        Ok(Self::new(tickers, volatility))
    }
}
