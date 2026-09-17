---
status: current
layer: domain
domain: app-lifecycle
canonical_for:
  - app-lifecycle-verification
related:
  - docs/current/product/acceptance.md
  - docs/evals/release-checklist.md
last_verified: 2026-09-17
---

# app-lifecycle 验证

## Purpose

给出外壳与应用更新的验证方式，重点是平台差异行为与"不能破坏"的系统集成语义。

## Current Behavior

### 自动化覆盖

| 关注点 | 方式 |
| --- | --- |
| 更新提示条状态机 | `tests/e2e/updater.spec.ts`（mock `updater_check/download/install` 与 `updater_*` 事件） |
| 头部输入与布局 | `tests/unit/header.spec.ts`、`footerMessage.spec.ts` |
| 事件订阅全量注册 | `tests/unit/tauriListeners.spec.ts` |
| 平台判定 | `src-tauri/src/paths.rs` 的单测（snap/portable/ms-store 目录解析） |
| Rust 编译期平台分支 | `cargo clippy --all-targets -D warnings`（保证 macOS/Linux 分支至少能通过本平台编译） |

窗口、托盘、菜单、全局快捷键、单实例、自启动没有自动化覆盖，必须手工验证。

### 手工验收步骤

1. **首次启动**：冷启动后窗口按记录尺寸出现（首次为 800×900）；不出现白屏（`app_ready` 已触发）。
2. **单实例**：应用运行时再次启动（双击图标/命令行）→ 不新开窗口，已有窗口被 show+focus。
3. **关闭行为**（三种组合）：
   - 托盘关 + 关闭 → 进程退出；
   - 托盘开 + `hide` → 窗口隐藏、进程存活、托盘图标可再显示；
   - 托盘开 + `exit` → 进程退出。
   - macOS 任意组合 → 关闭只隐藏。
4. **托盘菜单**：三个动作分别触发"入队/入队并下载/下载全部"，`hide_toggle` 切换可见性，`quit` 退出进程。
5. **全局快捷键**（Alt/Ctrl+Shift+V/D/Enter）：剪贴板有合法 URL 时生效；关闭"全局快捷键"后立即失效。
6. **自启动**：开启自启动 → 重新登录 → 进程自动启动；再开"最小化启动" → 窗口不出现（托盘/再次启动可唤出）。
7. **主题**：`system` 时跟随系统切换；选择 light/dark 后固定；重启保持。
8. **语言**：切换语言 → 界面、托盘菜单、通知文案全部切换；断开某语言的 key（临时删）→ 回退英文。
9. **应用更新**：在可更新形态下点击"下载" → 进度条推进 → "安装" → 应用重启并显示新版本号；portable/snap/MS Store 下不显示更新提示。
10. **窗口几何**：调整/最大化/移动 → 重启后恢复；最大化的窗口重启仍最大化且还原位置正确。
11. **窗口进度**：同时下载多条 → 任务栏/程序坞进度与徽标更新；下载结束后进度条归零、徽标消失，并触发一次注意力请求。

### 必须保持的既有语义（Must-Not-Break）

- `app_ready` 是显示窗口的唯一开关，且必须在 `initStores()` 的 `finally` 中调用（保证异常时窗口仍显示）。
- macOS 关闭窗口永远只隐藏（不退出进程）。
- 单实例必须复用现有实例（不得出现两个托盘图标/两份调度器）。
- 托盘与菜单文案必须走 `I18nManager`，不得硬编码文案。
- 更新安装在 portable/snap/MS Store 形态下必须被拒绝（本地文件不能被自更新覆盖）。
- `--auto-start` + `autoStartMinimised` 组合必须不显示窗口。

## Boundaries

- 不覆盖系统级的开机自启实现差异（LaunchAgent/systemd/注册表），只验证"配置与系统状态双向同步"。
- 不在 CI 中触发真实更新安装。

## Contracts

- 验收基线：`docs/current/product/acceptance.md` 的 A19–A22。
- 发布前检查：`docs/evals/release-checklist.md`。
- 命令与事件：`api-contract.md`。

## Failure And Edge Cases

- 若新增命令但忘记加入 `src-isolation/main.ts` 白名单，前端调用会抛 `Unauthorized command`（隔离模式下表现为功能静默失效），必须用真实应用验证而不是仅跑单测。
- 托盘图标在部分 Linux 桌面（无 AppIndicator 支持）不显示，属环境限制；不要把它当作缺陷。
- 更新安装后 `request_restart` 会结束进程；若有未保存的队列状态会丢失（符合产品边界）。

## Verification

- 自动化：`npm run test:e2e -- updater`；`npm run test:unit`；`cd src-tauri && cargo test`（paths/window 纯函数）。
- 手工：以上 11 步，重点是 3、5、6、8、9、11。
- 发布前：`docs/evals/release-checklist.md` 全项通过（含三平台打包冒烟）。
