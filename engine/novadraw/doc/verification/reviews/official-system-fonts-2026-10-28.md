# Print、Zoom 与 UML 系统字体采用

类型：`implementation-verification`

日期：2026-10-28

范围：Draw2D 官方 `PrintExample`、`ZoomExample` 与 `UMLClassDiagram` 的 Canvas
字体、字号和宿主资源采用。

## 实现

上游创建 Helvetica 16、Times New Roman bold 24 和 Perpetua 48 三个 SWT Font，
Label 显示对应请求名。Novadraw 原实现为三个请求统一使用 Inter，并直接把 point
数值当作逻辑像素。

Zoom 上游为两个类标题请求 Arial bold 12。原实现保留了 Arial 描述，但没有向显式
字体集合注入 Arial 字节，最终仍由 Inter 回退完成 shaping。

UML 上游使用空字族创建 10 point bold 标题，表示采用宿主默认 UI 字体；普通成员、
连接标签和便笺文字继承 LightweightSystem 的宿主字体。原实现分别固定为 Inter
10/12 logical px，没有表达根字体继承。

本批在官方样例 crate 增加宿主字体解析器。解析器使用 Fontique 的系统集合按字族和
字重查询，把返回的完整字体字节交给 Runtime 的普通字体资源入口。Core 仍使用显式
字体集合，不读取系统目录，也不增加平台字体依赖。请求字族不可用时，样例明确回退
已有 Inter；标签文本继续保留上游请求名。

SWT Font height 使用 point，Canvas 文字使用 96 DPI 逻辑像素，因此 Print 的
16/24/48 point 分别映射为 21.333/32/64，Zoom 的 12 point 映射为 16 逻辑像素。
字重继续由普通 FontDescriptor 提供，系统字体文件只负责可用 face，不建立样例专用
文字路径。

macOS 当前采用 Helvetica、Times New Roman 与 Arial。系统没有 Perpetua，Print
第三项按约定使用 Inter。系统字体不写入仓库或打包产物；Parley 和 Fontique 已是
现有依赖，本批没有增加新的传递依赖。

UML 使用 Fontique `GenericFamily::SystemUi`，macOS 对应 `.SF NS`，Windows 对应
Segoe UI。根 Figure 提供 regular system-ui 12 logical px，成员、连接标签与 Flow
文字继承该值；标题覆盖同一系统字族的 10 point bold。若平台没有 system-ui，
两种字重都明确回退 Inter。

## 验证

`canvas_outputs_fonts_icon_and_rerouted_connection_after_resize` 读取最终 Render IR，
逐项核对三个字体资源身份和 21.333/32/64 的 glyph run 字号，同时保留原始 ICO、
连接与 resize 后恢复断言。macOS 额外断言 Helvetica 和 Times New Roman 必须来自
系统集合。

Zoom 的结构测试使用同一宿主解析结果重新排版 `Table`，核对 Arial 的资源身份和
16 logical px 字号；macOS 额外断言 Arial 必须来自系统集合。

UML 的 Render IR 测试核对 regular 成员和 bold 标题均来自注入的 system-ui 字体，
字号分别为 12 logical px 与 13.333 logical px；同时断言上游 `class_obj` 图像进入
最终命令。既有测试继续覆盖 DogEar 的 5/6 单位折角、90→160 宽度下的便笺换行、
owner-side 标签定位和连接文字重新测量。Native 截图已检查图标、字体、折角与换行。

本批执行结果：

| 验证 | 结果 |
|---|---|
| `cargo test -p draw2d-official-examples --lib printing` | 2 passed，1 个 Native GPU 用例按原约定 ignored |
| Zoom 字体采用定向测试 | 1 passed |
| UML 字体继承与图像命令测试 | 1 passed |
| `cargo test -p draw2d-official-examples --lib -- --test-threads=4` | 99 passed，7 个已有显式 GPU 门禁 ignored |
| `cargo check -p draw2d-official-examples --lib` | 通过 |
| 样例自身严格 Clippy | 通过；允许已有 graph fixture 的 Rust 1.99 `chunks_exact` lint |
| `cargo test -p draw2d-official-examples --test source_contracts` | 2 passed |
| Print / Zoom Native 截图 | 两个入口均成功生成并完成人工检查 |
| `cargo fmt --all -- --check` / `git diff --check` | 通过 |

全依赖严格 Clippy 仍会被本批之外的 Rust 1.99 新 lint 阻断：Core 有两处
`drain_collect` 和一处 `chunks_exact_mut`，共享 graph fixture 有三处
`chunks_exact`。本批没有修改这些文件，也不把定向 Clippy 结果外推为 workspace
门禁通过。

`cargo xtask docs` 还被并行长文本记录中缺失的本地
`target/verification/long-text/scroll-resize-shared-20261028/hot-comparison.json`
阻断。本批新增链接未报错，没有改动该并行任务的证据文件。

## 剩余边界

UMLClassDiagram 的 Canvas 功能已闭合并升级为 `complete`。PrintExample 与
ZoomExample 继续保持 `in_progress`：Windows/Linux 的字族采用、系统回退差异和实际
窗口像素尚未验收；Perpetua 在当前 macOS 环境中没有真实 face。Zoom 的跨显示器
DPI 验收仍保留。打印对话框和打印作业不属于当前 Canvas 对等范围。
