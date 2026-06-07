# 精密数控机床主轴健康监控与剩余寿命预测系统

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-dea584.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/Docker-Compose-blue.svg)](https://www.docker.com/)
[![ClickHouse](https://img.shields.io/badge/ClickHouse-23.8-yellow.svg)](https://clickhouse.com/)
[![EMQX](https://img.shields.io/badge/EMQX-5.3-green.svg)](https://www.emqx.io/)

## 系统架构

### 总体架构图

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                                 工厂网络层                                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │  CNC机床1    │  │  CNC机床2    │  │  ...        │  │  CNC机床40   │            │
│  │ (EtherCAT)  │  │ (EtherCAT)  │  │             │  │ (EtherCAT)  │            │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘            │
└─────────┼────────────────┼────────────────┼────────────────┼───────────────────┘
          │                │                │                │
          └────────────────┴────────────────┴────────────────┘
                                    │ UDP 5555
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                          数据采集与处理层 (Rust)                                 │
│  ┌───────────────────────────────────────────────────────────────────────────┐  │
│  │  Pipeline:  tokio mpsc channel 管道架构                                   │  │
│  │  ┌──────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │  │
│  │  │EtherCAT  │───▶│ Vibration    │───▶│ RUL          │───▶│ Alarm        │ │  │
│  │  │ Driver   │    │ Analyzer     │    │ Predictor    │    │ Dispatcher   │ │  │
│  │  │ UDP采集   │    │ FFT+烈度计算  │    │ SKF+LSTM推理 │    │ ISO22400推送 │ │  │
│  │  └──────────┘    └──────────────┘    └──────────────┘    └──────┬───────┘ │  │
│  └────────────────────────────────────────────────────────────────────┼─────────┘  │
│                                                                      │            │
│  ┌───────────────────────┐  ┌───────────────────────┐  ┌────────────▼─────────┐  │
│  │   Prometheus Metrics  │  │   ClickHouse (时序)   │  │     EMQX MQTT Broker │  │
│  │   /metrics:9090       │  │   分区+TTL自动归档     │  │     QoS 1 保证送达    │  │
│  └───────────────────────┘  └───────────────────────┘  └───────────┬──────────┘  │
└──────────────────────────────────────────────────────────────────────┼─────────────┘
                                                                       │
┌──────────────────────────────────────────────────────────────────────┼─────────────┐
│                        可视化与集成层                                   │             │
│  ┌────────────────┐  ┌──────────────────────────┐  ┌────────────────▼──────────┐  │
│  │   前端Web界面   │  │      Grafana监控面板      │  │       MES / SCADA系统     │  │
│  │ (Nginx Gzip)   │  │ (Prometheus + ClickHouse) │  │   (ISO 22400-2 标准)     │  │
│  └────────────────┘  └──────────────────────────┘  └───────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────────────────┘
```

### 模块架构详情

| 模块 | 文件 | 核心职责 | 技术要点 |
|------|------|----------|----------|
| EtherCAT 驱动 | [ethercat_driver.rs](backend/src/ethercat_driver.rs) | UDP 异步多通道采集 + 时域特征提取 | tokio UdpSocket, 8MB 缓冲区, 40 机床扇入扇出 |
| 振动分析器 | [vibration_analyzer.rs](backend/src/vibration_analyzer.rs) | FFT 频谱分析 + 振动烈度计算 | rustfft, 批量特征计算 |
| RUL 预测器 | [rul_predictor.rs](backend/src/rul_predictor.rs) | 特征融合 + SKF+LSTM 混合推理 | 变工况自适应, 指数平滑滤波 |
| 告警调度器 | [alarm_dispatcher.rs](backend/src/alarm_dispatcher.rs) | 告警评估 + ISO 22400 适配 + MQTT 推送 | 两级告警, QoS 1 保证 |
| 指标采集 | [metrics.rs](backend/src/metrics.rs) | Prometheus 标准指标暴露 | Counter/Gauge/Histogram 三类指标 |
| 前端组件 | [spindle_profile.js](frontend/spindle_profile.js) | 主轴剖面图组件 | Canvas 2D, 脏矩形渲染 |
| 前端组件 | [waterfall_plot.js](frontend/waterfall_plot.js) | 频谱瀑布图组件 | 6色阶伪彩, 历史滚动 |

---

## 快速部署

### 环境要求

- Docker Engine 24.0+
- Docker Compose v2.20+
- 至少 8GB 可用内存
- 至少 50GB 可用磁盘空间

### 一键启动

```bash
# 克隆项目
git clone <repository-url>
cd AI_solo_coder_task_A_065

# 一键启动所有服务
docker-compose up -d

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f rust-backend
```

### 服务端口清单

| 服务 | 端口 | 说明 |
|------|------|------|
| 前端界面 | http://localhost:80 | CNC 监控主界面 (Nginx) |
| Rust API | http://localhost:8080 | REST API 接口 |
| WebSocket | ws://localhost:8081 | 实时数据推送 |
| Metrics | http://localhost:9090/metrics | Prometheus 指标 |
| Prometheus | http://localhost:9091 | Prometheus Web UI |
| Grafana | http://localhost:3000 | 监控面板 (admin/admin123) |
| EMQX Console | http://localhost:18083 | MQTT 管理控制台 (admin/public) |
| ClickHouse HTTP | http://localhost:8123 | ClickHouse HTTP 接口 |
| EMQX MQTT | tcp://localhost:1883 | MQTT Broker |

### 停止服务

```bash
# 停止所有服务
docker-compose down

# 停止并清除数据卷（注意：会丢失所有历史数据）
docker-compose down -v
```

---

## EtherCAT 模拟器使用说明

### 模拟器特性

- ✅ 支持 1~40 台机床配置
- ✅ 每台机床 14 个传感器（8 振动 + 4 温度 + 2 位移）
- ✅ 100ms 采样间隔可调
- ✅ 4 种振动异常注入（冲击/不平衡/不对中/保持架故障）
- ✅ 轴承退化模拟（时间累积 + 随机故障轴承）
- ✅ 交互式控制 Shell
- ✅ 实时发送统计

### 环境变量配置

```bash
# docker-compose.yml 中可配置
SIMULATOR_UDP_TARGET=rust-backend:5555    # 目标地址
SIMULATOR_MACHINE_COUNT=40                # 机床数量
SIMULATOR_SAMPLE_INTERVAL_MS=100          # 采样间隔
SIMULATOR_ANOMALY_INJECT=true             # 自动随机异常
SIMULATOR_DEGRADATION_INJECT=true         # 轴承退化模拟
```

### 交互式控制命令

```bash
# 进入模拟器容器
docker exec -it cnc-simulator python ethercat_simulator.py

# 或者直接 attach 到运行中的容器
docker attach cnc-simulator
```

**可用命令：**

| 命令 | 说明 | 示例 |
|------|------|------|
| `status` | 查看模拟器状态 | `status` |
| `list` | 列出所有机床退化状态 | `list` |
| `inject <mid> <sensor> <type> [duration]` | 注入异常 | `inject 5 2 impact 10` |
| `rpm <mid> <rpm>` | 设置机床转速 | `rpm 3 4500` |
| `degrade <mid> <level>` | 强制退化等级 | `degrade 10 0.8` |
| `stats` | 查看发送统计 | `stats` |
| `quit` | 退出模拟器 | `quit` |

**异常类型说明：**

| 类型 | 说明 | 典型场景 |
|------|------|----------|
| `impact` | 冲击脉冲 | 刀具破损、切削崩刃 |
| `unbalance` | 不平衡振动 | 主轴动平衡失效 |
| `misalignment` | 不对中 | 联轴器不对中、安装偏差 |
| `cage_fault` | 保持架故障 | 轴承保持架磨损、碎裂 |

### 使用示例

```bash
# 1. 查看所有机床状态
> list
Machine    RPM      Degradation  Faulty Bearings
--------------------------------------------------
1          1500     0.0012       2
2          3000     0.0008       None
...

# 2. 给 5 号机床的 2 号传感器注入 10 秒的冲击异常
> inject 5 2 impact 10
[ANOMALY] Machine 5, Sensor 2: impact injected

# 3. 将 8 号机床转速设为 4500（高速工况）
> rpm 8 4500
Machine 8 RPM set to 4500

# 4. 强制 15 号机床退化到 0.7（严重磨损）
> degrade 15 0.7
Machine 15 degradation forced to 0.7

# 5. 查看统计
> stats
{
  "total_packets": 52430,
  "send_errors": 0
}
```

---

## 数据存储策略（ClickHouse）

### 分区与 TTL 配置

| 表名 | 分区键 | TTL | 粒度 | 用途 |
|------|--------|-----|------|------|
| `sensor_raw` | 按天 (YYYYMMDD) | 30 天 | 原始采样点 | UDP 原始数据 |
| `sensor_metrics_1m` | 按月 (YYYYMM) | 12 个月 | 1 分钟聚合 | 统计特征 |
| `vibration_severity` | 按月 (YYYYMM) | 12 个月 | 分析结果 | 振动烈度 |
| `fft_spectrum` | 按月 (YYYYMM) | 90 天 | 频域数据 | 频谱分析 |
| `rul_predictions` | 按月 (YYYYMM) | 36 个月 | 预测结果 | RUL 预测 |
| `alarms` | 按月 (YYYYMM) | 60 个月 | 告警事件 | 审计追溯 |

### 自动归档机制

- **热数据**（30 天内）：`sensor_raw` 保存在 MergeTree 中，支持高速查询
- **温数据**（30天~1年）：通过物化视图预聚合到 `sensor_metrics_1m`
- **冷数据**（1~5年）：告警和 RUL 结果长期保存用于趋势分析
- **自动清理**：超过 TTL 的数据由 ClickHouse 后台 Merge 线程自动删除

---

## MQTT 消息规范（ISO 22400-2）

### QoS 配置

- 告警消息：QoS 1（至少送达一次），30 秒重试
- 状态消息：QoS 1，Session 过期 2 小时
- 最大飞行窗口：32 条消息

### 消息结构

```json
{
  "header": {
    "message_id": "uuid-v4",
    "message_type": "Alarm",
    "source": "CNC-SPINDLE-MONITOR",
    "timestamp": "2024-01-15T10:30:00+08:00",
    "version": "ISO22400-2:2022"
  },
  "body": {
    "alarm_id": "ALM-005-01",
    "machine_id": 5,
    "severity": "High",
    "description": "振动烈度超过阈值",
    "condition": "HighSpeed"
  }
}
```

---

## 前端部署优化

### Nginx 优化特性

| 优化项 | 配置 | 效果 |
|--------|------|------|
| Gzip 压缩 | 等级 6，22 种 MIME 类型 | 静态资源体积减少 ~60% |
| CDN 缓存 | 静态资源 30 天过期，immutable | 浏览器不重复请求 |
| 反向代理 | API/WebSocket 智能路由 | 统一入口，安全隔离 |
| 安全头 | X-Frame/XSS/Content-Type | 基础安全防护 |

### 资源缓存策略

```
静态资源 (js/css/png/woff2):  Cache-Control: public, max-age=2592000, immutable
HTML 文件:                    Cache-Control: no-cache, no-store, must-revalidate
API 响应:                     无缓存，实时更新
```

---

## 监控指标（Prometheus）

### 核心指标列表

| 指标名 | 类型 | 说明 |
|--------|------|------|
| `cnc_monitor_udp_packets_received_total` | Counter | UDP 接收总包数 |
| `cnc_monitor_udp_bytes_received_total` | Counter | UDP 接收总字节 |
| `cnc_monitor_udp_packet_errors_total` | Counter | UDP 解析错误数 |
| `cnc_monitor_active_machines` | Gauge | 当前在线机床数 |
| `cnc_monitor_vibration_alarms_total` | Counter | 振动告警次数 |
| `cnc_monitor_rul_alarms_total` | Counter | RUL 临界告警次数 |
| `cnc_monitor_fft_processing_seconds` | Histogram | FFT 处理耗时 |
| `cnc_monitor_rul_prediction_seconds` | Histogram | RUL 推理耗时 |
| `cnc_monitor_clickhouse_inserts_total` | Counter | ClickHouse 写入次数 |
| `cnc_monitor_mqtt_messages_sent_total` | Counter | MQTT 发送成功数 |
| `cnc_monitor_avg_health_score` | Gauge | 全局平均健康分 |
| `cnc_monitor_websocket_connections` | Gauge | 当前 WebSocket 连接数 |

### Grafana 导入

预置数据源配置位于 `deploy/grafana/datasources/`，启动后自动加载。可直接创建面板使用上述指标。

---

## 模型参数配置

所有模型参数已从硬编码迁移到配置文件，可实时调整无需重编译：

```toml
# backend/config.toml
[models.skf]
basic_rated_life_hours = 25000.0
vibration_factor_low_vib_low = 2.0
temp_factor_low = 50.0
# ... 共 10 个 SKF 参数

[models.lstm]
base_rul_hours = 18000.0
smoothing_factor = 0.3
# ... 共 10 个 LSTM 参数

[models.hybrid]
weights_low_speed = [0.35, 0.35, 0.30]
weights_medium_speed = [0.40, 0.35, 0.25]
weights_high_speed = [0.45, 0.35, 0.20]
# ... 工况权重配置
```

---

## 常见问题排查

### 1. UDP 丢包严重

```bash
# 检查 UDP 缓冲区
dmesg | grep -i udp

# 调整系统缓冲区（宿主机）
sysctl -w net.core.rmem_max=16777216
sysctl -w net.core.rmem_default=16777216
```

### 2. ClickHouse 磁盘空间不足

```bash
# 查看各表占用
docker exec -it cnc-clickhouse clickhouse-client -q "
  SELECT table, formatReadableSize(sum(bytes)) as size
  FROM system.parts
  GROUP BY table
  ORDER BY sum(bytes) DESC
"

# 手动触发 TTL 合并
docker exec -it cnc-clickhouse clickhouse-client -q "
  OPTIMIZE TABLE cnc_monitor.sensor_raw FINAL
"
```

### 3. MQTT 消息堆积

```bash
# 查看 EMQX 订阅状态
docker exec -it cnc-emqx emqx ctl subscriptions list

# 清理过期会话
docker exec -it cnc-emqx emqx ctl clients clean
```

### 4. 模拟器无法连接后端

```bash
# 检查网络连通性
docker exec cnc-simulator ping rust-backend

# 手动测试 UDP
docker exec cnc-simulator nc -u rust-backend 5555
```

---

## 项目结构

```
.
├── backend/                    # Rust 后端服务
│   ├── src/
│   │   ├── main.rs            # 模块组装 + 管道连接
│   │   ├── ethercat_driver.rs # UDP 采集 + 时域特征
│   │   ├── vibration_analyzer.rs # 频谱分析 + 烈度计算
│   │   ├── rul_predictor.rs   # 特征融合 + LSTM 推理
│   │   ├── alarm_dispatcher.rs # 告警评估 + MQTT 推送
│   │   ├── metrics.rs         # Prometheus 指标
│   │   ├── config.rs          # 配置解析
│   │   └── models.rs          # 数据模型
│   ├── config.toml            # 系统配置（含模型参数）
│   ├── Cargo.toml
│   └── Dockerfile             # 多阶段静态构建
├── frontend/                   # 前端可视化
│   ├── spindle_profile.js     # 主轴剖面图组件
│   ├── waterfall_plot.js      # 瀑布图组件
│   ├── app.js                 # 主应用逻辑
│   ├── nginx.conf             # Nginx 优化配置
│   └── Dockerfile
├── simulator/                  # EtherCAT 模拟器
│   ├── ethercat_simulator.py  # 模拟器主程序
│   ├── requirements.txt
│   └── Dockerfile
├── deploy/                     # 部署配置
│   ├── clickhouse/            # ClickHouse 配置 + SQL
│   ├── emqx/                  # EMQX 配置
│   ├── prometheus/            # Prometheus 配置
│   └── grafana/               # Grafana 数据源
├── docker-compose.yml          # 服务编排
├── .env                        # 环境变量
└── README.md
```

---

## License

Proprietary - For internal use only
