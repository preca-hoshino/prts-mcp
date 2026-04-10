# Justfile for prts-mcp
# 参考 package.json 的命名习惯，提供对标的 Rust 开发脚本
# 注意：just 不支持在任务名中使用冒号 (:)，因此改用横杠 (-)

set shell := ["powershell", "-c"]

# 默认显示任务列表
default:
    @just --list

# [dev] 开发模式：启动 MCP 服务器
dev:
    cargo run

# [build] 构建模式：发布版本构建
build:
    cargo build --release

# [check-format] 格式化检查
check-format:
    cargo fmt --all -- --check

# [check-lint] 静态代码检查 (Clippy)
check-lint:
    cargo clippy --all-targets --all-features -- -D warnings

# [check-types] 类型检查 (编译解析)
check-types:
    cargo check --all-targets --all-features

# [check-deps] 检查未使用的依赖 (需要安装 cargo-machete)
check-deps:
    cargo machete

# [check-test] 运行单元与集成测试
check-test:
    cargo test

# [check] 一键运行所有检查任务
check:
    @just check-format
    @just check-lint
    @just check-types
    @just check-deps
    @just check-test