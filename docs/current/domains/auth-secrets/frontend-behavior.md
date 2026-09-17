---
status: current
layer: domain
domain: auth-secrets
canonical_for:
  - auth-secrets-frontend-behavior
related:
  - docs/current/domains/auth-secrets/api-contract.md
  - docs/current/domains/settings-preferences/frontend-behavior.md
last_verified: 2026-09-17
---

# auth-secrets 前端行为

## Purpose

说明认证相关界面与 store 的行为契约：状态获取、表单绑定、保存判定、错误呈现。

## Current Behavior

### store（`src/stores/stronghold.ts`）

| 成员 | 说明 |
| --- | --- |
| `status: { unlocked, initError? }` | 由 `loadStatus()` / `initialize()` 更新 |
| `availableKeys: number[][]` | 已存在的键（字节数组）；仅用 `hasAvailableKeys()` 判空 |
| `loadStatus()` | `stronghold_status` → 解锁则再取 `stronghold_keys`；`initError` 存在时抛 `Error('Failed to retrieve stronghold status: …')` |
| `initialize()` | `stronghold_init` → 若 `initError` 抛错；否则更新状态并取 `stronghold_keys` |
| `getValues()` | 取 5 个键并用 `TextDecoder` 解码为 `StrongholdFields`（缺失为 `null`） |
| `setValues(fields)` | 用 `TextEncoder` 编码后 `stronghold_set`，随后刷新 `availableKeys` |
| `hasAvailableKeys()` | `availableKeys.length > 0`，供底部状态灯与折叠面板默认展开 |

`STRONGHOLD_KEYS` 是前后端共享的键名契约：

```ts
{ username: 'auth.username', password: 'auth.password', videoPassword: 'video.password',
  bearer: 'auth.bearer', headers: 'auth.headers' }
```

### 启动时的状态获取（`src/App.vue`）

`void strongholdStore.loadStatus()` 在挂载后立即执行；失败只 `console.error`。
因此"钥匙串不可用"不会阻塞启动，只会在认证页显示错误卡片。

### 认证页（`src/views/app/AuthenticationView.vue`）

- 表单由两个区块组成：
  - `CookiesConfig`：`cookieFile`（带最近使用下拉与清空）与 `cookieBrowser` 下拉（选项来自 `cookieBrowserOptions`，`none` 表示不使用）。
  - `CredentialsConfig`：可折叠面板，`:open="hasAvailableKeys()"`；未解锁时内部渲染 `CredentialsInit`，否则渲染 5 个 `BaseSecretInput`（用户名、密码、视频密码、Bearer、请求头多行文本）。
- 状态与快照：
  - `cookieFields` 深拷贝自 `settingsStore.settings.auth`；
  - `strongholdFields` 来自 `getValues()`，`strongholdSnapshot` 记录其 JSON 串；
  - `hasChanges` = Cookie 变化 **或**（机密变化 **且** 已解锁）。
- 保存顺序：先 Cookie（`settingsStore.patch`），后机密（`strongholdStore.setValues`）；两者任一失败都会 toast 错误并保留页面。
- 保存按钮在无变更或正在保存时禁用（`BaseButton` 的 `disabled`/`loading`）。

### 凭据初始化卡片（`CredentialsInit.vue`）

- 无错误：提示"启用安全存储" + `common.enable` 按钮 → `initialize()`。
- 有错误（`status.initError != null`）：切换为 error 样式，显示 `auth.init.error.hint`（带原始错误）与 `common.retry`。
- 失败时同时 toast（5 秒）。

### 状态灯（`src/components/TheFooter.vue`）

`hasAuthConfigured() || hasAvailableKeys()` → 底部"钥匙"图标点亮。注意：**只要保险库里存过任意键就会点亮**，
不会判断凭据是否有效。

### 表单输入（`src/components/base/BaseSecretInput.vue`）

- 非密码字段与密码字段共用组件；密码字段使用 `type=password`。
- 值直接绑定到 `StrongholdFields` 的字符串字段，`null` 表示"未设置"。

## Boundaries

- 前端不缓存/持久化机密（不写 localStorage、不进 pinia 持久化插件）。
- 前端不做字段级校验（除请求头由后端按 `:` 过滤）；空字符串会被当作"清空该键"。
- Cookie 与机密分属两个后端系统，保存是两个独立请求（无事务）。

## Contracts

- 键名与命令：`docs/current/domains/auth-secrets/api-contract.md`。
- 影响：凭据在下载时的注入顺序见 `../download-engine/api-contract.md`。
- 安全底线：`../rules/security.rules.md`。

## Failure And Edge Cases

- `loadStatus()` 在 `initError` 非空时抛错，`App.vue` 只 `console.error`；因此用户只有在打开认证页时才看到错误。
- `hasChanges` 对 `strongholdFields` 用 JSON 比较，字段顺序变化会误判；但快照与实际值都由同一 `getValues()` 生成，顺序稳定。
- 未解锁时编辑机密不会触发保存（`hasStrongholdChanges` 要求 `status.unlocked === true`），用户可能误以为已保存；此时应通过 `CredentialsInit` 先启用。
- Cookie 变更与机密变更同时存在时，若 Cookie 保存成功、机密保存失败，磁盘上会出现"Cookie 已更新"的中间状态。
- `setValues` 会把 `null` 转成删除操作，因此清空输入框 = 删除该键（不是保存空字符串）。

## Verification

见 `docs/current/domains/auth-secrets/verification.md`；E2E：`tests/e2e/authentication.spec.ts`。
