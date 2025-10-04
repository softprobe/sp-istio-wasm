# SP Istio WASM - 本地部署指南

本指南提供了在本地环境中从零开始部署 SP Istio WASM 演示环境的完整步骤，包括 Kubernetes 集群、Istio 服务网格、SP Istio Agent WASM 插件和 OpenTelemetry 分布式追踪。

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
- 创建 Kind 集群 (`sp-demo-cluster`)
- 安装 Istio 服务网格
- 启用 default namespace 的 Istio 注入
- 启动 Jaeger 追踪服务 (Docker)
- 安装 OpenTelemetry Operator (使用官方最新版本) 和 cert-manager
- 应用 OpenTelemetry Instrumentation 配置
- 创建 Jaeger ServiceEntry

#### 步骤 2：部署演示应用
```bash
# 部署演示应用（demo-ota 和 demo-airline）
./deploy-apps.sh
```

该脚本将部署：
- demo-ota 应用 (Java) - 端口 8080
- demo-airline 应用 (Java) - 端口 8081
- 配置 OpenTelemetry 自动注入
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
- demo-ota 应用：http://localhost:8080/
- demo-airline 应用：http://localhost:8081/
- Jaeger 追踪界面：http://localhost:16686

## 🔧 访问应用

### 方式一：端口转发
```bash
# 转发 demo-ota 应用
kubectl port-forward service/demo-ota 8080:8080

# 转发 demo-airline 应用
kubectl port-forward service/demo-airline 8081:8081
```

然后访问：
- demo-ota: http://localhost:8080/
- demo-airline: http://localhost:8081/

### 方式二：通过 Istio Gateway（如已配置）
如果配置了 Istio Gateway，可以通过 Ingress Gateway 访问应用。

## 📊 监控和追踪

### Jaeger 分布式追踪
访问 Jaeger UI 查看分布式追踪数据：
```bash
# 转发 Jaeger 服务
kubectl port-forward -n istio-system service/jaeger 16686:16686
```

访问：http://localhost:16686

### 验证 OpenTelemetry 注入
```bash
# 检查 demo-ota 的 OpenTelemetry 注解
kubectl get pod -l app=demo-ota -o jsonpath='{.items[0].metadata.annotations}' | grep -i otel

# 检查 demo-airline 的 OpenTelemetry 注解
kubectl get pod -l app=demo-airline -o jsonpath='{.items[0].metadata.annotations}' | grep -i otel
```

## 🧹 清理环境

```bash
# 删除 Kind 集群
kind delete cluster --name istio-demo
```

## 📁 文件结构

```
deploy/dev/
├── cluster-setup.sh          # 集群和基础设施设置脚本
├── deploy-apps.sh           # 演示应用部署脚本
├── readme.md               # 本部署指南
├── auto-instrumentation.yaml  # OpenTelemetry 自动注入配置
├── demo-ota-deployment.yaml   # demo-ota 应用部署配置
├── demo-airline-deployment.yaml # demo-airline 应用部署配置
└── sp-istio-agent-minimal.yaml # SP Istio Agent WASM 插件配置
```

## 🧪 测试分布式追踪

### 生成测试请求
```bash
# 对 demo-ota 应用发送请求
for i in {1..10}; do
  curl -s http://localhost:8080/ > /dev/null
  echo "Request $i sent to demo-ota"
  sleep 1
done

# 对 demo-airline 应用发送请求
for i in {1..10}; do
  curl -s http://localhost:8081/ > /dev/null
  echo "Request $i sent to demo-airline"
  sleep 1
done
```

### 查看追踪数据
1. 访问 Jaeger UI: http://localhost:16686
2. 在 Service 下拉菜单中选择 `demo-ota` 或 `demo-airline`
3. 点击 "Find Traces" 查看追踪数据

### 日志查看

```bash
# 查看应用日志
kubectl logs -l app=demo-ota
kubectl logs -l app=demo-airline

# 查看 Istio 代理日志
kubectl logs <pod-name> -c istio-proxy

# 查看 OpenTelemetry Operator 日志
kubectl logs -n opentelemetry-operator-system -l app.kubernetes.io/name=opentelemetry-operator
```

## 🔧 配置说明

### OpenTelemetry 自动注入配置
<mcfile name="auto-instrumentation.yaml" path="/Users/dongzq/code/softprobe/sp-istio-wasm/deploy/dev/auto-instrumentation.yaml"></mcfile> 文件配置了 OpenTelemetry 的自动注入：

- **Java 应用**: 自动注入 OpenTelemetry Java Agent
- **Python 应用**: 自动注入 OpenTelemetry Python SDK  
- **Node.js 应用**: 自动注入 OpenTelemetry Node.js SDK
- **OTLP 端点**: 配置为 `https://jaeger.softprobe.ai`

### 应用部署配置
演示应用通过以下注解启用 OpenTelemetry 自动注入：
```yaml
annotations:
  instrumentation.opentelemetry.io/inject-java: "true"
```

### SP Istio Agent WASM 插件
<mcfile name="sp-istio-agent-minimal.yaml" path="/Users/dongzq/code/softprobe/sp-istio-wasm/deploy/dev/sp-istio-agent-minimal.yaml"></mcfile> 文件配置了 SP Istio Agent WASM 插件，提供额外的监控和分析功能。

## 🎯 功能特性

- **零代码修改**: 通过 Istio 服务网格和 OpenTelemetry 自动注入实现分布式追踪
- **多语言支持**: 支持 Java、Python、Node.js 等多种编程语言
- **自动注入**: OpenTelemetry 自动注入，无需手动配置应用
- **WASM 扩展**: 使用 SP Istio Agent WASM 插件增强功能
- **可视化追踪**: 通过 Jaeger 查看完整的请求调用链
- **生产就绪**: 基于 Istio 和 OpenTelemetry 的企业级解决方案

### Jaeger 配置

`jaeger-service-entry.yaml` 设置：

- **外部 Jaeger 服务**: 连接到本地 Jaeger 实例
- **端口映射**: HTTP 端口 14268

### OpenTelemetry 配置

`instrumentation.yaml` 包含：

- **OpenTelemetry Instrumentation**: 自动注入 OpenTelemetry SDK 到多种语言的应用
  - **Java**: 支持 Reviews 服务 (Spring Boot)
  - **Node.js**: 支持 Ratings 服务
  - **Python**: 支持 Productpage 服务 (Flask)
  - **Ruby**: 支持 Details 服务
- **Traces 导出器**: 配置为 OTLP HTTP 协议，端点为 `http://host.docker.internal:4318`
- **Metrics 导出器**: 禁用 (`none`)
- **采样器**: 配置为 `parentbased_traceidratio`，采样率 100% 用于测试
- **传播器**: 配置为 `tracecontext,baggage,b3`

#### Endpoint 配置说明

```yaml
spec:
  exporter:
    endpoint: http://host.docker.internal:4318  # 指向本地Docker中的Jaeger
```

- `host.docker.internal`: Docker Desktop 提供的特殊域名，指向宿主机
- `4318`: Jaeger 的 OTLP HTTP 接收端口
- 这个配置允许 Kubernetes 集群中的应用将追踪数据发送到宿主机上运行的 Jaeger

#### 自动注入工作原理

OpenTelemetry Operator 通过以下方式实现自动注入：

1. **Webhook 拦截**: 拦截 Pod 创建请求
2. **语言检测**: 根据 Pod 注解自动检测应用语言
3. **SDK 注入**: 自动添加相应语言的 OpenTelemetry SDK
4. **环境变量**: 自动配置 OTEL_* 环境变量
5. **容器修改**: 修改容器启动命令以加载 OpenTelemetry Agent

该配置确保：
- 所有 Bookinfo 服务自动生成分布式追踪数据
- 追踪数据通过 HTTP/protobuf 协议发送到 Jaeger
- 支持标准的 W3C Trace Context 和 B3 传播
- 无需修改应用代码即可实现分布式追踪

## 🔍 故障排除

### 常见问题

1. **Pod 无法启动**
   ```bash
   kubectl describe pod <pod-name>
   kubectl logs <pod-name>
   ```

2. **OpenTelemetry 注入失败**
   ```bash
   # 检查 OpenTelemetry Operator 状态
   kubectl get pods -n opentelemetry-operator-system
   
   # 检查 Instrumentation 资源
   kubectl get instrumentation -A
   ```

3. **Istio 注入问题**
   ```bash
   # 检查命名空间标签
   kubectl get namespace default --show-labels
   
   # 检查 Istio 代理状态
   kubectl get pods -o wide
   ```

4. **追踪数据未显示**
   - 确认 OpenTelemetry 自动注入已启用
   - 检查应用日志中的追踪相关信息
   - 验证 Jaeger 服务正常运行

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

当你完成测试或需要重新开始时，可以使用清理脚本：

```bash
# 清理所有资源和集群
./cleanup.sh
```

清理脚本会按顺序执行以下操作：

1. **停止端口转发**: 终止所有 kubectl port-forward 进程
2. **清理 WASM 插件**: 删除 SP Istio Agent WASM 插件和相关配置
3. **清理 Bookinfo 应用**: 删除 Bookinfo 应用和 Gateway 配置
4. **清理 OpenTelemetry**: 删除 Instrumentation 配置和 Jaeger ServiceEntry
5. **清理 OpenTelemetry Operator**: 卸载 OpenTelemetry Operator
6. **清理 cert-manager**: 卸载 cert-manager
7. **清理 Istio**: 完全卸载 Istio 和相关 namespace
8. **停止 Jaeger**: 停止并删除本地 Jaeger Docker 容器
9. **删除集群**: 删除 Kind 集群
10. **清理镜像** (可选): 清理相关的 Docker 镜像

### 清理选项

脚本会询问是否清理 Docker 镜像，包括：
- Jaeger 镜像
- OpenTelemetry 相关镜像  
- Istio 相关镜像
- Kind 节点镜像

### 安全特性

- 使用 `safe_execute` 函数，即使某些资源不存在也不会报错
- 每个步骤都有清晰的状态提示
- 支持部分清理，不会因为单个步骤失败而中断整个过程

## 📚 参考资料

- [Istio Bookinfo 示例](https://istio.io/latest/docs/examples/bookinfo/)
- [Istio 官方文档](https://istio.io/latest/docs/)
- [OpenTelemetry 文档](https://opentelemetry.io/docs/)
- [OpenTelemetry Operator 文档](https://opentelemetry.io/docs/kubernetes/operator/)
- [OpenTelemetry 自动注入配置](https://opentelemetry.io/docs/kubernetes/operator/automatic/)
- [Jaeger 文档](https://www.jaegertracing.io/docs/)
- [Kind 文档](https://kind.sigs.k8s.io/docs/)

### 重要配置文件

- **OpenTelemetry Operator**: `https://github.com/open-telemetry/opentelemetry-operator/releases/latest/download/opentelemetry-operator.yaml`
- **cert-manager**: `https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml`
- **Instrumentation 配置**: `instrumentation.yaml` (本地文件)
- **Jaeger ServiceEntry**: `jaeger-service-entry.yaml` (本地文件)

## 🤝 支持

如遇问题，请检查：
1. 所有依赖工具是否正确安装
2. Docker Desktop 是否正在运行
3. 网络连接是否正常
4. 端口是否被其他进程占用
5. WASM 插件是否正确加载