# Linguist — LLM Gateway 开发指南

> **项目概述、快速启动、API 文档**：参见 [README.md](../README.md)
> **开发流程与原子提交规范**：参见 [.agents/instructions.md](../.agents/instructions.md)
> **各模块详细说明**：参见 `src/<module>/README.md`（每个二级目录均有）

---

## 架构核心概念

### GatewayContext 贯穿全生命周期

`GatewayContext` 是整个请求处理流程的唯一载体对象，所有阶段均通过读写 `ctx` 的字段完成工作：

```
创建 ctx → 用户适配(填充 ctx.request) → 请求中间件(ctx)
  → 路由(填充 ctx.route) → 提供商请求适配(ctx.request + ctx.route.model)
  → 提供商调用 → 提供商响应适配(填充 ctx.response)
  → 响应中间件(ctx) → 用户响应适配(从 ctx 组装) → 发送
```

### 双层适配器模式

- **用户适配器** (`src/users/`)：转换客户端格式 ↔ 内部类型。按外部格式顶级分类（如 `openaicompat/`、`gemini/`、`anthropic/`），内部再分 `chat/` 和 `embedding/` 模块及错误格式化。
- **API 格式路由** (`src/api/`)：每种用户 API 格式独立定义端点和 model 提取逻辑，调用 `processChatCompletion` 将处理委托给核心流程。
- **提供商适配器** (`src/providers/`)：转换内部类型 ↔ 厂商 API 格式。按厂商顶级分类（如 `deepseek/`、`gemini/`、`volcengine/`），内部包含 `error-mapping.ts` 和对应能力的适配（如 `chat/` 的请求/响应/客户端）。

### 内部统一类型 (`src/types/`)

所有模块间通信使用统一类型：
- `GatewayContext` — 请求生命周期载体
- `InternalChatRequest` / `InternalChatResponse` — 聊天请求/响应（无 `model`、无 `id`）
- `InternalEmbeddingRequest` / `InternalEmbeddingResponse` — 嵌入请求/响应

---

## 编码约定

- 适配器文件统一命名：`request.adapter.ts`、`response.adapter.ts`、`client.ts`、`index.ts`
- 每个适配器目录必须有 `index.ts` 简化导入路径
- 接口定义放在对应分类的 `interface.ts` 中（如 `providers/chat/interface.ts`）
- 错误统一由 `handleError(err, res, userFormat)` 捕获，抛出 `GatewayError(statusCode, errorCode, message)`
- 核心流程编排在 `src/app/`（拆分为 `process.ts`、`stream.ts`、`helpers.ts`），入口在 `src/index.ts`，HTTP 配置在 `src/server.ts`，API 格式路由在 `src/api/`
- `InternalRequest` 不含 `model` 字段，提供商适配器从 `ctx.route.model` 取实际模型名
- 用户响应适配器接收 `GatewayContext`，从中取 `ctx.id`、`ctx.timestamp`、`ctx.requestModel`、`ctx.response` 组装最终响应
- Chat 支持流式（SSE）和非流式两种响应模式；Embedding 仅支持非流式
- **不需考虑向后兼容性**：项目尚未上线，可以自由修改现有接口或重构代码，无需保留旧版本兼容代码
- **管理 API 变更必须同步 `admin.http`**：每次新增、修改或删除 `src/admin/` 下的路由，都必须同步更新项目根目录的 `admin.http` 文件；`admin.http` 中仅保留已实现适配器的提供商示例，不包含用户侧 API
- **修改完成必须进行逐步的全面代码检查**：每次代码修改完成后，先使用 `npm run check` 的具体子脚本（如 `npm run check:lint`、`npm run check:types` 等）逐项进行检查，并结合命令参数（如特定文件或限制输出行数）进行分批、逐步的修复，避免被海量报错淹没；在逐步修复完毕后，**最后必须再次执行一次整体的 `npm run check`** 来处理漏网之鱼，确保没有任何错误和警告，全部通过时任务才算真正完成。

