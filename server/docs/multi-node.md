# 在后台添加多个节点（多台不同 IP 的 VPS）

本系统的节点列表**不写在客户端里**，而是由鉴权服务端后台的「订阅模板」决定。模板是一份标准 Clash YAML，下发时服务端会把其中的占位符 `__NODE_TOKEN__` 替换成用户当前设备的 Token。因此**新增 / 删除节点、改 IP 都只需改后台模板，客户端无需改代码或重新发版**，用户下次自动刷新订阅即可拿到新节点。

> 相关源码：`server/src/clash.rs`（模板与占位符替换）、`server/src/routes/client.rs`（`/sub/:key` 下发、`/auth` 鉴权回调）。

---

## 一、打开「订阅模板」页

1. 浏览器访问管理后台：`https://你的域名/admin`（本项目为 `https://auth.ai-like-1688.com/admin`），用 `.env` 里的 `ADMIN_USERNAME` / `ADMIN_PASSWORD` 登录。
2. 顶部导航点 **「订阅模板」**（路由 `/admin/template`）。
3. 页面是一个大文本框 + 「保存模板」按钮，里面就是会下发给所有用户的 Clash YAML。

---

## 二、把模板改成多节点

在 `proxies:` 下面**每台 VPS 写一条**，`server` 填各自的 IP（或域名），`password` 统一写占位符 `__NODE_TOKEN__`；然后把这些节点名都列进 `proxy-groups`。示例（3 台不同 IP）：

```yaml
mixed-port: 7890
allow-lan: false
mode: rule
log-level: info

proxies:
  - name: "美国-77.73.8.38"
    type: hysteria2
    server: 77.73.8.38          # ← VPS 1 的 IP
    port: 443
    password: __NODE_TOKEN__     # ← 每个节点都写这个占位符
    sni: 77.73.8.38
    skip-cert-verify: true       # 自签证书 / 纯 IP 节点需为 true（见下）
  - name: "日本-1.2.3.4"
    type: hysteria2
    server: 1.2.3.4             # ← VPS 2 的 IP（不同 IP）
    port: 443
    password: __NODE_TOKEN__
    sni: 1.2.3.4
    skip-cert-verify: true
  - name: "香港-5.6.7.8"
    type: hysteria2
    server: 5.6.7.8            # ← VPS 3
    port: 443
    password: __NODE_TOKEN__
    sni: 5.6.7.8
    skip-cert-verify: true

proxy-groups:
  - name: "PROXY"
    type: select                 # 手动切换；想自动选最快改成 url-test（见下）
    proxies:
      - "美国-77.73.8.38"
      - "日本-1.2.3.4"
      - "香港-5.6.7.8"
  # 可选：自动测速选最快
  - name: "AUTO"
    type: url-test
    url: http://www.gstatic.com/generate_204
    interval: 300
    tolerance: 50
    proxies:
      - "美国-77.73.8.38"
      - "日本-1.2.3.4"
      - "香港-5.6.7.8"

rules:
  - MATCH,PROXY
```

要点：

- **同一份订阅可挂任意多个节点**，每条填各自 IP。
- 该用户的 **Token 会被注入到所有节点的 `password`**——所以这些 VPS 上的 hysteria2 都必须把鉴权回调指向**同一台**鉴权服务端（见 `add-hysteria2-node.sh`）。
- 想「手动切节点」用 `type: select`；想「自动选最快」用 `type: url-test`（客户端会定时测速自动切换）。
- `sni` / `skip-cert-verify`：
  - **纯 IP 节点 + 自签证书**（`add-hysteria2-node.sh` 默认方式）→ `skip-cert-verify: true`，`sni` 随便填（填该 IP 即可）。
  - **节点有域名 + ACME 正式证书** → `skip-cert-verify: false`，`sni` 填该域名。

改完点 **「保存模板」**。

---

## 三、生效方式

- 保存后，用户**下次自动刷新订阅**（或在客户端手动「更新订阅」）就会拿到新的节点列表，**不用重装客户端**。
- 各平台一致：桌面端 Clash Verge Rev、移动/桌面端 FlClash 吃的都是这份标准 Clash 订阅，节点列表、手动切换、自动测速全部通用。

---

## 四、新增一台 VPS 节点的完整流程

1. 在新 VPS 上装好 hysteria2 并把鉴权回调指向你的鉴权服务端——直接用脚本：`server/scripts/add-hysteria2-node.sh`（见该脚本头部用法）。
2. 回到后台「订阅模板」，按上面的格式把这台新 VPS 的 IP 加进 `proxies` 和 `proxy-groups`，保存。
3. 用任意已授权账号在客户端「更新订阅」，确认能看到并连上新节点。

> 排查：若新节点连不上，先在新 VPS 上 `journalctl -u hysteria-server -f` 看日志；常见原因是鉴权回调 URL 填错、或鉴权服务端域名在新机上解析/网络不通（脚本结尾会自动做一次 `/auth` 连通性自检）。
