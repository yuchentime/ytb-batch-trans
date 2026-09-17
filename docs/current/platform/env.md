---
status: current
layer: platform
domain: env
canonical_for:
  - environment-variables
related:
  - docs/current/platform/storage.md
  - docs/current/platform/build-test-lint.md
last_verified: 2026-09-17
---

# 环境变量

## Purpose

列出构建、测试、运行、发布各环节会读取的环境变量及其效果，避免"本地能跑 CI 挂"或"发布流水线行为不一致"。

## Current Behavior

### 构建期（Vite / npm）

| 变量 | 读取方 | 效果 |
| --- | --- | --- |
| `DEV` | `vite.config.ts` → `define: __DEV__` | `npm run dev` 设为 `true`；控制 `__DEV__`（Sentry 是否初始化、DEV 调试钩子） |
| `E2E` | `vite.config.ts` → `define: __E2E__`；`playwright.config.ts` 的 webServer env | 为 `true` 时安装 Tauri mock、关闭 Sentry 与窗口观察器 |
| `npm_package_version` | `vite.config.ts` → `__APP_VERSION__` | 由 npm 自动注入，值为 `package.json` 的 version |
| `TAURI_DEV_HOST` | `vite.config.ts` | 设置时 dev server 绑定该 host，HMR 走 `1421`（移动/远程调试用） |
| `TAURI_SKIP_UPDATE_CHECK` | Rust 构建 | CI 中设为 `1`，避免 tauri-cli 联网检查更新 |
| `CARGO_TARGET_DIR` | Rust 构建 | CI 设为 `src-tauri/target` 以便缓存 |

### 测试与 CI

| 变量 | 读取方 | 效果 |
| --- | --- | --- |
| `CI` | `playwright.config.ts` | 为真时 `reuseExistingServer=false`（每次起新 dev server） |
| `CODECOV_TOKEN` | 工作流 | 存在时上传覆盖率（unit/e2e/rust 三个 flag） |
| `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD` | 工作流 | 在 Playwright 容器内跳过浏览器下载 |

### 发布与清单

| 变量 | 读取方 | 效果 |
| --- | --- | --- |
| `GITHUB_TOKEN` / `GH_TOKEN` | `scripts/sources.ts`、`scripts/gen-manifest.ts` | 访问 GitHub API/下载产物时用于提高额度、避免限流 |
| `ED25519_PRIV_KEY_B64` | `scripts/sign-manifest.ts` | PKCS#8 DER（base64）私钥，用于生成 `manifest.sig` |
| `ED25519_PUB_KEY_HEX` | `scripts/verify-manifest.ts` | 十六进制公钥，用于校验签名（必须与 `binaries_manager.rs` 中硬编码值一致） |
| `WINDOWS_PFX_B64` / `WINDOWS_PFX_PASSWORD` | `publish.yml` | Windows 代码签名证书 |
| `SNAPCRAFT_STORE_CREDENTIALS` | `distribute.yml` | Snap Store 发布凭据 |
| `WINGET_TOKEN` | `distribute.yml` | 提交 winget 清单 |

### 运行时（应用进程）

| 变量 | 读取方 | 效果 |
| --- | --- | --- |
| `SNAP_USER_DATA` | `paths.rs` | 非空即判定为 snap 形态，`app_dir = $SNAP_USER_DATA/<identifier>` |
| `SNAP_USER_COMMON` | `paths.rs` | 非空时 `bin_dir = $SNAP_USER_COMMON/bin` |
| `PATH` | `ytdlp_runner.rs` | 被读取后**前置** `bin_dir`，再作为子进程环境变量传入 |
| 命令行参数 `--auto-start` | `app_ready` + autostart 插件 | 自启动标识；配合 `autoStartMinimised` 决定是否显示窗口 |

### 环境变量不作为配置来源

以下内容**不**通过环境变量配置，必须走应用设置：代理、Cookie、并发、下载目录、字幕语言、通知策略。
唯一的例外是运行时形态探测（snap 目录）与构建期开关（DEV/E2E）。

## Boundaries

- 没有 `.env` 加载机制（Vite 的 `import.meta.env` 只用于内置常量与 `import.meta.hot`）。
- 应用不使用 `HTTPS_PROXY` 等标准代理变量；代理只在设置中配置且只作用于 yt-dlp。
- `SNAP_USER_*` 之外没有其它"运行形态"环境开关。

## Contracts

- 形态与目录：`storage.md`。
- 构建/测试命令：`build-test-lint.md`。
- 发布流水线：`release-and-distribution.md`。

## Failure And Edge Cases

- 在 CI 中缺失 `TAURI_SKIP_UPDATE_CHECK` 会让构建尝试联网，导致偶发失败或变慢。
- 修改 `ED25519_PUB_KEY_HEX`（CI）而未同步 `binaries_manager.rs` 中的 `MANIFEST_PUB_KEY` 会导致所有用户安装失败——发布前必须核对。
- 开发时手动设置 `E2E=true` 会注入 mock，导致真实 IPC 被替换（排查"改了后端没生效"时先检查该变量）。
- 便携版用户若把 `ovd-portable` 目录放在受保护路径（如 Program Files），二进制与配置写入都会失败。

## Verification

- 本地：`DEV=true E2E=false npm run dev` 与 `npm run tauri dev` 对比行为差异。
- 发布前：核对 CI secrets 与应用内硬编码公钥；`npm run manifest:verify` 必须在签名后通过。
