# SP Istio WASM 插件 - 本地部署指南

**中文** | [English](readme.md)

SP Istio WASM 插件是一个基于 Istio 服务网格的分布式追踪增强插件，可以在不修改应用代码的情况下，为现有的 OpenTelemetry 追踪数据添加更丰富的服务网格层面的监控信息。

## 🚀 快速开始

### 环境要求

- **操作系统**: macOS
- **必需工具**:
  - [Docker Desktop](https://www.docker.com/products/docker-desktop)
  - [Kind](https://kind.sigs.k8s.io/) - `brew install kind`
  - [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl-macos/) - `brew install kubectl`
  - [Istio CLI](https://istio.io/latest/docs/setup/getting-started/#download) - `curl -L https://istio.io/downloadIstio | sh -`

### 一键部署步骤

#### 步骤 1：创建基础环境
```bash
./cluster-setup.sh
```
**作用**: 创建 Kind 集群，安装 Istio 服务网格，启动 Jaeger 追踪服务，安装 OpenTelemetry Operator

#### 步骤 2：部署演示应用
```bash
./deploy-apps.sh
```
**作用**: 部署 demo-ota 和 demo-airline 两个 Java 应用，配置 OpenTelemetry 自动注入

#### 步骤 3：安装 SP Istio WASM 插件
```bash
./install-wasm.sh
```
**作用**: 安装 SP Istio Agent WASM 插件，为服务网格添加增强的监控能力

#### 步骤 4：启动端口转发
```bash
./start-port-forward.sh
```
**作用**: 启动端口转发，使本地可以访问集群内的应用和 Jaeger UI

## 🎯 查看效果

### 访问应用
- **demo-ota 应用**: http://localhost:8080/
- **demo-airline 应用**: http://localhost:8081/
- **Jaeger 追踪界面**: https://jaeger.softprobe.ai/

### 测试分布式追踪
```bash
# 发送测试请求到 demo-ota
curl -X POST http://localhost:8080/api/flights/search \
  -H "Content-Type: application/json" \
  -d '{
    "fromCity": "New York",
    "toCity": "Los Angeles",
    "departureDate": "2025-09-30",
    "tripType": "ONE_WAY",
    "cabinClass": "ECONOMY",
    "passengerInfo": {
        "adults": 1
    }
  }'

# 发送测试请求到 demo-airline
curl http://localhost:8081/api/flights
```

### 在 Jaeger 中查看追踪数据
1. 访问 https://jaeger.softprobe.ai/
2. 在 Service 下拉菜单中选择 `demo-ota` 或 `demo-airline`
3. 点击 "Find Traces" 查看追踪数据

## 🧹 清理环境
```bash
./cleanup.sh
```
**作用**: 完全清理所有资源，包括集群、容器和镜像

## 📊 工作原理

### SP Istio WASM 插件的作用

SP Istio WASM 插件在 Istio 服务网格的 Envoy 代理中运行，为每个 HTTP 请求添加额外的监控信息，包括：

- **服务名称检测**: 自动从环境变量中检测服务名称
- **请求头注入**: 添加服务标识相关的 HTTP 头部
- **追踪增强**: 为现有的 OpenTelemetry 追踪添加服务网格层面的信息

### 对现有 OpenTelemetry 的影响

**重要**: SP Istio WASM 插件**完全不影响**现有的 OpenTelemetry 配置和数据收集。它只是在服务网格层面添加额外的信息，与应用层的 OpenTelemetry 追踪并行工作。

### 追踪数据对比

#### 安装插件前的追踪树
```
demo-ota
└── HTTP GET /api/flights/search
    ├── Business Logic Processing
    └── Database Query
```

#### 安装插件后的追踪树
```
demo-ota
├── [Istio Ingress] HTTP Request (新增)
│   ├── Service: demo-ota
│   ├── Headers: x-sp-service-name, x-service-name
│   └── Envoy Proxy Processing (新增)
└── HTTP GET /api/flights/search (原有)
    ├── Business Logic Processing (原有)
    ├── [Istio Egress] Outbound Request (新增)
    │   ├── Target Service: demo-airline
    │   └── Service Mesh Routing (新增)
    └── Database Query (原有)
```

#### 新增的追踪信息

1. **Envoy 代理层追踪**:
   - 入站请求处理 (SIDECAR_INBOUND)
   - 出站请求处理 (SIDECAR_OUTBOUND)
   - 服务网格路由信息

2. **服务标识信息**:
   - 自动检测的服务名称
   - 服务间调用关系
   - 网格内的流量路径

3. **增强的元数据**:
   - Pod 信息 (hostname, namespace)
   - 服务账户信息
   - Istio 代理版本和配置

### 数据流向图

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Client        │    │   demo-ota       │    │   demo-airline  │
│                 │    │                  │    │                 │
│                 │───▶│ [Envoy Proxy]    │───▶│ [Envoy Proxy]   │
│                 │    │ ├─ WASM Plugin   │    │ ├─ WASM Plugin  │
│                 │    │ └─ OTel Agent    │    │ └─ OTel Agent   │
│                 │    │                  │    │                 │
│                 │    │ [Application]    │    │ [Application]   │
│                 │    │ └─ OTel SDK      │    │ └─ OTel SDK     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                │                        │
                                ▼                        ▼
                       ┌─────────────────────────────────────┐
                       │         Jaeger Backend              │
                       │                                     │
                       │ ┌─────────────┐ ┌─────────────────┐ │
                       │ │ Istio Mesh  │ │ Application     │ │
                       │ │ Traces      │ │ Traces          │ │
                       │ │ (新增)      │ │ (原有)          │ │
                       │ └─────────────┘ └─────────────────┘ │
                       └─────────────────────────────────────┘
```

### 核心优势

1. **零侵入性**: 不需要修改应用代码或现有的 OpenTelemetry 配置
2. **增强可观测性**: 在服务网格层面提供额外的监控维度
3. **完整追踪链路**: 结合应用层和网格层的追踪数据，提供完整的请求生命周期视图
4. **自动服务发现**: 自动识别和标记服务，无需手动配置

## 📁 项目结构

```
deploy/dev/
├── cluster-setup.sh              # 集群和基础设施设置
├── deploy-apps.sh               # 演示应用部署
├── install-wasm.sh              # WASM 插件安装
├── start-port-forward.sh        # 端口转发启动
├── cleanup.sh                   # 环境清理
├── auto-instrumentation.yaml   # OpenTelemetry 自动注入配置
├── demo-apps-deployment.yaml   # 演示应用部署配置
└── sp-istio-agent-minimal.yaml # SP Istio Agent WASM 插件配置
```

## 🔍 故障排除

### 检查插件状态
```bash
# 检查 WASM 插件是否正确加载
kubectl get wasmplugin -n istio-system

# 检查 Envoy 配置
kubectl exec <pod-name> -c istio-proxy -- curl localhost:15000/config_dump
```

### 查看日志
```bash
# 查看应用日志
kubectl logs -l app=demo-ota
kubectl logs -l app=demo-airline

# 查看 Istio 代理日志
kubectl logs <pod-name> -c istio-proxy
```

## 📚 技术说明

- **WASM 插件**: 基于 WebAssembly 技术，在 Envoy 代理中运行
- **Istio 集成**: 利用 Istio 的 WasmPlugin CRD 进行配置和部署
- **OpenTelemetry 兼容**: 与标准的 OpenTelemetry 生态系统完全兼容
- **高性能**: WASM 运行时提供接近原生的性能表现