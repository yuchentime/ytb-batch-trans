---
status: current
layer: platform
domain: release-and-distribution
canonical_for:
  - release-and-distribution-facts
related:
  - docs/current/domains/toolchain/api-contract.md
  - docs/current/platform/env.md
last_verified: 2026-09-17
---

# 发布与分发

## Purpose

记录版本号来源、打包目标、CI 流水线、官网/清单托管与第三方分发渠道，是发布类改动与故障排查的对照表。

## Current Behavior

### 版本号

同一个版本号出现在三处，必须同步：

| 文件 | 字段 | 当前值 |
| --- | --- | --- |
| `package.json` | `version` | `3.2.1` |
| `src-tauri/tauri.conf.json` | `version` | `3.2.1` |
| `src-tauri/Cargo.toml` | `package.version` | `3.2.1` |

`__APP_VERSION__`（前端）取 `npm_package_version`；后端 `app.package_info().version` 取 `tauri.conf.json`；
Sentry release 名取 crate 版本。

### 打包目标（`tauri.conf.json` → `bundle`）

| 项 | 值 |
| --- | --- |
| `targets` | `nsis`、`app`、`dmg`、`appimage`、`deb`、`rpm` |
| `createUpdaterArtifacts` | `true`（生成 updater 需要的产物与签名） |
| 图标 | `icons/32x32.png`、`128x128.png`、`128x128@2x.png`、`icon.icns`、`icon.ico` |
| 附带资源 | `licenses/3rdpartylicenses.txt`（构建前由 `npm run licenses` 生成） |
| macOS | 签名身份 `Developer ID Application` |
| Windows NSIS | 语言选择器，含 11 种语言（与 locale 不完全一致） |
| identifier | `com.jelleglebbeek.youtube-dl-gui`（同时是钥匙串 service 名） |

### 发布流水线（`.github/workflows/publish.yml`）

- 触发：push 到 `release` 分支或手动。
- 矩阵：6 个目标（macos aarch64/x86_64、ubuntu-24.04 x64/arm、windows x64/arm64）。
- 步骤要点：先 `npm ci` 与隔离前端构建（CI 的 Rust 流程需要 `dist-isolation`），再 `tauri build`（Windows 使用证书 secrets 签名）。
- 产物名形如 `Open.Video.Downloader_<version>_x64-setup.exe`、`Open.Video.Downloader_<version>_<arch>.dmg`、`..._amd64.AppImage` 等（README 的下载表即此）。
- 应用内更新从此 Release 的 `latest.json` 读取（`tauri.conf.json` 的 `plugins.updater.endpoints`）。

### 官网与二进制清单（`.github/workflows/pages.yml`）

- 同时 checkout `release`（跑脚本）与 `main`（放站点内容）。
- 流程：`npm run manifest:gen`（对每个产物算 sha256）→ `manifest:sign`（minisign/ed25519 私钥）→ `manifest:verify`（公钥校验）→ 复制 `docs/manifest/*` 到 main 的 `docs/manifest/` → 上传 `main/docs` 作为 Pages artifact → 部署。
- 触发：`release` 分支的 docs/scripts/package 变更、每日 0 点定时、手动。
- 站点地址：https://jely2002.github.io/youtube-dl-gui ；清单地址：`https://jely2002.github.io/youtube-dl-gui/manifest/manifest.json`。
- `docs/manifest/` 在 `.gitignore` 中（仅构建产物），仓库中的 `docs/` 其余内容是站点与本文档库。

### 第三方分发（`.github/workflows/distribute.yml`，手动触发）

| 渠道 | 动作 |
| --- | --- |
| winget | `wingetcreate update jely2002.youtube-dl-gui --version <v> --urls <x64> <arm64>` |
| Snap Store | `snapcraft promote open-video-downloader --from-channel=edge --to-channel=stable` |
| Microsoft Store | `msix.yml`（MSIX 打包与提交） |

`snapcraft.yaml` 描述 snap 构建（严格模式、`SNAP_USER_COMMON` 用于共享 bin 目录）。

### 许可证与合规

- 应用自身：AGPL-3.0（`LICENSE`）。
- 第三方：`npm run licenses`（npm 与 cargo 两路 → `licenses/3rdpartylicenses.txt`），并作为打包资源随应用分发。
- `licenses/` 目录被 `.gitignore` 忽略（构建期生成）。

## Boundaries

- 没有自动版本递增脚本；改版本号需人工同步三处。
- 没有 beta/nightly 通道；`release` 分支即稳定通道。
- 不做代码混淆/加固；仅 Windows 代码签名与 macOS 公证（由 CI secrets 提供）。

## Contracts

- 清单格式与验签：`../domains/toolchain/api-contract.md`。
- 环境变量（签名与分发 secrets）：`env.md`。
- 发布前检查：`../evals/release-checklist.md`。

## Failure And Edge Cases

- 版本号三处不一致会导致：更新检查永远认为有新版本（或永远不提示）、Release 产物名与 winget 期望不符。
- `pages.yml` 从 `release` 分支生成清单，因此**只改 main 的 `scripts/sources.ts` 不会立刻生效**；工具版本升级必须落到 release 分支。
- 清单签名私钥只存在于 CI secrets；本地无法生成有效签名（可用自己的密钥对做本地演练）。
- NSIS 语言列表与 `src-tauri/locales`/`src/locales` 的语言集合不同步时，安装器语言选项与实际界面语言可能不一致。
- `createUpdaterArtifacts` 关闭会让 Tauri 更新失效（安装包存在但 `latest.json` 缺失）。

## Verification

- 本地打包冒烟：`npm run tauri build`（需系统依赖与证书配置）。
- 清单流水线：本地 `npm run manifest:gen && npm run manifest:sign && npm run manifest:verify`（配置 `ED25519_*` 环境变量）。
- 发布后核对：Release 附件齐全、`latest.json` 可访问、`manifest.json` 的 `generatedAt` 更新、应用内更新可下载。
