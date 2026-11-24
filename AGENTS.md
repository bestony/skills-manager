# AGENTS.md

## 项目概览

`skills-manager` — 基于 [GPUI Kit](https://gpui-kit.com) 的 Rust 原生桌面 GUI 应用（单 crate，无 workspace、无 CI）。

- 入口：`src/main.rs`（`gpui_kit::application()` → `init` → `open_window`）
- 唯一依赖：`gpui-kit = "0.11.0"`（re-export `gpui`/`gpui-component`/`gpui-base`）

## 技术栈

- Rust，edition 2024（注意新语法规则与旧版差异）
- GPUI + gpui-kit 0.11.0（GUI 框架，类似 retained-mode Elm 架构）

## 常用命令

```bash
cargo run                  # 运行应用（打开 GUI 窗口）
cargo test                 # 测试
cargo fmt --all            # 格式化（提交前必跑）
cargo clippy --all-targets # lint（提交前必跑）
cargo check                # 快速类型检查
```

无自定义 `rustfmt.toml`/`clippy.toml`，使用默认规则。

## 开发规范（重要）

**`GUIDE.md` 是本项目的权威开发规范**，修改 UI/架构/状态代码前必须先阅读相关章节。核心要点：

1. **先查证再编码**：不要从 React/CSS/Shadcn 或旧版 GPUI 示例类比翻译 API；以 `docs.rs/gpui-kit`、`https://gpui-kit.com/llms-full.txt` 及本地依赖源码（`~/.cargo/registry/src/*/gpui-kit-0.11.0`）为准。
2. **初始化**：`open_window` 已自动包裹 `base::Root`，不要再手动包一层；`init(cx)` 只调用一次。
3. **组件选择**：默认 `RenderOnce` 值组件；跨帧持久状态才用 `Entity<T>` + `Render`；禁止在 `render` 中重建 Entity、mutate 或 `notify()`。
4. **样式**：禁止裸 `px(...)` 和 raw color——用 rem helper（`p_2()`、`gap_3()`、`w_64()`）和 `cx.theme()` 语义色。
5. **标识**：重复元素用稳定 domain ID（`Button::new(("delete", item.id))`）；禁用 random/index/翻译 label 作 ID。
6. **状态**：持久 UI 文本用 `SharedString`；controlled 模式——callback 表达 intent，owner 更新后再 render；父子引用避免强 Entity 环（子指父用 `WeakEntity`）。
7. **架构**：依赖只向下 `app shell → feature → component → gpui-component → gpui-base`；按 feature 组织代码，不建全局 `models/`/`views/` 目录。
8. **优先用语义组件**（`Select`/`DropdownMenu`/`VirtualList`/`Table`），不要用 clickable `div` 重做——会丢失 focus/键盘导航/a11y。

## 提交

- Commit message 采用 Conventional Commits 风格（如 `feat: add ...`），与现有历史一致。
- 提交前通过：`cargo fmt --all` + `cargo clippy --all-targets` + `cargo test`。

## 测试

目前无测试。新增测试按 GUIDE.md §14 分层：pure test → GPUI context test → `VisualTestContext` 交互测试 → app smoke test；测 semantic contract 而非实现细节；可复现 bug 先加 regression test 再修。
