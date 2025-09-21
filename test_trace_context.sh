#!/bin/bash

set -e

echo "W3C Trace Context 端到端测试"
echo "============================"

# Function to print colored output
print_status() {
    echo -e "\033[1;34m[INFO]\033[0m $1"
}

print_success() {
    echo -e "\033[1;32m[SUCCESS]\033[0m $1"
}

print_error() {
    echo -e "\033[1;31m[ERROR]\033[0m $1"
}

print_warning() {
    echo -e "\033[1;33m[WARNING]\033[0m $1"
}

# 检查是否已编译WASM文件
if [ ! -f "target/wasm32-unknown-unknown/debug/sp_istio_agent.wasm" ]; then
    print_status "编译WASM文件..."
    cargo build --target wasm32-unknown-unknown
fi

# 启动测试环境
print_status "启动测试环境..."
docker rm -f test-envoy 2>/dev/null || true
docker run -d --name test-envoy -p 18000:18000 -p 18001:18001 \
    -v "$(pwd)/test/envoy.yaml:/etc/envoy/envoy.yaml" \
    -v "$(pwd)/target/wasm32-unknown-unknown/debug/sp_istio_agent.wasm:/etc/envoy/sp_istio_agent.wasm" \
    envoyproxy/envoy:v1.27-latest \
    envoy -c /etc/envoy/envoy.yaml --log-level debug

sleep 3

# 测试URL
TEST_URL="http://localhost:18000/delay/2"

print_status "测试1: 发送不带traceparent头的请求"
echo "预期: 代理应该生成并注入新的traceparent头"
echo

RESPONSE1=$(curl -s -v -X POST "$TEST_URL" \
    -H "accept: application/json" \
    -d "test data" 2>&1)

echo "响应头信息:"
echo "$RESPONSE1" | grep -i "traceparent\|tracestate" || echo "未找到trace context头"
echo

print_status "测试2: 发送带有现有traceparent头的请求"
echo "预期: 代理应该保留现有的traceparent头并可能更新span ID"
echo

# 生成一个测试用的traceparent头
TEST_TRACEPARENT="00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"

RESPONSE2=$(curl -s -v -X POST "$TEST_URL" \
    -H "accept: application/json" \
    -H "traceparent: $TEST_TRACEPARENT" \
    -d "test data" 2>&1)

echo "发送的traceparent: $TEST_TRACEPARENT"
echo "响应头信息:"
echo "$RESPONSE2" | grep -i "traceparent\|tracestate" || echo "未找到trace context头"
echo

print_status "测试3: 检查Envoy日志中的trace context处理"
echo "查看最近的日志..."
docker logs test-envoy --tail=100 | grep -i "SP.*traceparent\|SP.*tracestate\|SP.*inject.*trace\|SP.*propagat" || echo "未找到相关日志"
echo

print_status "测试4: 验证trace ID传播"
echo "发送多个请求验证trace ID的连续性..."

for i in {1..3}; do
    echo "请求 $i:"
    RESPONSE=$(curl -s -v -X POST "$TEST_URL" \
        -H "accept: application/json" \
        -H "traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-$(printf '%016x' $i)-01" \
        -d "test data $i" 2>&1)
    
    echo "$RESPONSE" | grep -i "traceparent" || echo "未找到traceparent头"
    echo "---"
done

print_status "清理测试环境..."
docker rm -f test-envoy

print_success "W3C Trace Context测试完成!"
echo
echo "注意事项:"
echo "1. 检查上述输出中是否包含traceparent和tracestate头"
echo "2. 验证trace ID在请求链中的传播是否正确"
echo "3. 查看Envoy日志确认代理正确处理了trace context"