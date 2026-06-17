
FROM python:3.12-slim-bookworm

LABEL maintainer="Military History Research Team"
LABEL description="古代临冲吕公车结构仿真与稳定性分析系统 - 传感器模拟器"
LABEL version="2.0.0"

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    TZ=Asia/Shanghai

WORKDIR /app

# 安装时区
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates tzdata curl && \
    cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
    echo "Asia/Shanghai" > /etc/timezone && \
    rm -rf /var/lib/apt/lists/*

# 先复制依赖描述并安装，充分利用缓存
COPY requirements.txt .
RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir -r requirements.txt || \
    pip install --no-cache-dir paho-mqtt

# 复制源代码
COPY sensor_simulator.py .

# 创建日志目录
RUN mkdir -p /app/logs

# 健康检查（检查Python进程是否存活）
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD python -c "import urllib.request,sys; urllib.request.urlopen('http://backend:8080/api/health', timeout=3).read(); sys.exit(0)" 2>/dev/null \
        || (pgrep -f sensor_simulator.py > /dev/null && exit 0 || exit 1)

ENTRYPOINT ["python", "-u", "sensor_simulator.py"]
CMD []
