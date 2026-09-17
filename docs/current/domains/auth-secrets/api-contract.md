---
status: current
layer: domain
domain: auth-secrets
canonical_for:
  - auth-secrets-api-contract
related:
  - docs/current/shared/ipc-conventions.md
  - docs/current/domains/auth-secrets/flow.md
last_verified: 2026-09-17
---

# auth-secrets 接口契约

## Scope

- 5 个命令：`stronghold_init`、`stronghold_status`、`stronghold_keys`、`stronghold_get`、`stronghold_set`。
- 无事件；所有状态通过命令返回值传递。
- 传输格式：**字节数组**（`number[]`），前端用 `TextEncoder`/`TextDecoder`（UTF-8）转换。

## Conventions

- 键名是稳定字符串常量，定义在前端 `STRONGHOLD_KEYS` 与后端 `load_auth_secrets` 中，双方必须一致。
- `null` 值在 `stronghold_set` 中表示**删除该键**；在 `stronghold_get` 中表示键不存在。
- 锁定时除 `stronghold_status`/`stronghold_init` 外的命令都返回 `Err("vault locked")`。

## 键名契约

| 前端字段 | 键名 | 内容 | 注入的 yt-dlp 参数 |
| --- | --- | --- | --- |
| `username` | `auth.username` | 用户名 | `--username <v>` |
| `password` | `auth.password` | 密码 | `--password <v>` |
| `videoPassword` | `video.password` | 视频密码 | `--video-password <v>` |
| `bearer` | `auth.bearer` | Bearer Token | `--add-header Authorization:Bearer <v>` |
| `headers` | `auth.headers` | 多行请求头文本 | 每个合法行一个 `--add-header <line>` |

## stronghold_init

- **功能职责**：创建（或按当前主密钥打开）保险库，并把主密钥写入系统钥匙串；用于首次启用与失败重试。
- **参数**：无。
- **返回**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `unlocked` | `boolean` | 初始化后是否可用 |
| `initError` | `string \| null` | 失败原因（成功时为 `null`） |

- **副作用**：可能覆盖已有 `vault.hold` 与钥匙串中的主密钥。
- **失败语义**：命令本身不返回 `Err`，失败信息放在 `initError`。

## stronghold_status

- **功能职责**：查询当前解锁状态与初始化错误。
- **参数**：无。
- **返回**：`{ unlocked: boolean, initError: string | null }`（与 `stronghold_init` 同结构）。

典型取值：

| 场景 | `unlocked` | `initError` |
| --- | --- | --- |
| 正常解锁 | `true` | `null` |
| 快照不存在（首次运行） | `false` | `null` |
| 钥匙串无记录 | `false` | `null` |
| 钥匙串不可用（权限/无 keyring 服务） | `false` | `Secure keyring unavailable: …` |
| 重建保险库也失败 | `false` | `Recreate vault failed …` |

## stronghold_keys

- **功能职责**：列出保险库中已存在的键，用于"是否已配置认证"与展开状态。
- **参数**：无。
- **返回**：`number[][]` —— 每个元素是某个键名 UTF-8 字节的数组（**不是字符串**）。
- **失败**：锁定时 `Err("vault locked")`。

## stronghold_get

- **功能职责**：批量读取指定键的原始字节。
- **参数**：

| 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `keys` | `string[]` | 是 | 键名列表（前端传 `Object.values(STRONGHOLD_KEYS)` 的 5 个键） |

- **返回**：`Record<string, number[] | null>`，键为**请求的键名**，值为 UTF-8 字节或 `null`。
- **失败**：锁定时 `Err("vault locked")`；`get_client` 失败返回其错误原文。

## stronghold_set

- **功能职责**：写入/删除多个键并立即落盘（`write_client` + `save`）。
- **参数**：

| 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `entries` | `Record<string, number[] \| null>` | 是 | 键 → 字节（`null` 删除） |

- **返回**：`Ok(())`；任何一步失败返回 `Err`（可能只写入了部分键，因为循环中没有事务）。
- **副作用**：更新 `vault.hold`；前端随后调用 `stronghold_keys` 刷新可用键。

示例：

```ts
await invoke('stronghold_set', {
  entries: {
    'auth.username': Array.from(new TextEncoder().encode('alice')),
    'video.password': null, // 删除
  },
});
```

## Failure And Edge Cases

- `stronghold_keys` 返回字节数组而非字符串，前端只用其长度（`hasAvailableKeys`）——这是有意的"最小暴露"。
- `stronghold_set` 没有原子性：中途失败会留下部分写入的键，且不会回滚。
- `stronghold_get` 的返回值以**请求键名**为 key，直接可用于前端字段映射（`STRONGHOLD_KEYS` 反查）。
- 命令都是 `async`，但内部使用同步 `Mutex`；真正耗时的是密钥派生/IO，会短暂阻塞调用线程。
