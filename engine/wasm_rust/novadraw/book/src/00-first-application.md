# 快速开始：构建第一个图形应用

本章先建立一条可执行的开发路径，再解释各步骤背后的原理。完成后，你应该能够：

- 选择 Core 或 Editor；
- 用公开 facade 构造场景；
- 在运行期安全修改图形；
- 知道平台输入和帧提交应接在哪里；
- 按功能找到后续章节和仓库示例。

## 1. 先运行现有应用

在仓库根目录运行最简单的图形和布局示例：

```bash
cargo run -p shape-app
cargo run -p layout-app
```

需要观察滚动缩放或完整编辑闭环时运行：

```bash
cargo run -p scroll-pane-demo
cargo run -p node-editor-demo
```

这些应用分成两层：

- `apps/scenes`：只构造可复用场景；
- `apps/native/*`：只负责窗口入口和场景选择；
- `apps/support`：仓库内部的 Winit/Vello 演示宿主。

产品应用可以采用这套分层，但不应把 `novadraw-apps` 当作稳定产品框架。应用自己的
composition root 应持有平台窗口、`Runtime` 或 `GraphicalViewer`，以及渲染后端。

## 2. 选择 Core 还是 Editor

### 直接使用 Core

以下场景优先使用 `FigureTree + Runtime`：

- 展示流程、拓扑、图表或仪表盘；
- 应用自己管理状态，只需要更新对应图形；
- 交互以按钮、滚动、缩放和自定义 Figure 事件为主；
- 不需要统一的选择、工具、编辑策略和撤销历史。

### 使用 Editor

以下场景优先使用 `GraphicalViewer + EditorDomain`：

- 业务对象需要映射为节点和连接；
- 用户可以选择、移动、调整尺寸或创建连接；
- 每次编辑必须形成业务命令并支持撤销重做；
- 同一模型可能被不同视图重新投影。

Editor 建立在 Core 之上。它不会替代业务模型，也不会把 `FigureId` 变成持久化 ID。

## 3. 依赖与导入

普通应用依赖聚合 crate：

```toml
[dependencies]
novadraw = { path = "../novadraw" }
```

默认构建不选择平台后端。桌面应用显式启用：

```toml
novadraw = { path = "../novadraw", features = ["native-vello"] }
```

网页应用使用 `web-vello`。常规场景代码从 prelude 开始：

```rust
use novadraw::prelude::*;
```

只有使用视口、连接、事件或 Editor 时才引入对应领域模块。不要从
`novadraw::advanced` 开始构建普通应用。

## 4. 构造第一棵图形树

下面是实际公开 API 的最小场景。它创建一个根容器，用 `XYLayout` 放置两个图形，并
保留其中一个 `FigureId` 供运行期更新。

```rust
use std::error::Error;

use novadraw::layout::XYConstraint;
use novadraw::prelude::*;

struct AppScene {
    runtime: Runtime,
    movable: FigureId,
}

fn build_scene() -> Result<AppScene, Box<dyn Error>> {
    let mut tree = FigureTree::new();

    let root = tree.builder().set_contents(Box::new(
        RectangleFigure::new_with_color(
            0.0,
            0.0,
            800.0,
            600.0,
            Color::from_hex("#f4f5f7")?,
        ),
    ));

    tree.builder()
        .set_layout_manager(root, Box::new(XYLayout::new()))?;

    let movable = tree.builder().add_child(
        root,
        Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            160.0,
            96.0,
            Color::from_hex("#2563eb")?,
        )),
    )?;
    tree.builder().set_layout_constraint(
        movable,
        XYConstraint::at_size(80.0, 72.0, 160.0, 96.0),
    )?;

    let label = tree.builder().add_child(
        root,
        Box::new(LabelFigure::new("Novadraw")),
    )?;
    tree.builder().set_layout_constraint(
        label,
        XYConstraint::at_size(80.0, 200.0, 220.0, 40.0),
    )?;

    tree.builder().validate_subtree(root)?;

    Ok(AppScene {
        runtime: Runtime::new(tree),
        movable,
    })
}
```

这段代码体现了三个边界：

1. `RectangleFigure`、`LabelFigure` 和 `XYLayout` 先作为独立值构造；
2. `FigureTreeBuilder` 只负责分配 ID、组装拓扑和设置初始节点状态；
3. 树交给 `Runtime` 后，运行期修改不再经过 Builder。

完整可运行场景见
[`apps/scenes/src/update.rs`](../../apps/scenes/src/update.rs) 和
[`apps/scenes/src/layout.rs`](../../apps/scenes/src/layout.rs)。

## 5. 在运行期修改图形

挂载后的图形通过 Runtime 的短生命周期编辑器修改：

```rust
fn move_right(scene: &mut AppScene) -> Result<(), Box<dyn std::error::Error>> {
    scene.runtime.figure(scene.movable)?.translate(24.0, 0.0)?;
    Ok(())
}
```

常用入口：

| 需求 | 入口 |
|---|---|
| 修改边界、可见性、样式或具体 Figure 内容 | `runtime.figure(id)?` |
| 添加、删除、重排子节点或更换布局器 | `runtime.container(id)?` |
| 滚动视口 | `runtime.viewport(id)?` |
| 修改缩放 | `runtime.scale(id)?` 或 `runtime.zoom(...)` |
| 替换整个场景内容 | `runtime.set_contents(...)` |

这些编辑器只暂借 `&mut Runtime`。它们不会形成第二份状态，也不会允许调用方同时拆借
`FigureTree` 与 `UpdateManager`。一次方法调用会同步维护失效、重绘区域和通知。

不要这样设计应用：

```text
保存 Figure 对象可变引用
-> 绕过 Runtime 修改
-> 再手工请求布局和重绘
```

正确做法是保存 `FigureId`，所有挂载后修改从 Runtime 进入。

## 6. 接入平台输入

平台层只完成两件事：

1. 把物理像素转换为逻辑表面坐标；
2. 把平台事件转换为 `novadraw::event` 中的统一事件。

然后把事件交给 Runtime：

```rust
use novadraw::event::{MouseButton, WheelEvent};

runtime.dispatch_mouse_moved(x, y);
runtime.dispatch_mouse_pressed(x, y, MouseButton::Left);
runtime.dispatch_mouse_released(x, y, MouseButton::Left);
runtime.dispatch_scroll(WheelEvent::new(x, y, dx, dy));
runtime.pointer_exited();
```

不要在平台适配层重复命中测试、滚动目标选择或坐标父链换算。这些规则由 Runtime
统一维护，否则绘制与交互会在嵌套、滚动或缩放后产生偏差。

桌面适配示例见
[`apps/support/src/input.rs`](../../apps/support/src/input.rs)，输入状态机原理见
[第 5 章](05-input-and-interaction.md)。

## 7. 驱动一帧

产品宿主拥有窗口和后端，Runtime 拥有场景事务。二者在帧边界交接
`RenderSubmission`。

下面是概念伪代码，省略了具体窗口和绘制表面创建：

```rust
fn redraw(
    runtime: &mut Runtime,
    backend: &mut impl RenderBackend,
    surface: novadraw::render::SurfaceInfo,
) {
    let Some(submission) =
        runtime.prepare_submission(surface, backend.capabilities())
    else {
        return;
    };

    let session_id = submission.session_id;
    let frame_id = submission.frame_id;
    let outcome = backend.submit(&submission);
    assert!(runtime.complete_submission(session_id, frame_id, outcome));
}
```

宿主必须把后端结果交还 `Runtime::complete_submission`。否则 Runtime 无法确认资源
增量是否已消费，也无法安全开始下一帧。

真实桌面实现见 [`apps/support/src/app.rs`](../../apps/support/src/app.rs)。帧准备状态和
失败恢复见[第 4 章](04-layout-update-and-frame.md)。

## 8. 何时引入业务模型

只使用 Core 时，应用状态与 `FigureId` 的映射由应用自己管理。这适合状态简单、无需
撤销重做的视图。

当编辑行为增多时，改用 Editor：

```rust
use novadraw::editor::{EditorDomain, GraphicalViewer};

let viewer = GraphicalViewer::new(
    model_adapter,
    edit_part_factory,
    Rectangle::new(0.0, 0.0, 800.0, 600.0),
)?;
let domain = EditorDomain::new();
```

这是接口形态示意，具体类型由应用的 `ModelAdapter` 和 `EditPartFactory` 决定。
应用模型保存自己的 `ModelId`；查看器内部管理 `EditPartId` 和 `FigureId`。命令只
修改模型，查看器再把模型变化投影到场景。

完整实现路线见[第 8 章](08-editor-framework.md)，可运行示例见
[`apps/native/node-editor-demo`](../../apps/native/node-editor-demo)。

## 9. 推荐的应用目录

一个产品应用可以按职责组织：

```text
src/
├── main.rs          # composition root：组装平台、后端与应用状态
├── model.rs         # 可持久化业务模型
├── scene.rs         # Core 场景构造，或 EditPartFactory
├── interaction.rs   # 应用级命令、策略和快捷键
├── platform.rs      # Winit/Web 输入与绘制表面适配
└── verification.rs  # 无窗口场景和关键交互回放
```

`main.rs` 应保持薄：创建依赖并连接生命周期，不承载布局、坐标换算或编辑业务规则。

## 10. 开发顺序

构建一个新图形应用时，按以下顺序推进：

1. 先决定 Core 或 Editor；
2. 用静态场景证明 Figure、布局和坐标；
3. 接入 Runtime 更新，确保只保存 ID；
4. 接入平台输入；
5. 再加入视口、连接或编辑工具；
6. 为关键不变量补无窗口契约测试；
7. 最后接入真实后端并做视觉验收。

遇到问题时按症状跳转：

| 症状 | 先读 |
|---|---|
| 图形位置、缩放或点击坐标不一致 | [第 2 章](02-geometry-and-coordinates.md) |
| 子节点顺序、裁剪或绘制异常 | [第 3 章](03-figure-tree-and-rendering.md) |
| 尺寸不对、更新后残影或不出帧 | [第 4 章](04-layout-update-and-frame.md) |
| 拖拽、捕获、焦点或滚轮异常 | [第 5 章](05-input-and-interaction.md) |
| 滚动范围或缩放反馈异常 | [第 6 章](06-layers-and-viewport.md) |
| 连接不更新或路由错误 | [第 7 章](07-connections.md) |
| 模型、选择、撤销或编辑策略异常 | [第 8 章](08-editor-framework.md) |
