---
status: current
layer: domain
domain: app-lifecycle
canonical_for:
  - app-lifecycle-frontend-behavior
related:
  - docs/current/platform/frontend-runtime.md
  - docs/current/domains/settings-preferences/frontend-behavior.md
last_verified: 2026-09-17
---

# app-lifecycle 前端行为

## Purpose

说明前端引导顺序、布局结构、主题/语言应用时机、更新提示条与窗口级反馈，作为修改外壳的约束。

## Current Behavior

### 引导（`src/main.ts`）

顺序（每一步失败都只 `console.error`，随后继续）：

1. `createPinia()` + `createSentryPiniaPlugin()`；`createApp(App)`；DEV 下挂 `__OVD_DEBUG__`（暴露 pinia 与 `getStore`）。
2. `createSentry(app)`（`src/sentry.ts`）。
3. `__E2E__` 为真时 `clearMocks()` + `installTauriMock(...)`（合并 media/config/binary/update/stronghold handlers）。
4. `app.use(i18n)`、`app.use(router)`、`app.use(pinia)`、`app.use(tauriListeners)`。
5. HMR 清理：`import.meta.hot.dispose` → `mediaStore.deleteAllGroups()`。
6. `initStores()`：`preferencesStore.load()` → `settingsStore.load()`（主题、语言）→ finally 中 `invoke('app_ready')`、非 E2E 时 `startWindowWatcher()`、`app.mount('#app')`。

关键点：`app_ready` 在 `finally` 中调用，因此即使 store 加载失败也会显示窗口。

### 根组件与布局

| 文件 | 职责 |
| --- | --- |
| `App.vue` | 渲染 `router-view` + `TheToaster`；挂载后并发执行 `checkTools()`（→ `/install`）、`strongholdStore.loadStatus()`、`checkUpdates()`；注册全局拖放 |
| `layouts/AppLayout.vue` | 主壳：`TheHeader` + `<main>`（路由视图 + `TheUpdater`）+ `TheFooter`；维护页面切换过渡方向（`meta.index` 比较）；`requiresGroup` 路由守卫（group 不存在时 `router.replace('home')`） |
| `layouts/FullLayout.vue` | 极简壳（仅 `router-view`），用于 `/install` |

### 头部/底部（系统入口）

| 组件 | 系统相关职责 |
| --- | --- |
| `TheHeader.vue` | URL 入队、剪贴板监听开关、文件导入、输入过滤入口、设置入口 |
| `TheFooter.vue` | 下载位置入口、认证状态灯、字幕开关状态、全局选项、队列批量操作、总进度 |
| `TheToaster.vue` | 全局 toast（成功/错误/警告/信息），`toastStore.showToast(message, { style, duration? })` |
| `TheUpdater.vue` | 更新提示条（`available` → 下载 → 安装），带 `data-testid="updater-alert"` |

### 主题与语言

- `useTheme()`：`theme` 计算属性直接读写 `settingsStore.settings.appearance.theme`（setter 会 `patch`）；
  `system` 模式移除 `data-theme`；`applyTheme(color)` 同时更新 `<meta name="color-scheme">`。
- 语言：`settingsStore.applySettings` 在每次加载/保存后设置 `i18n.global.locale` 与 `<html lang>`；
  `i18n.ts` 提供 `getDefaultLocale()`（浏览器语言 → 支持的 locale → `en`）。
- locale 文件：`src/locales/*.json`（13 种）；键结构与 `en.json` 必须一致（ESLint `@intlify` 规则会校验）。

### 窗口级反馈（`src/tauri/window.ts`）

`startWindowWatcher()`（E2E 下不启动）在 `watch` 中同步：

| 信号 | 规则 |
| --- | --- |
| 徽标（`setBadgeCount`） | 下载中 + 可下载数量；为 0 时传 `undefined` |
| 进度条（`setProgressBar`） | 无下载 → `None`；单条下载且有百分比 → 该条百分比；多条 → `done/total`；无数据 → `Indeterminate` |
| 用户注意力（`requestUserAttention`） | 从"有下载"变为"队列无下载且无可下载"时触发一次，2 秒冷却；此前并发 >1 时先发 `queueFinished` 通知 |

平台不支持这些 API 时静默忽略（try/catch）。

### 更新交互（`TheUpdater.vue` + `updater` store）

| 状态 | 展示 |
| --- | --- |
| `checkResult.available && !isIgnored` | 提示条 + "稍后"/"下载" |
| `isUpdating` | 进度条（`downloaded/received` 百分比）+ 旋转图标 |
| `isNeedingRestart` | "安装并重启"提示 + "稍后"/"安装" |
| `lastError` | 仅记录在 store（UI 未直接展示错误文案） |

`updaterStore.install()` 会先清 `isNeedingRestart` 再调用命令；失败时错误只 `console.error`。

### 事件订阅总表（`src/plugins/tauriListeners.ts`）

`app`（navigate）、`media`（media_add/media_size）、`progress`（media_progress/media_progress_stage/media_complete）、
`destination`（media_destination）、`binaries`（binary_download_*）、`updater`（updater_*）、
`diagnostics`（media_diagnostic/media_fatal）、`shortcuts`（shortcut_action）。

## Boundaries

- 前端不直接读写窗口几何（由后端 `window.rs` 负责）。
- 托盘/菜单文案来自后端 locale；前端不提供这些字符串。
- `TheUpdater` 不阻塞界面交互（绝对定位的提示条）。

## Contracts

- 命令/事件：`docs/current/domains/app-lifecycle/api-contract.md`。
- 设置字段：`../settings-preferences/data-model.md`。
- 前端运行时契约（隔离模式/CSP/router）：`../platform/frontend-runtime.md`。

## Failure And Edge Cases

- `i18n.global.locale.value` 只接受已注册 locale；后端返回未知语言代码时 `as Locale` 断言可能导致回退英文。
- 路由守卫在 group 被删除后自动回首页；`isMissingGroupRedirecting` 防止重复 replace。
- `TheFooter` 的认证状态灯只反映"配置过"，不反映"有效"。
- E2E 下不启动窗口观察器与 Tauri mock 之外的原生能力（窗口 API 在 mock 中为 no-op）。
- 页面过渡方向依赖 `meta.index`；新增路由若不设置 `meta.index`，过渡方向会按 0 处理。

## Verification

见 `docs/current/domains/app-lifecycle/verification.md`；单测：`tests/unit/header.spec.ts`、`footerMessage.spec.ts`、`tauriListeners.spec.ts`；E2E：`updater.spec.ts`。
