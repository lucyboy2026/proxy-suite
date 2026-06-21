<h1 align="center">
  <img src="./src-tauri/icons/icon.png" alt="Clash" width="128" />
  <br>
  Clash Verge Rev · 节点设备绑定鉴权分支
  <br>
</h1>

<h3 align="center">
基于 <a href="https://github.com/tauri-apps/tauri">Tauri</a> 的 Clash Meta GUI，额外内置「设备绑定 + 服务端授权」节点鉴权系统。
</h3>

<p align="center">
  Languages:
  <a href="./README.md">简体中文</a> ·
  <a href="./docs/README_en.md">English</a> ·
  <a href="./docs/README_es.md">Español</a> ·
  <a href="./docs/README_ru.md">Русский</a> ·
  <a href="./docs/README_ja.md">日本語</a> ·
  <a href="./docs/README_ko.md">한국어</a> ·
  <a href="./docs/README_fa.md">فارسی</a>
</p>

---

## 这个仓库是什么

本仓库是 [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev) 的一个分支，在保留上游全部桌面客户端能力的基础上，**新增了一整套「节点设备绑定 + 服务端授权」的鉴权系统**，用于按账号/设备粒度地分发与回收 hysteria2 节点的访问权限。

它由三部分组成：

| 组件 | 位置 | 说明 |
| --- | --- | --- |
| **桌面客户端（组件二）** | 本仓库 `src-tauri/` + `src/` | Clash Verge Rev 桌面端（Windows/macOS/Linux），内置 node-auth 客户端逻辑 `src-tauri/src/feat/node_auth.rs` |
| **鉴权服务端（组件一）** | 本仓库 [`server/`](./server/README.md) | Rust + axum + SQLite 单可执行文件，提供注册/登录/订阅/hysteria2 鉴权回调 + Web 管理后台 |
| **移动/多端客户端** | [lucyboy2026/flclash-nodeauth](https://github.com/lucyboy2026/flclash-nodeauth) | FlClash（Flutter，Android/Windows/macOS/Linux），与本仓库**共用同一套鉴权协议** |

三端共用一台鉴权服务器：同一个账号可在 Clash Verge 与 FlClash 上分别绑定设备、领取 Token。

## 工作原理（node-auth）

```
┌────────────┐  ① 注册(邮箱+密码+设备指纹)   ┌─────────────────────┐
│  客户端     │ ───────────────────────────▶ │  鉴权服务端 server/   │
│ (本仓库/    │  ② 管理员后台授权             │  axum + SQLite       │
│  FlClash)  │ ◀─────────────────────────── │  /admin 管理后台      │
│            │  ③ 登录 → 签发 7 天设备 Token  │                     │
└─────┬──────┘                               └──────────┬──────────┘
      │ ④ 把 Token 注入每个 hysteria2 节点的 password      │ ⑤ hysteria2 连接时
      ▼                                                  ▼   回调 /auth 校验
  本地内核配置(enhance)                              hysteria2 服务端
```

1. **注册**：用户输入「服务器地址 + 邮箱 + 密码」，客户端计算本机**设备指纹**，调用 `POST /register` 创建 `pending` 用户，等待管理员授权。
2. **授权**：管理员在 `/admin` 后台为该用户设定设备数上限与有效期并通过。
3. **登录**：`POST /login` 校验账号并绑定当前设备，签发一个 ≤7 天的 **64-hex 设备 Token**。
4. **注入**：生成内核配置时（`enhance` 阶段），把 Token 写入每个 `type: hysteria2` 节点的 `password` 字段，使节点连接与「设备 + 账号」绑定。
5. **回调校验**：hysteria2 服务端在每次连接时回调服务端 `/auth`，校验「Token → 设备 → 账号」有效后才放行。
6. **静默续期**：Token 临近过期（默认剩余 2 天内）时用本地保存的密码自动换取新 Token 并重新注入，用户无感。

## Features

- 基于性能强劲的 Rust 和 Tauri 2 框架
- 内置 [Clash.Meta(mihomo)](https://github.com/MetaCubeX/mihomo) 内核，并支持切换 `Alpha` 版本内核
- 简洁美观的用户界面，支持自定义主题颜色、代理组/托盘图标以及 `CSS Injection`
- 配置文件管理和增强（Merge 和 Script），配置文件语法提示
- 系统代理和守卫、`TUN(虚拟网卡)` 模式
- 可视化节点和规则编辑
- WebDav 配置备份和同步
- **🔐 节点设备绑定两步鉴权（本分支特性）**：自建鉴权服务端 + 客户端设备指纹换取 7 天 Token，连接时自动注入 hysteria2 `password`，支持静默续期

## 安装与使用

### 1. 部署鉴权服务端

服务端是单个 Rust 可执行文件，落地一个 SQLite 数据库即可运行。

```bash
cd server
cp .env.example .env   # 按需修改：管理员账号密码、Token 有效期、订阅域名等
cargo run              # 默认监听 0.0.0.0:8080，数据库落 data/nodeauth.db
```

- 配置项见 [`server/.env.example`](./server/.env.example)
- VPS 生产部署（systemd + Caddy 自动 HTTPS）见 [`server/DEPLOY.md`](./server/DEPLOY.md)
- 接口与管理后台说明见 [`server/README.md`](./server/README.md)

> **与 hysteria2 同机部署提示**：hysteria2 通常占用 **UDP 443（QUIC）**，而 Caddy 默认会尝试在 UDP 443 上起 HTTP/3 并报 `address already in use`。在 Caddyfile 全局段加 `servers { protocols h1 h2 }` 关闭 HTTP/3 即可让两者共存（详见 `server/DEPLOY.md` §5）。

### 2. 域名 + HTTPS（推荐）

把一个子域名（如 `auth.example.com`）解析到 VPS，并用 Caddy 反代到 `127.0.0.1:8080` 自动签发证书。之后在 `.env` 设置：

```ini
PUBLIC_BASE_URL=https://auth.example.com
```

客户端「服务器地址」即填该 HTTPS 域名。

### 3. 安装桌面客户端

请到本仓库的 [Release 页面](https://github.com/lucyboy2026/devin.ai001/releases) 下载对应安装包；支持 Windows (x64/arm64)、Linux (x64/arm64) 和 macOS 11+ (intel/apple)。

### 4. 客户端登录并使用

1. 打开客户端的「节点鉴权 / node-auth」入口，填入服务器地址、邮箱、密码 → **注册**。
2. 等待管理员在服务端 `/admin` 后台授权（可设置设备数与有效期）。
3. 授权后**登录**，客户端自动领取 Token 并拉取订阅。
4. 连接节点即可，Token 会自动注入 hysteria2 节点 `password` 并在到期前静默续期。

## Development

详见 [CONTRIBUTING.md](./CONTRIBUTING.md)。安装好 **Tauri** 所有前置依赖后：

```shell
pnpm i
pnpm run prebuild
pnpm dev
```

服务端开发与测试：

```shell
cd server
cargo run             # 本地运行
cargo test            # 单元 + 集成测试
cargo fmt --check && cargo clippy --all-targets
```

## Build & Release

本仓库通过 **GitHub Actions** 出包，发版由**推送版本 tag**触发：

- **完整发版** — `.github/workflows/release.yml`：推送 `vX.Y.Z` 形式的 tag（且必须来自 `main`、与 `package.json` 的 `version` 一致）后，自动为全平台构建安装包并发布 GitHub Release。
- **仅构建安装包（fork 友好）** — `.github/workflows/build-installers.yml`：在 Actions 页 **Run workflow** 手动触发，勾选目标平台，产物上传到 Actions Artifacts（无需发布/签名密钥，最快拿到可下载安装包）。

发版示例（当前 `package.json` 版本为 `2.5.2`）：

```bash
git checkout main && git pull
git tag v2.5.2
git push origin v2.5.2     # 触发 release.yml
```

## Acknowledgement

本分支基于 [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev)，并参考/受益于以下项目：

- [zzzgydi/clash-verge](https://github.com/zzzgydi/clash-verge)
- [tauri-apps/tauri](https://github.com/tauri-apps/tauri)
- [MetaCubeX/mihomo](https://github.com/MetaCubeX/mihomo)
- [vitejs/vite](https://github.com/vitejs/vite)

## License

GPL-3.0 License. See [License here](./LICENSE) for details.
