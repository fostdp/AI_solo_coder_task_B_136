
# ========================================
# Stage 1: Node Builder
# 前端构建：Node + Vite
# ========================================
FROM node:20-alpine AS builder

WORKDIR /app

# 先复制依赖描述
COPY package.json package-lock.json* ./

# 安装依赖（使用国内镜像源加速）
RUN npm config set registry https://registry.npmmirror.com && \
    npm ci || npm install --no-audit --no-fund

# 复制源代码
COPY index.html vite.config.js ./
COPY src/ ./src/

# 构建生产版本
RUN npm run build

# ========================================
# Stage 2: Nginx Runtime
# 轻量级Nginx托管前端静态资源 + Gzip压缩
# ========================================
FROM nginx:1.27-alpine AS runtime

LABEL maintainer="Military History Research Team"
LABEL description="古代临冲吕公车结构仿真与稳定性分析系统 - 前端静态资源服务"
LABEL version="2.0.0"

# 时区设置
ENV TZ=Asia/Shanghai
RUN apk add --no-cache tzdata curl && \
    cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
    echo "Asia/Shanghai" > /etc/timezone && \
    apk del tzdata

# 复制Nginx配置（含Gzip压缩、API反向代理）
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY nginx-gzip.conf /etc/nginx/conf-available/gzip.conf

# 确保nginx加载gzip配置
RUN mkdir -p /etc/nginx/snippets && \
    cp /etc/nginx/conf-available/gzip.conf /etc/nginx/snippets/gzip.conf

# 复制构建产物
COPY --from=builder /app/dist/ /usr/share/nginx/html/

# 健康检查
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://localhost/ || exit 1

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
