#!/bin/bash

# Demo Air - 端口转发脚本
# 该脚本用于启动必要的端口转发

set -e

echo "🔗 启动端口转发..."

# 检查是否已有端口转发在运行
if pgrep -f "kubectl port-forward.*istio-ingressgateway.*8080:80" > /dev/null; then
    echo "⚠️  端口 8080 的转发已在运行"
else
    echo "📱 启动应用端口转发 (8080 -> Istio Gateway)..."
    kubectl port-forward -n istio-system svc/istio-ingressgateway 8080:80 &
    APP_PF_PID=$!
    echo "✅ 应用端口转发已启动 (PID: $APP_PF_PID)"
fi

if pgrep -f "kubectl port-forward.*jaeger.*16686:16686" > /dev/null; then
    echo "⚠️  端口 16686 的转发已在运行"
else
    echo "🔍 启动 Jaeger 端口转发 (16686 -> Jaeger UI)..."
    # 注意：这里假设 Jaeger 运行在 Docker 中，如果部署在 K8s 中需要调整
    echo "ℹ️  Jaeger 运行在 Docker 中，直接访问 http://localhost:16686"
fi

echo ""
echo "🎉 端口转发设置完成！"
echo ""
echo "📋 可用服务："
echo "• 应用服务: http://localhost:8080"
echo "• Jaeger UI: http://localhost:16686"
echo ""
echo "📋 测试命令："
echo "curl -H 'traceparent: 00-$(openssl rand -hex 16)-$(openssl rand -hex 8)-01' http://localhost:8080/"
echo ""
echo "📋 停止端口转发："
echo "pkill -f 'kubectl port-forward'"