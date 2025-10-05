#!/bin/bash

set -e

echo "🚀 SP-Istio Agent 部署和重启脚本"
echo "================================"

# 函数：打印彩色输出
print_status() {
    echo -e "\033[1;34m[INFO]\033[0m $1"
}

print_success() {
    echo -e "\033[1;32m[SUCCESS]\033[0m $1"
}

print_error() {
    echo -e "\033[1;31m[ERROR]\033[0m $1"
}

# 检查 kubectl 是否可用
if ! command -v kubectl &> /dev/null; then
    print_error "kubectl 未找到，请先安装 kubectl"
    exit 1
fi

# 检查集群连接
if ! kubectl cluster-info &> /dev/null; then
    print_error "无法连接到 Kubernetes 集群，请检查 kubeconfig"
    exit 1
fi

# 第一步：构建 WASM 模块并获取 SHA256
print_status "第一步：构建 WASM 模块..."
./build.sh

if [ $? -ne 0 ]; then
    print_error "WASM 模块构建失败"
    exit 1
fi

# 获取新的 SHA256 值
if [ -f "target/wasm32-unknown-unknown/release/sp_istio_agent.wasm.sha256" ]; then
    NEW_SHA256=$(cat target/wasm32-unknown-unknown/release/sp_istio_agent.wasm.sha256)
    print_success "获取到新的 SHA256: $NEW_SHA256"
    
    # 更新配置文件中的 SHA256
    print_status "更新配置文件中的 SHA256..."
    sed -i.bak "s/sha256: .*/sha256: $NEW_SHA256/" /Users/dongzq/code/softprobe/sp-istio-wasm/deploy/dev/sp-istio-agent-minimal.yaml
    print_success "SHA256 已更新到配置文件"
else
    print_error "未找到 SHA256 文件"
    exit 1
fi

# 第二步：应用 WASM 插件配置
print_status "第二步：应用 SP-Istio Agent WASM 插件配置..."
kubectl apply -f /Users/dongzq/code/softprobe/sp-istio-wasm/deploy/dev/sp-istio-agent-minimal.yaml

if [ $? -eq 0 ]; then
    print_success "WASM 插件配置应用成功"
else
    print_error "WASM 插件配置应用失败"
    exit 1
fi

# 等待配置生效
print_status "等待配置生效..."
sleep 5

# 第三步：删除 airline 相关的 pod
print_status "第三步：删除 airline 相关的 pod..."

# 查找所有包含 airline 的 pod
AIRLINE_PODS=$(kubectl get pods --all-namespaces -o jsonpath='{range .items[*]}{.metadata.namespace}{" "}{.metadata.name}{"\n"}{end}' | grep -i airline || true)

if [ -z "$AIRLINE_PODS" ]; then
    print_status "未找到 airline 相关的 pod"
else
    print_status "找到以下 airline 相关的 pod："
    echo "$AIRLINE_PODS"
    
    # 删除找到的 pod
    echo "$AIRLINE_PODS" | while read namespace pod; do
        if [ -n "$namespace" ] && [ -n "$pod" ]; then
            print_status "删除 pod: $namespace/$pod"
            kubectl delete pod "$pod" -n "$namespace"
            if [ $? -eq 0 ]; then
                print_success "成功删除 pod: $namespace/$pod"
            else
                print_error "删除 pod 失败: $namespace/$pod"
            fi
        fi
    done
fi

# 等待 pod 重启
print_status "等待 pod 重启..."
sleep 10

# 检查部署状态
print_status "检查 WASM 插件状态..."
kubectl get wasmplugin -n istio-system sp-istio-agent-server
kubectl get wasmplugin -n istio-system sp-istio-agent-client

# 第四步：删除 ota 相关的 pod
print_status "第四步：删除 ota 相关的 pod..."

# 查找所有包含 ota 的 pod
OTA_PODS=$(kubectl get pods --all-namespaces -o jsonpath='{range .items[*]}{.metadata.namespace}{" "}{.metadata.name}{"\n"}{end}' | grep -i ota || true)

if [ -z "$OTA_PODS" ]; then
    print_status "未找到 ota 相关的 pod"
else
    print_status "找到以下 ota 相关的 pod："
    echo "$OTA_PODS"
    
    # 删除找到的 pod
    echo "$OTA_PODS" | while read namespace pod; do
        if [ -n "$namespace" ] && [ -n "$pod" ]; then
            print_status "删除 pod: $namespace/$pod"
            kubectl delete pod "$pod" -n "$namespace"
            if [ $? -eq 0 ]; then
                print_success "成功删除 pod: $namespace/$pod"
            else
                print_error "删除 pod 失败: $namespace/$pod"
            fi
        fi
    done
fi

# 等待 ota pod 重启
print_status "等待 ota pod 重启..."
sleep 10

# 第五步：查看 airline pod 的 istio-proxy 日志
print_status "第五步：查看 airline pod 的 istio-proxy 日志..."

# 查找第一个 airline pod
AIRLINE_POD=$(kubectl get pods --all-namespaces -o jsonpath='{range .items[*]}{.metadata.namespace}{" "}{.metadata.name}{"\n"}{end}' | grep -i airline | head -1)

if [ -n "$AIRLINE_POD" ]; then
    read namespace pod <<< "$AIRLINE_POD"
    print_status "开始查看 pod $namespace/$pod 的 istio-proxy 日志..."
    print_status "按 Ctrl+C 退出日志查看"
    echo ""
    kubectl logs -f "$pod" -n "$namespace" -c istio-proxy
else
    print_error "未找到 airline pod，无法查看日志"
fi

print_success "脚本执行完成！"