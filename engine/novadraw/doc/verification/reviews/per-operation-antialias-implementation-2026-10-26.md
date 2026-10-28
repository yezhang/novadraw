# 逐操作抗锯齿实施记录

类型：`verification`

日期：2026-10-26

依据：[ADR-047](../../adr/adr-047-per-operation-antialias.md)与用户批准的
[方案修订 2](../../design/rendering/shape-antialias-proposal.md)。
当前执行状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## AA-1 Core 契约

已接入 `graphics::AntialiasMode`、独立图形/文字提示、状态栈恢复与 draw payload
冻结。预备文字轮廓降为普通 path 时保留文字提示，SceneView 回放保留冻结值。
非 Default 提示需要 `RenderCapability::PerOperationAntialias`；
Runtime 和外部 backend 使用同一准入检查，不支持时返回结构化错误。

新增外部消费者 `per_operation_antialias_contract` 的 4 项测试通过：
提示冻结与状态恢复、glyph/path 文字独立性、Runtime/外部 backend 的相同拒绝、
源捕获和目标 SceneView 回放。已有 `p2_graphics_contract` 23 项、
`p2_text_graphics_contract` 11 项、`p2_scene_view` 11 项均通过。

Core Native/Wasm `--lib` check 与 Native `--lib` 严格 Clippy 通过；
workspace check 通过。Core `--lib --no-run` 编译通过，其已有测试目标仍有
dead_code 与未使用 SubmissionAcknowledgement 警告，本记录不宣称测试目标零警告。
公共符号快照更新与最终 workspace 门禁随完整增量退出执行。

生产 Vello 此阶段尚不声明逐操作 AA 能力，也未接入 OFF lowering。
这些 Core 结果不代表 AA-2–AA-4 或 ShapeStyles 完成。

## AA-2 算法准备

`antialias.rs` 已加入有界活动边扫描线。曲线按极值范围拆为单调段，
在设备像素中心求交；使用半开边界，保留屏幕外 x 交点的 winding，
并按填充语义隐式闭合开放轮廓。每次增加扫描行、活动交点、游程和输出元素前
检查预算，容器扩容使用可失败预留。

`antialias_stroke.rs` 已接入后端的有界逻辑描边准备。
Kurbo 的 `StrokeCtx` 不提供输出容量控制；其公开 `offset_cubic` 的误差细分
虽然有深度上限，但 cusp 分裂分支未受同一深度检查保护，不能据此承诺完整
输出上界。实现使用位置及法向投影误差约束的曲线细分，组装描边两侧轮廓，
在逻辑域处理 dash、cap、join，再由采样器处理设备域轮廓。
这仍采用 ADR-047 的整数游程路线，不增加依赖 fork 或 shader。

定向命令 `cargo test -p novadraw-backend-vello --lib antialias` 通过 6 项测试：
变换曲线对独立 winding、EvenOdd/自交/屏幕外边、半开边界/隐式闭合、
输出预算拒绝、cap/join/闭合虚线对 Kurbo 参考、圆形普通/虚线描边对参考。
早期矩形并集原型曾改变拐角内侧 winding，已改为两侧轮廓组装；
该发现说明不能用简单几何并集替代现有描边语义。
这组早期测试不作为生产能力完成证据。

后续已接入整帧 `antialias_frame` 准备与完整 key 校验缓存，Native、Web 共用编码器
以及 Texture 后端均在资源同步之前准备全部 OFF 命令。每条命令和整帧共享剩余
预算，缓存命中仍计入预算。成功准备的结果才在编码时发布；拒绝时不修改资源缓存
或目标纹理。缓存容量为零时禁用存储；每帧只保留容量内的已完成项。
自定义虚线数组在复制缓存身份前计入元素预算，避免长输入绕过准备上限。

字形使用 Skrifa 的显式 scratch buffer 与有界 OutlinePen。字体缺失使用现有
Retry；OFF 所需的有效矢量轮廓缺失时通过带命令索引的
`AntialiasOutlineUnavailable` 拒绝，不降级成抗锯齿字形。
Core 原有逻辑宽度 0 不绘制语义不变。

扩展后的同一定向命令通过 11 项测试，新增证据包括：

- 非均匀变换与 DPI 1/1.5/2 下的普通填充和虚线曲线描边，逐中心对照独立 Kurbo 参考。
- Msaa16 生产路径的二值覆盖、paint alpha、渐变、曲线 clip、绘制顺序，
  以及 TextureBackend 预算拒绝后目标像素不变。
- Default 与 On 像素一致，且与原有普通 lowering 的基线一致。
- 文字 OFF 矢量轮廓、文字/图形提示独立、虚线字形描边、字体缺失与 scratch 超限。
- 完整几何缓存命中、命中时的整帧超预算拒绝、DPI/采样域/session 变化与缓存关闭。
- 闭合虚线间隙恰好结束于接缝时，保留开放 cap，不能误作闭合描边。

Web 后端 `--lib --target wasm32-unknown-unknown --features web` check 已通过；
这不代表浏览器 WebGPU 像素或窗口呈现验收。
预算初值、固定精度、缓存峰值与最终产物成本仍待代表性 release 测量。

## AA-3 与 AA-4 自动验证

后端定向测试扩展到 13 项通过、1 项成本测试按需执行。新增退化曲线、
零切向量及反向路径验证；控制点凸包已坍缩的曲线按固定精度终止细分。
描边细分限制位置误差与半径加权的法向投影误差，不能据此宣称整条解析描边、
dash 弧长和端帽均具有同一个严格 Hausdorff 误差界。

生产 Msaa16 路径覆盖 Rectangle、Ellipse、Line、Polyline 和 Path，
验证状态恢复、文字独立性、局部编码 clip 的全局像素相位、DPI、
Retry/session reset 与擦除恢复。局部编码 clip 验证不等同于原生窗口 retained
target 的呈现验收。

ShapeStyles 使用外部 Figure 和公开 Graphics 接口，支持原始 8 个参数轴、
5 种图形、Y 轴覆盖 X 轴及最终线宽决定的虚线模式。真实 egui 控制面板可重建矩阵，
标题沿用固定官方序号 `[14/25]`。宽度 0 和 miter < 1 单元明确显示不支持。
40 个 CPU 矩阵场景通过；同一 Runtime/TextureRenderer 连续绘制的 40 个 GPU
场景通过，并检查缩小矩阵后旧区域恢复为背景。产物位于
`target/verification/per-operation-antialias/matrix/`。

RoundedRectangle/Triangle 等外部样例几何仍有近似，不能以这些验证关闭完整上游
对等；ShapeStyles 保留 `in_progress`。原生窗口呈现、浏览器 WebGPU 像素与
Windows/Linux runner 尚未验收。

## Release 成本

稳定入口为 `render.antialias` 与 `render.antialias-cost`。
成本测试使用 Apple M1 Pro / Metal、rustc 1.94.1，release 默认配置；
1024×1024 目标、25 个不同单元，预热后采样 60 帧。准备耗时包括编码；
帧耗时包括等待 GPU 完成，但不含窗口 present 或输入响应。以下为最后一次独立运行：

| 场景 | 冷准备/编码 µs | 热准备/编码 P95 µs | 帧等待 P95 µs | 输出元素 | 编码流字节 |
|---|---:|---:|---:|---:|---:|
| Default | 87 | 42 | 4,328 | 0 | 2,196 |
| 25 单元 OFF 填充 | 1,195 | 112 | 3,611 | 4,125 | 37,796 |
| 虚线曲线 OFF | 14,216 | 207 | 6,081 | 9,125 | 81,796 |
| 25 单元持续变换、DPI 2 | 16,292 | 21,864 | 25,010 | 18,625 | 165,446 |

编码流字节是 Vello CPU encoding 的大小，不能当作驱动实际上传量。
三组 OFF 缓存 payload 分别为 456,800、904,800、1,800,800 字节；
旧新两代同时存活的 payload 上界分别为 913,600、1,809,600、3,601,600 字节，
不含容器容量和 GPU 分配。前一轮独立测试的进程 RSS 为 92,520,448 字节，peak memory footprint
为 247,941,312 字节，均包含 GPU 和整个测试进程，不能归因为 AA 独占内存。

此前两次持续变换的帧等待 P95 分别为 19,696 和 28,889 µs；
最后一轮为 25,010 µs，均超过 60 fps 的 16.7 ms 帧预算。当前消费场景未制定统一延迟预算，
因此本记录只确认成本已测量，不宣称高 DPI 连续更新满足 60 fps。
默认路径与重复静态几何成本较低，持续变换仍需按消费者预算选择或优化。
原始数据见 `target/verification/per-operation-antialias/cost.json`、
`final-cost.log`、`standalone-cost.log` 和 `release-cost.log`。

产物基线为 `32f80ba4`，两侧使用相同 Cargo.lock、截图资源及 release 默认配置。
Native 使用同一 `aa_artifact_probe` 消费者，Web 使用 `web-validation`；
gzip 固定 mtime=0：

| 产物 | 原始基线 → 当前（字节） | gzip 基线 → 当前（字节） | 原始增量 | gzip 增量 |
|---|---:|---:|---:|---:|
| Native | 11,132,080 → 11,276,608 | 4,222,363 → 4,274,225 | 144,528 | 51,862 |
| Wasm | 52,306,993 → 52,463,191 | 28,210,579 → 28,242,264 | 156,198 | 31,685 |

无新增依赖、shader 或字体资源；AA 属于默认 Vello 后端，当前没有独立按需加载模块。
上述数字是这两个消费者的产物差额，不是任意宿主的统一增量；没有测量网络下载、
浏览器初始化或窗口输入延迟。hash 和配置见
`target/verification/per-operation-antialias/artifacts/sizes.json`。

## 退出边界

AA-1–AA-4 的既定自动验证切片已实现。最终架构/API 快照、workspace fmt 与
严格 Clippy、文档检查和三本书 HTML/PDF 构建通过；workspace 测试通过
1,501 项，8 项按既有策略忽略。预算补充后的 13 项后端测试另行通过；
GPU 矩阵和 release 成本由上述显式入口执行。

最终中文复核保留技术术语、数字及证据范围；半角标点、元话语和破折号机械扫描
命中 0，`git diff --check` 通过。发布准备的平台验收保持未完成；
Hairline、低 miter 与混合字体装饰保持独立，不随本记录晋级。
