---
description: "Use when writing or modifying Rust source code in this project. Covers error handling, module organization, type safety, async patterns, and all Clippy-enforced rules."
applyTo:
  - src/**/*.rs
---

# Rust 编码规范

本项目通过 `Cargo.toml` 中的 `[lints]` 配置强制执行严格的代码质量标准。以下规则在所有 Rust 源文件中**必须遵守**。

## 禁止项（编译级阻止）

以下模式被 Clippy 设置为 `deny`，会直接导致编译失败：

```rust
// ❌ 禁止——编译失败
let x = some_option.unwrap();          // clippy::unwrap_used = "deny"
let x = some_result.expect("msg");     // clippy::expect_used = "deny"
panic!("something went wrong");        // clippy::panic = "deny"
todo!();                               // clippy::todo = "deny"
unimplemented!();                      // clippy::unimplemented = "deny"
unsafe { /* ... */ }                   // unsafe_code = "forbid"
```

例外：内部辅助函数（私有、非 `pub`）可在文件开头添加 `#![allow(clippy::expect_used)]` 允许有限使用 `.expect()` 来构建 `LazyLock<Regex>`（参考 `basic.rs`、`wikitext.rs`）。

## 错误处理

### 公共 API：使用 `?` 传播

```rust
/// # Errors
/// 若网络请求失败，返回协议级 `CallToolError`。
pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
    let main_text = fetch_wikitext(&self.name)
        .await
        .map_err(|e| CallToolError::new(std::io::Error::other(e.to_string())))?;
    // ...
}
```

### 两种错误类型区分

| 场景 | 返回方式 | 含义 |
|------|---------|------|
| 用户输入错误（无效 category、干员不存在） | `CallToolResult::with_error(CallToolError::from_message(msg))` | 工具执行层错误，`isError: true` |
| 系统/网络错误 | `CallToolError::new(...)` | 协议层错误，中断执行 |

## 模块组织

本项目按**领域**拆分模块，而非按类型：

```
src/
├── tools/get_operator/    ← 按数据域拆分为独立解析器
│   ├── basic.rs           ← 基础信息解析
│   ├── combat.rs          ← 战斗数据解析
│   ├── build.rs           ← 养成材料解析
│   ├── lore.rs            ← 档案解析
│   ├── gallery.rs         ← 图鉴立绘解析
│   ├── voice.rs           ← 语音台词解析
│   ├── strings.rs         ← 文本常量集中管理
│   └── tool.rs            ← 路由与主逻辑
├── utils/                 ← 跨域共享工具
│   ├── wiki_client.rs     ← HTTP 请求封装
│   ├── wikitext.rs        ← wikitext 清洗
│   ├── categorizer.rs     ← 行语义分类
│   └── range_grid.rs      ← SVG→ASCII 转换
```

- 每个解析器模块**独立**、**无副作用**，输入 `&[String]` → 输出 `Vec<String>`
- 共享常量抽到 `strings.rs`，避免魔法字符串散落各处
- 公开符号通过 `mod.rs` 重导出

## 正则表达式

所有正则使用 `LazyLock<Regex>` 预编译，确保全局只编译一次：

```rust
use regex::Regex;
use std::sync::LazyLock;

static REX_PARAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\|(.*?)=(.*?)$").expect("regex PARAM"));
```

- 命名约定：`static REX_XXX: LazyLock<Regex>`
- `.expect("regex XXX")` 中的消息使用简短标识符（此处 `expect` 是允许的例外）

## 类型标注

- 纯函数标注 `#[must_use]`
- 所有公共结构体派生 `#[derive(Debug)]`
- Tool 参数结构体同时派生 `Deserialize, Serialize, JsonSchema`
- 使用 `#[async_trait]` 实现异步 trait

## 禁用模式

- 全局可变状态
- 同步阻塞 I/O（全部使用 async reqwest）
- 在 `pub fn` 中对 `Option`/`Result` 使用 `.unwrap()` 或 `.expect()`
