#!/bin/bash

# SP Istio WASM - Kubernetes 集群和 Istio 设置脚本
# 该脚本用于从零开始创建和配置整个环境，包括 Kind 集群、Istio 服务网格和 OpenTelemetry

set -e

echo "🚀 开始设置 SP Istio WASM 演示环境..."

# 1. 创建 Kind 集群
echo "📦 创建 Kind 集群..."
if kind get clusters | grep -q "sp-demo-cluster"; then
    echo "⚠️  集群 sp-demo-cluster 已存在，跳过创建"
else
    kind create cluster --name sp-demo-cluster
    echo "✅ Kind 集群创建完成"
fi

# 检查集群连接
echo "🔍 检查集群连接..."
if ! kubectl cluster-info &> /dev/null; then
    echo "❌ 无法连接到 Kubernetes 集群"
    exit 1
fi
echo "✅ 集群连接正常"

# 2. 安装 Istio
echo "🌐 安装 Istio..."
if kubectl get namespace istio-system &> /dev/null; then
    echo "⚠️  Istio 已安装，跳过安装步骤"
else
    istioctl install --set values.defaultRevision=default -y
    echo "✅ Istio 安装完成"
fi

# 3. 启用 Istio 注入
echo "💉 启用 default namespace 的 Istio 注入..."
kubectl label namespace default istio-injection=enabled --overwrite
echo "✅ Istio 注入已启用"

# 4. 安装 cert-manager (OpenTelemetry Operator 的依赖)
echo "🔐 安装 cert-manager..."
if kubectl get deployment cert-manager -n cert-manager &> /dev/null; then
    echo "⚠️  cert-manager 已安装，跳过安装步骤"
else
    kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml
    
    # 等待 cert-manager 就绪
    echo "⏳ 等待 cert-manager 就绪..."
    kubectl wait --for=condition=available --timeout=300s deployment/cert-manager -n cert-manager
    kubectl wait --for=condition=available --timeout=300s deployment/cert-manager-cainjector -n cert-manager
    kubectl wait --for=condition=available --timeout=300s deployment/cert-manager-webhook -n cert-manager
    echo "✅ cert-manager 安装完成"
fi

# 5. 安装 OpenTelemetry Operator
echo "📊 安装 OpenTelemetry Operator..."
if kubectl get deployment opentelemetry-operator-controller-manager -n opentelemetry-operator-system &> /dev/null; then
    echo "⚠️  OpenTelemetry Operator 已安装，跳过安装步骤"
else
    kubectl apply -f https://github.com/open-telemetry/opentelemetry-operator/releases/latest/download/opentelemetry-operator.yaml
    # 等待 OpenTelemetry Operator 就绪
    echo "⏳ 等待 OpenTelemetry Operator 就绪..."
    kubectl wait --for=condition=available --timeout=300s deployment/opentelemetry-operator-controller-manager -n opentelemetry-operator-system
    
    # 等待 webhook 服务就绪
    echo "⏳ 等待 OpenTelemetry Operator webhook 服务就绪..."
    kubectl wait --for=condition=ready --timeout=300s pod -l app.kubernetes.io/name=opentelemetry-operator -n opentelemetry-operator-system
    
    # 额外等待时间确保 webhook 完全启动
    echo "⏳ 等待 webhook 服务完全启动..."
    sleep 30
    
    echo "✅ OpenTelemetry Operator 安装完成"
fi

# 6. 应用 OpenTelemetry 自动注入配置
echo "📊 应用 OpenTelemetry 自动注入配置..."
kubectl apply -f auto-instrumentation.yaml

# 等待配置处理
echo "⏳ 等待配置处理..."
sleep 10
echo "✅ OpenTelemetry 自动注入配置已应用"

echo ""
echo "🎉 基础环境设置完成！"
echo ""
echo "📋 下一步操作："
echo "1. 运行 ./deploy-apps.sh 部署演示应用"
echo "2. 运行 ./install-wasm.sh 安装 WASM 插件"
echo "3. 运行 ./start-port-forward.sh 启动端口转发"
echo "4. 访问应用进行测试"