---
status: current
layer: platform
domain: frontend-runtime
canonical_for:
  - frontend-runtime-facts
related:
  - docs/current/platform/backend-runtime.md
  - docs/current/rules/frontend.rules.md
last_verified: 2026-09-18
---

# 前端运行时

## Purpose

记录前端技术栈、构建配置、路由/i18n/状态骨架与安全边界（隔离模式 + CSP），供任何前端改动做前置判断。

## Current Behavior

### 技术栈与入口

| 项 | 值 |
| --- | --- |
| 框架 | Vue 3（`<script setup lang="ts">` 组合式 API） + TypeScript 5.9 |
| 状态 | Pinia 3（组合式 store） |
| 路由 | vue-router 5（`createWebHistory`，无 hash） |
| i18n | vue-i18n 11（`legacy: false`，`globalInjection: false`） |
| 样式 | Tailwind CSS 4 + daisyUI 5（`app.css`） |
| 图标 | `@heroicons/vue`（24/solid 与 24/outline） |
| 工具 | `uuid`（v4）、`iso-639-1`、`@sentry/vue` |
| 入口 | `index.html` → `src/main.ts` → `src/App.vue` |

`main.ts` 的注入顺序与全局注入见 `docs/current/domains/app-lifecycle/frontend-behavior.md`。

### Vite 配置（`vite.config.ts`）

| 项 | 值 |
| --- | --- |
| 端口 | `1420`（`strictPort: true`），HMR 走 `1421`（设置了 `TAURI_DEV_HOST` 时） |
| 构建产物 | `dist/`（默认） |
| 编译期常量 | `__DEV__`（`DEV=true`）、`__E2E__`（`E2E=true`）、`__APP_VERSION__`（`npm_package_version`） |
| 监听忽略 | `src-tauri/**`（但保留 `tauri.conf.json`）、`coverage/`、`manifest/`、`test-results/` |
| 插件 | `@vitejs/plugin-vue`、`@tailwindcss/vite` |

`vite.config.isolation.ts` 单独构建隔离页面：`root: src-isolation` → `dist-isolation/`。

### 路由表（`src/router.ts`）

| 路径 | 名称 | 说明 |
| --- | --- | --- |
| `/install` | `install` / `install.index` | 独立壳（`FullLayout`） |
| `/` | `home` | 队列首页 |
| `/settings` | `settings` + `settings.downloads` / `.app` / `.network` / `.system` / `.about` | 设置（子路由作为页签） |
| `/location` | `location` | 下载位置 |
| `/authentication` | `authentication` | 认证 |
| `/subtitles` | `subtitles` | 字幕 |
| `/input-filters` | `input-filters` | 输入过滤 |
| `/group/:groupId` | `group` + `group.metadata` / `group.logs` | 媒体详情（`requiresGroup`） |
| `/preferences/:groupId` | `preferences` + `preferences.quality` / `.network` / `.output` / `.subtitles` | 组级偏好（`requiresGroup`） |

`meta.index` 用于页面切换方向；`requiresGroup` 在 group 不存在时回首页。

### 状态骨架

- 每个文件一个 `defineStore`，命名前缀：`media*`（队列）、`settings`、`preferences`、`binaries`、`updater`、
  `stronghold`、`toast`、`dragDrop`、`watch-clipboard`。
- 事件驱动：所有 Tauri 事件在 `src/tauri/listeners/*` 注册（`src/plugins/tauriListeners.ts` 汇总安装），
  只允许在 listener 中调用 store 的 `process*` 方法。
- 类型定义集中在 `src/tauri/types/*`，与 Rust 结构体字段一一对应（camelCase）。

### 安全边界

| 机制 | 配置 | 效果 |
| --- | --- | --- |
| Isolation pattern | `tauri.conf.json` 的 `app.security.pattern = { use: "isolation", options: { dir: "../dist-isolation" } }` | 前端消息先经 `src-isolation/main.ts` 的 `__TAURI_ISOLATION_HOOK__` |
| 命令白名单 | `src-isolation/main.ts` 的 `allowedCommands` Set | 未列出的命令抛 `Unauthorized command: <cmd>` |
| CSP | `app.security.csp`：`default-src 'self' asset:`、`connect-src ipc: http://ipc.localhost https://*.sentry.io`、`style-src 'self'`（禁内联样式）、`frame-ancestors 'none'` | 无外部脚本/样式来源；渲染远端图片靠 `img-src http: https:` |
| Capability | `src-tauri/capabilities/default.json` | 只开放 opener/clipboard-read/dialog/store/updater/notification 与 `core:window` 的进度条/徽标/注意力 |
| 拖放 | 窗口配置 `dragDropEnabled: false` | 拖放由前端 HTML5 DnD 处理（避免与 Tauri 原生拖放冲突） |

新增命令时**必须**同时更新 `main.rs`/`lib.rs` 的 `invoke_handler` 与 `src-isolation/main.ts` 白名单。

### i18n 细节（`src/i18n.ts`）

- 支持的 locale（`availableLocales`）：`en, es, nl, it, fr, de, nb, ru, tr, pt-PT, pt-BR, zh-CN`。
  **已知缺口**：`src/locales/ko.json`、`src-tauri/locales/ko.json` 已存在（提交 `4501754`），但 `ko` 未注册进 `availableLocales`，
  因此设置页不会列出韩文，`getDefaultLocale()` 也不会命中韩文（会回退英文）。要启用需在 `src/i18n.ts` 注册 `ko` 的 import/messages 与别名。
- 别名：`pt → pt-PT`、`zh/zh-Hans/zh-CN → zh-CN`、`no/nb-NO → nb`。旧的 `zh-TW`/`zh-Hant` 会经语言主码回落到 `zh-CN`（应用不再提供繁体界面）。
- `getDefaultLocale()`：按 `detectBrowserLanguageCodes()` 依次尝试精确匹配 → 别名 → 语言主码 → 回退 `en`。
- `fallbackLocale: 'en'`；`globalInjection: false`（组件内显式 `useI18n()`）。
- 类型安全：`MessageSchema = typeof en`，其它语言用 `as unknown as MessageSchema` 强转（**不做键校验**），键一致性由 ESLint 的 `@intlify/eslint-plugin-vue-i18n` 保证。

### 观测

- `src/sentry.ts`：`__E2E__` 或 `__DEV__` 时**不初始化**；否则 `Sentry.init({ app, dsn, integrations: browserTracingIntegration({ router }) })`；Pinia 通过 `createSentryPiniaPlugin()` 接入。
- 手动上报仅发生在诊断卡片（`useDiagnostic.report`）。

## Boundaries

- 不引入 UI 组件库（除 daisyUI 类名）；不引入状态持久化插件（持久化都在 Rust 侧）。
- 路由为 `createWebHistory`：从浏览器直接刷新深层路径需要 dev server（打包后由 Tauri 处理）。
- 前端不访问网络（除 Sentry 上报）；所有站点请求由 Rust 侧的 yt-dlp 发起。

## Contracts

- IPC 约定：`../shared/ipc-conventions.md`。
- 命名与文件组织：`../shared/naming.md`。
- 前端约束：`../rules/frontend.rules.md`。

## Failure And Edge Cases

- `dist-isolation` 未构建时开发/打包会失败（`beforeDevCommand` 已包含 `npm run build:isolation`）。
- CSP 禁止内联样式：任何 `style="..."` 或运行时注入 `<style>` 会被拦截（历史上已有一次修复）。
- 白名单机制会让"忘记登记"的命令以异常形式暴露，且错误信息为英文（`Unauthorized command`）。
- `__E2E__` 下不初始化 Sentry、不启动窗口观察器，并用 mock 替换全部 IPC；因此 E2E 不能验证隔离模式与真实 IPC。

## Verification

- `npm run build`（含 `vue-tsc --noEmit`）与 `npm run build:isolation` 必须通过。
- `npm run test:unit`（jsdom 环境）覆盖 store/helper。
- 隔离与 CSP 的行为需在 `npm run tauri dev` 中手工验证（修改 `src-isolation/main.ts` 后重启）。
