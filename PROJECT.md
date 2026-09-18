# kid-control3

## 一句话定位
家庭平板管控系统的 Rust 重写版 — 更高性能，双端口（管理端+孩子积分页），含积分系统。

## 技术栈
- 后端：Rust（Axum 0.8 + rusqlite + reqwest + tracing）
- 前端：React 18 + TypeScript + Vite + TailwindCSS 4
- 数据库：SQLite
- 部署：Docker on NAS110
- CI：GitHub Actions → DockerHub

## 代码规范
- 加载 Skill：`code-standards`
- 格式化：rustfmt 默认配置（缩进 4 空格，行宽 100）
- 错误处理：`anyhow::Result`，禁止 `.unwrap()`
- 日志：`tracing` crate，禁止 `println!`

## 目录结构
```
kid-control3/
├── src/
│   ├── main.rs        # Axum 入口，路由注册
│   ├── config.rs      # 配置管理
│   ├── db.rs          # SQLite 数据库
│   ├── ros_client.rs  # ROS REST API 客户端
│   ├── firewall.rs    # 防火墙操作（IPv4/IPv6）
│   ├── detector.rs    # 活动检测引擎
│   ├── dns_collector.rs # DNS 采集
│   ├── collector.rs   # 定时采集调度
│   ├── points.rs      # 积分系统
│   └── error.rs       # 错误类型
├── web/
│   ├── src/
│   │   ├── api.ts
│   │   ├── types.ts
│   │   └── pages/
│   └── package.json
├── Cargo.toml
└── docker-compose.yml
```

## 核心模块
| 模块 | 职责 | 文件 |
|------|------|------|
| ROS 客户端 | 调用 RouterOS REST API | `ros_client.rs` |
| 防火墙 | IPv4/IPv6 阻止/解封 | `firewall.rs` |
| 活动检测 | DNS + 带宽双信号判定 | `detector.rs` |
| DNS 采集 | 从 mosdns 采集 | `dns_collector.rs` |
| 积分系统 | 申请/兑换/审批/Telegram 通知 | `points.rs` |
| 定时采集 | 60 秒周期采集 | `collector.rs` |

## 部署信息
- 位置：NAS10.1.1.110
- 端口：18089（管理端）、18090（孩子积分页）
- 容器名：kidcontrol
- 镜像：`kirin1989/kid-control3:latest`
- GitHub：`zza774886285/kid-control3`

## 已知坑点
1. **SQLite datetime('now') 返回 UTC** — 需显式加 8 小时转 CST
2. **Vite 多入口构建** — build 脚本需动态替换 JS 哈希
3. **SFTP 不可用** — 绿联 NAS 只能用 base64 管道传输文件
4. **config.rs 增减 30 分钟 bug** — `> 0` 判断导致 0 分钟 override 无效
5. **积分兑换不生效** — 需用 `get_current_limit` 获取实际限额
6. **SPA 路由 404** — ServeDir 需添加 SPA fallback
7. **ip6.arpa 噪声** — 逆向 DNS 查询填满 200 条配额

## 依赖关系
- ROS 250（RouterOS）：防火墙操作
- mosdns：DNS 采集
- kid-control2：前身项目（Python 版本）

## 当前状态
- 最后更新：2026-09
- 稳定性：开发中（已修复 10 个 bug，编译通过）
- 未解决问题：积分系统 Telegram 通知需 .env 配置
- 备注：Rust 版本性能更好，但开发迭代速度慢于 Python
