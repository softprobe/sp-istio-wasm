#!/bin/bash

# SP Istio WASM - 环境清理脚本
# 该脚本用于清理所有部署的资源和集群

set -e

echo "🧹 开始清理 SP Istio WASM 环境..."

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

# 1. 停止 WASM 文件服务器
echo "🛑 停止 WASM 文件服务器..."
pkill -f "python3 -m http.server 8000" 2>/dev/null || echo "⚠️  没有运行中的 WASM 文件服务器"

# 2. 停止端口转发
echo "🛑 停止端口转发..."
pkill -f "kubectl port-forward" 2>/dev/null || echo "⚠️  没有运行中的端口转发"

# 3. 清理 WASM 插件
echo "🔧 清理 WASM 插件..."
safe_execute "kubectl delete wasmplugin -n istio-system sp-istio-agent" "删除 WASM 插件"
safe_execute "kubectl delete serviceentry -n istio-system softprobe-backend" "删除 SoftProbe ServiceEntry"
safe_execute "kubectl delete destinationrule -n istio-system softprobe-backend-tls" "删除 SoftProbe DestinationRule"

# 4. 清理演示应用
echo "📱 清理演示应用..."
safe_execute "kubectl delete -f deploy/demo-apps-deployment.yaml" "删除演示应用"
safe_execute "kubectl delete -f deploy/demo-istio-gateway.yaml" "删除 Istio Gateway"

# 5. 清理 OpenTelemetry 配置
echo "📊 清理 OpenTelemetry 配置..."
safe_execute "kubectl delete instrumentation default-instrumentation" "删除 OpenTelemetry Instrumentation"

# 6. 清理 OpenTelemetry Operator
echo "🔧 清理 OpenTelemetry Operator..."
safe_execute "kubectl delete -f opentelemetry-operator.yaml" "删除 OpenTelemetry Operator"

# 7. 清理 cert-manager
echo "🔐 清理 cert-manager..."
safe_execute "kubectl delete -f https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml" "删除 cert-manager"

# 8. 清理 Istio
echo "🌐 清理 Istio..."
safe_execute "istioctl uninstall --purge -y" "卸载 Istio"
safe_execute "kubectl delete namespace istio-system" "删除 istio-system namespace"

# 9. 清理其他可能的资源
echo "🧽 清理其他资源..."
safe_execute "kubectl delete namespace opentelemetry-operator-system" "删除 opentelemetry-operator-system namespace"
safe_execute "kubectl delete namespace cert-manager" "删除 cert-manager namespace"

# 等待资源清理完成
echo "⏳ 等待资源清理完成..."
sleep 10

# 10. 删除 Kind 集群
echo "🗑️  删除 Kind 集群..."
if kind get clusters | grep -q "sp-demo-cluster"; then
    kind delete cluster --name sp-demo-cluster
    echo "✅ Kind 集群已删除"
else
    echo "⚠️  sp-demo-cluster 集群不存在"
fi

echo ""
echo "🎉 环境清理完成！"
echo ""
echo "📋 清理总结："
echo "✅ WASM 文件服务器已停止"
echo "✅ 端口转发已停止"
echo "✅ WASM 插件已删除"
echo "✅ 演示应用已删除"
echo "✅ OpenTelemetry 配置已删除"
echo "✅ OpenTelemetry Operator 已删除"
echo "✅ cert-manager 已删除"
echo "✅ Istio 已卸载"
echo "✅ Kind 集群已删除"
echo ""
echo "💡 提示："
echo "- 如需重新部署，请运行 ./scripts/cluster-setup.sh"
echo "- 所有本地数据已清理，包括追踪数据"