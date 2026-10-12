# ADR-035：显式领域导入与 Crate Root 收口

类型：`architecture-decision`

## 状态

已接受并实现，修订 1（2026-10-09）。完成证据见
[P2-S02 完成记录](../verification/reviews/p2-s02-explicit-domain-imports-completion-2026-10-09.md)。
目标是关闭
[API 与架构 Review R02](../verification/reviews/api-architecture-review-2026-10-09.md#r02root-pub-use-与-prelude-重复暴露违反-adr-021)。

## 背景

ADR-021 建立了 crate root、prelude 和领域模块三层公开表面。当前实现中：

- root snapshot 有 59 项；
- prelude snapshot 有 51 项，其中 38 项与 root 重复；
- 相同 Figure、Layout、Geometry、Graphics 类型同时存在 root、prelude 和领域模块路径；
- API diff 必须重复展示同一符号，且调用点看不出类型的领域归属。

原草案试图保留 prelude，再让 snapshot 只显示其相对 root 的差集。这只能隐藏重复，
不能消除重复；它还会让 snapshot 不再忠实表达真实公开路径。

现有领域模块已经稳定：`geometry`、`graphics`、`figure`、`layout`、`tree`、
`runtime`、`event`、`connection`、`container`、`host`、`render`、`text` 和
`advanced` 足以构成可学习的 API。库级 prelude 不再提供必要抽象价值。

## 决策

### 1. 删除库级 prelude

删除：

- `novadraw/src/prelude.rs`；
- `novadraw::prelude` 模块；
- README、book、测试、examples 中对 `novadraw::prelude::*` 的使用；
- public API snapshot 的 prelude scope；
- 针对 prelude 的 facade contract。

不提供 deprecated module、空模块、兼容 glob 或迁移 alias。编译错误应直接引导调用者
使用领域模块。

### 2. Crate root 只公开领域模块

`novadraw` crate root 的公共表面只包含 `pub mod`：

```rust,ignore
pub mod advanced;
pub mod animation;
pub mod connection;
pub mod container;
pub mod event;
pub mod figure;
pub mod geometry;
pub mod graphics;
pub mod host;
pub mod layout;
pub mod render;
pub mod runtime;
pub mod text;
pub mod tree;
```

root 不再 `pub use` struct、enum、trait、type、constant、function 或 macro。
crate 内为实现便利保留的 `pub(crate) use` 不属于公共 API，可以继续存在。

该规则比 ADR-021 的“root 高频入口”更严格，并替代 ADR-021 §1.1 与 §1.2。

### 3. 每个公开符号只有一个规范领域路径

| 领域 | 规范路径示例 |
|---|---|
| Geometry | `novadraw::geometry::{Point, Vec2, Rectangle, Dimension, Insets, Affine2D}` |
| Graphics / Color | `novadraw::graphics::{Color, Graphics, PaintContext, Paint, StrokeStyle}` |
| Figure / Style | `novadraw::figure::{Figure, FigureStyle, RectangleFigure, Border, Shape}` |
| Layout | `novadraw::layout::{LayoutManager, XYLayout, GridConstraint}` |
| Tree / Identity | `novadraw::tree::{FigureId, FigureTree, FigureTreeBuilder}` |
| Runtime | `novadraw::runtime::{Runtime, FramePreparation, RuntimeMutationError}` |
| Host | `novadraw::host::PlatformHost` |
| Event | `novadraw::event::{FigureEventHandler, MouseEvent, KeyEvent}` |

内部私有实现模块可以被领域模块重导出。例如 `color` 可以继续是 crate-private，
但 `Color` 的唯一公共路径是 `graphics::Color`；`style` 可以继续是 crate-private，
但 `FigureStyle` 的唯一公共路径是 `figure::FigureStyle`。

领域模块之间不得为了便利互相重复公开无所有权关系的类型。确需跨领域使用时，文档和
签名引用规范路径，不建立第二个 public re-export。

### 4. 显式导入是公共 API 的使用方式

应用和框架扩展代码显式声明依赖：

```rust,ignore
use novadraw::figure::{Figure, FigureStyle, RectangleFigure};
use novadraw::geometry::{Point, Rectangle};
use novadraw::graphics::{Color, Graphics, PaintContext};
use novadraw::layout::{LayoutManager, XYLayout};
use novadraw::runtime::Runtime;
use novadraw::tree::{FigureId, FigureTreeBuilder};
```

导入略长是有意取舍：调用点直接表达领域依赖，IDE 导航、API review 和 breaking diff
都能定位到责任模块。

### 5. 应用可以拥有本地 prelude

本决策禁止的是框架 crate 的全局便利入口，不禁止应用 composition root 聚合依赖。

例如 `examples/support/src/prelude.rs` 可以保留，但必须显式重导出领域路径：

```rust,ignore
pub use novadraw::figure::{Figure, FigureStyle, RectangleFigure};
pub use novadraw::geometry::{Point, Rectangle};
pub use novadraw::runtime::Runtime;
```

应用本地 prelude：

- 只服务该应用或 example 集；
- 不成为 Novadraw 的公共稳定承诺；
- 可以随场景调整；
- 不得重新从 `novadraw::*` glob 导入。

### 6. API snapshot 按领域冻结

[check_public_api_surface.sh](../../scripts/check_public_api_surface.sh) 调整为：

1. 删除 `prelude:prelude/index.html` scope；
2. root scope 只允许 `mod`；
3. 每个领域模块独立冻结 struct、enum、trait、type、constant、function 和 macro；
4. 若 root 出现非 `mod` 项，门禁直接失败；
5. snapshot 忠实记录真实公开路径，不做“相对差集”隐藏。

`verification/public-api/novadraw-symbols.txt` 因此成为：

```text
root mod ...
module.figure ...
module.geometry ...
module.graphics ...
...
```

### 7. 内置与扩展使用同一路径

内置 Figure、Layout、Request 或 Runtime service 不因“常用”而提升到 root。外部扩展
同样通过所属领域模块取得 trait、typed key 和 registration API。

这避免把 root/prelude 变成内置能力目录。内置功能只是对应领域模块中预先实现的类型。

### 8. 直接 breaking

项目仍为 `0.1.0`。本批次不保留：

- `novadraw::prelude::*`；
- `novadraw::Runtime`、`novadraw::Figure`、`novadraw::RectangleFigure` 等 root 路径；
- root 到领域模块的 deprecated alias。

所有 workspace 消费者一次迁移到规范领域路径。

## 错误处理

- 某个 root 类型尚无领域路径：先在其所有者模块建立规范导出，再删除 root re-export；
- 同一类型出现多个候选所有者：在 Design 阶段裁决唯一所有者，不通过重复 re-export
  绕过；
- snapshot 发现 root 非 module 项：门禁失败；
- 文档、example 或测试仍引用 prelude/root type：编译或文档门禁失败，不添加兼容层。

## 外部消费者证据

新增外部 facade consumer，仅使用显式领域路径构建：

1. detached Figure 和 FigureTree；
2. Runtime；
3. 自定义 Figure / LayoutManager；
4. Graphics paint；
5. host-neutral frame preparation。

测试源文件禁止 `use novadraw::*` 和 `use novadraw::prelude::*`。内置与外部类型都从
相同领域模块取得协议。

## 验证

1. source gate：`novadraw/src/lib.rs` 的公共项只有 `pub mod`；
2. source gate：workspace 无 `novadraw::prelude`；
3. compile-fail：`use novadraw::Runtime` 和 `use novadraw::prelude::*` 不可用；
4. compile-pass：外部 consumer 只用显式领域路径；
5. API snapshot：删除 prelude scope，root 仅有 module 行；
6. README、book、examples、benchmarks、tests 和各公开 crate 完成迁移；
7. `cargo xtask docs`、`cargo xtask check --quick`；
8. 因涉及公共跨 crate API，完成边界执行一次 full gate。

## 实施与回滚

按可独立回退的 commit 推进：

1. 补齐并冻结所有规范领域导出；
2. 迁移 Core 外部公开 crate、examples/support 与 examples；
3. 迁移 tests、benchmarks、README 和 book；
4. 删除 `prelude.rs`、`pub mod prelude` 与 root `pub use`；
5. 更新 snapshot 脚本、snapshot 和 facade contract；
6. 写完成记录并关闭 R02。

历史 `verification/evidence/` 中已冻结的源码快照不改写。

迁移 commit 可以逐组回退；删除旧入口必须是最后一个代码 commit。若最终门禁失败，
先回退删除入口的 commit，不恢复新的长期双入口。

## 后果

### 正面

- 每个符号有唯一规范路径；
- root 和 snapshot 不再重复；
- 调用点显式表达领域依赖；
- 新领域能力不会扩大一个中心便利清单；
- 应用仍可自行决定本地导入体验。

### 负面

- 普通示例的 import 行变长；
- 现有 root/prelude 调用点需要一次性迁移；
- ADR-021、README、book 和 facade 测试需要同步修订。

## 关系

- 落实 [R02](../verification/reviews/api-architecture-review-2026-10-09.md#r02root-pub-use-与-prelude-重复暴露违反-adr-021)；
- 替代 [ADR-021](adr-021-public-facade-and-feature-boundary.md) §1.1 和 §1.2；
- 保留 ADR-021 的领域模块、backend feature 和独立 package 边界；
- 为 [ADR-033](adr-033-runtime-subsystem-facade.md) 与
  [ADR-034](adr-034-editor-tool-request-handle-protocol.md) 提供无重复的 API
  snapshot；
- 不改变各领域运行时语义。

## 日期

2026-10-09
