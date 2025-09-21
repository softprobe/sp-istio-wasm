# 使用 Go 1.23 或更高版本
FROM golang:1.23-alpine AS builder

# 设置工作目录
WORKDIR /app

# 复制 go mod 和 sum 文件
COPY go.mod go.sum ./

# 下载依赖
RUN go mod download

# 复制源代码
COPY . .

# 构建应用
RUN go build -o main .

# 使用更小的基础镜像
FROM alpine:latest

# 安装 ca-certificates（如果需要 HTTPS 请求）
RUN apk --no-cache add ca-certificates

WORKDIR /root/

# 从 builder 阶段复制构建好的二进制文件
COPY --from=builder /app/main .

# 暴露端口 8081
EXPOSE 80

# 运行应用
CMD ["./main"]
