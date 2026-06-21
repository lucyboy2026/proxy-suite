# 部署指南（VPS 77.73.8.38）

> 你从 Windows PowerShell `ssh` 登录到这台 **Linux** VPS 后，按下面逐条执行即可。
> 占位符（如域名、邮箱、Bot Token）按提示替换。

## 0. 登录

```powershell
# Windows PowerShell
ssh root@77.73.8.38
```

下面命令均在 VPS（Linux）上执行。

## 1. 安装依赖 & 拉取代码

```bash
apt-get update && apt-get install -y git curl build-essential pkg-config
# 安装 Rust（用于编译服务端）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"

# 拉取代码（本仓库）
git clone https://github.com/lucyboy2026/devin.ai001.git /opt/nodeauth
cd /opt/nodeauth/server
```

## 2. 编译

```bash
cargo build --release
# 产物：/opt/nodeauth/server/target/release/nodeauth-server
```

## 3. 配置 `.env`

```bash
cp .env.example .env
nano .env       # 或 vi
```

需要填写的关键字段（其余可用默认值）：

```ini
BIND_ADDR=127.0.0.1:8080            # 反向代理在前面，绑定本地即可
PUBLIC_BASE_URL=https://你的域名     # 没有域名就填 http://77.73.8.38:8080
ADMIN_USERNAME=admin
ADMIN_PASSWORD=改成强密码            # 后台登录密码，务必修改

DEFAULT_MAX_DEVICES=1
DEFAULT_VALID_DAYS=30
TOKEN_TTL_DAYS=7

# ---- Gmail（应用专用密码）----
SMTP_HOST=smtp.gmail.com
SMTP_PORT=465
SMTP_USERNAME=你的@gmail.com
SMTP_PASSWORD=16位应用专用密码        # 见下方「如何获取」
SMTP_FROM=你的@gmail.com
ADMIN_EMAIL=收注册通知的邮箱

# ---- Telegram ----
TELEGRAM_BOT_TOKEN=从@BotFather获取
TELEGRAM_ADMIN_CHAT_ID=你的数字chat id   # 见下方「如何获取」
```

### 如何获取这些值

- **Gmail 应用专用密码**：Google 账号 → 安全性 → 两步验证（需先开启）→ 应用专用密码 → 生成 16 位密码，去掉空格填入 `SMTP_PASSWORD`。
- **Telegram Bot Token**：在 Telegram 找 `@BotFather` → `/newbot` → 拿到形如 `123456:ABC...` 的 token。
- **管理员 chat id**：把你的 bot 加为好友并发一条消息，然后访问
  `https://api.telegram.org/bot<TOKEN>/getUpdates`，在返回 JSON 里找 `chat.id`（一串数字）。

## 4. systemd 守护

```bash
cat >/etc/systemd/system/nodeauth.service <<'EOF'
[Unit]
Description=Clash Verge node-auth server
After=network.target

[Service]
WorkingDirectory=/opt/nodeauth/server
EnvironmentFile=/opt/nodeauth/server/.env
ExecStart=/opt/nodeauth/server/target/release/nodeauth-server
Restart=always
RestartSec=3
User=root

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now nodeauth
systemctl status nodeauth --no-pager
curl -s http://127.0.0.1:8080/healthz   # 应返回 ok
```

## 5. HTTPS（推荐用 Caddy，自动证书）

> 有域名时强烈建议；客户端默认会把不带 scheme 的地址补成 `https://`。

```bash
apt-get install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list
apt-get update && apt-get install -y caddy

cat >/etc/caddy/Caddyfile <<'EOF'
{
    # 若本机已有服务占用 UDP 443（如 hysteria2/QUIC），关闭 Caddy 的 HTTP/3，
    # 只用 TCP 80/443，避免 "listen udp :443: bind: address already in use"
    servers {
        protocols h1 h2
    }
}

你的域名 {
    reverse_proxy 127.0.0.1:8080
}
EOF
systemctl enable --now caddy
systemctl restart caddy
```

> ⚠️ **与 hysteria2 共存（UDP 443 冲突）**：本服务常和 hysteria2 部署在同一台机器，而 hysteria2 通常监听 **UDP 443（QUIC）**。Caddy 默认会尝试在 UDP 443 上同时起 HTTP/3 监听，于是启动失败并报 `listen udp :443: bind: address already in use`。上面 Caddyfile 全局段的 `protocols h1 h2` 即用于**关闭 HTTP/3**——HTTPS 仍走 TCP 443，与 hysteria2 的 UDP 443 互不冲突。可用 `ss -ulnp | grep :443` 查看 UDP 443 的占用方。

无域名（仅测试）：把 `.env` 的 `BIND_ADDR=0.0.0.0:8080`、`PUBLIC_BASE_URL=http://77.73.8.38:8080`，并放行防火墙端口 8080。客户端服务器地址填 `http://77.73.8.38:8080`。

## 6. 接入 hysteria2 节点（关键）

在你的 hysteria2 **服务端**配置里，把鉴权改为 HTTP 后端，指向本服务的 `/auth`：

```yaml
# hysteria2 server config.yaml
auth:
  type: http
  http:
    url: http://127.0.0.1:8080/auth   # 与本服务同机；或 https://你的域名/auth
```

原理：客户端登录后拿到设备 Token，写入节点 `password`；hysteria2 收到连接时回调 `/auth`，本服务校验「Token→设备→账号」有效后返回 `{"ok":true,"id":"<邮箱>#<设备>"}`，否则拒绝。

> **新增一台 VPS 节点**：在新机上跑 `scripts/add-hysteria2-node.sh`（自动装 hysteria2 + 配好指向本服务的 `/auth` 回调 + 自检连通性，并打印可粘贴的订阅模板节点片段）：
> ```bash
> AUTH_URL=https://你的域名/auth ./scripts/add-hysteria2-node.sh
> ```
> **挂多个不同 IP 的节点**：见 [`docs/multi-node.md`](docs/multi-node.md)。

## 7. Telegram Webhook（审批按钮）

部署完且有公网 HTTPS 后，注册一次 webhook：

```bash
curl "https://api.telegram.org/bot<TOKEN>/setWebhook?url=https://你的域名/tg/webhook"
```

之后有人注册，管理员会在 Telegram 收到带「✅ 同意 / ❌ 拒绝」按钮的消息，点击即授权（按默认设备数/期限）。也可在 Web 后台 `https://你的域名/admin` 精细授权。

## 8. 后台地址

- 管理后台：`https://你的域名/admin`（用 `.env` 里的 `ADMIN_USERNAME/ADMIN_PASSWORD` 登录）
- 订阅模板：后台「订阅模板」页，把示例换成你真实的 hysteria2 节点，节点 `password` 写 `__NODE_TOKEN__`。

## 9. 升级

```bash
cd /opt/nodeauth && git pull
cd server && cargo build --release
systemctl restart nodeauth
```
