# kid-control3 — 家庭平板控制系统 v3

Rust 重写版。管理孩子平板的上网时间、检测刷视频/玩游戏行为、自动封禁/放行。

## 技术栈

- **后端**: Rust + Axum 0.8 + rusqlite + reqwest
- **前端**: React + TypeScript + Vite + TailwindCSS
- **数据库**: SQLite
- **部署**: Docker on NAS

## 功能

- 📱 ROS 防火墙管控（IPv4 + IPv6）
- 🔍 DNS 域名分类（蛋仔/超自然/小红书/腾讯视频/红果）
- ⏱️ 每日限额 + 设备级覆盖
- 🏖️ 假期模式
- ⭐ 积分系统（作业/补课/其他 → 兑换平板时间）
- 🌐 Web UI 仪表盘

## 快速开始

```bash
# 复制环境变量
cp .env.example .env
# 编辑 .env 填写实际的 ROS/MOSDNS 地址

# Docker 启动
docker compose up -d
```

访问 http://YOUR_IP:18089

## 开发

```bash
# 后端（需要 Rust 工具链）
cargo check
cargo build

# 前端
cd web && npm install && npm run build
```

## 项目结构

```
src/
├── main.rs          # 入口
├── config.rs        # 配置管理
├── db.rs            # 数据库操作
├── models.rs        # 数据模型
├── ros/             # RouterOS 客户端
├── detector/        # 活动检测引擎
├── scheduler/       # 定时任务
└── api/             # API 端点
web/
├── src/
│   ├── App.tsx      # 路由
│   ├── api.ts       # API 客户端
│   ├── types.ts     # TypeScript 类型
│   └── components/  # 页面组件
```
