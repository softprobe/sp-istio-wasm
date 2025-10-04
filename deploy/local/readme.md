# Istio Bookinfo - 本地部署指南

本指南提供了在本地环境中从零开始部署 Istio Bookinfo 示例应用的完整步骤，包括 Kubernetes 集群、Istio 服务网格、SP Istio Agent WASM 插件和 OpenTelemetry 分布式追踪。

## 📋 环境要求

- **操作系统**: macOS
- **工具依赖**:
  - [Docker Desktop](https://www.docker.com/products/docker-desktop)
  - [Kind](https://kind.sigs.k8s.io/) - Kubernetes in Docker
  - [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl-macos/)
  - [Istio CLI](https://istio.io/latest/docs/setup/getting-started/#download)

### 安装依赖工具

```bash
# 安装 Homebrew (如果未安装)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 安装 Kind
brew install kind

# 安装 kubectl
brew install kubectl

# 安装 Istio CLI
curl -L https://istio.io/downloadIstio | sh -
export PATH=$PWD/istio-*/bin:$PATH
```

## 🚀 快速开始

### 1. 环境准备

确保已安装以下工具：
- Docker Desktop
- kubectl
- kind
- istioctl

### 2. 部署步骤

#### 步骤 1：设置基础环境
```bash
# 创建 Kind 集群并安装 Istio
./cluster-setup.sh
```

该脚本将自动完成：
- 创建 Kind 集群 (`istio-testing`)
- 安装 Istio
- 启用 Istio 注入
- 启动 Jaeger
- 创建 Jaeger ServiceEntry

#### 步骤 2：部署 Bookinfo 应用
```bash
# 部署 Bookinfo 示例应用
./deploy-apps.sh
```

该脚本将部署：
- Istio Bookinfo 示例应用 (productpage, details, ratings, reviews)
- Bookinfo Gateway 和 VirtualService
- 等待所有 Pod 就绪

#### 步骤 3：安装 WASM 插件
```bash
# 安装 SP Istio Agent WASM 插件
./install-wasm.sh
```

#### 步骤 4：启动端口转发
```bash
# 启动端口转发服务
./start-port-forward.sh
```

或手动启动：

```bash
kubectl port-forward -n istio-system svc/istio-ingressgateway 8080:80
```

#### 步骤 5：访问应用
- Bookinfo 应用：http://localhost:8080/productpage
- Jaeger 追踪界面：http://localhost:16686

## 📁 文件结构

```
deploy/local/
├── cluster-setup.sh              # 基础环境设置脚本
├── deploy-apps.sh               # Bookinfo 应用部署脚本
├── install-wasm.sh              # WASM 插件安装脚本
├── start-port-forward.sh        # 端口转发脚本
├── cleanup.sh                   # 环境清理脚本
├── sp-istio-agent-minimal.yaml  # SP Istio Agent WASM 插件配置
├── jaeger-service-entry.yaml    # Jaeger ServiceEntry 配置
└── readme.md                    # 本文档
```

### 脚本说明

- **cluster-setup.sh**: 创建 Kind 集群，安装 Istio，启动 Jaeger
- **deploy-apps.sh**: 部署 Bookinfo 应用和 Gateway 配置
- **install-wasm.sh**: 安装 SP Istio Agent WASM 插件并重启应用
- **start-port-forward.sh**: 启动端口转发服务
- **cleanup.sh**: 清理所有资源和集群

## 🧪 测试分布式追踪

### 发送测试请求

```bash
# 访问 Bookinfo 应用主页
curl http://localhost:8080/productpage

# 发送带追踪头的请求
curl -H "traceparent: 00-$(openssl rand -hex 16)-$(openssl rand -hex 8)-01" \
     -H "x-request-id: test-$(date +%s)" \
     http://localhost:8080/productpage

# 发送多个请求进行测试
for i in {1..5}; do
  curl -H "traceparent: 00-$(openssl rand -hex 16)-$(openssl rand -hex 8)-01" \
       -H "x-request-id: test-$i-$(date +%s)" \
       http://localhost:8080/productpage
  sleep 1
done
```

### 查看追踪数据

1. 访问 Jaeger UI: http://localhost:16686
2. 在服务下拉菜单中选择 `productpage.default` 或其他 Bookinfo 服务
3. 点击 "Find Traces" 查看追踪数据

## 🔧 配置说明

### SP Istio Agent WASM 插件

`sp-istio-agent-minimal.yaml` 包含：

- **WasmPlugin 配置**: 配置 SP Istio Agent WASM 插件
- **ServiceEntry 配置**: 配置 SoftProbe 后端服务入口

### Jaeger 配置

`jaeger-service-entry.yaml` 设置：

- **外部 Jaeger 服务**: 连接到本地 Jaeger 实例
- **端口映射**: HTTP 端口 14268

## 🐛 故障排除

### 常见问题

1. **端口冲突**
   ```bash
   # 检查端口占用
   lsof -i :8080
   lsof -i :16686
   
   # 停止端口转发
   pkill -f "kubectl port-forward"
   ```

2. **Pod 未就绪**
   ```bash
   # 检查 Pod 状态
   kubectl get pods
   kubectl describe pod <pod-name>
   kubectl logs <pod-name> -c istio-proxy
   ```

3. **追踪数据缺失**
   ```bash
   # 检查 Istio 配置
   kubectl get configmap istio -n istio-system -o yaml
   
   # 检查 WASM 插件
   kubectl get wasmplugin -A
   
   # 检查 Envoy 配置
   kubectl exec <pod-name> -c istio-proxy -- curl localhost:15000/config_dump
   ```

4. **WASM 插件问题**
   ```bash
   # 检查 WASM 插件状态
   kubectl get wasmplugin -n istio-system sp-istio-agent -o yaml
   
   # 查看 Envoy 日志
   kubectl logs <pod-name> -c istio-proxy
   ```

### 日志查看

```bash
# Bookinfo 应用日志
kubectl logs -l app=productpage
kubectl logs -l app=details
kubectl logs -l app=ratings
kubectl logs -l app=reviews

# Istio 控制平面日志
kubectl logs -n istio-system -l app=istiod

# Envoy 代理日志
kubectl logs <pod-name> -c istio-proxy
```

## 🧹 清理环境

```bash
# 停止端口转发
pkill -f "kubectl port-forward"

# 删除 Kind 集群
kind delete cluster --name istio-testing

# 停止 Jaeger
docker stop jaeger
docker rm jaeger
```

## 📚 参考资料

- [Istio Bookinfo 示例](https://istio.io/latest/docs/examples/bookinfo/)
- [Istio 官方文档](https://istio.io/latest/docs/)
- [OpenTelemetry 文档](https://opentelemetry.io/docs/)
- [Jaeger 文档](https://www.jaegertracing.io/docs/)
- [Kind 文档](https://kind.sigs.k8s.io/docs/)

## 🤝 支持

如遇问题，请检查：
1. 所有依赖工具是否正确安装
2. Docker Desktop 是否正在运行
3. 网络连接是否正常
4. 端口是否被其他进程占用
5. WASM 插件是否正确加载