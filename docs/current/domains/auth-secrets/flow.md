---
status: current
layer: domain
domain: auth-secrets
canonical_for:
  - auth-secrets-flow
related:
  - docs/current/domains/auth-secrets/backend-behavior.md
  - docs/current/domains/download-engine/flow.md
last_verified: 2026-09-17
---

# auth-secrets 流程

## Purpose

描述机密从"用户输入"到"出现在 yt-dlp argv"之间经过的所有节点与判定，以及保险库的创建/解锁/降级路径。

## Current Behavior

```mermaid
flowchart TD
  A[应用启动 setup] --> B[PathsManager.app_dir/vault.hold]
  B --> C[StrongholdState::new(snapshot_path)]
  C --> D{macOS debug 构建?}
  D -- 是 --> E[跳过自动解锁, 等待用户点启用]
  D -- 否 --> F[init_on_startup]
  F --> G{vault.hold 存在?}
  G -- 否 --> H[保持锁定, 首次保存时创建]
  G -- 是 --> I[keyring.get_password(service, master_key)]
  I -- 返回 base64 密钥 --> J[Stronghold::new 打开 + load_client('ovd')]
  J -- 失败 --> K[create_and_store_new: 重新生成密钥与保险库]
  I -- 无记录(None) --> L[保持锁定且无错误]
  I -- Err --> M[init_error = 'Secure keyring unavailable']
  J -- 成功 --> N[unlocked = true]
  N --> O[前端 loadStatus -> stronghold_keys]
  O --> P[认证页 getValues -> stronghold_get]
  P --> Q[用户编辑 -> stronghold_set 写入字节数组]
  Q --> R[write_client + save 落盘 vault.hold]
  N --> S[下载时 load_auth_secrets -> resolve_with_patch(overrides)]
  S --> T[with_auth_args 注入 --username/--password/--video-password/--add-header]
```

### 关键节点

| 节点 | 输入 | 职责 | 输出/状态变化 | 边界 |
| --- | --- | --- | --- | --- |
| `StrongholdState::new` | `app_dir/vault.hold` | 持有快照路径、`Mutex<Option<Stronghold>>`、`Mutex<Option<String>> init_error` | 初始为锁定 | 不触碰磁盘 |
| `init_on_startup` | 钥匙串 + 快照 | 尝试用钥匙串中的主密钥解锁 | `inner = Some` 或 `init_error` | 快照不存在直接返回（锁定且无错误） |
| `create_and_store_new` | 随机 32 字节 | 生成主密钥 → 创建/覆盖保险库 → 写入钥匙串（base64） | 解锁 | 会**覆盖旧保险库**（旧密钥不可恢复时） |
| `stronghold_init`（命令） | 用户点击"启用" | 显式创建新保险库并写钥匙串 | `VaultStatus` | 与自动路径共用 `create_and_store_new` |
| `stronghold_status` | — | 报告 `{ unlocked, initError }` | 前端状态 | 不抛错 |
| `stronghold_keys` | — | 返回已存在的键（字节数组列表） | 前端 `availableKeys` | 锁定时返回 `Err("vault locked")` |
| `stronghold_get` | 键名列表 | 返回 `{ key: bytes \| null }` | 前端解码为字符串 | 锁定时 `Err` |
| `stronghold_set` | `{ key: bytes \| null }` | 逐个 insert/delete → `write_client` → `save` | 快照更新 | `null` 表示删除该键 |
| `load_auth_secrets` | 保险库 | 组装 `AuthSecrets`（trim、丢弃非法 header 行） | `AuthSecrets` | 锁定时返回 `Err`；调用方 `.ok()` 吞掉错误 |
| `resolve_with_patch` | `AuthSecrets` + `AuthOverrides` | 组级覆盖 | 最终凭据 | 只覆盖显式提供的字段 |
| `with_auth_args` | 最终凭据 | 生成 `--*` 参数 | argv | 值绝不进日志 |

### Cookie 与机密的存储分工

| 数据 | 存储 | 说明 |
| --- | --- | --- |
| Cookie 文件路径 | `config.store.json`（`auth.cookieFile`） | 明文路径，不含内容 |
| Cookie 浏览器名 | `config.store.json`（`auth.cookieBrowser`） | 明文 |
| 用户名 / 密码 | `vault.hold`（加密） | 主密钥在系统钥匙串 |
| 视频密码 | `vault.hold` | 键 `video.password` |
| Bearer Token | `vault.hold` | 键 `auth.bearer`，注入为 `Authorization:Bearer <token>` |
| 自定义请求头 | `vault.hold` | 键 `auth.headers`，多行文本，每行 `Name: value` |

### 前端保存流程（`AuthenticationView.vue`）

1. `onMounted`：`strongholdStore.status.unlocked` 为真时 `getValues()` 并记录快照字符串。
2. 表单分两部分：`cookies-config`（Cookie，绑定到 `settings.auth` 草稿）与 `credentials-config`（机密，绑定到 `StrongholdFields`）。
3. 保存：
   - Cookie 有变更 → `settingsStore.patch({ auth: cookieFields })`；
   - 机密有变更且已解锁 → `strongholdStore.setValues(fields)` → 刷新 `availableKeys` 与快照；
   - toast 成功/失败，失败信息包含原始错误。
4. 未解锁时 `credentials-config` 内显示 `CredentialsInit`（启用/重试按钮），Cookie 部分仍可编辑。

### 状态灯（`TheFooter.vue`）

`hasAuth = settingsStore.hasAuthConfigured() || strongholdStore.hasAvailableKeys()`：

- Cookie 文件或浏览器任一配置，或保险库中已有任意键，即视为"已配置认证"。

## Boundaries

- 保险库只保存文本凭据；不做证书、OAuth 刷新、密钥轮换。
- 前端没有"删除保险库"入口；清空字段会删除对应键（`null`），但不会清空整个快照。
- 后端不校验凭据内容（header 只要求含 `:`，其余原样下发）。

## Contracts

- 命令与键名：`api-contract.md`。
- 注入路径：`../download-engine/api-contract.md` 的"认证参数"。
- 安全底线：`../rules/security.rules.md`。

## Failure And Edge Cases

- **keyring 返回 None 但快照存在**：保险库保持锁定且 `initError` 为空，前端只会显示"启用"按钮；再次点击会**新建保险库**并覆盖旧快照（旧凭据不可恢复）。
- **主密钥损坏**：`open_with_key` 失败 → 自动 `create_and_store_new`，同样是覆盖式重建（静默丢失旧凭据）；仅有"重建也失败"时才写 `init_error`。
- **macOS debug 构建**：启动时不自动解锁（`cfg(not(all(target_os = "macos", debug_assertions)))`），这是为了避开开发期钥匙串反复授权；因此 dev 环境需手动点"启用"。
- **下载路径吞错**：`with_auth_args` 中 `load_auth_secrets` 失败会被忽略（`if let Ok(...)`），表现为"凭据未生效但下载继续"。
- **空值语义**：`load_auth_secrets` 只有非空且 trim 后非空的字节才会 `Some`；header 行不含 `:` 会被丢弃。

## Verification

见 `docs/current/domains/auth-secrets/verification.md`。
