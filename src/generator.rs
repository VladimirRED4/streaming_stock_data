use crate::models::StockQuote;
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crossbeam_channel::{Sender, Receiver, unbounded};

#[derive(Clone)]
pub struct QuoteGenerator {
    ticker_prices: Arc<Mutex<HashMap<String, f64>>>,
    base_volumes: Arc<Mutex<HashMap<String, u32>>>,
    volatility: f64,
    quote_sender: Arc<Mutex<Vec<Sender<StockQuote>>>>,  // Храним все senders для клиентов
}

impl QuoteGenerator {
    pub fn new(tickers: Vec<String>, volatility: f64) -> Self {
        let mut ticker_prices = HashMap::new();
        let mut base_volumes = HashMap::new();

        let mut rng = rand::thread_rng();

        // Инициализируем начальные цены для каждого тикера
        for ticker in tickers {
            // Начальная цена от 50 до 1000
            let initial_price = rng.gen_range(50.0..1000.0);
            ticker_prices.insert(ticker.clone(), initial_price);

            // Базовый объем в зависимости от тикера
            let base_volume = match ticker.as_str() {
                "AAPL" | "MSFT" | "GOOGL" => 5000,
                "TSLA" | "AMZN" | "NVDA" => 3000,
                "META" | "JPM" | "JNJ" => 2000,
                _ => 1000,
            };
            base_volumes.insert(ticker, base_volume);
        }

        QuoteGenerator {
            ticker_prices: Arc::new(Mutex::new(ticker_prices)),
            base_volumes: Arc::new(Mutex::new(base_volumes)),
            volatility,
            quote_sender: Arc::new(Mutex::new(Vec::new())),
        }
    }

    // Создание нового ресивера для клиента
    pub fn create_client_receiver(&self) -> Receiver<StockQuote> {
        let (tx, rx) = unbounded();

        // Добавляем sender в список для рассылки
        {
            let mut senders = self.quote_sender.lock().unwrap();
            senders.push(tx);
        }

        rx
    }

    // Запуск генерации котировок в отдельном потоке
    pub fn start(self, interval_ms: u64) {
        thread::spawn(move || {
            let tickers: Vec<String> = {
                let prices = self.ticker_prices.lock().unwrap();
                prices.keys().cloned().collect()
            };

            loop {
                // Генерируем котировки для всех тикеров
                for ticker in &tickers {
                    let price = {
                        let mut prices = self.ticker_prices.lock().unwrap();
                        if let Some(last_price) = prices.get_mut(ticker) {
                            let mut rng = rand::thread_rng();
                            let change = rng.gen_range(-self.volatility..self.volatility);
                            *last_price *= 1.0 + change;

                            if *last_price < 1.0 {
                                *last_price = 1.0;
                            }

                            *last_price
                        } else {
                            continue;
                        }
                    };

                    let volume = {
                        let base_volumes = self.base_volumes.lock().unwrap();
                        let base_volume = base_volumes.get(ticker).copied().unwrap_or(1000);
                        drop(base_volumes);

                        let mut rng = rand::thread_rng();
                        let std_dev = (base_volume as f64 * 0.3) as u32;
                        let normal_sample = rng.gen_range(-2.0..2.0);
                        let volume_f64 = base_volume as f64 + normal_sample * std_dev as f64;

                        if rng.gen_bool(0.05) {
                            (volume_f64.max(100.0) as u32) * 3
                        } else {
                            volume_f64.max(100.0) as u32
                        }
                    };

                    let quote = StockQuote::new(ticker.clone(), price, volume);

                    // Отправляем котировку всем клиентам
                    let mut senders = self.quote_sender.lock().unwrap();

                    // Удаляем отключившихся клиентов
                    senders.retain(|sender| {
                        if sender.send(quote.clone()).is_err() {
                            // Клиент отключился
                            false
                        } else {
                            true
                        }
                    });
                }

                thread::sleep(Duration::from_millis(interval_ms));
            }
        });
    }

    // Проверка существования тикера
    pub fn has_ticker(&self, ticker: &str) -> bool {
        let prices = self.ticker_prices.lock().unwrap();
        prices.contains_key(ticker)
    }

    // Загрузка тикеров из файла
    pub fn from_file(filename: &str, volatility: f64) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(filename)?;
        let tickers: Vec<String> = content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(Self::new(tickers, volatility))
    }
}