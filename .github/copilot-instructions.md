# PRTS MCP Server 开发指南

## 项目概述

这是一个 Rust 编写的 MCP（Model Context Protocol）服务器，为 LLM 提供明日方舟干员数据。数据源为 [PRTS Wiki](https://prts.wiki)，通过 MediaWiki API 拉取 wikitext 并解析为 Markdown。

## 技术栈

- **语言**: Rust (edition 2024)
- **运行时**: Tokio (async)
- **MCP 框架**: `rust-mcp-sdk` v0.9.0
- **通信方式**: Stdio Transport
- **关键依赖**: reqwest, serde/serde_json, regex, scraper

## 目录结构

```
src/
├── main.rs           # 入口，MCP Server 初始化
├── handler.rs        # ServerHandler trait 实现，请求路由
├── prompts/          # MCP Prompts 特性（预留）
├── resources/        # MCP Resources（干员列表）
├── tools/            # MCP Tools
│   ├── get_operator/ # 单干员数据查询（6 个数据域解析器）
│   └── search_operators/  # 干员搜索 + 属性过滤
└── utils/            # 共享工具：wikitext 清洗、语义分类、HTTP 请求
temp/                 # 原始 JS 原型与测试工具（非 Rust 代码）
```

## 编码原则

以下规则适用于所有 Rust 源文件：

1. **禁止 panic/unwrap/expect** 在公共 API 中——Clippy 会拒绝编译
2. **所有 pub 项必须文档化** — `///` 或 `//!`，包含 `# Errors` 节（对 `Result` 返回值）
3. **中文注释**用于业务逻辑说明，**英文术语**保留 Rust 标准命名
4. **字符串常量集中管理**在对应模块的 `strings.rs`，命名遵循 `FIELD_`/`ERR_`/`VALID_` 前缀
5. **防御式输入校验** — MCP Tool 参数在外部调用前验证，不信任 LLM 输入
6. **异步 I/O 全覆盖** — 所有 HTTP 请求通过 async reqwest，禁止同步阻塞

## 开发工作流

```bash
# 全量检查（格式化 + Lint + 测试）
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
# 或使用 just
just check
```

## 提交规范

提交信息格式：`[Type](scope): 描述`

| Type | 用途 |
|------|------|
| `[Add]` | 新增功能 |
| `[Fix]` | 修复 Bug |
| `[Ref]` | 代码重构 |
| `[Del]` | 删除冗余代码 |
| `[Doc]` | 文档修改 |
| `[Chore]` | 日常维护 |
| `[Style]` | 代码风格调整 |
| `[Test]` | 测试代码 |
| `[Merge]` | 分支合并 |

详见 [CONTRIBUTING.md](CONTRIBUTING.md)。
