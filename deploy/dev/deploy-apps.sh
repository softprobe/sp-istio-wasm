#!/bin/bash

# SP Istio WASM - 演示应用部署脚本
# 该脚本部署 demo-ota 和 demo-airline 应用，并配置 OpenTelemetry 自动注入

set -e

echo "🚀 部署演示应用（demo-ota 和 demo-airline）..."

# 检查集群连接
echo "🔍 检查集群连接..."
if ! kubectl cluster-info &> /dev/null; then
    echo "❌ 无法连接到 Kubernetes 集群，请先运行 ./cluster-setup.sh"
    exit 1
fi
echo "✅ 集群连接正常"

# 检查 OpenTelemetry Operator 是否就绪
echo "📊 检查 OpenTelemetry Operator 状态..."
if ! kubectl get deployment opentelemetry-operator-controller-manager -n opentelemetry-operator-system &> /dev/null; then
    echo "❌ OpenTelemetry Operator 未安装，请先运行 ./cluster-setup.sh"
    exit 1
fi
echo "✅ OpenTelemetry Operator 已就绪"

# 检查并拉取 demo-ota 镜像
echo "📥 准备 demo-ota 镜像..."
OTA_IMAGE="gcr.io/cs-poc-sasxbttlzroculpau4u6e2l/demo-ota:v1.2.1"
if ! docker image inspect $OTA_IMAGE > /dev/null 2>&1; then
    echo "🔄 拉取 demo-ota 镜像..."
    docker pull $OTA_IMAGE
else
    echo "✅ demo-ota 镜像已存在本地"
fi
echo "📥 加载 demo-ota 镜像到 Kind 集群..."
kind load docker-image $OTA_IMAGE --name sp-demo-cluster

# 检查并拉取 demo-airline 镜像
echo "📥 准备 demo-airline 镜像..."
AIRLINE_IMAGE="gcr.io/cs-poc-sasxbttlzroculpau4u6e2l/demo-airline:v0.0.3"
if ! docker image inspect $AIRLINE_IMAGE > /dev/null 2>&1; then
    echo "🔄 拉取 demo-airline 镜像..."
    docker pull $AIRLINE_IMAGE
else
    echo "✅ demo-airline 镜像已存在本地"
fi
echo "📥 加载 demo-airline 镜像到 Kind 集群..."
kind load docker-image $AIRLINE_IMAGE --name sp-demo-cluster

# 部署应用
echo "📦 部署 demo-ota 应用（带 OpenTelemetry 自动注入）..."
kubectl apply -f demo-ota-deployment.yaml

echo "📦 部署 demo-airline 应用（带 OpenTelemetry 自动注入）..."
kubectl apply -f demo-airline-deployment.yaml

echo "🌐 部署 Istio Gateway 和 VirtualService..."
kubectl apply -f demo-istio-gateway.yaml

# 等待部署就绪
echo "⏳ 等待应用部署就绪..."
kubectl wait --for=condition=available --timeout=300s deployment/demo-ota
kubectl wait --for=condition=available --timeout=300s deployment/demo-airline

# 检查 Pod 状态
echo "📋 检查 Pod 状态..."
kubectl get pods -l service=demo-ota
kubectl get pods -l service=demo-airline

# 验证 OpenTelemetry 注入
echo ""
echo "🔍 验证 OpenTelemetry 自动注入..."
echo "检查 demo-ota 服务 (Java):"
kubectl get pod -l service=demo-ota -o jsonpath='{.items[0].metadata.annotations}' | grep -i otel || echo "⚠️  未发现 OpenTelemetry 注解"

echo "检查 demo-airline 服务 (Java):"
kubectl get pod -l service=demo-airline -o jsonpath='{.items[0].metadata.annotations}' | grep -i otel || echo "⚠️  未发现 OpenTelemetry 注解"

echo ""
echo "✅ 演示应用部署成功！"
echo ""
echo "📝 已部署的服务："
echo "  - demo-ota (Java) - 端口 8080"
echo "  - demo-airline (Java) - 端口 8081"
echo ""
echo "🔧 访问应用："
echo "  1. 通过 Istio Gateway 访问（推荐）："
echo "     kubectl port-forward -n istio-system service/istio-ingressgateway 8080:80"
echo "     然后访问："
echo "       http://localhost:8080/ (demo-ota)"
echo "       http://localhost:8080/airline/ (demo-airline)"
echo "       curl -H \"Host: ota.local\" http://localhost:8080/ (demo-ota)"
echo "       curl -H \"Host: airline.local\" http://localhost:8080/ (demo-airline)"
echo "  2. 直接端口转发："
echo "     kubectl port-forward service/demo-ota 8080:8080"
echo "     kubectl port-forward service/demo-airline 8081:8081"
echo "     然后访问："
echo "       http://localhost:8080/ (demo-ota)"
echo "       http://localhost:8081/ (demo-airline)"
