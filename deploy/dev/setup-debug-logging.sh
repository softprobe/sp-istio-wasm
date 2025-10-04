#!/bin/bash

# Istio WASM 调试日志配置脚本
# 用于配置 Istio 和 Envoy 的调试日志以便查看 WASM 插件的详细日志

set -e

echo "🔧 配置 Istio WASM 调试日志..."

# 方法1: 通过注解直接修改 demo-ota deployment
echo "📝 方法1: 为 demo-ota 添加调试日志注解..."
kubectl patch deployment demo-ota -p '{
  "spec": {
    "template": {
      "metadata": {
        "annotations": {
          "sidecar.istio.io/logLevel": "debug",
          "sidecar.istio.io/accessLogFile": "/dev/stdout"
        }
      }
    }
  }
}'

# 方法2: 设置环境变量 (如果上面的注解不生效)
echo "🔧 方法2: 为 istio-proxy 容器设置环境变量..."
kubectl patch deployment demo-ota --type='json' -p='[
  {
    "op": "add",
    "path": "/spec/template/spec/containers/-",
    "value": {
      "name": "istio-proxy-env-patch",
      "image": "busybox:latest",
      "command": ["sh", "-c", "echo Environment variables set for debugging"],
      "env": [
        {"name": "ENVOY_LOG_LEVEL", "value": "debug"},
        {"name": "WASM_LOG_LEVEL", "value": "debug"},
        {"name": "PILOT_ENABLE_WASM_TELEMETRY_V2", "value": "true"}
      ]
    }
  }
]' || echo "⚠️  环境变量补丁可能已存在或不适用"

# 方法3: 应用 Telemetry 配置 (如果集群支持)
echo "📊 方法3: 应用 Telemetry 调试配置..."
kubectl apply -f istio-debug-config.yaml || echo "⚠️  Telemetry API 可能不可用"

# 等待 Pod 重启
echo "⏳ 等待 demo-ota Pod 重启..."
kubectl rollout status deployment/demo-ota --timeout=120s

# 获取新的 Pod 名称
POD_NAME=$(kubectl get pods -l app=demo-ota -o jsonpath='{.items[0].metadata.name}')
echo "✅ 新的 Pod: $POD_NAME"

# 验证配置
echo "🔍 验证日志配置..."
echo "检查 istio-proxy 容器的环境变量:"
kubectl exec $POD_NAME -c istio-proxy -- env | grep -E "(LOG_LEVEL|WASM)" || echo "未找到相关环境变量"

echo "📋 查看 WASM 相关日志的命令:"
echo "kubectl logs -f $POD_NAME -c istio-proxy | grep -E '(SP|wasm|WASM)'"

echo "🎯 发送测试请求的命令:"
echo "kubectl port-forward svc/istio-ingressgateway -n istio-system 18080:80"
echo "curl -H 'Host: ota.local' http://localhost:18080/api/hotels"

echo "✅ 调试日志配置完成!"