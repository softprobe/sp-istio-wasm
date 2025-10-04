#!/bin/bash

# Bookinfo - 应用部署脚本
# 该脚本用于部署 Istio Bookinfo 示例应用

set -e

echo "🚀 开始部署 Bookinfo 应用..."

# 1. 部署 Bookinfo 应用
echo "📚 部署 Bookinfo 应用..."
kubectl apply -f https://raw.githubusercontent.com/istio/istio/release-1.22/samples/bookinfo/platform/kube/bookinfo.yaml
echo "✅ Bookinfo 应用部署完成"

# 2. 部署 Bookinfo Gateway
echo "🌐 部署 Bookinfo Gateway..."
kubectl apply -f https://raw.githubusercontent.com/istio/istio/release-1.22/samples/bookinfo/networking/bookinfo-gateway.yaml
echo "✅ Bookinfo Gateway 部署完成"

# 3. 等待 Pod 就绪
echo "⏳ 等待 Pod 就绪..."
kubectl wait --for=condition=ready pod -l app=productpage --timeout=300s
kubectl wait --for=condition=ready pod -l app=details --timeout=300s
kubectl wait --for=condition=ready pod -l app=ratings --timeout=300s
kubectl wait --for=condition=ready pod -l app=reviews --timeout=300s
echo "✅ 所有 Pod 已就绪"

echo ""
echo "🎉 Bookinfo 应用部署完成！"
echo ""
echo "📋 验证部署："
echo "kubectl get pods"
echo "kubectl get services"
echo ""
echo "📋 下一步操作："
echo "1. 运行 ./install-wasm.sh 安装 WASM 插件"
echo "2. 运行 ./start-port-forward.sh 启动端口转发"
echo "3. 访问 http://localhost:8080/productpage 测试应用"