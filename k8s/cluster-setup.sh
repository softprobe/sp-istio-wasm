#!/bin/bash

# Demo Air - Kubernetes 集群和 Istio 设置脚本
# 该脚本用于从零开始创建和配置整个环境

set -e

echo "🚀 开始设置 Demo Air 环境..."

# 1. 创建 Kind 集群
echo "📦 创建 Kind 集群..."
if kind get clusters | grep -q "sp-demo-cluster"; then
    echo "⚠️  集群 sp-demo-cluster 已存在，跳过创建"
else
    kind create cluster --name sp-demo-cluster
    echo "✅ Kind 集群创建完成"
fi

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

# 4. 启动 Jaeger (本地 Docker)
echo "🔍 启动 Jaeger..."
if docker ps | grep -q jaeger; then
    echo "⚠️  Jaeger 已运行，跳过启动"
else
    docker run -d --name jaeger \
        -p 16686:16686 \
        -p 14268:14268 \
        -p 4317:4317 \
        -p 4318:4318 \
        jaegertracing/all-in-one:latest
    echo "✅ Jaeger 启动完成"
fi

# 5. 应用 Istio 网格配置
echo "⚙️  应用 Istio 网格配置..."
kubectl apply -f istio-mesh-config.yaml
echo "✅ Istio 网格配置已应用"

# 6. 创建 Jaeger ServiceEntry
echo "🔗 创建 Jaeger ServiceEntry..."
kubectl apply -f jaeger-service-entry.yaml
echo "✅ Jaeger ServiceEntry 已创建"

# 7. 应用 Telemetry 配置
echo "📊 应用 Telemetry 配置..."
kubectl apply -f default-telemetry-config.yaml
echo "✅ Telemetry 配置已应用"

# 8. 重启 Istio 控制平面
echo "🔄 重启 Istio 控制平面..."
kubectl rollout restart deployment/istiod -n istio-system
kubectl rollout status deployment/istiod -n istio-system
echo "✅ Istio 控制平面重启完成"

echo ""
echo "🎉 环境设置完成！"
echo ""
echo "📋 下一步操作："
echo "1. 运行 ./deploy-apps.sh 部署应用"
echo "2. 运行 ./start-port-forward.sh 启动端口转发"
echo "3. 访问 http://localhost:8080 测试应用"
echo "4. 访问 http://localhost:16686 查看 Jaeger 追踪"