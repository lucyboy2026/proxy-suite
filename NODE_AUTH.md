# 节点设备绑定鉴权（node-auth）

FlClash 的 node-auth 客户端实现「设备绑定 + 服务端授权」的节点鉴权。它与 Clash Verge Rev 客户端共用同一套
Auth Server 协议（服务端代码见 [`devin.ai001/server`](https://github.com/lucyboy2026/devin.ai001/tree/main/server)），
即同一台鉴权服务器可同时给 FlClash 与 Clash Verge 两端发放凭据。

## 工作原理

1. **注册** —— 用户输入「服务器地址 + 邮箱 + 密码」。客户端计算本机**设备指纹**，调用 `POST /register`
   创建 `pending` 用户，等待管理员在服务端后台授权。
2. **登录** —— 管理员授权后，`POST /login` 校验账号并绑定当前设备，签发一个 ≤7 天的 **64-hex 设备 Token**，
   连同账号期限、设备数等信息持久化到本地会话。
3. **注入** —— 生成内核配置时，把该 Token 写入每个 `type: hysteria2`（含旧版 `hysteria`）节点的 `password` 字段，
   使节点连接与「设备 + 账号」绑定。
4. **静默续期** —— Token 临近过期时（默认剩余 2 天内）用本地保存的密码重新登录换取新 Token，并重新注入配置。

## 关键代码

| 关注点 | 位置 |
| --- | --- |
| 客户端逻辑（注册/登录/续期/设备指纹） | `lib/common/node_auth.dart` |
| 本地会话模型（含 `loginEmail`/`password`/过期判定） | `lib/models/node_auth.dart` |
| 登录/状态 UI | `lib/views/node_auth.dart` |
| Token 注入 hysteria2 `password` | `lib/providers/action.dart` → `injectNodeAuthToken()`（在 `makeRealProfileTask` 前调用） |
| 续期调度（启动一次 + 每 12 小时） | `lib/manager/core_manager.dart` |

## 设备指纹

`NodeAuth.deviceFingerprint()` 取 `SHA-256(installId | 硬件标识)`：

- `installId`：首次启动生成的 16 字节随机数，持久化在 `SharedPreferences`，保证跨重启稳定。
- 硬件标识：按平台取 `device_info_plus` 字段（Android 的 brand/model/id、Windows 的 computerName/deviceId 等），
  取不到时回退为平台名。

因此指纹**每设备/每安装唯一且稳定**，是「设备绑定」与设备数上限的依据。

## 续期策略

- 阈值：`NodeAuth.renewWindow = Duration(days: 2)`，与 Clash Verge 客户端的 `RENEW_BEFORE` 对齐。
- 调度：`CoreManager` 在启动后（post-frame）续期一次，之后每 `Duration(hours: 12)` 触发一次。
- 续期为**尽力而为**：`renewIfNeeded()` 吞掉所有网络/服务端异常，绝不让定时器或配置流程崩溃；
  仅当确实换到新 Token 时返回 `true`，并触发 `updateConfigDebounce()` 重新注入。
- 续期用 `NodeAuthSession.loginEmail`（用户实际输入的邮箱），不依赖服务端返回的 `username`；
  旧会话回退到 `email`，无需迁移。

> **安全提示**：为支持静默续期，账号密码会明文存于本地 `SharedPreferences`（与 Clash Verge 参考客户端一致），
> UI 不展示、日志不打印。如需更强保护，可改用平台安全存储（Keystore/Keychain/libsecret）。

## 对接的服务端接口

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| POST | `/register` | `{email, password, device_fp, platform}` → `202 {status, message}` |
| POST | `/login` | `{email, password, device_fp, platform}` → `{token, expires_at, username, max_devices, active_devices, account_expires_at, subscription_url}` |
| GET | `/sub/:key` | 固定订阅链接，登录后据 `subscription_url` 自动拉取 |

非 2xx 响应时服务端以纯文本返回错误原因，客户端用 `NodeAuthException` 透出给用户。

## 测试

```bash
flutter test test/models/node_auth_test.dart
flutter test test/common/node_auth_test.dart
```

覆盖会话编解码与回退、Token/账号过期判定、续期窗口判断、服务器地址归一化等。
