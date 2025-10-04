# Demo Air - 本地部署指南

本指南提供了在本地环境中从零开始部署 Demo Air 应用的完整步骤，包括 Kubernetes 集群、Istio 服务网格和 OpenTelemetry 分布式追踪。

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

### 1. 设置集群和 Istio

运行集群设置脚本，自动完成所有基础配置：

```bash
./cluster-setup.sh
```

该脚本将自动完成：
- 创建 Kind 集群
- 安装 Istio
- 启用 Istio 注入
- 启动 Jaeger
- 配置网格和追踪

### 2. 部署应用

运行应用部署脚本：

```bash
./deploy-apps.sh
```

该脚本将部署：
- demo-ota 服务
- demo-airline 服务
- Istio Gateway

### 3. 启动端口转发

```bash
./start-port-forward.sh
```

### 4. 验证部署

访问以下地址验证部署：

- **应用服务**: http://localhost:8080
- **Jaeger UI**: http://localhost:16686

## 📁 文件结构

```
local-setup/
├── cluster-setup.sh              # 集群和 Istio 设置脚本
├── deploy-apps.sh               # 应用部署脚本
├── start-port-forward.sh        # 端口转发脚本
├── istio-mesh-config.yaml       # Istio 网格配置
├── default-telemetry-config.yaml # 默认遥测配置
├── jaeger-service-entry.yaml    # Jaeger 服务入口
├── demo-ota-deployment.yaml     # OTA 服务部署配置
├── demo-airline-deployment.yaml # 航空公司服务部署配置
├── demo-istio-gateway.yaml      # Istio 网关配置
└── README.md                    # 本文档
```

## 🧪 测试分布式追踪

### 发送测试请求

```bash
# 发送带追踪头的请求
curl -H "traceparent: 00-$(openssl rand -hex 16)-$(openssl rand -hex 8)-01" \
     -H "x-request-id: test-$(date +%s)" \
     http://localhost:8080/

# 发送多个请求进行测试
for i in {1..5}; do
  curl -H "traceparent: 00-$(openssl rand -hex 16)-$(openssl rand -hex 8)-01" \
       -H "x-request-id: test-$i-$(date +%s)" \
       http://localhost:8080/
  sleep 1
done
```

### 查看追踪数据

1. 访问 Jaeger UI: http://localhost:16686
2. 在服务下拉菜单中选择 `demo-ota.default`
3. 点击 "Find Traces" 查看追踪数据

## 🔧 配置说明

### Istio 网格配置

`istio-mesh-config.yaml` 包含：

- **OpenTelemetry 追踪配置**: 连接到本地 Jaeger
- **代理统计过滤**: 减少不必要的指标
- **扩展提供者**: 配置 OTLP HTTP 导出

### 遥测配置

`default-telemetry-config.yaml` 设置：

- **采样率**: 100% (开发环境)
- **追踪提供者**: otel-tracing

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
   
   # 检查 Envoy 配置
   kubectl exec <pod-name> -c istio-proxy -- curl localhost:15000/config_dump
   ```

### 日志查看

```bash
# 应用日志
kubectl logs -l app=demo-ota
kubectl logs -l app=demo-airline

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
kind delete cluster --name sp-demo-cluster

# 停止 Jaeger
docker stop jaeger
docker rm jaeger
```

## 📚 参考资料

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