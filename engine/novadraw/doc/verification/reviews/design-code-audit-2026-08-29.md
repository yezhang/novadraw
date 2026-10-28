# 文档与源码一致性审计（2026-08-29）

类型：`verification`

## 范围

- Draw2D/GEF：`/Users/bytedance/Documents/code/GitHub/gef-classic`
- Draw2D/GEF 基线：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`
- Novadraw：当前工作树
- 文档：`doc/reference`、`doc/parity`、`doc/design`
- 派生知识库：`.trae/deepwiki` 中引用的文档与源码路径

## Draw2D 复核结论

本轮重新核对了容易产生架构误读的核心路径：

| 主题 | 源码证据 | 结论 |
|---|---|---|
| Figure paint | `Figure.paint/paintClientArea/paintChildren` | `paint()` 非 final；Figure 自己编排 self、children、border |
| 坐标域 | `Figure.translateToParent/translateFromParent/useLocalCoordinates` | bounds 属于父域；local 模式改变提供给 children 的域 |
| Validation | `Figure.revalidate/validate` | revalidate 向 validation root 传播；validate 先 layout 后递归 children |
| Damage | `DeferredUpdateManager.performUpdate/repairDamage` | Validation 先于 Damage Repair；dirty rect 沿祖先转换、裁剪并 union |
| 事件点 | `MouseEvent` constructor | source 在构造时执行 `translateToRelative` |
| 交互状态 | `EventDispatcher`、`SWTEventDispatcher` | 抽象类定义契约；默认实现持有 focus、hover、capture 等状态 |
| Scroll | `Viewport`、`RangeModel`、`ViewportLayout` | Viewport 的 view location 来自 RangeModel |
| Zoom | `ScalableFigure`、`ScalableLayeredPane`、`zoom.AbstractZoomManager` | scale 属于 scalable Figure；manager 协调 scale、validate 和 view location |
| GEF helper | `ViewportMouseWheelHelper`、`AutoexposeHelper` | 两者属于 GEF 交互层，不是 Draw2D Figure 核心 |

未发现 `doc/reference/` 在上述核心语义上仍与基线源码冲突。详细纠错历史见
[`../reference/draw2d-source-audit.md`](../reference/draw2d-source-audit.md)。

## Novadraw 复核结论

### 已修复：手势 fallback 被实现成通用祖先冒泡

设计契约规定：

```text
fixed target callback -> if unhandled -> nearest typed container fallback
```

原实现会循环调用 target 的每一级祖先，导致普通中间 Figure 隐式获得 Wheel/Zoom
处理权。这与 Novadraw 的“普通输入无通用冒泡”原则冲突，也把类型选择逻辑泄漏到
分发器。

本次先在
[`../../design/input/scroll-zoom-gesture-contract.md`](../../design/input/scroll-zoom-gesture-contract.md)
明确 target 与 fallback 的边界，再调整实现：

- `BasicEventDispatcher` 只向固定 target 回调一次。
- `DispatchContext` 提供 `apply_scroll_fallback` 和 `apply_zoom_fallback`。
- `SceneDispatchContext` 负责查找最近的 ScrollPane 或 Scalable pane。
- 新增测试锁定“单次 target 回调 + 专用 fallback”。

### 已修复：架构总览包含过期 milestone 状态

`architecture/overview.md` 曾把 M5-M8 描述为待实现，但对应约束、状态机、通知和
Viewport/Scroll/Zoom 已有契约测试。合理方案不是回退代码，而是：

- 总览只记录结构一致性；
- milestone 状态统一引用 `roadmap/00-index.md`；
- M9/M10 和候选 DisplayList 明确保持目标态，不因文档存在而提前实现。

### 已记录：组合根事务编排尚未完全下沉

`SceneDispatchContext`、坐标适配和容器 fallback 已位于引擎层，但
`dispatch -> PendingMutation flush -> update scheduling` 的公共事务外壳仍部分存在于
`novadraw-apps` 和 editor。

目标设计仍合理，但该调整会改变组合根 API 和多个应用入口，属于独立架构增量，
不应夹带在文档迁移中实施。总览已将其标记为 `open structural delta`，后续实现
不得继续复制应用层事务逻辑。

### 已同步：派生 Wiki 路径

`.trae/deepwiki` 不是设计 SSOT，但它受版本控制并承担浏览入口职责。本次同步了
旧 `doc/01-*` 分类路径和目录重组前的 `scene/update/event` 源码路径；迭代渲染入口
改为指向归档决策，避免把已删除 POC 描述成当前实现。

## 有意保留的差异

- Novadraw 使用 `BlockId + SlotMap`，不是 Draw2D 对象引用树。
- 交互状态归 `FigureGraph`，不是无状态的 `BasicEventDispatcher`。
- RangeModel 使用 `f64` 和 typed handle 事务入口。
- Scroll/Zoom fallback 位于引擎层，是吸收 GEF helper 行为后的合理变体。
- 二进制 DisplayList 文档是候选协议，不表示当前代码已经实现。
- M9/M10 文档定义目标契约，不表示 milestone 已完成。

## 验证

- `cargo fmt --check`
- `cargo check`
- `cargo clippy -- -D warnings`
- `cargo test`
- Markdown 相对链接解析
- 派生 Wiki 仓库路径存在性检查
- Draw2D 核心 class/method 定向源码抽查

严格 Clippy 的存量测试代码告警与本次文档/行为一致性修改无关，单独记录在评审
报告中，不作为本次契约修复的失败条件。
