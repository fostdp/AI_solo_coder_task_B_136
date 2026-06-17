
# ========================================
# Stage 1: Rust Builder
# 多阶段构建：编译静态二进制
# ========================================
FROM rust:1.78-bookworm AS builder

WORKDIR /app

# 先复制依赖描述文件，利用Docker缓存
COPY Cargo.toml Cargo.lock ./
COPY config/ ./config/

# 创建空的src以预构建依赖（减少重复编译）
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "" > src/lib.rs && \
    cargo build --release 2>&1 || true

# 复制真实源代码
COPY src/ ./src/

# 构建release版本，静态链接musl以减少依赖
RUN apt-get update && \
    apt-get install -y --no-install-recommends musl-tools && \
    rustup target add x86_64-unknown-linux-musl && \
    cargo build --release --target x86_64-unknown-linux-musl 2>&1 || \
    cargo build --release 2>&1

# 查找并复制二进制到固定位置
RUN if [ -f target/x86_64-unknown-linux-musl/release/siege-tower-server ]; then \
      cp target/x86_64-unknown-linux-musl/release/siege-tower-server /app/server; \
    else \
      cp target/release/siege-tower-server /app/server; \
    fi

# ========================================
# Stage 2: Runtime Image
# 轻量级运行时镜像
# ========================================
FROM debian:bookworm-slim AS runtime

LABEL maintainer="Military History Research Team"
LABEL description="古代临冲吕公车结构仿真与稳定性分析系统 - 后端服务"
LABEL version="2.0.0"

# 安装必要的运行时库和ca-certificates
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        tzdata \
        libssl3 \
        openssl && \
    rm -rf /var/lib/apt/lists/* && \
    cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
    echo "Asia/Shanghai" > /etc/timezone

WORKDIR /app

# 创建非root用户
RUN groupadd -r appuser && \
    useradd -r -g appuser -m -d /app -s /usr/sbin/nologin appuser && \
    mkdir -p /app/config /app/logs && \
    chown -R appuser:appuser /app

# 复制二进制和配置文件
COPY --from=builder /app/server /app/server
COPY --from=builder /app/config/ /app/config/

ENV RUST_BACKTRACE=1
ENV RUST_LOG=siege_tower_backend=info,tower_http=info
ENV TZ=Asia/Shanghai

# 健康检查
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -fsS http://localhost:8080/api/health || exit 1

EXPOSE 8080 9090

USER appuser

ENTRYPOINT ["/app/server"]
