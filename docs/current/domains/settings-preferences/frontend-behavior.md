---
status: current
layer: domain
domain: settings-preferences
canonical_for:
  - settings-preferences-frontend-behavior
related:
  - docs/current/domains/settings-preferences/data-model.md
  - docs/current/domains/media-queue/frontend-behavior.md
last_verified: 2026-09-17
---

# settings-preferences 前端行为

## Purpose

说明设置与偏好在前端的读写模型、草稿机制与各设置页面职责，避免"改了 UI 却没接上后端"。

## Current Behavior

### store

| store | 文件 | 关键行为 |
| --- | --- | --- |
| `settings` | `src/stores/settings.ts` | `load()` 启动加载；`patch(partial)` 提交并整体覆盖；`reset()` 恢复默认；`hasAuthConfigured()` 供底部状态灯使用；`applySettings` 同步 i18n locale 与 `<html lang>` |
| `preferences` | `src/stores/preferences.ts` | `load()`/`patch()`/`reset()`；`getRecentPaths`/`addRecentPath`（去重、unshift、截断 5 条）/`clearRecentPaths`；`pathExamples`/`filenameExamples` 供模板预览；`getPathExample(trackType)` |

`patch` 使用 `DeepPartial<Settings>` 类型，但设置页实际提交的是**整份草稿**（见下）。

### 设置页草稿机制（`src/views/app/SettingsView.vue`）

- `draft = structuredClone(toRaw(settingsStore.settings))`：进入页面时深拷贝。
- 子页面通过 `v-model="draft"` 直接改草稿；`hasChanges` 用 `JSON.stringify` 比较草稿与 store。
- "保存"→ `settingsStore.patch(draft)` → 成功后 `setTheme(draft.appearance.theme)` → toast。
- "重置"→ `settingsStore.reset()` → 用返回的默认值替换草稿 → toast。
- 未保存离开页面会丢失修改（无导航守卫、无提示）。

### 页面与组件职责

| 页面/组件 | 负责的设置子树 |
| --- | --- |
| `settings/SettingsDownloadsTab.vue` | `output`（容器/编码/模板/元数据/SponsorBlock 入口等） |
| `settings/SettingsAppTab.vue` | `appearance`（主题/语言/展开项）、`input.autoFillClipboard` |
| `settings/SettingsNetworkTab.vue` | `network`（代理/impersonate/extractor-args） |
| `settings/SettingsSystemTab.vue` | `system`（托盘/自启动/关闭行为）、`update`、`input.globalShortcuts` |
| `settings/SettingsAboutTab.vue` | 版本信息、链接、许可证 |
| `components/settings/SettingsPerformance.vue` | `performance`（并发/阈值/体积预载） |
| `components/settings/SettingsNotifications.vue` | `notifications` |
| `components/settings/SettingsSponsorBlock.vue` | `sponsorBlock` |
| `components/settings/SettingsOutput.vue` | `output` 的分段编辑（含自定义 ffmpeg 参数） |
| `views/app/LocationView.vue` | `preferences.paths`（目录选择、目录模板、最近路径）与 `output.downloadDir` |
| `views/app/SubtitleView.vue` | `subtitles`（总开关/自动字幕/语言/格式/嵌入） |
| `views/app/InputFiltersView.vue` | `inputFilters`（体积/日期/匹配过滤 + 预设） |
| `views/app/MediaPreferencesView.vue` | 单 group 的下载偏好入口（组级 override，见 media-queue） |

### 偏好相关交互

- **最近路径**：`DirectoryPresetSelector` 与目录选择器调用 `addRecentPath`，下拉展示最多 5 条，提供"清除"。
- **模板预览**：`PreferencesFilename.vue`/`PreferencesDirectory.vue` 通过
  `setPathExample(trackType, path)` 与 `setFilenameExample(trackType, name)` 写入预览片段，
  `getPathExample` 拼接展示最终路径样子；预览值来自 `useOutputSettingsEditor` 的示例数据。
- **格式预设**：`FormatPresetSelector` / `DirectoryPresetSelector` 提供常用模板，选择后写入 `custom`/对应枚举。
- **输入过滤预设**：`applyDatePreset('today'|'yesterday'|'last7Days'|'last30Days')` 生成日期值（UTC 计算）。

### 主题与语言

- `useTheme()`：`theme` 为 `'system'` 时移除 `data-theme`（交给系统媒体查询），否则写 `document.body[data-theme]` 与 `<meta name="color-scheme">`；同时监听 `prefers-color-scheme` 变化。
- 语言切换在 `applySettings` 中即时生效（`i18n.global.locale` + `<html lang>`）；后端托盘/通知文案由 Rust `I18nManager` 独立更新。

### 通知策略的前端侧

前端只决定"何时调用 `notify`"（业务事件点），是否真正弹出由后端按 `notificationBehavior`/`disabledNotifications` 判定。
`notifyGroup` 会把 `group.fromShortcut` 作为 `force` 传入，保证快捷键触发的操作必弹提醒。

### 输入过滤的换算

`settingsToInputFilterOverride(settings)`（`helpers/inputFilters.ts`）把 UI 结构转成 yt-dlp 值：

| UI | 输出 |
| --- | --- |
| `minSize { value, unit }` | `"10M"` 形式（`B`→`B`、`KB`→`K`、`MB`→`M`、`GB`→`G`、`TB`→`T`） |
| `dateFilter { mode, value }` | `date` / `datebefore` / `dateafter`（`YYYY-MM-DD` → `YYYYMMDD`） |
| `matchFilters` / `breakMatchFilters` | 原样（trim 后非空） |

只有至少一个字段非空时才返回对象；否则返回 `undefined`（不写 override）。

## Boundaries

- 设置页不直接 `invoke`；一律经 store。
- 偏好与设置是两个独立文件与两个独立 store，UI 上可以出现在同一页面（如 LocationView 同时改 `preferences.paths` 与 `output.downloadDir`）。
- 组级 override 不写这里（见 `../media-queue/frontend-behavior.md`）。

## Contracts

- 字段与默认值：`data-model.md`。
- 命令：`api-contract.md`。
- 模板渲染（预览与实际输出的一致性）：`../download-engine/api-contract.md` 的"位置参数"。

## Failure And Edge Cases

- 草稿用 `structuredClone(toRaw(...))`；若某个设置字段是 `undefined`，JSON 序列化后该键消失，等于"不修改"。
- `JSON.stringify` 比较对键顺序敏感；`defaultSettings` 与后端返回对象的键顺序不同不会导致误判（对象由同一对象派生），但手工构造的草稿可能误报"有改动"。
- `maxConcurrency` 前端默认 `1`，首屏若在 `config_get` 返回前渲染会短暂显示 1。
- `addRecentPath` 在 patch 期间直接修改 store 数组（乐观更新），失败时会留下未持久化的最近路径。

## Verification

见 `docs/current/domains/settings-preferences/verification.md`；单测：`tests/unit/settingsView.spec.ts`、`networkSettings.spec.ts`、`subtitleSettings.spec.ts`、`postprocessSettings.spec.ts`、`inputFilters.spec.ts`（最近路径与目录/文件名预览目前只有手工验证）。
