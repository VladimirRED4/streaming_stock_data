#!/bin/bash

echo "=== Testing Multiple Clients ==="

# Очистка
pkill -f quote-server 2>/dev/null || true
pkill -f quote-client 2>/dev/null || true
sleep 2

# Запуск сервера
echo "Starting server..."
cargo run --bin quote-server -- --log-level warn > server.log 2>&1 &
SERVER_PID=$!
sleep 5

# Функция запуска клиента
start_client() {
    local name=$1
    local tickers=$2
    local port=$3
    local format=$4
    local duration=$5

    echo "Starting $name on port $port (tickers: $tickers)"

    cargo run --bin quote-client -- \
        --server-addr 127.0.0.1:8080 \
        --udp-port $port \
        --tickers $tickers \
        --output-format $format \
        --duration $duration \
        --log-level error \
        > "$name.log" 2>&1 &

    eval "${name}_PID=$!"
}

# Запускаем клиентов
start_client "tech" "AAPL,MSFT,GOOGL" 55560 "line" 25
sleep 2
start_client "cars" "TSLA,F" 55561 "detailed" 20
sleep 2
start_client "chips" "NVDA,INTC,AMD" 55562 "simple" 15
sleep 2
start_client "single" "AAPL" 55563 "json" 10

echo -e "\nAll clients started. Running for 30 seconds...\n"

# Мониторинг
for i in {1..6}; do
    sleep 5
    echo "=== Update after $((i*5)) seconds ==="

    for client in tech cars chips single; do
        if [ -f "${client}.log" ]; then
            echo "--- $client ---"
            tail -2 "${client}.log" | grep -E "(AAPL|MSFT|GOOGL|TSLA|F|NVDA|INTC|AMD|\$|quotes received)" || echo "Waiting for quotes..."
        fi
    done

    echo "====================="
    echo ""
done

# Останавливаем
echo "Stopping..."
kill $SERVER_PID 2>/dev/null || true
pkill -f quote-client 2>/dev/null || true

# Статистика
echo -e "\n=== Final Statistics ==="
for client in tech cars chips single; do
    if [ -f "${client}.log" ]; then
        echo "$client:"
        grep -o "Total quotes received: [0-9]*" "${client}.log" | tail -1 || echo "  No quotes count"
        grep -o "Average rate: [0-9.]*" "${client}.log" | tail -1 || echo "  No rate info"
    fi
done

# Очистка
rm -f server.log tech.log cars.log chips.log single.log 2>/dev/null

echo -e "\nTest completed!"
