<div>

[**English**](README.md)

</div>

## FlClash

[![Downloads](https://img.shields.io/github/downloads/chen08209/FlClash/total?style=flat-square&logo=github)](https://github.com/chen08209/FlClash/releases/)[![Last Version](https://img.shields.io/github/release/chen08209/FlClash/all.svg?style=flat-square)](https://github.com/chen08209/FlClash/releases/)[![License](https://img.shields.io/github/license/chen08209/FlClash?style=flat-square)](LICENSE)

[![Channel](https://img.shields.io/badge/Telegram-Channel-blue?style=flat-square&logo=telegram)](https://t.me/FlClash)

基于ClashMeta的多平台代理客户端，简单易用，开源无广告。

on Desktop:
<p style="text-align: center;">
    <img alt="desktop" src="snapshots/desktop.gif">
</p>

on Mobile:
<p style="text-align: center;">
    <img alt="mobile" src="snapshots/mobile.gif">
</p>

## Features

✈️ 多平台: Android, Windows, macOS and Linux

💻 自适应多个屏幕尺寸,多种颜色主题可供选择

💡 基本 Material You 设计, 类[Surfboard](https://github.com/getsurfboard/surfboard)用户界面

☁️ 支持通过WebDAV同步数据

✨ 支持一键导入订阅, 深色模式

🔐 节点设备绑定鉴权（本分支特性）：邮箱/密码 + 设备指纹换取可续期的 7 天 Token，自动注入 hysteria2 `password`

## 节点设备绑定鉴权（本分支特性）

本分支新增「设备绑定 + 服务端授权」的节点鉴权系统，与 [Clash Verge Rev 客户端](https://github.com/lucyboy2026/devin.ai001)
共用同一台 Auth Server（同一台服务器可同时给 FlClash 与 Clash Verge 两端发凭据）：
注册 → 管理员授权 → 登录绑定设备并签发 ≤7 天 Token → Token 注入每个 hysteria2 节点的 `password` 并在临近过期时静默续期。

**端到端使用流程：**

1. **部署 Auth Server** —— 单个 Rust 可执行文件（axum + SQLite），见
   [`devin.ai001/server`](https://github.com/lucyboy2026/devin.ai001/tree/main/server) 及其 `DEPLOY.md`（systemd + Caddy 自动 HTTPS）。
2. **注册** —— 在 FlClash 的节点鉴权界面填入「服务器地址 + 邮箱 + 密码」→ 注册，创建一个绑定本机设备指纹的 `pending` 账号。
3. **授权** —— 管理员在服务端 `/admin` 后台通过该账号（设定设备数与有效期）。
4. **登录** —— 登录后 FlClash 领取 ≤7 天 Token，拉取订阅并把 Token 注入每个 hysteria2 节点的 `password`，到期前自动续期。

实现细节（设备指纹、续期调度、服务端接口）见 [`NODE_AUTH.md`](NODE_AUTH.md)。

## Use

### Linux

⚠️ 使用前请确保安装以下依赖

   ```bash
    sudo apt-get install libayatana-appindicator3-dev
    sudo apt-get install libkeybinder-3.0-dev
   ```

### Android

支持下列操作

   ```bash
    com.follow.clash.action.START
    
    com.follow.clash.action.STOP
    
    com.follow.clash.action.TOGGLE
   ```

## Download

<a href="https://chen08209.github.io/FlClash-fdroid-repo/repo?fingerprint=789D6D32668712EF7672F9E58DEEB15FBD6DCEEC5AE7A4371EA72F2AAE8A12FD"><img alt="Get it on F-Droid" src="snapshots/get-it-on-fdroid.svg" width="200px"/></a> <a href="https://github.com/chen08209/FlClash/releases"><img alt="Get it on GitHub" src="snapshots/get-it-on-github.svg" width="200px"/></a>

## 发版（本分支）

发版由 **GitHub Actions**（`.github/workflows/build.yaml`）在**推送 `v*` tag** 时触发：跑测试 → 构建
Android / Windows / macOS / Linux（amd64 + arm64）→ 通过 `softprops/action-gh-release` 发布带产物的 GitHub Release。

```bash
git tag v0.1.0
git push origin v0.1.0   # 触发 build.yaml -> 全平台构建 -> 创建 Release
```

触发 tag 构建前需在仓库 **Secrets**（Settings → Secrets and variables → Actions）配置：

| Secret | 用途 |
| --- | --- |
| `KEYSTORE`、`KEY_ALIAS`、`STORE_PASSWORD`、`KEY_PASSWORD` | Android APK 签名（base64 keystore）|
| `SERVICE_JSON` | Android `google-services.json`（base64）|
| `TELEGRAM_API_ID`、`TELEGRAM_API_HASH`、`TELEGRAM_BOT_TOKEN` | 可选，发版通知 |
| `SSH_DEPLOY_KEY` | 可选，部署步骤 |

> 注意：构建会以 git **submodule** 拉取 mihomo Go 内核（`core/Clash.Meta`），CI 用 `submodules: recursive` 检出，需保证子模块可访问。
> 暂时不需要 Android 的话，可临时从构建矩阵里去掉 `android` 项，先发桌面端而不配签名密钥。

## Build

1. 更新 submodules
   ```bash
   git submodule update --init --recursive
   ```

2. 安装 `Flutter` 以及 `Golang` 环境

3. 构建应用

    - android

        1. 安装  `Android SDK` ,  `Android NDK`

        2. 设置 `ANDROID_NDK` 环境变量

        3. 运行构建脚本

           ```bash
           dart setup.dart android
           ```

    - windows

        1. 你需要一个windows客户端

        2. 安装 `GCC`，`Inno Setup`

        3. 运行构建脚本

           ```bash
           dart setup.dart windows
           ```

    - linux

        1. 你需要一个linux客户端

        2. 依赖会由 setup 脚本自动安装，也可以手动安装：
           ```bash
           sudo apt-get install -y libayatana-appindicator3-dev libkeybinder-3.0-dev
           ```

        3. 运行构建脚本

           ```bash
           dart setup.dart linux
           ```

    - macOS

        1. 你需要一个macOS客户端

        2. 运行构建脚本

           ```bash
           dart setup.dart macos
           ```

## Star

支持开发者的最简单方式是点击页面顶部的星标（⭐）。

<p style="text-align: center;">
    <a href="https://api.star-history.com/svg?repos=chen08209/FlClash&Date">
        <img alt="start" width=50% src="https://api.star-history.com/svg?repos=chen08209/FlClash&Date"/>
    </a>
</p>
