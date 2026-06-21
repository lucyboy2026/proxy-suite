<div>

[**简体中文**](README_zh_CN.md)

</div>

## FlClash

[![Downloads](https://img.shields.io/github/downloads/chen08209/FlClash/total?style=flat-square&logo=github)](https://github.com/chen08209/FlClash/releases/)[![Last Version](https://img.shields.io/github/release/chen08209/FlClash/all.svg?style=flat-square)](https://github.com/chen08209/FlClash/releases/)[![License](https://img.shields.io/github/license/chen08209/FlClash?style=flat-square)](LICENSE)

[![Channel](https://img.shields.io/badge/Telegram-Channel-blue?style=flat-square&logo=telegram)](https://t.me/FlClash)

A multi-platform proxy client based on ClashMeta, simple and easy to use, open-source and ad-free.

on Desktop:
<p style="text-align: center;">
    <img alt="desktop" src="snapshots/desktop.gif">
</p>

on Mobile:
<p style="text-align: center;">
    <img alt="mobile" src="snapshots/mobile.gif">
</p>

## Features

✈️ Multi-platform: Android, Windows, macOS and Linux

💻 Adaptive multiple screen sizes, Multiple color themes available

💡 Based on Material You Design, [Surfboard](https://github.com/getsurfboard/surfboard)-like UI

☁️ Supports data sync via WebDAV

✨ Support subscription link, Dark mode

🔐 Device-bound node auth (this fork): exchange email/password + a device fingerprint for a renewable 7-day token, auto-injected into hysteria2 `password`

## Node auth (this fork)

This fork adds a device-bound, server-authorized node authentication system, sharing one Auth Server with the
[Clash Verge Rev client](https://github.com/lucyboy2026/devin.ai001) (the same server issues credentials to both
FlClash and Clash Verge). Register → admin authorizes → login binds the device and issues a ≤7-day token → the token is
injected into every hysteria2 node's `password` and silently renewed near expiry.

**End-to-end usage:**

1. **Deploy the Auth Server** — single Rust binary (axum + SQLite). See
   [`devin.ai001/server`](https://github.com/lucyboy2026/devin.ai001/tree/main/server) and its `DEPLOY.md`
   (systemd + Caddy auto-HTTPS).
2. **Register** — in FlClash, open the node-auth screen, enter `server URL + email + password` → register. This creates a
   `pending` account bound to this device's fingerprint.
3. **Authorize** — the admin approves the account in the server's `/admin` panel (sets max devices + expiry).
4. **Login** — log in; FlClash receives a ≤7-day token, pulls the subscription, and injects the token into every
   hysteria2 node's `password`. The token is renewed automatically before expiry.

Implementation details (device fingerprint, renewal scheduling, server API) are in [`NODE_AUTH.md`](NODE_AUTH.md).

## Use

### Linux

⚠️ Make sure to install the following dependencies before using them

   ```bash
    sudo apt-get install libayatana-appindicator3-dev
    sudo apt-get install libkeybinder-3.0-dev
   ```

### Android

Support the following actions

   ```bash
    com.follow.clash.action.START
    
    com.follow.clash.action.STOP
    
    com.follow.clash.action.TOGGLE
   ```

## Download

<a href="https://chen08209.github.io/FlClash-fdroid-repo/repo?fingerprint=789D6D32668712EF7672F9E58DEEB15FBD6DCEEC5AE7A4371EA72F2AAE8A12FD"><img alt="Get it on F-Droid" src="snapshots/get-it-on-fdroid.svg" width="200px"/></a> <a href="https://github.com/chen08209/FlClash/releases"><img alt="Get it on GitHub" src="snapshots/get-it-on-github.svg" width="200px"/></a>

## Releases (this fork)

Releases are produced by **GitHub Actions** (`.github/workflows/build.yaml`) when a `v*` tag is pushed: it runs tests,
builds Android / Windows / macOS / Linux (amd64 + arm64), and publishes a GitHub Release with the artifacts via
`softprops/action-gh-release`.

```bash
git tag v0.1.0
git push origin v0.1.0   # triggers build.yaml -> builds all platforms -> creates the Release
```

Required repo **secrets** (Settings → Secrets and variables → Actions) before a tag build will succeed:

| Secret | Used for |
| --- | --- |
| `KEYSTORE`, `KEY_ALIAS`, `STORE_PASSWORD`, `KEY_PASSWORD` | Android APK signing (base64 keystore) |
| `SERVICE_JSON` | Android `google-services.json` (base64) |
| `TELEGRAM_API_ID`, `TELEGRAM_API_HASH`, `TELEGRAM_BOT_TOKEN` | optional release notification |
| `SSH_DEPLOY_KEY` | optional deploy step |

> Note: the build pulls the mihomo Go core as a git **submodule** (`core/Clash.Meta`); CI checks out with
> `submodules: recursive`, so make sure the submodule is reachable. If you don't need Android yet, you can temporarily
> drop the `android` entry from the build matrix to release the desktop platforms without signing secrets.

## Build

1. Update submodules
   ```bash
   git submodule update --init --recursive
   ```

2. Install `Flutter` and `Golang` environment

3. Build Application

    - android

        1. Install `Android SDK`, `Android NDK`

        2. Set `ANDROID_NDK` environment variable

        3. Run build script

           ```bash
           dart setup.dart android
           ```

    - windows

        1. Requires a Windows client

        2. Install `GCC`, `Inno Setup`

        3. Run build script

           ```bash
           dart setup.dart windows
           ```

    - linux

        1. Requires a Linux client

        2. Dependencies are auto-installed by setup script, or manually:
           ```bash
           sudo apt-get install -y libayatana-appindicator3-dev libkeybinder-3.0-dev
           ```

        3. Run build script

           ```bash
           dart setup.dart linux
           ```

    - macOS

        1. Requires a macOS client

        2. Run build script

           ```bash
           dart setup.dart macos
           ```

## Star

The easiest way to support developers is to click on the star (⭐) at the top of the page.

<p style="text-align: center;">
    <a href="https://api.star-history.com/svg?repos=chen08209/FlClash&Date">
        <img alt="start" width=50% src="https://api.star-history.com/svg?repos=chen08209/FlClash&Date"/>
    </a>
</p>