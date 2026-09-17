---
status: current
layer: domain
domain: auth-secrets
canonical_for:
  - auth-secrets-backend-behavior
related:
  - docs/current/domains/auth-secrets/flow.md
  - docs/current/domains/download-engine/backend-behavior.md
last_verified: 2026-09-17
---

# auth-secrets 后端行为

## Purpose

说明保险库的实现细节、主密钥生命周期、失败恢复策略与凭据注入路径，供安全审计与故障排查使用。

## Current Behavior

### 常量与状态

| 名称 | 值 | 说明 |
| --- | --- | --- |
| `CLIENT` | `b"ovd"` | stronghold 客户端标识；键空间属于该 client |
| `KR_SERVICE` | `com.jelleglebbeek.youtube-dl-gui` | 钥匙串 service（与 Tauri identifier 一致） |
| `KR_ACCOUNT` | `master_key` | 钥匙串账号 |
| 快照路径 | `{app_dir}/vault.hold` | 由 `lib.rs` 拼接 |
| `StrongholdState.inner` | `Mutex<Option<Stronghold>>` | `Some` 即已解锁 |
| `StrongholdState.init_error` | `Mutex<Option<String>>` | 仅记录"自动解锁/重建失败"与"钥匙串不可用" |

### 解锁与创建路径

1. `lib.rs` 先 `StrongholdState::new(app_dir/vault.hold)`。
2. 非 macOS-debug 时调用 `init_on_startup`；macOS debug 下跳过（等待用户显式 `stronghold_init`）。
3. `init_on_startup`：
   - 快照不存在 → 直接返回（不创建、不报错，锁定状态）。
   - 清除旧的 `init_error`。
   - 读钥匙串：`Err` → `init_error = Secure keyring unavailable: {e}`；`Ok(None)` → 什么都不做；
     `Ok(Some(b64))` → base64 解码 → `open_with_key`：
     - 成功 → `inner = Some`；
     - 失败（密钥损坏/无法打开）→ `create_and_store_new`（覆盖重建），若重建仍失败则写 `init_error`。
4. `open_with_key`：`Stronghold::new(path, key)` → `load_client(CLIENT)`；client 不存在时 `create_client` + `write_client` + `save`。
5. `create_and_store_new`：`rand::rng().fill_bytes` 生成 32 字节主密钥 → `create_new_stronghold` → 把 base64 主密钥写入钥匙串。

### 凭据读取（`load_auth_secrets`）

- 需要 `inner` 已解锁，否则 `Err("vault locked")`。
- 逐个读取 5 个键，值为空字节或 trim 后为空时视为 `None`。
- `auth.headers` 按行拆分：跳过空行与不含 `:` 的行，其余 trim 后保留。
- 返回 `AuthSecrets { username, password, video_password, bearer_token, headers }`。

### 凭据注入

`YtdlpRunner::with_auth_args`：

1. `resolve_with_patch(&cfg.auth, overrides.auth)` 得到 Cookie 设置（`cookieBrowser != "none"` → `--cookies-from-browser`；`cookieFile` → `--cookies`）。
2. `app.try_state::<StrongholdState>()` → `load_auth_secrets()`（失败静默忽略，得到默认空值）。
3. `resolve_with_patch(&secrets, overrides.auth)`：组级 `auth` override 可覆盖任意机密字段。
4. `apply_auth_secrets`：按固定顺序追加 `--username`、`--password`、`--video-password`、`--add-header Authorization:Bearer …`、逐条 `--add-header`。

### 日志与遥测

- `log_run_summary` 只输出 `has_auth`/`has_cookies`/`has_browser_cookies`/`has_proxy` 布尔与 `arg_count`，不读取值。
- Sentry 上报路径（`should_report_to_sentry`）只上报错误类型与错误文本；错误文本来自 yt-dlp/系统，不包含应用注入的凭据（yt-dlp 自身可能回显用户名，属于上游行为，需在评审时注意）。
- 前端 `Sentry` 初始化开启 `traces_sample_rate: 0.05`、`sample_rate: 0.25`；不含保险库内容。

### 与前端的数据契约

- 传输始终是字节数组，避免把明文当作 JSON 字符串出现在调试工具/日志中。
- `availableKeys` 只用于长度判断，前端不解析键名（键名常量在 `STRONGHOLD_KEYS` 中硬编码）。

## Boundaries

- 保险库的加密与 KDF 由 `tauri_plugin_stronghold` 提供，应用不自实现密码学。
- 不做自动锁定/超时锁定；应用运行期间保持解锁。
- 不提供导出/导入保险库功能。

## Contracts

- 命令与键名：`api-contract.md`。
- 安全底线与禁止项：`../rules/security.rules.md`、`../rules/forbidden.rules.md`。

## Failure And Edge Cases

- **静默重建**：任何"主密钥无法打开保险库"的情况都会走重建流程，用户已保存的凭据会丢失且界面不会给出明确警告（仅 `init_error` 在重建失败时出现）。
- **headers 的字节精度**：写入时按 UTF-8 存整段多行文本；读取时按行解析，因此行内容中的换行不能被表示。
- **并发写入**：`stronghold_set` 持有 `Mutex`，多个前端调用会串行；但没有快速连续保存的合并。
- **Windows 凭据管理器**：钥匙串依赖 `tauri_plugin_keyring` 的凭据管理器后端；企业策略禁用时会命中 `Secure keyring unavailable`，此时所有机密功能不可用（Cookie 路径仍可用）。

## Verification

见 `docs/current/domains/auth-secrets/verification.md`。
