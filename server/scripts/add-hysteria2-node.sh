#!/usr/bin/env bash
#
# add-hysteria2-node.sh —— 在一台新的 Linux VPS 上安装 hysteria2，并把鉴权回调
# 指向 node-auth 鉴权服务端，使该节点纳入「设备绑定 + 7 天 Token」体系。
#
# 原理：客户端登录后拿到设备 Token，被注入到 Clash 节点的 password；hysteria2 收到
# 连接时回调鉴权服务端的 /auth，服务端校验「Token → 设备 → 账号」有效后才放行。
# 对应服务端源码：server/src/routes/client.rs 的 hysteria_auth (`POST /auth`)。
#
# 用法（在新 VPS 上以 root 运行）：
#   AUTH_URL=https://auth.ai-like-1688.com/auth ./add-hysteria2-node.sh
#
# 可配置环境变量：
#   AUTH_URL   （必填）鉴权服务端 /auth 地址，例：https://auth.ai-like-1688.com/auth
#   PORT       （可选）hysteria2 监听 UDP 端口，默认 443
#   DOMAIN     （可选）本节点的域名；填了则用 ACME 申请正式证书（需该域名解析到本机、放行 TCP/UDP 443）。
#              不填则生成自签证书（客户端订阅里该节点需 skip-cert-verify: true）。
#   ACME_EMAIL （可选，配合 DOMAIN）ACME 注册邮箱，默认 admin@<DOMAIN>。
#   OBFS_PASSWORD （可选）启用 salamander 混淆的口令；不填则不开启混淆。
#
# 安装后会打印一段「订阅模板节点片段」，把它粘到后台「订阅模板」的 proxies 下即可。
set -euo pipefail

# ---------- 参数 ----------
AUTH_URL="${AUTH_URL:-}"
PORT="${PORT:-443}"
DOMAIN="${DOMAIN:-}"
ACME_EMAIL="${ACME_EMAIL:-}"
OBFS_PASSWORD="${OBFS_PASSWORD:-}"

CONFIG_DIR="/etc/hysteria"
CONFIG_FILE="${CONFIG_DIR}/config.yaml"

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
info()  { printf '\033[36m%s\033[0m\n' "$*"; }

if [[ "${EUID}" -ne 0 ]]; then
  red "请用 root 运行（sudo -i 后再执行）。"; exit 1
fi
if [[ -z "${AUTH_URL}" ]]; then
  red "缺少 AUTH_URL。示例：AUTH_URL=https://auth.ai-like-1688.com/auth $0"; exit 1
fi

# ---------- 1. 安装 hysteria2 ----------
info "[1/5] 安装 hysteria2 ..."
if ! command -v hysteria >/dev/null 2>&1; then
  bash <(curl -fsSL https://get.hy2.sh/)
else
  green "hysteria 已安装：$(hysteria version 2>/dev/null | head -1 || true)"
fi

# ---------- 2. 准备 TLS 证书 ----------
mkdir -p "${CONFIG_DIR}"
TLS_BLOCK=""
SERVER_SNI=""
SKIP_CERT_VERIFY="true"
PUBLIC_ADDR=""

if [[ -n "${DOMAIN}" ]]; then
  info "[2/5] 使用 ACME 正式证书（域名 ${DOMAIN}）..."
  [[ -z "${ACME_EMAIL}" ]] && ACME_EMAIL="admin@${DOMAIN}"
  TLS_BLOCK=$(cat <<EOF
acme:
  domains:
    - ${DOMAIN}
  email: ${ACME_EMAIL}
EOF
)
  SERVER_SNI="${DOMAIN}"
  SKIP_CERT_VERIFY="false"
  PUBLIC_ADDR="${DOMAIN}"
else
  info "[2/5] 未提供 DOMAIN —— 生成自签证书（客户端需 skip-cert-verify: true）..."
  IP="$(curl -fsSL https://api.ipify.org || hostname -I | awk '{print $1}')"
  SERVER_SNI="${IP}"
  PUBLIC_ADDR="${IP}"
  openssl req -x509 -nodes -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout "${CONFIG_DIR}/server.key" -out "${CONFIG_DIR}/server.crt" \
    -subj "/CN=${IP}" -days 3650 >/dev/null 2>&1
  chmod 600 "${CONFIG_DIR}/server.key"
  TLS_BLOCK=$(cat <<EOF
tls:
  cert: ${CONFIG_DIR}/server.crt
  key: ${CONFIG_DIR}/server.key
EOF
)
fi

# ---------- 3. 写 hysteria2 服务端配置 ----------
info "[3/5] 写入 ${CONFIG_FILE} ..."
OBFS_BLOCK=""
if [[ -n "${OBFS_PASSWORD}" ]]; then
  OBFS_BLOCK=$(cat <<EOF
obfs:
  type: salamander
  salamander:
    password: ${OBFS_PASSWORD}
EOF
)
fi

cat > "${CONFIG_FILE}" <<EOF
listen: :${PORT}

${TLS_BLOCK}
${OBFS_BLOCK}
# 鉴权改为 HTTP 后端，指向 node-auth 服务端 /auth。
# 客户端注入的设备 Token 会作为 hysteria2 的 auth 字段被回调校验。
auth:
  type: http
  http:
    url: ${AUTH_URL}

# 客户端连接时把流量转发出去（默认放行）。
masquerade:
  type: proxy
  proxy:
    url: https://www.bing.com
    rewriteHost: true
EOF

# ---------- 4. 放行端口 + 启动 ----------
info "[4/5] 放行 UDP ${PORT} 并启动服务 ..."
if command -v ufw >/dev/null 2>&1; then
  ufw allow "${PORT}"/udp >/dev/null 2>&1 || true
  [[ -n "${DOMAIN}" ]] && ufw allow "${PORT}"/tcp >/dev/null 2>&1 || true
fi
systemctl enable --now hysteria-server.service
sleep 1
systemctl restart hysteria-server.service
systemctl --no-pager status hysteria-server.service | head -5 || true

# ---------- 5. 自检：鉴权服务端是否可达 ----------
info "[5/5] 自检鉴权回调连通性（用一个假 Token，预期返回 ok:false）..."
RESP="$(curl -fsS -m 10 -X POST "${AUTH_URL}" \
  -H 'Content-Type: application/json' \
  -d '{"addr":"selftest","auth":"__invalid_selftest_token__","tx":0}' || true)"
if echo "${RESP}" | grep -q '"ok"'; then
  green "鉴权服务端可达，回包：${RESP}"
else
  red "⚠ 未能从本机访问 ${AUTH_URL}（回包：${RESP:-<空>}）。"
  red "  请检查：DNS 解析、鉴权服务端是否在线、防火墙；否则该节点无法校验 Token。"
fi

# ---------- 输出：可粘贴到后台「订阅模板」的节点片段 ----------
NODE_NAME="节点-${PUBLIC_ADDR}"
green ""
green "=============================================================="
green " 安装完成。把下面这段加进后台「订阅模板」的 proxies 列表里："
green "=============================================================="
cat <<EOF
  - name: "${NODE_NAME}"
    type: hysteria2
    server: ${PUBLIC_ADDR}
    port: ${PORT}
    password: __NODE_TOKEN__
    sni: ${SERVER_SNI}
    skip-cert-verify: ${SKIP_CERT_VERIFY}$( [[ -n "${OBFS_PASSWORD}" ]] && printf '\n    obfs: salamander\n    obfs-password: %s' "${OBFS_PASSWORD}" )
EOF
green "--------------------------------------------------------------"
green " 别忘了把 \"${NODE_NAME}\" 也加进 proxy-groups 的 proxies 列表。"
green " 保存模板后，用户「更新订阅」即可看到并连上本节点。"
green "=============================================================="
