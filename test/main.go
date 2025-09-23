package main

import (
	"context"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"

	"go.opentelemetry.io/contrib/instrumentation/net/http/otelhttp"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/exporters/otlp/otlptrace"
	"go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracehttp"
	"go.opentelemetry.io/otel/propagation"
	"go.opentelemetry.io/otel/sdk/resource"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	semconv "go.opentelemetry.io/otel/semconv/v1.4.0"
	"go.opentelemetry.io/otel/trace"
)

// 初始化OpenTelemetry
func initTracer() *sdktrace.TracerProvider {
	// 创建OTLP HTTP客户端
	client := otlptracehttp.NewClient(
		otlptracehttp.WithEndpoint(os.Getenv("OTEL_EXPORTER_OTLP_ENDPOINT")),
		otlptracehttp.WithInsecure(),
	)

	// 创建OTLP trace exporter
	exporter, err := otlptrace.New(context.Background(), client)
	if err != nil {
		log.Fatal(err)
	}

	// 创建资源
	res, err := resource.New(context.Background(),
		resource.WithAttributes(
			semconv.ServiceNameKey.String("my-app"),
		),
	)
	if err != nil {
		log.Fatal(err)
	}

	// 创建TracerProvider
	tp := sdktrace.NewTracerProvider(
		sdktrace.WithBatcher(exporter),
		sdktrace.WithResource(res),
	)

	otel.SetTracerProvider(tp)
	otel.SetTextMapPropagator(propagation.NewCompositeTextMapPropagator(propagation.TraceContext{}, propagation.Baggage{}))

	return tp
}

func handler(w http.ResponseWriter, r *http.Request) {
	// 添加方法验证
	if r.Method != http.MethodPost {
		w.WriteHeader(http.StatusMethodNotAllowed)
		w.Write([]byte("只支持 POST 方法"))
		return
	}
	// 在控制台打印请求的header
	fmt.Println("Request Headers:")
	for name, values := range r.Header {
		for _, value := range values {
			fmt.Printf("  %s: %s\n", name, value)
		}
	}
	// 设置响应头类型（可选但推荐）
	w.Header().Set("Content-Type", "text/plain")

	fmt.Fprintln(w, "Hello, World!")
}

// 处理延迟路由的函数
func delayHandler(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	tracer := otel.Tracer("my-app")
	_, span := tracer.Start(ctx, "delayHandler")
	defer span.End()

	// 解析路径以获取延迟时间
	pathParts := strings.Split(r.URL.Path, "/")
	if len(pathParts) < 3 {
		span.RecordError(fmt.Errorf("invalid path"))
		http.Error(w, "Invalid path", http.StatusBadRequest)
		return
	}

	delay, err := strconv.Atoi(pathParts[2])
	if err != nil || delay < 1 || delay > 3 {
		span.RecordError(fmt.Errorf("invalid delay value: %d", delay))
		http.Error(w, "Invalid delay value", http.StatusBadRequest)
		return
	}

	// 添加事件记录
	span.AddEvent("Processing delay request", trace.WithAttributes(
		semconv.HTTPRouteKey.String(r.URL.Path),
		attribute.Int("delay", delay),
	))

	// 构建目标URL
	targetURL := fmt.Sprintf("http://httpbin.org/delay/%d", delay)

	// 添加HTTP请求事件
	span.AddEvent("Making HTTP request to httpbin", trace.WithAttributes(
		semconv.HTTPURLKey.String(targetURL),
	))

	// 创建带trace context的HTTP请求
	client := &http.Client{Timeout: 10 * time.Second}
	req, err := http.NewRequestWithContext(ctx, "GET", targetURL, nil)
	if err != nil {
		span.RecordError(err)
		http.Error(w, "Failed to create request", http.StatusInternalServerError)
		return
	}

	// 注入trace context到HTTP请求头
	otel.GetTextMapPropagator().Inject(ctx, propagation.HeaderCarrier(req.Header))

	// 发起HTTP GET请求到httpbin
	resp, err := client.Do(req)
	if err != nil {
		span.RecordError(err)
		http.Error(w, "Failed to fetch from httpbin", http.StatusInternalServerError)
		return
	}
	defer resp.Body.Close()

	// 将响应头复制到我们的响应中
	for key, values := range resp.Header {
		for _, value := range values {
			w.Header().Add(key, value)
		}
	}

	// 设置状态码
	w.WriteHeader(resp.StatusCode)

	// 将响应体复制到我们的响应中
	_, err = io.Copy(w, resp.Body)
	if err != nil {
		span.RecordError(err)
		// 如果已经写入了响应头，我们无法再发送错误信息
		return
	}

	span.AddEvent("Successfully completed request")
}

func main() {
	// 初始化OpenTelemetry
	tp := initTracer()
	defer func() {
		if err := tp.Shutdown(context.Background()); err != nil {
			log.Printf("Error shutting down tracer provider: %v", err)
		}
	}()

	// 使用otelhttp包装处理程序以自动添加HTTP指标和追踪
	http.HandleFunc("/", otelhttp.NewHandler(http.HandlerFunc(handler), "root").ServeHTTP)
	http.HandleFunc("/test/1", otelhttp.NewHandler(http.HandlerFunc(delayHandler), "delay-1").ServeHTTP)
	http.HandleFunc("/test/2", otelhttp.NewHandler(http.HandlerFunc(delayHandler), "delay-2").ServeHTTP)
	http.HandleFunc("/test/3", otelhttp.NewHandler(http.HandlerFunc(delayHandler), "delay-3").ServeHTTP)

	// 启动服务
	log.Println("Starting server on :80")
	http.ListenAndServe(":80", nil)
}
