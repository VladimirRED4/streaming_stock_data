# Streaming Stock Data

[![CI](https://github.com/vladimirred4/streaming_stock_data/actions/workflows/ci.yml/badge.svg)](https://github.com/vladimirred4/streaming_stock_data/actions)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![Tests](https://img.shields.io/badge/tests-passing-brightgreen)]()

Реализация системы потоковой передачи биржевых котировок на Rust.

## 🚀 Особенности

| Feature | Status | Description |
| --------- | -------- | ------------- |
| TCP сервер | ✅ Работает | Управление подключениями |
| UDP стриминг | ✅ Работает | Потоковая передача данных |
| Многопоточность | ✅ Работает | Поддержка множества клиентов |
| Фильтрация | ✅ Работает | Клиенты получают только свои тикеры |
| Ping/Pong | ✅ Работает | Keep-alive механизм |
| Логирование | ✅ Работает | Структурированные логи |
| Конфигурация | ✅ Работает | Аргументы командной строки |

## 📊 Статистика проекта

![Lines of code](https://img.shields.io/tokei/lines/github/vladimirred4/streaming_stock_data)
![GitHub repo size](https://img.shields.io/github/repo-size/vladimirred4/streaming_stock_data)
![GitHub last commit](https://img.shields.io/github/last-commit/vladimirred4/streaming_stock_data)
![GitHub issues](https://img.shields.io/github/issues/vladimirred4/streaming_stock_data)
![GitHub pull requests](https://img.shields.io/github/issues-pr/vladimirred4/streaming_stock_data)

## 🤝 Contributing

[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/vladimirred4/streaming_stock_data/pulls)
[![GitHub contributors](https://img.shields.io/github/contributors/vladimirred4/streaming_stock_data)](https://github.com/vladimirred4/streaming_stock_data/graphs/contributors)

## 🏆 Code Quality

[![CodeFactor](https://www.codefactor.io/repository/github/vladimirred4/streaming_stock_data/badge)](https://www.codefactor.io/repository/github/vladimirred4/streaming_stock_data)
[![Bugs](https://sonarcloud.io/api/project_badges/measure?project=vladimirred4_streaming_stock_data&metric=bugs)](https://sonarcloud.io/summary/new_code?id=vladimirred4_streaming_stock_data)
[![Code Smells](https://sonarcloud.io/api/project_badges/measure?project=vladimirred4_streaming_stock_data&metric=code_smells)](https://sonarcloud.io/summary/new_code?id=vladimirred4_streaming_stock_data)
[![Maintainability Rating](https://sonarcloud.io/api/project_badges/measure?project=vladimirred4_streaming_stock_data&metric=sqale_rating)](https://sonarcloud.io/summary/new_code?id=vladimirred4_streaming_stock_data)

## Реализация системы потоковой передачи биржевых котировок на Rust

## Архитектура

Система состоит из двух основных компонентов:

### Сервер (quote-server)

* TCP сервер для управления подключениями

* Генератор случайных котировок

* UDP рассылка котировок клиентам

* Управление клиентами и ping/pong для поддержания соединения

### Клиент (quote-client)

* TCP клиент для управления подписками

* UDP клиент для получения котировок

* Поддержка нескольких форматов вывода

* Фильтрация котировок по тикерам

### Запуск сервера

```bash
cargo run --bin quote-server
```

По умолчанию сервер запускается на:

* TCP порт: 8080 (управление подключениями)

* UDP порт: 34254 (ping/pong сообщения)

### Параметры сервера

```bash
cargo run --bin quote-server -- --help
```

```text
Usage: quote-server [OPTIONS]

Options:
  -p, --port <PORT>                    TCP server port [default: 8080]
      --ping-port <PING_PORT>          UDP port for ping handler [default: 34254]
  -v, --volatility <VOLATILITY>        Volatility for price generation (0.0 to 1.0) [default: 0.01]
  -i, --interval-ms <INTERVAL_MS>      Generation interval in milliseconds [default: 500]
  -t, --ping-timeout <PING_TIMEOUT>    Ping timeout in seconds [default: 5]
  -f, --ticker-file <TICKER_FILE>      Ticker file path [default: tickers.txt]
      --log-level <LOG_LEVEL>          Log level (error, warn, info, debug, trace) [default: info]
      --color <COLOR>                  Enable colored output [default: true]
  -h, --help                           Print help
```

### Параметры клиента

```bash
cargo run --bin quote-client -- --help
```

```text
Usage: quote-client [OPTIONS]

Options:
  -s, --server-addr <SERVER_ADDR>      TCP server address [default: 127.0.0.1:8080]
  -p, --udp-port <UDP_PORT>            Local UDP port for receiving quotes [default: 55555]
      --server-ping-port <SERVER_PING_PORT>  Server UDP port for ping messages [default: 34254]
  -f, --ticker-file <TICKER_FILE>      Ticker file path (alternative to --tickers)
  -t, --tickers <TICKERS>              Comma-separated list of tickers (alternative to --ticker-file) [default: ]
      --ping-interval <PING_INTERVAL>  Ping interval in seconds [default: 2]
      --output-format <OUTPUT_FORMAT>  Output format (simple, json, detailed, line) [default: line]
  -d, --duration <DURATION>            Run duration in seconds (0 for unlimited) [default: 0]
      --log-level <LOG_LEVEL>          Log level (error, warn, info, debug, trace) [default: info]
      --color <COLOR>                  Enable colored output [default: true]
      --show-timestamp                 Show timestamp in output
  -h, --help                           Print help
```

### Форматы вывода

line (по умолчанию) - Каждая котировка на новой строке

```text
AAPL $185.23 (1250)
TSLA $245.67 (890)
```

detailed - Детализированный формат с временем

```text
[1706495234.123] AAPL: $185.23 (volume: 1250)
```

simple - Простой pipe-разделенный формат

```text
AAPL|185.23|1250|1706495234123
```

json - формат

```json
{"ticker":"AAPL","price":185.23,"volume":1250,"timestamp":1706495234123}
```

### Примеры использования

Пример 1: Базовое использование

```bash
# Запуск сервера
cargo run --bin quote-server

# В другом терминале - запуск клиента
cargo run --bin quote-client -- --tickers AAPL,TSLA,GOOGL --duration 10
```

Пример 2: Несколько клиентов одновременно

```bash
# Клиент 1
cargo run --bin quote-client -- \
  --tickers AAPL,MSFT,GOOGL \
  --udp-port 55560 \
  --duration 20

# Клиент 2
cargo run --bin quote-client -- \
  --tickers TSLA,F \
  --udp-port 55561 \
  --duration 15 \
  --output-format detailed

# Клиент 3 - один тикер в JSON
cargo run --bin quote-client -- \
  --tickers NVDA \
  --udp-port 55562 \
  --duration 10 \
  --output-format json
```

Пример 3: Тестирование с разными параметрами

```bash
# Сервер с высокой волатильностью
cargo run --bin quote-server -- --volatility 0.05 --interval-ms 100

# Клиент с отладочным выводом
cargo run --bin quote-client -- \
  --tickers AAPL,TSLA \
  --log-level debug \
  --show-timestamp \
  --duration 30
```

### Структура проекта

```text
streaming_stock_data/
├── Cargo.toml
├── README.md
├── tickers.txt                    # Файл с тикерами по умолчанию
├── src/
│   ├── lib.rs                     # Общие структуры
│   ├── models.rs                  # Модели данных (StockQuote, ClientConfig, Command)
│   ├── generator.rs               # Генератор котировок
│   ├── tcp_server.rs              # TCP сервер
│   ├── client_manager.rs          # Менеджер клиентов
│   ├── udp_sender.rs              # UDP отправитель котировок
│   ├── server/
│   │   └── main.rs                # Серверное приложение
│   └── client/
│       └── main.rs                # Клиентское приложение
└── examples/
    └── test_client.rs             # Пример простого клиента
```

### Тестирование

```bash
# Запустите тестовый скрипт
chmod +x test_multiple.sh
./test_multiple.sh
```
