#!/bin/bash

# 演示应用端口转发脚本
# 该脚本用于启动演示应用的端口转发

set -e

echo "🔗 启动端口转发..."

# 检查 kubectl 连接
if ! kubectl cluster-info &> /dev/null; then
    echo "❌ 无法连接到 Kubernetes 集群"
    exit 1
fi

# 检查应用是否运行
if ! kubectl get pod -l app=demo-ota | grep -q Running; then
    echo "❌ demo-ota 应用未运行，请先运行 ./deploy-apps.sh"
    exit 1
fi

# 停止现有的端口转发
echo "🛑 停止现有的端口转发..."
pkill -f "kubectl port-forward.*demo-ota" 2>/dev/null || true
pkill -f "kubectl port-forward.*demo-airline" 2>/dev/null || true

sleep 2

# 启动 demo-ota 端口转发 (8080)
echo "📱 启动 demo-ota 端口转发 (8080 -> 8080)..."
kubectl port-forward -n istio-system svc/istio-ingressgateway 8080:80 &
OTA_PF_PID=$!
echo "✅ demo-ota 端口转发已启动 (PID: $OTA_PF_PID)"

sleep 3

echo ""
echo "🎉 端口转发已启动！"
echo ""
echo "📱 访问应用："
echo "   demo-ota: http://localhost:8080"
echo ""
echo "🧪 测试命令："
echo "   curl http://localhost:8080/api/hotels"
echo ""
echo "🛑 停止端口转发："
echo "   按 Ctrl+C 或运行: pkill -f 'kubectl port-forward'"