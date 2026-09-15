# kid-control3 Rust 后端代码审计报告

## P0 — 严重（需立即修复）

### 1. SQL 注入：`update_weekly_quota` 使用 `format!` 拼接 SQL
**文件**: `src/db.rs:465`  
**问题**: `&format!("UPDATE weekly_quota SET {}=1 WHERE user_id=?1 AND week_start=?2", col)` 将变量 `col` 直接拼接到 SQL 中。虽然当前调用者 `points.rs:58` 限制了 `req_type` 为三个固定值，但函数本身无防护，若未来有其他调用路径传入不可信输入，将导致 SQL 注入。  
**修复建议**: 改为 match 枚举或白名单校验 `col`，不要用 `format!` 拼接 SQL 列名。

### 2. 积分兑换无原子性：余额检查与扣减之间存在 TOCTOU 竞争
**文件**: `src/api/points.rs:153-178`  
**问题**: `get_user_points_balance` 与后续 `record_exchange` 之间无事务保护。并发请求同一 `user_id` 时，两次检查都通过，导致余额扣成负数。  
**修复建议**: 用 `BEGIN IMMEDIATE` 事务包裹余额检查 + 扣减，或使用 `UPDATE ... WHERE balance >= ?` 的原子 SQL。

### 3. 积分审批无原子性：余额读取与写入竞争
**文件**: `src/api/points.rs:133-141`  
**问题**: `get_user_points_balance` 读余额，然后 `INSERT` 新交易记录，无事务保护。并发审批同一用户时，`balance_after` 可能计算错误。  
**修复建议**: 用事务包裹 `SELECT balance + points` + `INSERT`，或使用子查询计算 `balance_after`。

---

## P1 — 重要（影响稳定性/正确性）

### 4. `set_points` 中 `unwrap()` 导致 panic
**文件**: `src/api/points.rs:255`  
**问题**: `conn.execute(...).unwrap()` — 若 DB 写入失败（磁盘满、锁超时），整个服务 panic 退出。  
**修复建议**: 改为 `match` 或 `if let Err(e)`，返回错误 JSON。

### 5. `db.rs` 中多处 `unwrap()` 可致 panic
**文件**: `src/db.rs:372, 391, 467`  
**问题**: `apply_points`、`record_exchange`、`update_weekly_quota` 中的 `conn.execute(...).unwrap()` 均可在 DB 异常时 panic。  
**修复建议**: 统一用 `?` 或 `map_err` 返回 `Result`，让调用者处理。

### 6. `Mutex::lock().unwrap()` 在 `ActivityDetector` 中
**文件**: `src/detector/activity.rs:36,43,50,82`  
**问题**: 若任何 panic 导致 Mutex 中毒（poisoned），后续所有 `lock().unwrap()` 都会 panic，级联崩溃。  
**修复建议**: 使用 `lock().unwrap_or_else(|e| e.into_inner())` 或 `tokio::sync::Mutex` 替代。

### 7. 时区硬编码 `+8 hours`
**文件**: `src/db.rs:371, 390`（`datetime('now', '+8 hours')`）  
**问题**: 依赖 SQLite 时区偏移，换时区或 DST 变化时时间戳错误，且难以发现。  
**修复建议**: 在 Rust 侧用 `chrono::Local::now()` 生成时间字符串，传入参数。

### 8. Telegram 回调处理中积分审批同样无原子性
**文件**: `src/telegram.rs:221-234`  
**问题**: `process_callback` 中 `update_weekly_quota` + `approve_request` 非原子。与 P0-2 同类问题。  
**修复建议**: 同 P0-2，在 DB 层实现事务。

---

## P2 — 改进建议

### 9. ROS 密码明文存储
**文件**: `src/ros/client.rs:8,22`  
**问题**: `pass: String` 字段明文保存 ROS 密码（`890405`），可通过 debug 日志泄露。  
**修复建议**: 使用环境变量或加密配置读取；Rust 的 `Secret` 类型或 `zeroize` crate 清理内存。

### 10. API 无认证鉴权
**文件**: `src/main.rs`（路由定义）  
**问题**: 所有 `/api/*` 端点无 auth 中间件，局域网内任何设备可修改设置、封禁设备、操作积分。  
**修复建议**: 至少添加 API Key 或 Basic Auth 保护写操作端点。

### 11. DNS 采集注释与实际不一致
**文件**: `src/detector/dns_collector.rs:15,38`  
**问题**: 注释写"拉最近1000条"但实际 `limit=200`；注释写"最近 2 分钟"但 cutoff 是 1 分钟。  
**修复建议**: 更正注释或调整代码。

### 12. Telegram Bot Token 出现在 URL 中
**文件**: `src/telegram.rs:45,76,160,266`  
**问题**: `format!("https://api.telegram.org/bot{}/sendMessage", token)` — token 可能被 reqwest 或 tracing 日志记录。  
**修复建议**: 使用 header 而非 URL 传递 token（Telegram API 不支持，但可确保日志不打印完整 URL）。

### 13. `ROS REST API` 响应错误静默忽略
**文件**: `src/ros/client.rs:30-33`  
**问题**: 所有 HTTP/JSON 错误被 `.ok()?` 吞掉，返回 `None`，调用者无法区分网络错误、401 认证失败、500 服务端错误。  
**修复建议**: 返回 `Result<Option<Value>, Error>` 或至少在 `None` 时记录日志。

### 14. `is_in_time_window` 对无效时间字符串静默返回 false
**文件**: `src/config.rs:115-119`  
**问题**: 若时间格式非法（如 `"abc"`），`parse` 失败返回 0，导致 `start=0, end=0`，结果为 `false`（被封禁），可能意外禁用设备。  
**修复建议**: 返回 `Result` 或在解析失败时返回默认窗口。

### 15. `switch_device` 绕过防火墙状态缓存
**文件**: `src/api/control.rs:57-80`  
**问题**: `switch_device` 直接调用 `fw::block_ip`/`fw::unblock_ip`，不经过 `scheduler/collect.rs` 中的 `FW_STATE` 缓存，可能导致缓存与实际状态不一致。  
**修复建议**: 统一通过缓存层操作，或在手动操作后同步更新 `FW_STATE`。

### 16. `block()` 函数存在 TOCTOU 竞争
**文件**: `src/scheduler/collect.rs:19-31`  
**问题**: `cache.get` 检查后 `drop(cache)`，再 `fw::block_ip`，最后 `FW_STATE.write().await.insert`。在 drop 和 insert 之间，另一个任务可能也通过检查并重复执行 block。  
**修复建议**: 保持锁持有直到操作完成，或使用 `tokio::sync::Mutex` + 单次检查。

---

## 未发现的问题（已验证安全）

- **SQL 注入（参数化查询）**: `db.rs` 中所有用户输入查询均使用 `rusqlite::params![]` 参数化，无拼接风险（除 P0-1 的列名拼接）。
- **未使用导入**: 无 `#[allow(dead_code)]`，代码干净。
- **硬编码敏感信息**: ROS 密码通过环境变量传入（`client.rs:12` 接收参数），DNS 注释中的 `10.1.1.97` 仅为示例说明，非实际代码。
