# 发布指南（RELEASE.md）

本仓库通过推送版本 tag 触发 GitHub Actions（`.github/workflows/build.yaml`）自动构建并发布
**Android / Windows / macOS / Linux** 安装包到 GitHub Release。

> 默认分支是 **`upstream`**（本仓库没有 `main`）。

---

## 一、构建产物（一次发布会生成）

| 平台 | 产物示例 |
| --- | --- |
| Android | `FlClash-<ver>-android-arm64-v8a.apk`、`-armeabi-v7a.apk`、`-x86_64.apk` |
| Windows | `FlClash-<ver>-windows-amd64-setup.exe`、`-windows-arm64-setup.exe` |
| macOS | `FlClash-<ver>-macos-arm64.dmg`、`-macos-amd64.dmg` |
| Linux | `FlClash-<ver>-linux-amd64.AppImage` / `.deb` / `.rpm`、`-linux-arm64.deb` |
| 校验 | `SHA256SUMS` |

构建矩阵（7 个目标）：android(ubuntu) / windows(amd64) / windows-11-arm(arm64) /
macos-15-intel(amd64) / macos-latest(arm64) / linux ubuntu-22.04(amd64) / linux ubuntu-24.04-arm(arm64)。

> 产物文件名里的 `<ver>` 取自 `pubspec.yaml` 的 `version`（例如 `0.8.93`），**不是** tag 名。
> 想让文件名版本号跟着变，请先改 `pubspec.yaml` 的 `version` 再发 tag。

---

## 二、一次性准备：Android 签名密钥（4 个仓库 Secret）

Android 要出**正式签名**包，需要在仓库
`Settings → Secrets and variables → Actions` 配置 4 个 secret：

| Secret 名 | 含义 |
| --- | --- |
| `KEYSTORE` | keystore.jks 文件的 **base64** 文本 |
| `KEY_ALIAS` | 密钥别名（如 `upload`） |
| `STORE_PASSWORD` | keystore 口令 |
| `KEY_PASSWORD` | 密钥口令（一般与 store 相同） |

> 不配这些也能构建，但会回退到 **debug 签名**、包名变成 `com.follow.clash.dev`，不适合正式分发。
> `google-services.json` 用仓库内已有的占位文件即可，**不需要** `SERVICE_JSON`。

### 生成 keystore

**Windows / PowerShell**（keytool 来自 JDK，可 `winget install --id EclipseAdoptium.Temurin.21.JDK -e --source winget` 安装）：

```powershell
# 在一个独立目录里生成（别放进 git 仓库）
mkdir "$env:USERPROFILE\flclash-keystore" -Force; cd "$env:USERPROFILE\flclash-keystore"
keytool -genkeypair -v -keystore keystore.jks -alias upload -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=FlClash, OU=Dev, O=Personal, L=City, ST=State, C=CN"

# 转 base64（注意 .NET 方法要用绝对路径）
[Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $PWD 'keystore.jks'))) | Set-Content -NoNewline (Join-Path $PWD 'keystore.b64')
notepad keystore.b64   # 全选复制 = KEYSTORE 的值
```

**Linux / macOS**：

```bash
keytool -genkeypair -v -keystore keystore.jks -alias upload \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -dname "CN=FlClash, OU=Dev, O=Personal, L=City, ST=State, C=CN"

base64 -w0 keystore.jks > keystore.b64   # macOS: base64 -i keystore.jks -o keystore.b64
```

把值填进 4 个 secret：`KEYSTORE`=keystore.b64 全文、`KEY_ALIAS`=`upload`、
`STORE_PASSWORD` / `KEY_PASSWORD`=你设置的口令。

> ⚠️ **务必长期备份 `keystore.jks` 和口令**：App 升级必须用同一 keystore 签名，否则用户无法覆盖更新。

---

## 三、发布新版本（出四端包）

1. （可选）需要改版本号时，先改 `pubspec.yaml` 的 `version` 并合并到 `upstream`。
2. 打开 `Releases → Draft a new release`：
   `https://github.com/lucyboy2026/flclash-nodeauth/releases/new`
3. **Choose a tag** 输入新版本号，例如 `v0.1.1`，点 **“Create new tag on publish”**。
   - tag **不能带 `-`**（如 `v0.1.1-rc.1` 是预发布，**不会**创建 Release，只上传临时 artifacts）。
4. **Target** 选 `upstream`。
5. 点 **Publish release**。

发布后会创建 tag → 触发 `build` 工作流（`Test → build(7 目标) → upload`，约 20–40 分钟）→
安装包自动上传到该 Release。

进度查看：`https://github.com/lucyboy2026/flclash-nodeauth/actions`

---

## 四、本 fork 对构建脚本的改动（与上游 chen08209/FlClash 的差异）

为让 fork 能独立出包，`build.yaml` 相比上游移除了这些**仅适用于上游基础设施**的部分：

- 删除 `changelog` job（它 checkout 不存在的 `main` 并自动回推 commit）。
- 删除 `upload` job 里的 **Telegram** 服务容器 + “Push to telegram” 步骤（需上游的 TELEGRAM_* secret）。
- 删除 **F-Droid** 推送步骤（硬编码推到 `chen08209/FlClash-fdroid-repo`，需上游 SSH_DEPLOY_KEY）。
- 删除 “Setup Android Signing” 里覆盖 `google-services.json` 的那一行（改用仓库内已有占位文件）。

保留的流程：`test → build → upload(softprops/action-gh-release 发布 Release)`。

---

## 五、常见问题

- **某个平台 job 变红**：点开该 job 看日志。第一次最容易出问题的是 Android 签名（检查 4 个 secret 是否齐全且正确）。
- **Release 里只有 Source code、没有安装包**：说明用了带 `-` 的预发布 tag，或 `upload` job 失败；改用不带 `-` 的 tag 重发。
- **想撤销一次发布**：在 Release 页删除该 Release，并删除对应 tag（`git push origin :refs/tags/vX.Y.Z` 或网页 Tags 页删除），再重发。
