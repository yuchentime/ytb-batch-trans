---
status: current
layer: domain
domain: auth-secrets
canonical_for:
  - auth-secrets-verification
related:
  - docs/current/rules/security.rules.md
  - docs/evals/security-checklist.md
last_verified: 2026-09-17
---

# auth-secrets 验证

## Purpose

给出机密存取链路的验证方式与安全回归清单；任何涉及凭据的改动都必须跑完本文的检查项。

## Current Behavior

### 自动化覆盖

| 关注点 | 方式 |
| --- | --- |
| 认证页行为 | `tests/e2e/authentication.spec.ts`（mock `stronghold_*`，见 `tests/utils/mocks/strongholdHandlers.ts`） |
| YtdlpRunner 日志摘要（不泄漏） | `ytdlp_runner.rs` 的 `summary_*` 测试（含"值以 `-` 开头"的回归） |
| 凭据合并语义 | `override_resolver.rs` 的 `resolve_auth_settings_and_secrets` |
| 参数注入 | `runners/ytdlp_args/tests.rs`（认证相关断言） |

E2E mock 行为：`stronghold_status` 默认返回 `{ unlocked: false }`，`stronghold_init` 默认返回 `{ unlocked: true }`，
`stronghold_get/keys` 由 `window.E2E.stronghold.fields` 驱动——因此 E2E 验证的是前端编排，不是真实加密。

### 手工验收步骤

1. **首次启用**：清空 `vault.hold` 并从系统钥匙串删除 `com.jelleglebbeek.youtube-dl-gui/master_key` → 启动 → 认证页显示"启用" → 点击后出现 5 个字段。
2. **写入与持久化**：填入用户名/密码/视频密码/Bearer/两行请求头 → 保存 → 重启 → 字段仍然存在（保险库自动解锁，字段回填）。
3. **注入生效**：对需要登录的站点（或本地 mock 代理）下载，检查进程参数包含 `--username` 等（用 `ps`/进程查看器；**不要**把值贴进 issue）。
4. **Cookie 路径**：配置浏览器名 → 下载受保护内容成功；配置 Cookie 文件 → 文件路径出现在 `config.store.json`，内容不入库。
5. **删除字段**：把某个字段清空并保存 → `stronghold_keys` 不再包含该键（底部状态灯在其它键存在时仍亮）。
6. **钥匙串不可用**：在无 keyring 后端的环境（或禁用凭据管理器策略）启动 → 认证页显示 `Secure keyring unavailable` 错误卡片，Cookie 部分仍可编辑。
7. **主密钥失效**：手工把钥匙串中的主密钥改成随机 base64 → 启动 → 保险库被重建（旧凭据丢失），状态仍为 `unlocked`（验证"静默重建"这一已知行为）。
8. **macOS dev**：在 macOS debug 构建下启动 → 不自动解锁（需手动启用），确认这是预期而非缺陷。
9. **泄漏检查**：搜索日志文件与 Sentry 事件，确认没有出现密码/Bearer/请求头内容；`RunLogSummary` 只有布尔字段。

### 必须保持的既有语义（Must-Not-Break）

- 机密只落在 `vault.hold` + 系统钥匙串；`config.store.json`/`preferences.store.json`/日志/Sentry 中不得出现明文。
- 前端与后端之间只传字节数组，不传明文 JSON 字符串。
- `stronghold_keys` 只返回键的字节数组（不返回键名之外的任何信息）。
- 解锁失败不得导致启动失败（只有命令返回错误/错误卡片）。
- 清空字段语义 = 删除键，而不是写入空字符串。

## Boundaries

- 不覆盖 stronghold 加密算法本身的正确性（依赖上游插件）。
- 不覆盖真实浏览器的 Cookie 解密行为（由 yt-dlp 处理）。

## Contracts

- 验收基线：`docs/current/product/acceptance.md` 的 A16（凭据不外泄）。
- 命令与键名：`api-contract.md`。
- 安全清单：`docs/evals/security-checklist.md`。

## Failure And Edge Cases

- 测试中的 mock 不会暴露真实钥匙串问题；发布前必须在目标平台做一次手工验证（尤其 Windows 与 Linux 的 keyring 差异）。
- 若 CI 环境有 keyring 后端且测试触发真实 `stronghold_init`，可能污染开发机凭据；E2E 已用 mock 规避，新增用例不得绕过 mock。
- 排查用户问题时不要让用户提供 `vault.hold`；它会暴露全部凭据。

## Verification

- 自动化：`npm run test:e2e -- authentication`；`cd src-tauri && cargo test`（`summary_*`、`resolve_auth_*`）。
- 手工：以上 9 步，重点 3、6、7、9。
- 发布前：`docs/evals/security-checklist.md` 全项通过。
