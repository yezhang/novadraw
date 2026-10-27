# 逐角圆角与矩形分边描边实施记录

类型：`implementation-verification`

契约：[ADR-052](../../adr/adr-052-independent-corners-and-edge-borders.md)。
本记录对应 2026-10-27 的共享工作区，包含其他正在进行的长文本与官方样例改动。
状态：Core 实现与自动验证完成；实际平台像素支持范围见下文。

后续关闭邻边时的描边端点行为已由
[平齐自由端修订](shape-free-end-butt-2026-10-27.md) 承接；本页保留首批快照数据。

## 实现

- `figure::ShapeContour` 公开矩形与 simple polygon 不可变轮廓，绘制和命中同源。
- `RectangleCorners` 按左上、右上、右下、左下给出真实二维半径；
  Polygon 按顶点次序，Triangle 按既有三个顶点次序给出圆形半径。
- `RectangleEdges` 按上、右、下、左配置继承、颜色或关闭，第一批共用线宽、
  实线及内侧 ring；角在参数中点处分给相邻边。
- Runtime 使用开放 `FigureComponentUpdate`；无 concrete dispatch 或新 Graphics
  命令。同值无 revision/damage，非法配置保持旧状态；点与半径一起原子替换。
- `RectangleShapeBorder` 通过既有 BoundsDrawing snapshot 录制，旧 RectangleBorder
  struct literal 和旧 corner dimensions 单位保留。
- 独立 `composite-shapes-app` 只用已有 child tree、Figure 和 Label 组合标题区；
  字体显式注册既有 Inter 资源，不新增字体、图片或依赖。

## 定向验证与修复

Core 集成测试 16 项及一个布局缓存单元测试通过，另有一个按需运行的 release
成本测试。覆盖非对称、
椭圆、凹角、反向顶点、超大半径、边区域无重叠、退化和非法输入、外部 Figure、
运行时原子更新、直接 Shape 入口与 Border snapshot。

GPU 测试实际运行十一组：64 逻辑像素在 DPI 1/1.5/2、800 逻辑像素在 DPI 1/2、
220×150 非对称圆形与椭圆圆角在 DPI 1/1.5/2。检查角区内部全部采样中心的
半透明 alpha、关闭底边后的清除，以及稳定重绘和首次重绘的一致性。
修复后的[非对称圆角 GPU 回读图](../evidence/independent-shape-corners/independent-corner-gpu.png)
已保存并目视复核；这张图保留了测试的半透明红/蓝配色与透明背景。

采证发现并修复两处边界问题：

1. 单独 AA 填充相邻边会稀释半透明接缝；改为单次 ring AA clip 和 Off 分区。
   同色四边只填一次 ring。
2. 反向共享边的浮点交点会产生一像素缺口；Off 扫描统一端点方向。新增后端
   共享边 mask 分区测试，既有曲线、孔洞、半开边界及预算测试同时通过。

完整工作区测试首次暴露工具栏方向切换回归：新增 rectangle lifecycle 把拉伸尺寸
写回了源 Figure bounds。已改为只更新派生缓存，typed rectangle update 也保留源
bounds；既有 stretch/orientation/shrink/reset 测试复验通过。
后续复审补齐了按实际布局尺寸保存的 Polygon 派生缓存：绘制和命中复用
同一 Arc，尺寸变化才重建，原始点和固有 bounds 保持不变；单元测试验证了
缓存复用与源几何隔离。“仅设置分边、保留旧圆角尺寸”也使用同源轮廓命中。

macOS Native 样例已启动、截图并退出。第一次启动缺字体，显式注册后成功。
截图为发现共享边缺口之前的中间证据，不能作为修复后完整画面的证据；最终
像素结论来自修复后的十一组原生 GPU 回读。尚未进行用户人工验收，也未运行
Web/Windows/Linux 像素测试。

## 成本

release 轮廓构造每组 100 次样本，两轮 p95：

| 顶点数 | 首轮 µs | 末轮 µs |
|---|---:|---:|
| 4 | 1.583 | 1.458 |
| 32 | 7.959 | 39.875 |
| 128 | 54.792 | 169.458 |

末轮与 Cargo 全量验证并发，存在明显调度扰动；这些样本只作成本量级证据，
不作性能承诺。rounded simple-polygon 预检 O(n²)，输出与存储 O(n)；
稳定输入共享缓存。路径访问计时 0.361 ns/op 仅是无额外构造的 smoke，
其精度不足以作为性能门槛。

release 真实后端测量包含 Core prepare 和 backend submit，明确排除 GPU readback：
最终一轮稳定 Core prepare 31–69 µs；稳定 submit 0.55–1.14 ms，
首个 64px submit 约 4.05 ms 包含初始化。每组只有首次、边更新、稳定三个样本，
不是 p95，也不是完整 GPU 帧延迟或大量节点基准。Off 准备继续受已有 budget
和缓存限制，超预算不绕过结构化失败。

同源码、同 `opt-level=s/lto/codegen-units=1/strip/panic=abort` 的独立 cdylib
消费者测量见 `tests/consumers/shape-artifact`。它比较普通 Figure 与
显式启用独立圆角/分边 builder 的可达产物，**不是 Core 修改前后差值**；
普通 Figure 绘制路径也已经含有新圆角分支。

最终产物复测：

| 产物 | 普通 Figure | 独立圆角/分边 builder | 差值 |
|---|---:|---:|---:|
| Native dylib | 352000 bytes | 352016 bytes | +16 bytes |
| WASM | 177630 bytes | 180390 bytes | +2760 bytes |
| WASM gzip level 9，mtime=0 | 68593 bytes | 69698 bytes | +1105 bytes |

Native 差值包含链接对齐，不能解释为新增能力只占 16 bytes。Web loader、
网络加载时延与平台 GPU runner 不在本探针内。
Native 两种模式在半径 0/10/30 下均成功执行并录制三个形状命令。
WASM 两种模式保留相同的三项 wasm-bindgen 导入，本次只完成编译和体积测量，
未把原始模块的 Node 导入检查当作浏览器执行或 GPU 证据。

## 复现

```bash
cargo xtask verify shape.independent-corners
cargo test -p novadraw --release --test independent_shape_corners contour_cost -- --ignored --nocapture
cargo run -p composite-shapes-app
cargo run -p composite-shapes-app -- --screenshot
```

包体积探针的命令见其 README。最终验收已通过 workspace fmt、Clippy（`-D warnings`）
与 test：1557 项通过、16 项按各自条件忽略；所需 GPU 项已通过对应 suite 单独执行。

已通过 `shape.independent-corners` 的全部执行步骤及 `render.antialias`；
文档检查通过 399 个 Markdown 文件，公开 API 边界 Native/WASM 检查通过。
最后一次 Core 实际 bounds 溢出拒绝、旧圆角尺寸与分边的共同命中路径、
布局缓存复用与源几何隔离用例，以及精确 Clippy 均已通过。
