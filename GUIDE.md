# GUIDE.md — Coding Agent 开发指引

本文件提炼 GPUI Kit 官方[设计指南](https://gpui-kit.com/zh-CN/docs/design-guides/)与[编码指南](https://gpui-kit.com/zh-CN/docs/coding-guides/)，作为本项目（`skills-manager`，Rust + `gpui-kit = "0.11.0"`）的开发规范。

约定：**必须**表示生命周期、正确性或生态约束；**应该**表示默认架构，偏离需有具体理由；**可以**表示可选方法。精确方法签名以当前依赖源码和 API 文档为准。

## 权威资料（先查证，再编码）

修改前必须阅读与改动最相关的实现、测试与组件文档。**不能**从 React、CSS、Shadcn 或旧 GPUI 示例类比翻译 API；不能因为方法名"听起来合理"就发明 GPUI 方法。

| 资料 | 位置 |
|---|---|
| 全部文档（LLM 友好，单文件） | https://gpui-kit.com/llms-full.txt |
| API 文档 | https://docs.rs/gpui-kit |
| 组件文档 | https://gpui-kit.com/zh-CN/component |
| 本地依赖源码 | `~/.cargo/registry/src/*/gpui-kit-0.11.0`、`gpui-component-0.11.0`、`gpui-base-0.11.0` |

## 一、初始化与窗口

```rust
fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);   // 只调用一次，在创建任何组件 view 之前
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Workspace::new(window, cx))
            })
            .expect("failed to open window");
        });
}
```

- `gpui_kit::open_window` **已自动用 `base::Root` 包裹** view，返回 `(AnyWindowHandle, Entity<V>)`。不要再手动包一层 `Root`——一个窗口只能有一个 `Root`。`Root` 协调 dialog、sheet、notification、tooltip/menu layer、modal focus restore、focus trap 与窗口级文本选择。
- 菜单快捷键：先 `cx.bind_keys`，再 `cx.set_menus`（`set_menus` 在调用时固化 keymap）；keymap 变更后需重调 `set_menus`。

## 二、架构分层

依赖只向下：`app shell → feature crate → app component → gpui-component → gpui-base`。上层承载领域语义与流程协调，下层承载可复用表现与行为；下层不得反向依赖。

- **app shell**：只组合窗口与 Feature，不承载 Feature 逻辑。
- **feature**：同一业务能力的 model（Entity）、service、View、command、dialog 放在一起。小能力先留在 module，有具体收益再拆 crate。**不要**建全局 `models/`、`views/`、`modals/`、`commands/` 目录。
- 同一 Feature 内传递 clone 后的 `Entity<T>` handle；跨 Feature 只暴露有意公开的 Entity 类型、小型 handle、领域 ID、command、event。model 字段和实现 module 保持私有（可用可 clone 的 `XxxHandle` 包一层）。
- `Global` 只用于确实属于整个应用的 service/registry；仅供一个 Feature 使用的 model 不应成为 Global。

## 三、组成单元选择

| 单元 | 用途 |
|---|---|
| `RenderOnce` 值类型 | **默认首选**：轻量、可复用组件值。caller 传当前 props/handler，GPUI 消费后丢弃（如 `Button`、`Checkbox`） |
| `Entity<T>` + `Render` | 跨 frame 的持久状态/独立生命周期：subscription、异步工作、history、子 Entity、增量更新。Entity 存在于 owning view，**不能**在 `render` 里重建 |
| keyed state | 少量 element-local 状态（如 `window.use_keyed_state` 建 `FocusHandle`） |

不要把每个视觉 fragment 都做成 Entity。Element 内部可复杂，但对 caller 仍是值。

## 四、状态所有权

- **持久 UI 文本必须用 `SharedString`**（label、title、placeholder、API 响应中长期持有的字段）。`String` 只用于可变缓冲/构建阶段；`&str` 用于单次调用内临时只读。`SharedString` clone 不复制字节，但非零成本。
- **Controlled 模式**：传入当前值，接收 requested change，由 owner 更新后再 render。callback 表达 intent，不能建第二份隐藏真相。同步外部值时不得再经 user callback 回传（防反馈环）；调用路径必须可重入。
- 状态放在能保持正确性的**最窄 owner**：domain state → model/feature view；transient view state → 绘制它的 view；可复用行为 → component state；应用级 service → Global。
- 改变渲染结果后调 `cx.notify()`（只 notify 最窄 owner，一次逻辑变化 notify 一次）；语义事件用 `cx.emit(...)`；用 `cx.subscribe/observe` 跟随 Entity 生命周期。**禁止在 `render` 中无条件 mutate 或 notify**（会造成永久重绘）。
- **所有权环**：父持子用强 `Entity<T>`，子回指父必须用 `WeakEntity<T>`。callback 捕获自身 View 强 handle 也会闭环。
- **Subscription 生命周期**：保存在需要 callback 的 View/Entity 上（`_subscriptions: Vec<Subscription>`），owner drop 时自动取消。局部变量会过早 drop；`detach()` 会让 callback 累积到被订阅 Entity 释放——默认不 detach。
- View 无法释放时优先排查：强 Entity 环、捕获强 handle 的 callback、detach 的订阅/Task。

## 五、稳定标识 `ElementId`

- row、tab、tree node、重复 control 使用稳定 domain ID；重复出现的 control 用 owning object 做 namespace：`Button::new(("delete-project", project.id))`。
- **禁止**：可重排数据用翻译 label 或 mutable index 作 ID；`render` 中生成 random ID。
- ID 改变 = UI identity 改变，其状态 reset 是有意行为。transition channel、overlay token、scroll handle、persistence ID 同规则。

## 六、渲染与组合

- `render` 保持 declarative：读 state、派生 presentation、组合 element。domain operation/parsing/复杂 mutation 放具名 method 或 service。
- helper 能命名有意义区域时才提取 `render_*`；只有该区域有可复用契约或 retained lifecycle 才提取新 component。builder chain 长不是理由。
- 用 fluent trait（`Sizable`、`Disableable`、`Selectable`）；小条件分支用 `.when(...)` / `.when_some(...)`；不同 interface 用普通 Rust 控制流。
- **先用标准语义组件，再造 custom surface**。不能用 generic `div` 重做 menu/select/dropdown/command palette——会丢失 geometry、focus transfer、键盘导航、selection、disabled、dismissal、a11y 契约。
- 应用提供的 render callback（list item renderer、menu builder 等）**必须无副作用**：可能因 measurement/redraw 被多次调用。

## 七、主题与样式

- **禁止**在应用代码中写 raw hex、`rgb()`/`hsla()`——颜色一律从 `cx.theme()` 按语义角色取（`background`/`foreground`/`muted(_foreground)`/`primary`/`danger`/`warning`/`success`/`info`/`border`…）；缺 role 时补进 product theme。
- **禁止**在 application layout 直接用 `px(...)`——用 rem-based helper：`p_2()`、`gap_3()`、`w_64()`、`text_sm()`、`h_8()`、`size_4()`。例外仅限物理/平台边界、实测 geometry、raster 尺寸、theme 定义本身，且须记录理由。每个裸 `px(...)` 和 raw color 都视为 review finding。
- 样式 precedence：instance style → active semantic state → disabled → runtime interaction refinement。
- 新代码优先用 `Theme::semantic_tokens()`；**注意**：`Theme::apply_semantic_tokens` 不保存 custom spacing/elevation scale，需要自定义的应用须自行持有 token snapshot。
- 修改主题用 `Theme::update(cx, |theme| ...)`（自动同步 `colors`/`tokens`/base projection 并刷新窗口）；`Theme::global_mut` 只改局部字段，容易造成不同步。
- **应用缩放 = theme `font_size`**（`Root` 将其设为 window `rem`）。验证须覆盖多档 base font：wrap、truncate、最小窗口、pane resize、focus-ring、popup placement、virtualized row measurement。不要把 application zoom 与 Dock panel zoom 混淆。
- `overflow_hidden()` 会裁掉向外绘制的 focus ring——优先让布局留空间，不能悄悄消除键盘焦点可见性。
- 嵌套圆角保持同心：`inner radius ≈ max(0, outer − inset)`，取最近主题档位；不要假定父级圆角会自动裁剪子元素背景。

## 八、事件、Action 与焦点

- pointer 专属行为用 pointer callback；需要 key binding/menu/多输入源的 command 用 GPUI `Action`，handler 靠近 command owner。
- **一个逻辑命令只建模一次**：toolbar button、dropdown/context menu item、menu bar、key binding dispatch 同一 Action 或调同一 owner method；label/icon/shortcut/enabled 尽量来自同一 command policy。
- `Link` **只用于**交给浏览器/邮件客户端的外部资源（URL、文档、Email）。应用内导航与命令用 `Button`/`Action`/对应导航组件。
- 焦点规则：keyboard 交互的 Entity 持有 `FocusHandle`；`key_context` 与 `on_action` 附着于同一 focused region；overlay 打开转移 focus、关闭恢复；画 `focus_visible` 态；禁止在 `render` 中无条件 request focus。`track_focus` 的 handle 须 `tab_stop(true)`/`tab_index(n)` 才是 Tab stop。
- Modal 必须 trap focus 并在关闭后恢复；嵌套 overlay 从最上层关闭。
- 只有 nested interaction 确实需要时才 stop propagation。

## 九、异步任务

- `Task` 从 event、lifecycle hook 或具名 method 启动，**不能**作为 `render` 的副作用。需要延长已关闭 view 生命周期的 task capture weak entity。
- 重复任务把 handle 存在 owner 上（owner 释放即取消）；一次性独立任务才 `Task::detach()`。
- 用 idle/loading/loaded/failed 显式状态；refresh 尽量保留旧数据；防重复提交；可恢复错误显示在 UI。
- 昂贵计算走 background executor；异步结果绑定 revision/ID，拒绝 stale result。

## 十、布局、滚动与列表

- **`h_flex`/`v_flex` 不对称**：`h_flex` 让 child 在交叉轴居中，`v_flex` 保持 stretch。row 里放 column（尤其带 header/footer/scroll region 的）要显式 `items_stretch()` 或给 child 加 `h_full()`，否则高 column 会被居中、header 被裁出窗口。
- 可收缩 flex child 用 `min_w_0()`/`min_h_0()`（flex item 默认不放弃基于内容的 min size，`Scrollable` 与 flex 容器之间的普通 `div` 也要处理）。
- 每个 scrollable region 只有一个 owner：`Scrollable` 附着在拥有完整 viewport 的 element 上，内容内边距放 scroll owner **内部**，不要用带 padding 的容器包住 scroll owner（否则滚动条悬在中间）。避免嵌套 scroll。
- 大数据集用 `VirtualList`/`Table`/`DockArea`，不要用多层 `div` 重做。分离 source data、filter/sort、selection、viewport、row rendering；item renderer 只管 row 表现。width/typography/rem/row content 变化要 invalidate measurement。
- Measurement 只用于 popup、virtualization、editor、resize 等依赖 resolved geometry 的行为；prepaint 不能每帧改无关 state。对齐 invariant 靠构造保证（共享 spacing token/inset），不靠事后 `px` 微调。

## 十一、设计规则（写 UI 前必读）

**设计主张**：原生、安静、准确；先清晰后个性；先组合后发明；先 token 后数值；桌面约定优先；状态必须可见；几何表达包含关系；反馈必须真实。

**不要照搬网页习惯**：

| 网页习惯 | GPUI/桌面方式 |
|---|---|
| 按钮手形光标 | 按钮用箭头，仅 link 用手形 |
| 页面跳转主导 | 持久窗口、pane、sidebar、tab、menu |
| 浏览器兜底焦点/滚动 | 明确焦点 owner 与各区域 scroll owner |
| hover 才出现关键操作 | 键盘和指针都可达；hover 图标只能是快捷入口 |
| 每行一排 hover 图标按钮 | 一个可见主操作，次要命令进 `DropdownMenu`/`ContextMenu` |
| 全局大触摸密度 | 默认 medium，专业数据区局部紧凑 |

- **变体语义**：`primary` 只给决策区域真正的默认提交/Enter 动作（不是因为唯一、高频或想被注意）；`danger` = 破坏性提交；`ghost` = 低强调工具栏操作；`outline` = 要低强调但清晰边界；默认 Button = 普通可见操作。Badge/Alert variant 保持 semantic 且稀缺。
- **命令入口**：主要/高频 → 可见按钮/工具栏；区域次要操作 → `DropdownMenu`；作用于指针下对象 → `ContextMenu`（不能是唯一入口）；有键盘形式的 → `Action` + 快捷键。
- **浮层选择**（用最小够用 surface）：tooltip < popover < menu < notification < dialog < alert dialog < sheet。可逆变更优先 undo/notification，严重不可逆才 `AlertDialog`（文案写清对象与后果）。避免 overlay 叠 overlay。
- **选中态**：tab、列表行、导航项的选中由条目自身表面表达（填充/前景/字重），**不用**左侧竖条或单侧边框。带弹出层的按钮在关闭前保持按下/打开外观。
- **间距语义**：同控件部件 `xs`(4)、紧密控件 `sm`(8)、同 group `md`(12)、同 section 异 group `lg`(16)、异 section `xl`(24)、大区块 `xxl`(32)。组件 padding 归组件，组件间 gap 归父容器。
- **密度**：medium 是默认；toolbar/menu/table 用 compact；密度按局部上下文整体变，用组件 `Size` API 而非只改高度。
- **窗口收窄优先级**：保留主要任务 → 到达文档化最小尺寸 → 折叠次要 label/inspector → 低频操作进 menu → 只滚动真正 overflow 的区域。
- **表单**：`Checkbox` 独立选择、`RadioGroup` 少量互斥、`Select` 较长集合、`Switch` 立即生效；进行中禁重复提交并保留输入。

## 十二、界面用词

- 让上下文承担上下文：`Users` 而非 "User Management"；入口/对象用名词，命令用动词，状态用形容词。避免"管理/模块/页面/功能"等包装词。
- 按钮 1–2 词说明结果："删除"而非"确认删除操作"；放弃操作统一"取消"；`OK` 仅用于纯信息确认。确认按钮用最具体的结果词（"删除"、"放弃修改"）。
- 确认对话框：标题写决策（"删除'路线图'？"），正文只补作用范围/后果/恢复方式。
- 英文 UI 用 sentence case；短控件文字不加句号；用单个 `…` 字符（会再要输入或打开对话框的命令末尾加省略号，立即执行的不加）。
- 每个领域对象/命令/状态用固定术语；翻译 key 描述稳定意图（`dialog.delete_project.title`），不拼接翻译片段造句。

## 十三、命名规范

| 概念 | 模式 | 示例 |
|---|---|---|
| 值类型 control | 名词 | `Button`, `Checkbox` |
| Retained 行为 model | `<Control>State` | `InputState`, `TableState` |
| Imperative 共享引用 | `<Control>Handle` | `DialogHandle` |
| 语义通知 | `<Control>Event` | `TableEvent`, `SelectEvent` |
| 键盘命令 (Action) | 动词/intent 名词 | `Confirm`, `SelectNext` |
| 可插拔 owner | `<Role>Delegate` / `<Role>Provider` | `TableDelegate` |
| Caller 提供的表现 | `render_<part>` / `<part>_renderer` | `render_item` |
| Fluent builder（消费返回 `Self`） | 名词/形容词，不加 `set_` | `label`, `disabled`, `placement` |
| `&mut self` 原地修改 | `set_<field>` | `set_items` |
| Boolean reader | `is_<adj>` / `has_<noun>`（不再用 `can_`） | `is_open`, `has_selection` |
| 非布尔 builder/reader 配对 | `with_<field>` / `<field>()` | `with_size` / `size()` |
| Callback | `on_<event/intent>` | `on_click`, `on_open_change` |
| 命名区域渲染 | `render_<region>` | `render_toolbar` |

- 标识符：稳定 identity 后缀 `Id`；零基 index 后缀 `_ix`（新代码，不混 `idx`）；`readonly` 一个词。
- 精确词汇：selected（持久选中）/focused（键盘目标）/hovered（指针）；open-close（overlay）/show-hide（transient）/expand-collapse（结构）；disabled/readonly/loading 不混用；index（当前位置）≠ id（稳定 identity）；value（controlled 数据）/presentation（渲染快照）/state（retained 行为）；placement（策略）/position（resolved geometry）；size（语义档位）/width-height-bounds（geometry）。
- 外层名字承担上下文：`with_item_ix(ix)` 而非 `with_item_ix(item_ix)`。避免 `data`/`manager`/`update_ui`/`process` 等模糊名。
- 含公开字段的 `pub struct` 标 `#[non_exhaustive]` 并提供构造方法；内部重组保持 public module path（re-export seam）。

## 十四、测试与验证

层级（用能证明行为的最低层）：
1. pure test：state transition、geometry、parsing；
2. GPUI context test：entity、event、subscription；
3. `VisualTestContext` interaction test：focus、keyboard、pointer、layout；
4. example/app smoke test：完整 workflow。

- 测 semantic contract 而非实现细节：pointer/keyboard activation、controlled change、disabled、focus movement、event 顺序、stable ID、empty/failure 态。可复现 bug 先加 regression test 再修。
- 每次改 state 后重读 accessibility tree（element index 只是 snapshot）。截图只验证 semantic tree 表达不了的视觉事实。

常用命令：

```bash
cargo fmt --all
cargo clippy --all-targets
cargo test
cargo run
```

## 十五、提交前检查表

**实现检查**：
- [ ] state 与 side-effect ownership 明确；`RenderOnce`/`Entity<T>` 是有意选择
- [ ] 重复元素用稳定 domain ID；无 `render` 内 mutate/notify/重建 Entity
- [ ] theme token 与组件 `Size` 替代了孤立视觉字面量（无裸 `px()`/raw color，或例外已记录）
- [ ] keyboard Action、focus、disabled、overlay 协同工作；focus ring 未被裁剪
- [ ] loading/empty/error/cancellation 路径存在；异步结果防 stale
- [ ] 长数据用 virtualized 组件；scroll owner 单一且滚动条贴边
- [ ] public API 保持依赖方向与封装；命名符合上表
- [ ] 测试在适当层证明行为；`fmt` + `clippy` + 相关测试通过

**设计评审**：
- [ ] 主要任务一眼可辨，主要操作/下一步清楚
- [ ] 文案、控件、状态、范围、反馈、结果描述同一件事
- [ ] primary/强色/Badge/Alert 保持稀缺；层级靠结构与间距而非堆装饰
- [ ] 同级内容共享对齐轴；相等间距精确到渲染像素（多窗口尺寸/zoom/dpi 验证过）
- [ ] 键盘全路径可达；focus 可见且 overlay 关闭后恢复；仅图标按钮有无障碍名称
- [ ] 空/加载/失败/无权限/长翻译/最小窗口/reduced motion 都验证过
- [ ] 在真实窗口中用代表性内容亲手完成任务，而非只看截图

## 十六、常见失败模式（禁止）

- 一个 Entity 存整个应用互不相关的 state；`Global` 滥用
- 长 `render` 混入 business logic / 网络请求 / task 启动
- 可重排内容用 random/index `ElementId`
- literal color/radius/px 破坏主题与缩放
- 已有语义组件仍用 clickable `div` 重做 focus/keyboard/disabled/a11y
- 局部 state 与 controlled model 漂移成两份真相
- `render` mutation 引发 `cx.notify()` 循环
- 无 owner 的嵌套 scroll；滚动条悬在内容中间
- 为一次性页面新增 component variant；满屏多色 Badge
- 可逆低风险操作弹 `AlertDialog`；关键命令只藏在 hover/context menu 里
- `Link` 样式执行应用内命令；`primary` 按钮滥用
- 测试只调 internal method，从不执行 pointer/keyboard 行为
