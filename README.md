
# 古代临冲吕公车 · 结构仿真与稳定性分析系统 v2.0

> **军事史研究数字化工程** · 还原明代临冲吕公车的结构力学行为，实现实时结构健康监测。

---

## 目录

- [系统架构](#系统架构)
- [技术栈](#技术栈)
- [模块拆分说明](#模块拆分说明)
- [快速部署](#快速部署)
  - [Docker Compose 一键部署（推荐）](#docker-compose-一键部署推荐)
  - [本地开发部署](#本地开发部署)
- [传感器模拟器使用](#传感器模拟器使用)
  - [风荷载模式](#风荷载模式)
  - [土壤条件配置](#土壤条件配置)
  - [故障注入](#故障注入)
  - [场景示例](#场景示例)
- [API 参考](#api-参考)
- [Prometheus 指标采集](#prometheus-指标采集)
- [ClickHouse 降采样与保留策略](#clickhouse-降采样与保留策略)
- [配置文件说明](#配置文件说明)
- [前端可视化](#前端可视化)
- [FAQ](#faq)
- [许可证](#许可证)

---

## 系统架构

### 架构图

```
                        ┌──────────────────────────────────────────────────────────┐
                        │                        用户                              │
                        └────────────────┬─────────────────────────────────────────┘
                                         │ HTTP :80 / HTTPS
                                         ▼
                        ┌──────────────────────────────────────────────────────────┐
                        │  Nginx 前端 (Gzip压缩 + 反向代理 + SPA路由)              │
                        │  ├─ /         → Three.js 3D攻城塔可视化                  │
                        │  ├─ /api/*    → Rust后端 :8080                           │
                        │  ├─ /metrics  → Prometheus指标                          │
                        │  └─ SSE长连接 → 实时数据流                                │
                        └────────────────────────────┬─────────────────────────────┘
                                                     │
                                                     ▼
                ┌───────────────────────────────────────────────────────────────────┐
                │  Rust 后端服务 :8080  (tokio async + axum)                        │
                │                                                                    │
                │  ┌─────────────┐   mpsc    ┌──────────────┐                       │
                │  │ DTU Receiver│ ────────► │ Structural   │  FEM 四面体有限元     │
                │  │ 传感器采集  │           │ Simulator    │  + Riks弧长法二阶效应 │
                │  └─────────────┘           └──────┬───────┘                       │
                │        │ mpsc                       │ oneshot                     │
                │        ├──────────────┐             ▼                             │
                │        ▼              ▼        SSE Broadcast                      │
                │  ┌──────────┐ ┌──────────┐                                        │
                │  │   Soil   │ │  Alarm   │  rumqttc → MQTT Broker :1883          │
                │  │ Analyzer │ │  MQTT    │  告警推送 + 传感器上报订阅             │
                │  └────┬─────┘ └────┬─────┘                                        │
                └───────┼─────────────┼─────────────────────────────────────────────┘
                        │             │
                        ▼             ▼
                ┌──────────────────────────────────────────┐     ┌────────────────┐
                │  ClickHouse :8123  时序数据库            │     │ Eclipse        │
                │  ├─ MergeTree: 传感器/FEM/告警/分析       │────►│ Mosquitto      │
                │  ├─ SummingMergeTree: 小时/天/月降采样    │     │ MQTT Broker    │
                │  └─ TTL 自动过期: 5年/1年/10年           │     │ :1883 / :9001  │
                └───────────────┬──────────────────────────┘     └───────┬────────┘
                                │ Prometheus :9363                       │
                                ▼                                         │
                        ┌──────────────────┐                           ▲
                        │  Prometheus :9090│◄──────── 指标采集 ────────┘
                        │  + 告警规则      │
                        └────────┬─────────┘
                                 │
                                 ▼
                        ┌──────────────────┐
                        │   Grafana :3000  │  可视化监控面板
                        └──────────────────┘
                                 ▲
                                 │ HTTP POST/MQTT
                                 │
                        ┌──────────────────┐
                        │  Python 模拟器    │  ← 风荷载 + 土壤条件 可配置
                        │  (DTU 仿真)       │    + 故障注入
                        └──────────────────┘
```

---

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| **前端** | Vue 3 + Three.js + Chart.js + Vite | 3D攻城塔渲染 + 应力云图 + 稳定性面板 |
| **Web服务** | Nginx 1.27 + Gzip 压缩 | 静态资源托管 + API反向代理 |
| **后端语言** | Rust 1.78 + tokio 1.35 + axum 0.7 | 异步高性能服务 |
| **有限元分析** | nalgebra 0.32 | 四面体4节点单元 + Riks/Crisfield 弧长法 |
| **土壤力学** | Terzaghi 极限承载力公式 | 含含水率修正 (c, φ修正) |
| **消息队列** | Eclipse Mosquitto 2.0 (MQTT 3.1.1) | 告警推送 + 传感器订阅 |
| **时序数据库** | ClickHouse 24.3 | MergeTree + TTL + 物化视图降采样 |
| **可观测性** | Prometheus 2.53 + Grafana 11.1 | 指标采集 + 可视化 + 告警规则 |
| **容器化** | Docker + Docker Compose 3.9 | 多阶段构建 + 一键编排 |
| **模拟器** | Python 3.12 + paho-mqtt | 风荷载5模式 + 5种土壤 + 故障注入 |

---

## 模块拆分说明

### 后端 Rust 服务 4 模块 (mpsc channel 解耦)

| 模块文件 | 职责 | 输入 | 输出 |
|----------|------|------|------|
| `src/dtu_receiver.rs` | 传感器数据采集、校验、广播分发 | HTTP POST, SSE模拟 | mpsc→sim/soil/alarm + broadcast→SSE |
| `src/structural_simulator.rs` | FEM有限元 + 稳定性 + Riks二阶效应 | mpsc 通道 (SimCommand) | oneshot 返回 + SSE Broadcast |
| `src/soil_analyzer.rs` | Terzaghi承载力 + 含水率修正 + 通过性 | mpsc 通道 (SoilCommand) | oneshot 返回土壤分析 |
| `src/alarm_mqtt.rs` | 告警阈值评估 + rumqttc异步推送 | mpsc 通道 (AlarmCommand) | MQTT Broker 发布 + 数据库 |
| `src/metrics.rs` | Prometheus指标 (18个计数器/直方图/仪表盘) | /metrics HTTP GET | text/plain Prometheus格式 |

### 前端拆分

| 文件 | 职责 |
|------|------|
| `src/siege_tower_3d.js` | Three.js 三维：攻城塔模型、应力云图着色器、剖切视图、相机动画 |
| `src/stability_panel.js` | 面板逻辑：API调用、SSE长连接、Chart.js图表、告警弹窗、状态机 |

### 配置外置 (JSON)

| 文件 | 内容 |
|------|------|
| `backend/config/tower_config.json` | 2种塔型的几何尺寸/材料属性/重量/层数 |
| `backend/config/soil_config.json` | 5种土壤的γ/c/φ/修正系数/压缩指数 |

---

## 快速部署

### Docker Compose 一键部署（推荐）

> 适用：生产环境、演示环境、完整功能测试

```bash
# 1. 克隆仓库
git clone <repo> && cd AI_solo_coder_task_A_136

# 2. (可选) 配置环境变量
cp .env.example .env   # 如需要，修改端口、密码、风荷载等
#   编辑 .env 自定义：
#   - FRONTEND_PORT / BACKEND_PORT / CLICKHOUSE_PORT / MQTT_PORT
#   - WIND_PROFILE / WIND_BASE_MPS / WIND_MAX_MPS (风荷载)
#   - SOIL_TYPE / SOIL_MOISTURE_PCT (土壤条件)
#   - INJECT_FAULTS (故障注入开/关)

# 3. 首次构建 + 启动 (7个服务)
docker compose up -d --build

# 仅启动核心服务 (6个，不含Grafana)
docker compose --profile monitoring up -d --build

# 4. 查看健康状态
docker compose ps
# NAME                        STATUS             PORTS
# siege-tower-frontend       Up (healthy)       0.0.0.0:80->80/tcp
# siege-tower-backend        Up (healthy)       0.0.0.0:8080->8080/tcp
# siege-tower-clickhouse     Up (healthy)       0.0.0.0:8123->8123/tcp
# siege-tower-mqtt           Up (healthy)       0.0.0.0:1883->1883/tcp
# siege-tower-simulator      Up                 ...
# siege-tower-prometheus     Up (healthy)       0.0.0.0:9090->9090/tcp

# 5. 访问入口
#   前端可视化:   http://localhost
#   API文档/健康: http://localhost:8080/api/health
#   Prometheus:   http://localhost:9090
#   Grafana:      http://localhost:3000 (admin / admin123)

# 6. 查看日志
docker compose logs -f --tail=100 backend simulator

# 7. 停止
docker compose down

# 清理数据卷 (危险！删除所有ClickHouse数据)
docker compose down -v
```

### 本地开发部署

#### 后端 Rust
```bash
cd backend
# 需要 ClickHouse 和 MQTT 可用 (或本地mock)
cargo run --release
# 监听 0.0.0.0:8080
```

#### 前端 (开发模式，含Vite热更新)
```bash
cd frontend
npm install
npm run dev          # 开发: http://localhost:3000 (自动代理 /api → 8080)
npm run build        # 生产构建 → dist/
```

#### 模拟器 (本地运行)
```bash
cd simulator
pip install -r requirements.txt
python sensor_simulator.py --help
```

---

## 传感器模拟器使用

Python 模拟器提供 5 种风工况 + 5 种土壤 + 5 类故障注入，支持 CLI 参数和环境变量双配置。

### 基础命令

```bash
python simulator/sensor_simulator.py [OPTIONS]

# 查看帮助
python simulator/sensor_simulator.py --help
```

### 风荷载模式 (--wind-profile / -w)

| 模式 | 说明 | 适用场景 |
|------|------|----------|
| `calm` | 静风，风速 < 0.3×基础值 | 基准测试、无干扰测量 |
| `steady` | 稳定风，±8%波动 | 长期应力监测 |
| `gusty` (默认) | 阵风：30s周期 + 脉冲峰 | 常规测试 |
| `ramp` | 渐变：线性增加到最大风速 | 结构失效过程分析 |
| `typhoon` | 台风：70%最大值 + 高频湍流 + 15s周期峰值 | 极端工况验证 |

```bash
# 台风工况 基础30m/s 极限60m/s
python sensor_simulator.py -w typhoon -b 30 --wind-max 60

# 渐变风速 + 旋转风向
python sensor_simulator.py -w ramp --wind-direction rotate --wind-change-interval 300
```

### 土壤条件配置 (--soil-type / -s)

| 类型 | 基础承载力kPa | 压缩指数 | 典型应用 |
|------|--------------|---------|----------|
| `sand` | 180 | 0.02 | 砂土、砂石地基 |
| `silt` | 90 | 0.20 | 粉土、粉质黏土 |
| `clay` | 120 | 0.35 | 黏土、高含水率 |
| `silt_soft` | 40 | 0.55 | 软土、淤泥 |
| `rock` | 800 | 0.001 | 岩石、碎石 |

```bash
# 高含水率黏土 40%
python sensor_simulator.py -s clay --moisture 40

# 碎石地基 + 禁用含水率动态变化 (静态)
python sensor_simulator.py -s rock --soil-static

# 初始沉降10mm
python sensor_simulator.py -s silt_soft --soil-settlement 10
```

### 故障注入 (--faults / --no-faults)

| 故障类型 | 概率模型 | 效果 |
|----------|---------|------|
| 传感器漂移 | 2% × 10% | 应力/倾斜指标持续累积偏移 |
| 数据卡死 | 2% × 2% | 所有读数冻结在原值 |
| 异常突变 | 0.5%~3% | 应力瞬间放大 2.5×~6× |
| 字段缺失 | 0.3% | 振动/湿度/温度字段随机为null |
| 严重越界 | 0.1% | 倾斜瞬间放大15倍 |

```bash
# 基准测试: 无故障、高频率、稳定风、碎石地基
python sensor_simulator.py --no-faults -i 0.5 -w steady -s rock

# 压力测试: 5%故障概率 + 台风 + 淤泥
python sensor_simulator.py --fault-probability 0.05 -w typhoon -s silt_soft

# 可重复场景: 指定随机种子
python sensor_simulator.py --seed 42 -w ramp -s clay
```

### 输出模式 (-o)

| 模式 | 说明 |
|------|------|
| `http` (默认) | POST JSON 到 `http://backend:8080/api/towers/1/sensor` |
| `mqtt` | 发布到 `siege_tower/sensor` 主题 |
| `both` | 同时输出 HTTP + MQTT |

```bash
# 仅MQTT输出 (不依赖后端API)
python sensor_simulator.py -o mqtt --mqtt-broker mqtt --mqtt-port 1883
```

### 场景示例

**场景1：台风侵袭高含水率黏土**
```bash
docker compose run --rm simulator \
    --wind-profile typhoon -b 25 --wind-max 50 \
    --soil-type clay --moisture 45 \
    --interval-sec 3 --no-faults
```

**场景2：通过性评估 - 不同土壤**
```bash
for soil in sand silt clay silt_soft rock; do
  echo "=== Testing $soil ==="
  timeout 60 python sensor_simulator.py \
    -s $soil --moisture 25 \
    -w steady -b 15 --interval-sec 2 \
    --soil-static --no-faults
done
```

**场景3：鲁棒性测试 - 全故障 + 极端**
```bash
python sensor_simulator.py \
    -w typhoon -b 35 --wind-max 60 \
    -s silt_soft --moisture 60 --soil-settlement 20 \
    --faults --fault-probability 0.08
```

---

## API 参考

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/health` | 健康检查 |
| GET | `/metrics` | **Prometheus指标** (text/plain 格式) |
| GET | `/api/towers` | 所有攻城塔元数据 |
| GET | `/api/towers/:tower_id` | 指定塔详情 |
| GET | `/api/config/towers/:tower_id` | 塔结构配置 (JSON) |
| GET | `/api/config/soils` | 所有土壤类型配置 |
| POST | `/api/towers/:tower_id/sensor` | **接收传感器数据** (模拟器POST到此) |
| GET | `/api/towers/:tower_id/sensor` | 查询历史传感器数据 |
| POST | `/api/towers/:tower_id/analysis` | 快速稳定性仿真 (不做完整FEM) |
| GET | `/api/towers/:tower_id/analysis` | 获取最新分析结果 |
| **GET** | **`/api/towers/:tower_id/analysis/full`** | **完整FEM分析** (20-60s, 四面体+二阶) |
| GET | `/api/towers/:tower_id/analysis/structure` | 纯结构分析 |
| GET | `/api/towers/:tower_id/ground` | 地面承载力+通过性评估 |
| GET | `/api/towers/:tower_id/alerts` | 历史告警事件 |
| GET | `/api/stream/sensor` | **SSE**: 实时传感器数据流 |
| GET | `/api/stream/analysis` | **SSE**: 分析结果推送 |
| GET | `/api/stream/alerts` | **SSE**: 告警事件推送 |

### MQTT 主题

| 主题 | 方向 | 说明 |
|------|------|------|
| `siege_tower/sensor` | PUB (模拟器→Broker) | 原始传感器JSON |
| `siege_tower/alert` | PUB (后端→Broker) | 告警事件JSON |
| `siege_tower/analysis` | PUB (后端→Broker) | 分析结果摘要JSON |

---

## Prometheus 指标采集

**端点**: `http://backend:8080/metrics` 或 `http://localhost:9090` (代理)

**18个自定义指标** (命名空间 `siege_tower_`)：

| 指标 | 类型 | 标签 | 说明 |
|------|------|------|------|
| `http_requests_total` | CounterVec | method, endpoint, status | HTTP请求计数 |
| `http_request_duration_seconds` | HistogramVec | method, endpoint | 请求耗时 (50ms~120s桶) |
| `http_errors_total` | CounterVec | method, endpoint, error_type | 错误计数 |
| `sensor_data_received_total` | CounterVec | tower_id, source | 接收传感器条数 |
| `sensor_data_valid_total` / `invalid` | CounterVec | tower_id[, reason] | 校验通过/失败 |
| `sensor_data_bytes_total` | Counter | - | 总字节数 |
| `fem_analysis_total` / `duration` | Counter+Histogram | tower_id, type | FEM计数+耗时 |
| `fem_analysis_nodes` | IntGaugeVec | tower_id | 最近分析节点数 |
| `fem_analysis_errors_total` | CounterVec | tower_id, error_type | FEM错误 |
| `soil_analysis_total` / `duration` | Counter+Histogram | tower_id, soil_type | 土壤分析 |
| `alerts_triggered_total` | CounterVec | tower_id, alert_type, level | 告警触发 |
| `alerts_mqtt_sent_total` / `errors` | Counter | - | MQTT推送统计 |
| `structure_safety_factor` | GaugeVec | tower_id | 当前安全系数 |
| `structure_stable` | IntGaugeVec | tower_id | 稳定状态(0/1) |
| `soil_bearing_ratio` | GaugeVec | tower_id, soil_type | 承载力利用率 |
| `clickhouse_inserts_total` / `errors` | CounterVec | table | 数据库操作 |
| `active_sse_connections` | IntGaugeVec | stream_type | SSE连接数 |
| `module_channel_depth` | IntGaugeVec | channel | mpsc队列深度 |

### 快速查看指标值

```bash
curl -s http://localhost:8080/metrics | grep siege_tower | head -30
```

---

## ClickHouse 降采样与保留策略

### 表结构与 TTL (自动过期)

| 表 | 引擎 | TTL | 说明 |
|----|------|-----|------|
| `sensor_data` | MergeTree | 5 年 | 原始秒级传感器 |
| `sensor_stats_minutely` | SummingMergeTree | 3 年 | 每分钟聚合 (物化视图自动维护) |
| `sensor_stats_hourly` | SummingMergeTree | 3 年 | 每小时聚合 |
| `sensor_stats_daily` | SummingMergeTree | 10 年 | 每天聚合 |
| `fem_node_results` | MergeTree | **1 年** | FEM节点结果 (高基数) |
| `structure_analysis` | ReplacingMergeTree | 10 年 | 结构分析结果 |
| `alert_events` | MergeTree | 永久 | 审计用 |
| `alert_events_monthly` | SummingMergeTree | 10 年 | 月度告警摘要 |
| `ground_analysis` | MergeTree | 10 年 | 地面分析 |

### 降采样链路

```
sensor_data (原始)
    │
    ├── MV: sensor_stats_minutely_mv → sensor_stats_minutely (每分钟max/avg/p95)
    │        │
    │        └── MV: sensor_stats_hourly_mv → sensor_stats_hourly (每小时)
    │                  │
    │                  └── MV: sensor_stats_daily_mv → sensor_stats_daily (每天)
    │
alert_events
    │
    └── MV: alert_events_monthly_mv → alert_events_monthly (按月分级别汇总)

structure_analysis
    │
    └── MV: structure_analysis_monthly_mv → structure_analysis_monthly (月度摘要)
```

### 辅助视图

| 视图 | 用途 |
|------|------|
| `current_sensor_status` | 各塔各层最近1小时状态 |
| `daily_alert_summary` | 按日期/塔/级别统计告警数 |

---

## 配置文件说明

### .env 环境变量 (完整列表)

| 分组 | 变量 | 默认值 | 说明 |
|------|------|--------|------|
| 端口 | `FRONTEND_PORT` | 80 | 前端HTTP端口 |
| 端口 | `BACKEND_PORT` | 8080 | Rust后端端口 |
| 端口 | `CLICKHOUSE_PORT` | 8123 | ClickHouse HTTP |
| 端口 | `MQTT_PORT` | 1883 | MQTT |
| 端口 | `PROM_PORT` | 9090 | Prometheus |
| 端口 | `GRAFANA_PORT` | 3000 | Grafana |
| DB | `CLICKHOUSE_USER/PASSWORD` | tower_user / tower_secure_2024 | 数据库凭证 |
| 风 | `WIND_PROFILE` | gusty | 风工况 |
| 风 | `WIND_BASE_MPS` / `WIND_MAX_MPS` | 10 / 40 | 风速边界 |
| 风 | `WIND_GUST_FACTOR` | 2.5 | 阵风峰值放大 |
| 风 | `WIND_DIRECTION` | random | 风向: random/fixed/rotate |
| 土 | `SOIL_TYPE` | sand | 土壤类型 |
| 土 | `SOIL_MOISTURE_PCT` | 15 | 含水率% |
| 土 | `SOIL_DYNAMIC` | true | 动态变化模拟降雨 |
| 故障 | `INJECT_FAULTS` | true | 是否注入故障 |
| 故障 | `FAULT_PROBABILITY` | 0.02 | 每包故障概率 |

### JSON 配置文件

`backend/config/tower_config.json`: 定义塔型参数
```json
{
  "towers": [{
    "tower_id": 1, "name": "临冲吕公车-一号",
    "total_height": 18.5, "total_layers": 5,
    "base_width": 6.2, "base_depth": 4.8,
    "total_weight_tons": 28.5,
    "elastic_modulus_mpa": 12000,
    "poisson_ratio": 0.38,
    "design_wind_speed_mps": 35
  }]
}
```

---

## 前端可视化

访问 `http://localhost/` 打开系统，包含：

1. **攻城塔3D视图** (`siege_tower_3d.js`)
   - 多层木结构几何模型
   - 应力云图 (viridis色彩映射 von Mises)
   - 剖切视图 (X/Y/Z平面切分，观察内部应力)
   - 风荷载向量箭头动画
   - 可交互：OrbitControls 缩放/旋转/平移

2. **稳定性监控面板** (`stability_panel.js`)
   - 关键指标卡：安全系数、稳定裕度、最大应力、最大倾斜
   - 时序图：应力曲线、风速-倾斜相关性、土壤承载力趋势
   - 告警面板：实时SSE推送 + 声光提示 + 确认操作
   - 分析控制：一键运行完整FEM、自定义参数分析、土壤对比

---

## FAQ

### Q1: 首次启动 ClickHouse 需要多久？
A: 首次初始化会执行 init.sql + rollup_ttl.sql (建表+物化视图)，大约 45-60 秒，`docker compose ps` 看到 healthy 后即可使用。

### Q2: FEM分析耗时太久怎么办？
A: 完整FEM (120节点四面体+弧长法) 通常 20-60s。可使用：
- `POST /api/towers/1/analysis` (快速仿真，无完整FEM，~3s)
- 增加 backend 的 `mem_limit` (建议 ≥ 4GB) 和 `cpus`

### Q3: 模拟器如何实时调整参数？
A: 两种方法：
1. 修改 `.env` 后 `docker compose up -d --force-recreate simulator` (不中断其他服务)
2. 直接运行本地 Python 脚本，CLI 传参无需重启

### Q4: 如何添加新的塔型？
A: 编辑 `backend/config/tower_config.json`，重新构建后端镜像 (`docker compose build backend`) 或重启后端服务。

### Q5: 如何持久化数据？
A: docker-compose 使用命名卷 (volumes 段)：
- `clickhouse-data`: 数据库
- `mosquitto-data`: MQTT会话
- `prometheus-data` / `grafana-data`: 监控
使用 `docker compose down -v` **会删除所有数据**，谨慎！

---

## 许可证

军事史研究用途 · 内部学术交流

---

## 版本历史

- **v2.0** (2026-06-17) 工程化重构：Docker多阶段构建、docker-compose 7服务编排、Prometheus+Grafana、ClickHouse降采样+TTL、Python模拟器(5风型+5土壤+5故障)、Nginx Gzip、前端拆分
- **v1.5** 修复 FEM 矩阵维度 (6×6→12×12)、Riks弧长法、Terzaghi含水率修正、前后端拆分
- **v1.0** 初始版本：单仓单体 Rust + Vue 前端
