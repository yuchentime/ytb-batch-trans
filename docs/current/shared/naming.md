---
status: current
layer: shared
domain: naming
canonical_for:
  - naming-conventions
related:
  - docs/current/rules/coding.rules.md
  - docs/current/rules/documentation.rules.md
last_verified: 2026-09-17
---

# 命名规范

## Purpose

统一前后端代码、IPC、i18n、文档与提交信息的命名，避免同一概念出现多种写法。

## Current Behavior

### Rust

| 对象 | 规则 | 示例 |
| --- | --- | --- |
| 模块/文件 | `snake_case`，一个文件一个职责 | `download_pipeline.rs`、`template_context.rs` |
| 结构体/枚举 | `PascalCase` | `DownloadEntry`、`ProgressStage` |
| 函数/方法 | `snake_case`，动词开头 | `build_output_args`、`resolve_with_patch` |
| 常量/静态 | `SCREAMING_SNAKE_CASE` | `MANIFEST_URL`、`RUNNING_GROUPS` |
| Tauri 命令 | `snake_case`，领域前缀 | `media_info`、`config_set`、`updater_install` |
| serde 字段 | `rename_all = "camelCase"` | `group_id` ↔ `groupId` |
| 测试 | `#[cfg(test)] mod tests`，用例名描述行为 | `subtitles_skip_unavailable_requested_languages` |

### TypeScript / Vue

| 对象 | 规则 | 示例 |
| --- | --- | --- |
| 文件（模块/工具） | `camelCase.ts` | `playlistSelection.ts`、`useGroupLog.ts` |
| 组件 | `PascalCase.vue`，基础组件前缀 `Base`，单例外壳前缀 `The`，领域前缀按区域 | `BaseButton.vue`、`TheHeader.vue`、`MediaCard.vue` |
| store | `defineStore('<kebab-name>')`，导出 `use<Name>Store` | `useMediaGroupStore` / `'media-group'` |
| 类型/接口 | `PascalCase` | `DownloadOverrides`、`MediaItem` |
| 枚举 | `PascalCase` 类型 + `camelCase`/字面量成员（与 Rust serde 对齐） | `TrackType.audio`、`ProgressStage.downloading` |
| 函数/变量 | `camelCase`，布尔用 `is/has/can/should` 前缀 | `isCombined`、`hasChanges`、`canPause` |
| 常量 | `SCREAMING_SNAKE_CASE` | `STRONGHOLD_KEYS`、`DEFAULT_SUBTITLE_FORMAT_ORDER` |
| 事件监听文件 | `src/tauri/listeners/<topic>.ts` | `progress.ts`、`binaries.ts` |

### IPC

| 对象 | 规则 | 示例 |
| --- | --- | --- |
| 命令 | `snake_case`（领域_动作） | `media_playlist_expand` |
| 事件 | `snake_case`（领域_事实） | `media_progress_stage` |
| 载荷类型 | `XxxPayload` / 领域模型名 | `MediaAddPayload`、`MediaFatalPayload` |
| 前端类型镜像 | 与 Rust 同名同字段 | `src/tauri/types/media.ts` |

### i18n

| 对象 | 规则 | 示例 |
| --- | --- | --- |
| 键路径 | `区域.子区域.名称`（camelCase 段） | `media.steps.configure.metadata.size` |
| 插值 | 花括号命名参数 | `{count}`、`{percent}`、`{title}` |
| 复数/变量 | 使用 vue-i18n 语法，不硬编码数字拼接 | `{amount} items` |
| 后端键 | `tray.*`、`notifications.<kind>.title|body`、`install.*` | 由 Rust `I18nManager` 渲染 |
| 错误码文案 | `errors.runner.<code>.message` / `.shortMessage` | 与 `diagnostic_rules.json` 一一对应 |

**两份 locale 文件必须保持键一致**：`src/locales/*.json`（界面）与 `src-tauri/locales/*.json`（托盘/通知/安装提示）。
ESLint 的 `@intlify/eslint-plugin-vue-i18n` 会校验前端；后端一致性靠人工 review。

### 文档与提交

| 对象 | 规则 |
| --- | --- |
| 当前事实文档 | 放在 `docs/current/**`，文件名固定（`flow.md`/`api-contract.md` 等） |
| 变更目录 | `docs/changes/YYYY-MM-DD-slug/` |
| ADR | `docs/decisions/ADR-XXXX-<slug>.md` |
| 复盘 | `docs/postmortems/YYYY-MM-DD-<slug>.md` |
| 提交信息 | Conventional Commits：`type(scope): summary`，例如 `fix(header): handle invalid URL input gracefully`；仓库内既有风格也接受 `type: summary` |

常用 `type`：`feat`、`fix`、`chore`、`docs`、`refactor`、`test`、`build`、`ci`、`perf`、`style`。

### 领域术语（跨层统一）

| 概念 | 唯一叫法 | 禁止的别名 |
| --- | --- | --- |
| 队列条目 | `Group` / `group` | item-card、entry-card、队列项 |
| 组内媒体系目 | `Item` / `MediaItem` | entry（`EntryItem` 专指播放列表条目）、video |
| 组级覆盖 | `Overrides` / `DownloadOverrides` | patch、diff、custom settings |
| 合并解析 | `resolve_with_patch` | merge、apply、inherit |
| 进度阶段 | `ProgressStage` | phase、step（`step` 仅指 UI 步骤组件） |

## Boundaries

- 文件重命名属于跨层改动：需同步 Rust `mod` 声明、前端 import、文档与 `manifest.yaml` 的 `code_globs`。
- 不引入新的缩写（除既有 `ovd`、`ytdlp`、`ipc`、`ui`、`i18n`）。

## Contracts

- 代码风格细节：`../rules/coding.rules.md`、`../rules/frontend.rules.md`、`../rules/backend.rules.md`。
- 文档库命名：`../rules/documentation.rules.md`。

## Failure And Edge Cases

- 前端 locale 缺键不会报错，只会显示原始 key（英文回退），因此"新增文案忘记补全语言"容易漏过 review。
- Rust serde 的 `rename_all` 只作用于该结构体；手工写 `serde(rename = ...)` 时必须核对前端字段名。
- 事件名拼写错误不会在编译期暴露（`emit` 接收字符串），只会在运行期表现为"前端没反应"。

## Verification

- `npm run lint`（含 i18n 规则与 stylistic 规则）。
- 新增 IPC 后：`tests/unit/tauriListeners.spec.ts` + E2E mock 一致性检查。
