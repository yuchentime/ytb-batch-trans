---
status: current
layer: rules
domain: frontend
canonical_for:
  - frontend-rules
related:
  - docs/current/platform/frontend-runtime.md
  - docs/current/domains/media-queue/frontend-behavior.md
last_verified: 2026-09-17
---

# 前端规则

## 组件与文件

1. 组件放 `src/components/<区域>/`；基础组件前缀 `Base`，单例外壳前缀 `The`，领域组件按区域命名（`Media*`、`Settings*`）。
2. 视图放 `src/views/app/`（应用内）或 `src/views/full/`（独立壳）；布局放 `src/layouts/`。
3. 组件必须用 `<script setup lang="ts">`；props 用 `defineProps`（对象式声明 + `PropType`），事件用 `defineEmits`。
4. 组件内不写业务换算逻辑，放到 `src/helpers/`（纯函数，可单测）；组件内不直接 `invoke`（除 `useGroupLog` 的日志订阅与 `useOpener` 的外链）。

## 状态

1. 一个关注点一个 store；跨 store 调用通过 `useXxxStore()` 在 store 内部完成，不把 store 实例当参数传。
2. store 只暴露"动作 + 派生值"；组件不直接改 `store.itemStates` 之类的原始 ref（只读渲染除外）。
3. 派生展示值用 `computed`，不要在模板里做多步计算。
4. 不引入持久化插件；需要跨会话的数据一律走 Rust 命令。

## 样式

1. 只用 Tailwind 工具类 + daisyUI 组件类；**禁止内联 `style`**（CSP 已阻止，历史上有过修复）。
2. 需要自定义样式时用 `<style scoped>`（现有 `MediaView.vue`/`SettingsView.vue` 的 tabs 样式是唯一例外场景）。
3. 颜色/间距使用 daisyUI 语义变量（`bg-base-300`、`text-base-content/70`、`alert-error` 等），不硬编码色值。
4. 无障碍：交互元素必须有可读标签（`aria-label` 或 `sr-only` 文本），图标按钮必须有 `title` 或 `tooltip`。

## 文案与 i18n

1. 所有面向用户的字符串必须走 `t('...')`；新增 key 必须同时补 `src/locales/en.json`（其它语言可后补）。
2. 后端产生的文案（托盘/通知/安装）走 `src-tauri/locales/*.json`，前端不重复定义。
3. 插值使用命名参数（`{count}`），复数组件用 vue-i18n 的复数语法。

## 事件与 IPC

1. 事件监听只在 `src/tauri/listeners/<topic>.ts` 注册，并在 `src/plugins/tauriListeners.ts` 中登记；组件不得自行 `listen`（除 `useGroupLog`）。
2. 事件处理器只做"转交 store 的 `process*` 方法"；不得在监听器里做 UI 逻辑。
3. 前端类型镜像必须与 Rust 结构体同步（`src/tauri/types/*`）。

## 主题与平台

1. 主题只通过 `useTheme()`/`applyTheme()` 修改，不直接操作 `document.body`（`useTheme` 与 `applyTheme` 除外）。
2. 平台差异用 `usePlatform()` 的 `isWindows`/`isMac` 判断，或直接使用后端返回的平台值；不读 `navigator.userAgent`。
3. 窗口级效果（进度条/徽标/注意力）只放在 `src/tauri/window.ts`。

## 测试

1. 新增 helper/store 逻辑必须加 Vitest 用例；新增用户流程至少加一条 E2E 或说明为何无法自动化。
2. 组件测试需要 i18n 时，按既有做法在 `mount` 的 `global.plugins` 传入 `src/i18n.ts` 导出的 `i18n`（见 `tests/unit/header.spec.ts`）。
3. 新增命令/事件后必须更新 `tests/utils/mocks/*`。

## Verification

- `npm run lint:fix`、`npm run test:unit`、`npm run build`。
- 涉及 CSP/隔离/窗口行为的改动必须在 `npm run tauri dev` 中手工验证。
