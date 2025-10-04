#!/bin/bash

# Demo Air - 环境清理脚本
# 该脚本用于清理所有部署的资源和集群

set -e

echo "🧹 开始清理 Demo Air 环境..."

# 函数：安全执行命令，忽略错误
safe_execute() {
    local cmd="$1"
    local description="$2"
    echo "🔄 $description..."
    if eval "$cmd" 2>/dev/null; then
        echo "✅ $description 完成"
    else
        echo "⚠️  $description 跳过（资源可能不存在）"
    fi
}

# 1. 停止端口转发
echo "🛑 停止端口转发..."
pkill -f "kubectl port-forward" 2>/dev/null || echo "⚠️  没有运行中的端口转发"

# 2. 清理 WASM 插件
echo "🔧 清理 WASM 插件..."
safe_execute "kubectl delete wasmplugin -n istio-system sp-istio-agent" "删除 WASM 插件"
safe_execute "kubectl delete serviceentry -n istio-system softprobe-backend" "删除 SoftProbe ServiceEntry"

# 3. 清理 Bookinfo 应用
echo "📚 清理 Bookinfo 应用..."
safe_execute "kubectl delete -f https://raw.githubusercontent.com/istio/istio/release-1.22/samples/bookinfo/networking/bookinfo-gateway.yaml" "删除 Bookinfo Gateway"
safe_execute "kubectl delete -f https://raw.githubusercontent.com/istio/istio/release-1.22/samples/bookinfo/platform/kube/bookinfo.yaml" "删除 Bookinfo 应用"

# 4. 清理 OpenTelemetry 配置
echo "📊 清理 OpenTelemetry 配置..."
safe_execute "kubectl delete instrumentation default-instrumentation" "删除 OpenTelemetry Instrumentation"
safe_execute "kubectl delete serviceentry jaeger-external" "删除 Jaeger ServiceEntry"

# 5. 清理 OpenTelemetry Operator
echo "🔧 清理 OpenTelemetry Operator..."
safe_execute "kubectl delete -f https://github.com/open-telemetry/opentelemetry-operator/releases/latest/download/opentelemetry-operator.yaml" "删除 OpenTelemetry Operator"

# 6. 清理 cert-manager
echo "🔐 清理 cert-manager..."
safe_execute "kubectl delete -f https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml" "删除 cert-manager"

# 7. 清理 Istio
echo "🌐 清理 Istio..."
safe_execute "istioctl uninstall --purge -y" "卸载 Istio"
safe_execute "kubectl delete namespace istio-system" "删除 istio-system namespace"

# 8. 清理其他可能的资源
echo "🧽 清理其他资源..."
safe_execute "kubectl delete namespace opentelemetry-operator-system" "删除 opentelemetry-operator-system namespace"
safe_execute "kubectl delete namespace cert-manager" "删除 cert-manager namespace"

# 等待资源清理完成
echo "⏳ 等待资源清理完成..."
sleep 10

# 9. 停止本地 Jaeger
echo "🔍 停止本地 Jaeger..."
if docker ps | grep -q jaeger; then
    docker stop jaeger 2>/dev/null || echo "⚠️  Jaeger 容器停止失败"
    docker rm jaeger 2>/dev/null || echo "⚠️  Jaeger 容器删除失败"
    echo "✅ Jaeger 容器已停止并删除"
else
    echo "⚠️  没有运行中的 Jaeger 容器"
fi

# 10. 删除 Kind 集群
echo "🗑️  删除 Kind 集群..."
if kind get clusters | grep -q "sp-demo-cluster"; then
    kind delete cluster --name sp-demo-cluster
    echo "✅ Kind 集群已删除"
else
    echo "⚠️  sp-demo-cluster 集群不存在"
fi

# 11. 清理 Docker 镜像（可选）
echo ""
echo "🐳 Docker 镜像清理选项："
echo "是否要清理相关的 Docker 镜像？这将删除："
echo "  - Jaeger 镜像"
echo "  - OpenTelemetry 相关镜像"
echo "  - Istio 相关镜像"
echo "  - Kind 节点镜像"
echo ""
read -p "清理 Docker 镜像？(y/N): " cleanup_images

if [[ "$cleanup_images" =~ ^[Yy]$ ]]; then
    echo "🧹 清理 Docker 镜像..."
    
    # 清理 Jaeger 镜像
    safe_execute "docker rmi jaegertracing/all-in-one:latest" "删除 Jaeger 镜像"
    
    # 清理 OpenTelemetry 镜像
    safe_execute "docker images | grep opentelemetry | awk '{print \$3}' | xargs docker rmi" "删除 OpenTelemetry 镜像"
    
    # 清理 Istio 镜像
    safe_execute "docker images | grep istio | awk '{print \$3}' | xargs docker rmi" "删除 Istio 镜像"
    
    # 清理 Kind 节点镜像
    safe_execute "docker images | grep kindest/node | awk '{print \$3}' | xargs docker rmi" "删除 Kind 节点镜像"
    
    # 清理悬空镜像
    safe_execute "docker image prune -f" "清理悬空镜像"
    
    echo "✅ Docker 镜像清理完成"
else
    echo "⚠️  跳过 Docker 镜像清理"
fi

echo ""
echo "🎉 环境清理完成！"
echo ""
echo "📋 清理总结："
echo "✅ 端口转发已停止"
echo "✅ WASM 插件已删除"
echo "✅ Bookinfo 应用已删除"
echo "✅ OpenTelemetry 配置已删除"
echo "✅ OpenTelemetry Operator 已删除"
echo "✅ cert-manager 已删除"
echo "✅ Istio 已卸载"
echo "✅ Jaeger 容器已停止"
echo "✅ Kind 集群已删除"
echo ""
echo "💡 提示："
echo "- 如需重新部署，请运行 ./cluster-setup.sh"
echo "- 所有本地数据已清理，包括追踪数据"
echo "- Docker Desktop 仍在运行，如需停止请手动操作"