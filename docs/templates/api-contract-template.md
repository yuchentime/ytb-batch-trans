---
status: current
layer: domain
domain: <domain>
canonical_for:
  - <domain>-api-contract
last_verified: <YYYY-MM-DD>
---

# <Domain> 接口契约

## Scope

- 本文档覆盖的接口范围：<store / owner / admin / shared>
- 相关共享约定：`docs/current/shared/api-conventions.md`

## Conventions

- 所有接口必须给出完整的输入/输出 schema，而不只是 DTO/VO 名称。
- 每个接口必须先给出一段简短描述，说明它的功能职责、面向谁、解决什么场景。
- 每个字段都应说明：字段名、类型、是否必填、来源位置、含义、允许值/格式、为空语义。
- 如存在分页、枚举、嵌套对象、数组元素、二进制响应或错误返回，必须展开说明。

## <Endpoint Group Name>

### <Endpoint Title>

- **方法**：`GET`
- **路径**：`/app-api/pet/...`
- **功能职责**：<一句话说明该接口做什么、给谁用、解决什么场景>
- **鉴权/归属**：<例如 `userId` 必填；仅返回当前门店数据>

#### Request Summary

| 位置 | 名称 | 类型 | 必填 | 说明 |
|---|---|---|---|---|
| Query | `userId` | `Long` | 是 | 当前门店用户 id |

#### Request Schema

##### Query Params

```json
{
  "userId": 10001,
  "id": 20001
}
```

| 字段 | 类型 | 必填 | 说明 | 允许值/格式 | 为空语义 |
|---|---|---|---|---|---|
| `userId` | `Long` | 是 | 当前门店用户 id | 正整数 | 不允许为空 |
| `id` | `Long` | 是 | 报告 id | 正整数 | 不允许为空 |

##### Request Body

```json
{
  "name": "demo"
}
```

| 字段 | 类型 | 必填 | 说明 | 允许值/格式 | 为空语义 |
|---|---|---|---|---|---|
| `name` | `String` | 是 | 示例字段 | UTF-8 字符串 | 不允许为空 |

#### Response Schema

```json
{
  "id": 20001,
  "name": "demo"
}
```

| 字段 | 类型 | 必填 | 说明 | 允许值/格式 | 为空语义 |
|---|---|---|---|---|---|
| `id` | `Long` | 是 | 主键 id | 正整数 | 不允许为空 |
| `name` | `String` | 是 | 示例字段 | UTF-8 字符串 | 返回空字符串表示未填写 |

#### Nested Objects

##### `items[]`

| 字段 | 类型 | 必填 | 说明 | 允许值/格式 | 为空语义 |
|---|---|---|---|---|---|
| `code` | `String` | 是 | 子项编码 | 业务枚举值 | 不允许为空 |
| `label` | `String` | 是 | 子项名称 | UTF-8 字符串 | 不允许为空 |

#### Error Cases

| 场景 | code | 说明 |
|---|---|---|
| <not found> | `<ERROR_CODE>` | <错误含义> |
| <forbidden> | `<ERROR_CODE>` | <错误含义> |

#### Notes

- <分页返回可引用 `PageResult<T>`，但仍需展开 `list[]` 元素结构和分页字段含义>
- <二进制下载响应应展开 header/body 结构，不可只写 binary>
