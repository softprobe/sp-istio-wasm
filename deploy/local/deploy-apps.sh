#!/bin/bash

# Demo Air - 应用部署脚本
# 该脚本用于部署所有应用组件

set -e

echo "🚀 开始部署 Demo Air 应用..."

# 0. 加载 Docker 镜像到 Kind 集群
echo "📦 加载 Docker 镜像到 Kind 集群..."

# 检查并拉取 demo-ota 镜像
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
AIRLINE_IMAGE="gcr.io/cs-poc-sasxbttlzroculpau4u6e2l/demo-airline:v0.0.3"
if ! docker image inspect $AIRLINE_IMAGE > /dev/null 2>&1; then
    echo "🔄 拉取 demo-airline 镜像..."
    docker pull $AIRLINE_IMAGE
else
    echo "✅ demo-airline 镜像已存在本地"
fi
echo "📥 加载 demo-airline 镜像到 Kind 集群..."
kind load docker-image $AIRLINE_IMAGE --name sp-demo-cluster

echo "✅ 所有镜像已加载到 Kind 集群"

# 1. 部署 demo-ota 服务
echo "📱 部署 demo-ota 服务..."
kubectl apply -f demo-ota-deployment.yaml
echo "✅ demo-ota 服务部署完成"

# 2. 部署 demo-airline 服务
echo "✈️  部署 demo-airline 服务..."
kubectl apply -f demo-airline-deployment.yaml
echo "✅ demo-airline 服务部署完成"

# 3. 部署 Istio Gateway
echo "🌐 部署 Istio Gateway..."
kubectl apply -f demo-istio-gateway.yaml
echo "✅ Istio Gateway 部署完成"

# 4. 等待 Pod 就绪
echo "⏳ 等待 Pod 就绪..."
kubectl wait --for=condition=ready pod -l app=demo-ota --timeout=300s
kubectl wait --for=condition=ready pod -l app=demo-airline --timeout=300s
echo "✅ 所有 Pod 已就绪"

# 5. 重启部署以确保配置生效
echo "🔄 重启部署以确保配置生效..."
kubectl rollout restart deployment/demo-ota
kubectl rollout restart deployment/demo-airline
kubectl rollout status deployment/demo-ota
kubectl rollout status deployment/demo-airline
echo "✅ 部署重启完成"

echo ""
echo "🎉 应用部署完成！"
echo ""
echo "📋 验证部署："
echo "kubectl get pods"
echo "kubectl get services"
echo ""
echo "📋 下一步操作："
echo "1. 运行 ./start-port-forward.sh 启动端口转发"
echo "2. 访问 http://localhost:8080 测试应用"
echo "3. 访问 http://localhost:16686 查看 Jaeger 追踪"