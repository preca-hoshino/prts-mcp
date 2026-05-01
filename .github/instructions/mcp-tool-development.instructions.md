---
description: "Use when creating, modifying, or debugging MCP tools. Covers tool registration, input validation, error handling, description format, and parameter design."
applyTo:
  - src/tools/**/*.rs
---

# MCP 工具开发规范

## 工具注册

每个 Tool 使用 `#[mcp_tool]` 宏声明。所有工具汇总在 `src/tools/mod.rs` 中通过枚举和 `tool_box!` 宏统一注册。

### description 格式

Tool 的 `description` **必须严格遵循**以下 XML 结构化格式（中文），不可省略任何部分：

```xml
<when_to_use>
- 具体场景 1
- 具体场景 2
</when_to_use>

<when_not_to_use>
- 不适用场景 1
</when_not_to_use>

<parameters>
- param_name: 参数说明
</parameters>

<output_format>
返回格式说明
</output_format>

<important>
注意事项
</important>
```

此格式为**强制结构**，每个新工具 description 必须包含全部 5 个部分。

Tool 属性：`read_only_hint = true`（只读工具，本服务器所有工具均为只读）。

## 输入校验（防御式编程）

**必须**在任何外部调用之前校验所有参数，不信任 LLM 填写的值：

```rust
pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
    // 1. 服务端输入验证（不信任 LLM 填写的参数，防止越界调用）
    let category_upper = self.category.to_uppercase();
    if !VALID_CATEGORIES.contains(&category_upper.as_str()) {
        return Ok(CallToolResult::with_error(CallToolError::from_message(
            ERR_INVALID_CATEGORY.replace("{category}", &self.category),
        )));
    }
    // 2. 拉取外部数据...
}
```

校验顺序：
1. 枚举值校验（category、参数范围）
2. 外部调用
3. 空值处理（页面不存在 → 友好错误）

## 错误消息

### 用户错误 → `CallToolResult::with_error`

错误消息使用中文，带 emoji 前缀：

```rust
// 无效参数
ERR_INVALID_CATEGORY: "❌ 错误：无效的 `category` 参数「{category}」。\n..."

// 资源不存在
ERR_OPERATOR_NOT_FOUND: "🔍 未找到干员「{name}」的 Wiki 页面，请检查名称是否正确。\n..."
```

### 系统错误 → `CallToolError::new`

```rust
let text = fetch_wikitext(&name)
    .await
    .map_err(|e| CallToolError::new(std::io::Error::other(e.to_string())))?;
```

## 工具命名

- 工具名：`snake_case`（如 `get_operator`、`search_operators`）
- 参数名：`snake_case`
- 枚举值：大写（如 `BASIC`、`COMBAT`），但通过 `to_uppercase()` 实现大小写不敏感

## 并发处理

多个独立的外部 API 调用使用 `tokio::join!` 并发执行，而非串行：

```rust
let (a_result, b_result) = tokio::join!(
    fetch_set_a(&query),
    fetch_set_b(&query),
);
```
