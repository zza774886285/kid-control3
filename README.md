# kid-control3

家庭平板管控系统 v3 — Rust 重写版。

管理孩子的华为/苹果平板上网时间，通过 RouterOS 防火墙实现设备级网络管控，配合 Telegram 审批的积分系统激励学习。

## 功能

**管控核心**
- 基于 RouterOS 防火墙的设备级 IPv4/IPv6 封禁
- 每日使用时间限制 + 设备级覆盖
- 30 秒控制周期，实时封禁/解封
- 大人模式、允许时段、假期模式
- DNS 采集 + MOSDNS 域名分类识别视频/游戏/社交应用

**积分系统**
- 孩子通过 Telegram 申请积分（补课/作业/其他）
- 管理员 Telegram 内联按钮审批（✅同意 / ❌拒绝）
- 积分兑换平板使用时间（30min / 60min）
- 周额度管理（每种类型每周一次）
- 完整交易记录 + 余额查询

**管理后台**
- Web 管理界面（设备卡片、积分管理、设置）
- 孩子专属积分页（申请/兑换/历史）
- API 端口：管理端 18089，孩子端 18090

## 技术栈

| 层 | 技术 |
|---|------|
| 后端 | Rust + Axum 0.8 + rusqlite |
| 前端 | React 18 + TypeScript + Vite + TailwindCSS |
| 数据库 | SQLite (WAL mode) |
| 通知 | Telegram Bot API (getUpdates polling) |
| 网络 | RouterOS REST API (IPv4/IPv6 firewall) |
| DNS | MOSDNS API 域名分类 |

## 项目结构

```
kid-control3/
├── src/
│   ├── main.rs           # 入口，双端口启动
│   ├── api/
│   │   ├── points.rs     # 积分 CRUD + 审批
│   │   ├── control.rs    # 设备封禁/解封
│   │   ├── data.rs       # 数据查询
│   │   └── config_api.rs # 配置管理
│   ├── db.rs             # SQLite 数据库
│   ├── config.rs         # 配置管理
│   ├── telegram.rs       # Telegram 通知 + 轮询
│   ├── ros/
│   │   ├── client.rs     # RouterOS REST 客户端
│   │   ├── firewall.rs   # IPv4 封禁
│   │   ├── ipv6.rs       # IPv6 封禁
│   │   └── arp.rs        # ARP 表查询
│   ├── detector/
│   │   ├── activity.rs   # 活动检测引擎
│   │   ├── dns_collector.rs # DNS 域名采集
│   │   └── domain_rules.rs  # 域名分类规则
│   └── scheduler/
│       ├── collect.rs    # 控制周期 + 数据采集
│       └── cache.rs      # 缓存刷新
└── web/
    └── src/
        ├── App.tsx       # 路由
        ├── components/
        │   ├── Dashboard.tsx    # 管理仪表盘
        │   ├── PointsPage.tsx   # 积分管理
        │   ├── SettingsPage.tsx # 设置页
        │   └── Layout.tsx       # 布局
        ├── api.ts        # API 调用
        └── kid.tsx       # 孩子端入口
```

## 部署

### Docker（推荐）

```bash
# 1. 构建镜像
docker build -t kirin1989/kid-control3:latest .

# 2. 准备 .env 文件
cp .env.example .env
# 编辑 .env 填入实际值

# 3. 启动容器
docker run -d \
  --name kid-control3 \
  --restart unless-stopped \
  --env-file .env \
  -v /path/to/data:/data \
  -p 18089:18089 \
  -p 18090:18090 \
  kirin1989/kid-control3:latest
```

### 环境变量

| 变量 | 必填 | 说明 |
|------|------|------|
| `ROS_HOST` | ✅ | RouterOS IP |
| `ROS_PORT` | | RouterOS API 端口，默认 80 |
| `ROS_USER` | ✅ | RouterOS 用户名 |
| `ROS_PASS` | ✅ | RouterOS 密码 |
| `TELEGRAM_BOT_TOKEN` | ✅ | Telegram Bot Token |
| `TELEGRAM_CHAT_ID` | ✅ | 审批通知群组/私聊 ID |
| `HTTP_PROXY` | | Telegram API 代理 |
| `MOSDNS_URL` | | MOSDNS API 地址（IP:端口） |
| `DB_PATH` | | 数据库路径，默认 `/data/kid-control.db` |
| `LISTEN_ADDR` | | 管理端监听地址，默认 `0.0.0.0:18089` |
| `KID_LISTEN_ADDR` | | 孩子端监听地址，默认 `0.0.0.0:18090` |

### GitHub Actions

推送 `main` 分支自动构建 Docker 镜像并推送到 Docker Hub：
```
docker pull kirin1989/kid-control3:latest
```

## API

### 管理端 (18089)

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/data` | 设备状态、活动数据 |
| GET | `/api/points/balance` | 所有用户积分余额 |
| POST | `/api/points/set` | 管理员手动调整积分 |
| POST | `/api/points/apply` | 申请积分（模拟） |
| POST | `/api/points/approve` | 审批积分请求 |
| POST | `/api/points/exchange` | 兑换积分 |
| GET | `/api/points/pending` | 待审批列表 |
| GET | `/api/points/my?user_id=N` | 个人积分历史 |
| POST | `/api/pause` | 暂停设备 |
| POST | `/api/switch` | 切换设备开关 |
| GET | `/api/settings` | 获取设置 |
| POST | `/api/settings` | 更新设置 |

### 孩子端 (18090)

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/points/balance` | 积分余额 |
| POST | `/api/points/apply` | 申请积分 |
| POST | `/api/points/exchange` | 兑换时间 |
| GET | `/api/points/my` | 积分历史 |
| GET | `/api/points/pending` | 待审批列表 |

## 数据库

SQLite WAL 模式，关键表：

- `users` — 用户（admin/lisa/huawei）
- `point_transactions` — 积分交易记录
- `point_requests` — 积分申请审批
- `weekly_quota` — 周额度跟踪
- `config` — 配置键值对
- `video_windows` — 5 分钟滑动窗口活动数据
- `video_sessions` — 设备使用会话

## 开发

```bash
# 后端
cargo check        # 类型检查
cargo build        # 编译

# 前端
cd web && npm install
npx tsc --noEmit   # TypeScript 检查
npx vite build     # 构建

# 提交前验证
cargo check && cd web && npx tsc --noEmit && npx vite build
```

## 已知问题

- SFTP 在绿联 NAS 上不可用，传输文件用 base64 管道
- SQLite `datetime('now')` 返回 UTC，代码中已加 `+8 hours`
- NAS 容器只能通过 WebUI 操作，不能 SSH docker compose
