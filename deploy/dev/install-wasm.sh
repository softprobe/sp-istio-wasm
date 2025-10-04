#!/bin/bash

# SP Istio Agent WASM 插件安装脚本
# 该脚本用于安装和配置 SP Istio Agent WASM 插件

set -e

echo "🔧 开始安装 SP Istio Agent WASM 插件..."

# 检查 Kubernetes 集群连接
if ! kubectl cluster-info &> /dev/null; then
    echo "❌ 无法连接到 Kubernetes 集群，请确保集群正在运行"
    exit 1
fi

# 检查 Istio 是否已安装
if ! kubectl get namespace istio-system &> /dev/null; then
    echo "❌ Istio 未安装，请先运行 ./cluster-setup.sh"
    exit 1
fi

# 检查 Bookinfo 应用是否已部署
if ! kubectl get deployment productpage-v1 &> /dev/null; then
    echo "❌ Bookinfo 应用未部署，请先运行 ./deploy-apps.sh"
    exit 1
fi

# 安装 SP Istio Agent WASM 插件
echo "📦 安装 WASM 插件配置..."
kubectl apply -f sp-istio-agent-minimal.yaml
echo "✅ SP Istio Agent WASM 插件已安装"

# 等待插件生效
echo "⏳ 等待 WASM 插件生效..."
sleep 10

# 重启 Bookinfo 应用以应用 WASM 插件
echo "🔄 重启 Bookinfo 应用以应用 WASM 插件..."
kubectl rollout restart deployment/productpage-v1
kubectl rollout restart deployment/details-v1
kubectl rollout restart deployment/ratings-v1
kubectl rollout restart deployment/reviews-v1
kubectl rollout restart deployment/reviews-v2
kubectl rollout restart deployment/reviews-v3

# 等待重启完成
echo "⏳ 等待应用重启完成..."
kubectl rollout status deployment/productpage-v1
kubectl rollout status deployment/details-v1
kubectl rollout status deployment/ratings-v1
kubectl rollout status deployment/reviews-v1
kubectl rollout status deployment/reviews-v2
kubectl rollout status deployment/reviews-v3

echo ""
echo "🎉 WASM 插件安装完成！"
echo ""
echo "📋 下一步操作："
echo "1. 运行 ./start-port-forward.sh 启动端口转发"
echo "2. 访问 http://localhost:8080/productpage 测试应用"
echo "3. 访问 http://localhost:16686 查看 Jaeger 追踪"
echo ""
echo "💡 提示："
echo "- WASM 插件会拦截所有 HTTP 请求并发送追踪数据到 Jaeger"
echo "- 在 Jaeger UI 中可以看到详细的请求追踪信息"