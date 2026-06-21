# proxy-suite

设备绑定节点鉴权系统的 **monorepo**：一台自建鉴权服务端 + 两个客户端，按账号/设备粒度分发与回收 hysteria2 节点访问权。三个组件共用同一套 node-auth 协议与同一台服务端。

```
proxy-suite/
├── clash-verge/        # Clash Verge Rev 分支（Tauri 桌面端，Win/macOS/Linux）
│   └── server/         # 鉴权服务端（Rust + axum + SQLite，单可执行文件）—— 系统的核心
├── flclash/            # FlClash 分支（Flutter 多端，Android/Win/macOS/Linux）
└── .github/workflows/  # monorepo 根 CI（GitHub 只识别根目录下的 workflow）
```

> 本仓库由两个上游 fork 通过 `git subtree` 合并而来，**完整保留各自历史**：
> - `clash-verge/` = [lucyboy2026/devin.ai001](https://github.com/lucyboy2026/devin.ai001)
> - `flclash/` = [lucyboy2026/flclash-nodeauth](https://github.com/lucyboy2026/flclash-nodeauth)（`upstream` 分支）

## 三个组件

| 组件 | 位置 | 技术栈 | 作用 |
| --- | --- | --- | --- |
| **鉴权服务端** | `clash-verge/server/` | Rust + axum + SQLite | 注册/登录/签发 7 天设备 Token、`/admin` 后台、hysteria2 `/auth` 回调、`/sub` 订阅下发 |
| **桌面客户端** | `clash-verge/` | Tauri（Rust + TS） | Clash Verge Rev 桌面端，内置 node-auth 客户端 |
| **多端客户端** | `flclash/` | Flutter（Dart + Go 内核） | FlClash，**唯一支持 Android**，含 node-auth |

工作流：客户端注册 → 管理员在 `/admin` 授权 → 登录拿设备 Token → Token 自动注入每个 hysteria2 节点的 `password` → 连接时 hysteria2 回调 `/auth` 校验。详见 `clash-verge/server/README.md`、`flclash/NODE_AUTH.md`。

## 统一管理（重点）
**节点列表不写在客户端里**，而是由服务端 `/admin` 的「订阅模板」集中下发。加 / 删节点、改 IP **只改后台一处，两个客户端都自动生效，无需改代码或重发版**。详见 `clash-verge/server/docs/multi-node.md`。

## 克隆（含子模块）
FlClash 依赖 3 个 git 子模块（mihomo 内核等），已提升到本仓库根 `.gitmodules`：
```bash
git clone --recurse-submodules <repo-url>
# 或克隆后：
git submodule update --init --recursive
```

## 各组件构建

### 鉴权服务端
```bash
cd clash-verge/server
cp .env.example .env       # 改管理员密码、域名等
cargo build --release      # 产物 target/release/nodeauth-server
cargo test                 # 25 个单元+集成测试
```
VPS 部署（systemd + Caddy 自动 HTTPS）见 `clash-verge/server/DEPLOY.md`。

### 桌面客户端（Clash Verge Rev）
```bash
cd clash-verge
pnpm i && pnpm run prebuild && pnpm dev
```

### 多端客户端（FlClash）
```bash
cd flclash
git submodule update --init --recursive
flutter pub get
dart setup.dart <android|windows|linux|macos>
```
Linux 依赖：`sudo apt-get install libayatana-appindicator3-dev libkeybinder-3.0-dev`。

## 发版（CI）
GitHub Actions 只识别**根目录** `.github/workflows/`，因此发版用**标签前缀**区分两端：

| 触发 | workflow | 产物 |
| --- | --- | --- |
| `workflow_dispatch` / 推 `verge-v*` 标签 | `verge-build-installers.yml` | Clash Verge Rev 桌面安装包 |
| 推 `flclash-v*` 标签 | `flclash-build.yml` | FlClash 全平台安装包（含 Android） |
| PR / push 改动 `clash-verge/server/**` | `server-ci.yml` | 服务端 fmt/clippy/test |

```bash
git tag verge-v2.5.2   && git push origin verge-v2.5.2     # 出桌面端
git tag flclash-v0.1.2 && git push origin flclash-v0.1.2   # 出 FlClash
```

> 各子项目**原始的** workflow 仍保留在 `clash-verge/.github/workflows/` 与 `flclash/.github/workflows/` 下作为参考（不在仓库根，GitHub 不会执行），可按需继续移植到根目录。

## License
GPL-3.0（沿用上游）。
