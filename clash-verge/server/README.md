# nodeauth-server — 设备绑定两步鉴权服务端（组件一）

Clash Verge 客户端「节点设备绑定两步鉴权」的服务端，配合仓库内 `src-tauri/src/feat/node_auth.rs`（组件二/客户端）使用。

- 语言/框架：Rust + axum + SQLite（sqlx），单可执行文件，便于部署到 VPS。
- 通知：Gmail / 通用 SMTP（lettre）+ Telegram Bot。

## 功能

客户端 API：

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| POST | `/register` | 注册（邮箱+密码+设备指纹）→ 建 pending 用户 → 通知管理员 |
| POST | `/login` | 校验账号、绑定设备、签发 64-hex 设备 Token（≤7 天） |
| GET | `/config?token=` | 下发该用户的 Clash YAML（hysteria2 `password` 注入 Token） |
| GET | `/sub/:key` | 长期固定订阅链接，自动注入该用户最近活跃设备的 Token |
| POST | `/auth` | hysteria2 HTTP 鉴权回调，返回 `{ok,id}` |
| POST | `/tg/webhook` | Telegram 审批按钮回调 |
| GET | `/healthz` | 健康检查 |

Web 后台（`/admin`，服务端渲染）：

- 概览：总用户 / 正常 / 待审核 / 已过期
- 用户与设备列表
- 授权（设备数 + 有效天数，0=长期）、续期、停用/恢复、解绑设备、删除
- 订阅模板编辑（占位符 `__NODE_TOKEN__`）

## 设计要点

- 设备 Token = 32 字节随机数的 hex（64 字符），按「用户+设备指纹」绑定，存于 `devices.token`。
- Token 过期时间 = `min(now + TOKEN_TTL_DAYS, 账号到期)`；客户端按此静默续期。
- 登录响应额外返回 `account_expires_at`（账号期限）与 `max_devices/active_devices`，供客户端展示。
- 密码用 Argon2 哈希存储；管理员会话为内存级 Cookie。

## 本地运行

```bash
cd server
cp .env.example .env   # 按需修改
cargo run              # 默认监听 0.0.0.0:8080，SQLite 落在 data/nodeauth.db
```

配置项见 [`.env.example`](./.env.example)。部署到 VPS 见 [`DEPLOY.md`](./DEPLOY.md)。

## 测试

```bash
cd server
cargo test            # 单元 + 集成测试（25 个）
cargo fmt --check
cargo clippy --all-targets
```

- **单元测试**（各模块内 `#[cfg(test)]`）：Argon2 哈希往返与加盐、Token 生成格式与唯一性、管理员会话过期回收、Clash 模板 `__NODE_TOKEN__` 注入、以及用真实 `init_pool` 起独立 SQLite 的数据层查询（订阅 key 幂等、设备计数忽略已禁用、最近活跃 Token 选取等）。
- **集成测试**（`src/main.rs` 的 `integration_tests`）：把整套路由起在本机随机端口上，用真实 HTTP 走通 `注册 → 授权前登录被拒 → 管理员授权 → 登录拿 Token → /config 与 /sub 注入 Token → hysteria2 /auth 回调` 全流程，并覆盖错误凭据、无效 Token、设备数上限等负向用例。
- 每个 DB/集成用例使用独立临时 SQLite 文件，互不干扰、可并行。
