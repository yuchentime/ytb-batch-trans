---
status: current
layer: domain
domain: settings-preferences
canonical_for:
  - settings-preferences-verification
related:
  - docs/current/product/acceptance.md
  - docs/evals/regression-matrix.md
last_verified: 2026-09-17
---

# settings-preferences 验证

## Purpose

给出设置/偏好的验证方式：哪些可以自动化、哪些必须手工、以及哪些语义不能被破坏。

## Current Behavior

### 自动化覆盖

| 关注点 | 测试 |
| --- | --- |
| 设置页渲染与保存/重置 | `tests/unit/settingsView.spec.ts` |
| 网络设置换算 | `tests/unit/networkSettings.spec.ts`、`networkOverrides.spec.ts` |
| 字幕设置归一化 | `tests/unit/subtitleSettings.spec.ts`、`subtitlePreferences.spec.ts`、`subtitleOverridePreferences.spec.ts` |
| 后处理设置 | `tests/unit/postprocessSettings.spec.ts`、`postprocessOverrides.spec.ts` |
| 输入过滤换算与预设 | `tests/unit/inputFilters.spec.ts`、`inputFiltersView.spec.ts` |
| 目录/最近路径与体积 | ⚠️ 当前**无自动化用例**；需手工验证（见下方步骤 6、7） |
| 完整链路 | `npm run rust:test`（`paths.rs`/`window.rs` 相关的纯函数测试） |

E2E（`tests/e2e/persist-selection.spec.ts`、`queue-actions.spec.ts`）通过 mock 验证选择在会话内被持久保留；
真实跨会话持久化需手工验证。

### 手工验收步骤

1. **首次启动默认值**：删除 `config.store.json` 与 `preferences.store.json` → 启动 → 下载目录等于系统下载目录，主题跟随系统，语言跟随系统，并发数 = `ceil(CPU/2)`。
2. **保存生效（无需重启）**：设置 → 系统：开启托盘 → 托盘图标立即出现；关闭"全局快捷键" → 快捷键立即失效；把并发从 2 改为 6 → 同时下载数立即变化（用多 group 观察）。
3. **重启保持**：修改主题/语言/模板/代理/过滤器 → 重启 → 值仍为修改后的值。
4. **深合并**：只改 `performance.maxConcurrency`（不传其它字段）→ 其它设置保持不变。
5. **重置**：点击重置 → 所有页面回到默认值，且托盘/快捷键/并发随之调整。
6. **窗口几何**：调整窗口大小并移动 → 重启 → 尺寸与位置恢复；最大化状态重启后仍最大化；把窗口移到副屏后拔掉副屏再启动 → 窗口出现在主屏（不会跑到屏幕外）。
7. **最近路径**：在下载位置页选择两个不同目录 → 下拉出现两条（最近的在前）→ 重复选择同一目录不产生重复项 → 超过 5 条时最旧的被丢弃。
8. **通知策略**：`never` 时任何事件都不弹；`onBackground` 时前台不弹、切到后台后弹；`always` 时都弹；在"禁用通知"里勾选某类后该类不再弹。
9. **宽松/严格 JSON**：手工在 `config.store.json` 里删掉 `appearance` 整个对象 → 重启后自动补默认值且其余设置保留。

### 必须保持的既有语义（Must-Not-Break）

- 深合并语义：缺键不改、`null` 覆盖、数组整体替换。
- `apply_patch` 失败时内存与磁盘都不变（不得出现"半个配置"）。
- `on_updated` 必须在写盘前触发（否则会出现"已保存但未生效"）。
- 密码类字段不得进入 `config.store.json`。
- 前端提交整份草稿时，后端返回完整对象并整体覆盖内存（不得只更新 patch 的键，否则前端镜像与后端分离）。

## Boundaries

- 不覆盖真实显示器热插拔的多屏矩阵；只覆盖"矩形是否落在某显示器内"的分支。
- 不覆盖 `tauri_plugin_store` 自身的原子写行为（依赖插件实现）。

## Contracts

- 验收基线：`docs/current/product/acceptance.md` 的 A21（设置即时生效）。相关：A19（关闭行为）、A22（语言）。
- 字段契约：`data-model.md`。

## Failure And Edge Cases

- 手工破坏 JSON 结构（例如把 `config` 键的值改成字符串）会让 `init` 失败 → 应用无法启动；这是唯一需要用户手动删文件恢复的配置类故障。
- 并发上限调小时 `DynamicSemaphore::resize` 会"借走"许可，正在运行的下载不会被中断（符合预期，但观察到的并发下降会滞后）。
- 修改语言时若托盘开启，`on_updated` 会重建托盘；某些桌面环境下托盘图标可能短暂消失。

## Verification

- 自动化：`npm run test:unit`；`cd src-tauri && cargo test`。
- 手工：以上 9 步，重点是第 2、5、6、9 步。
- 回归矩阵：`docs/evals/regression-matrix.md` 中 settings-preferences 相关行。
