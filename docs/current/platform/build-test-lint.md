---
status: current
layer: platform
domain: build-test-lint
canonical_for:
  - build-test-lint-facts
related:
  - docs/current/shared/testing-strategy.md
  - docs/current/rules/coding.rules.md
last_verified: 2026-09-18
---

# 构建、测试与静态检查

## Purpose

给出本仓库所有可执行的工程命令及其作用、CI 对应关系与已知的环境限制，是"改完代码要跑什么"的唯一出处。

## Current Behavior

### 版本要求

| 工具 | 版本 |
| --- | --- |
| Node.js | v24+（CI 使用 24） |
| Rust | CI 固定 `1.94.1`（含 `clippy`、`rustfmt`） |
| Tauri CLI | 通过 `@tauri-apps/cli`（`npm run tauri`） |
| Playwright | `1.60.0`（CI 使用官方 noble 容器镜像） |

### npm 脚本（`package.json`）

| 命令 | 作用 |
| --- | --- |
| `npm run dev` | `cross-env DEV=true vite`（纯前端，需自行 mock 或配合 tauri dev） |
| `npm run tauri dev` | 完整应用（`beforeDevCommand` 先构建隔离页面再起 dev server） |
| `npm run build` | `build:app` + `build:isolation`（两个产物都必须成功） |
| `npm run build:app` | `vue-tsc --noEmit && vite build` |
| `npm run build:isolation` | `vue-tsc --noEmit && vite build --config vite.config.isolation.ts` |
| `npm run lint` / `lint:fix` | ESLint（含 stylistic、vue、i18n 规则） |
| `npm run test:unit` | Vitest（jsdom） |
| `npm run test:e2e` | Playwright（自动起 dev server，`E2E=true`） |
| `npm run test` | 单测 + E2E |
| `npm run rust:fmt` / `rust:clippy` / `rust:lint` / `rust:test` | 等价于在 `src-tauri/` 下执行 cargo 命令 |
| `npm run manifest:gen` / `manifest:sign` / `manifest:verify` / `manifest:fetch` | 二进制清单流水线 |
| `npm run licenses:npm` / `licenses:rust` / `licenses:gen` / `licenses` | 第三方许可证汇总 |

### 测试分层

| 层 | 目录 | 环境 | 说明 |
| --- | --- | --- | --- |
| 前端单元 | `tests/unit/*.spec.ts` | jsdom + `vitest.setup.ts` | store/helper/组件；`globals: true` |
| 前端 E2E | `tests/e2e/*.spec.ts` | Playwright + Chromium | 用 `installTauriMock` 替换 IPC |
| Rust 单元 | 各模块内 `#[cfg(test)] mod tests` | cargo test | 解析器、参数矩阵、调度器、路径、覆盖合并 |

覆盖率：Vitest 输出 `coverage/units`，Playwright 通过 `monocart-coverage-reports` 输出 `coverage/e2e`，CI 上传 Codecov（flags `unit`/`e2e`/`rust-units`）。

### E2E mock 机制

- `main.ts` 在 `__E2E__` 时调用 `installTauriMock({...mediaHandlers, ...configHandlers, ...binaryHandlers, ...updateHandlers, ...strongholdHandlers})`。
- `tests/utils/tauriMock.ts` 提供 `IPCHandler` 与参数校验工具；handler 返回 Promise 或同步值。
- 事件可用 `window.E2E.emit(...)` 触发（`tests/e2e/utils/fixtures.ts` 封装了常用场景）。
- 新增命令/事件必须同步扩展 mock，否则 E2E 会超时（而不是明确失败）。

### CI 工作流

| 工作流 | 触发 | 内容 |
| --- | --- | --- |
| `vue-ci.yml` | 对 `main` 的 push/PR（前端相关路径） | Lint、Unit Tests、E2E Tests（Playwright 容器）、Build |
| `rust-ci.yml` | 对 `main` 的 push/PR（`src-tauri/**`） | 先构建隔离前端安装依赖 → Clippy、Fmt check、cargo-llvm-cov 单测与覆盖率 |
| `pages.yml` | `release` 分支 push（docs/scripts/package 路径）、每日 0 点、手动 | 在 release 分支生成/签名/校验清单 → 复制进 main 的 `docs/manifest/` → 部署 GitHub Pages |
| `publish.yml` | push 到 `release` 或手动 | 6 个目标矩阵（macOS x64/arm64、Ubuntu x64/arm64、Windows x64/arm64）打 Tauri 包并发布 Release |
| `distribute.yml` | 手动 | winget 清单提交、Snap Store 从 edge 提升到 stable |
| `msix.yml` | 见工作流 | Microsoft Store MSIX 打包 |

### 已知环境限制

- Rust 构建/测试需要系统库（Linux 上 `glib-2.0`、`webkit2gtk` 等，见 `.github/actions/install-tauri-deps`）；
  缺失时 `cargo test`/`cargo clippy` 会因构建失败而无法运行（本仓库的 AGENTS.md 已记录该限制）。
- E2E 需要 Playwright 浏览器：`npx playwright install --with-deps`。
- Windows 本地跑 `npm run tauri dev` 必须在“最小 PATH + vcvars64”环境里（JDK 的 `api-ms-win-*.dll` shim 会遮蔽系统 DLL、Git 的 `link.exe` 会与 MSVC 链接器重名，导致链接失败或进程启动即 `STATUS_ENTRYPOINT_NOT_FOUND`）：仓库提供 `scripts/dev.cmd`，它从当前 PATH 发现并保留 cargo/node/python/ffmpeg 目录，再引入 MSVC 环境，用法 `scripts\dev.cmd npm run tauri dev`。
- `cargo test` 不能替代真实下载验证（无网络与真实 yt-dlp）。

## Boundaries

- 没有 Rust 集成测试目录（`src-tauri/tests/`）；所有 Rust 测试都是模块内单测。
- 没有移动端/浏览器端构建目标。
- `npm run build` 不生成安装包；打包走 `npm run tauri build`（发布由 CI 执行）。
- `README.md` 的 `Building and packaging` 段是面向贡献者的英文摘要；**事实与细节以本文与 `release-and-distribution.md` 为准**，两处不一致时改 README。

## Contracts

- 提交前检查顺序：`npm run lint:fix` → `npm run test:unit` → `npm run test:e2e` → `npm run build`；Rust 侧 `cargo fmt --all` → `cargo clippy --all-targets -- -D warnings` → `cargo test`（见根目录 `AGENTS.md`）。
- 测试约定与 mock 扩展：`../shared/testing-strategy.md`。

## Failure And Edge Cases

- `vue-tsc --noEmit` 在 `build:app`/`build:isolation` 中各跑一次，类型错误会同时阻断两个产物。
- E2E 与单测共用 `tests/` 目录但配置不同（`vitest.config.ts` 只收 `tests/unit/**`，Playwright 只收 `tests/e2e/**`）。
- `npm run test` 串行跑单测与 E2E，本地耗时较长；CI 分开执行以便并行。
- 修改 `src-isolation/**` 后必须重新 `npm run build:isolation`，否则 dev/打包仍使用旧产物。

## Verification

```bash
# 前端
npm run lint:fix && npm run test:unit && npm run test:e2e && npm run build
# 后端（在 src-tauri/）
cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test
```
