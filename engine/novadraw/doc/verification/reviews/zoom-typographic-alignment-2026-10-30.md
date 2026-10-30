# Zoom 图标按文字指标对齐

类型：`verification`

日期：2026-10-30

能力归属：Zoom 样例的 Figure 组合与文字服务配置，没有新增 Core API。

状态：当前实现已按用户确认的基线／大写字母中心方案修改。
上一版 SVG 留白校正已撤回，原始截图继续保留在
[首轮记录](zoom-icon-alignment-2026-10-30.md)中。

## 对齐关系

字段、方法图标的可见底部对齐文字 baseline；类图标的可见中心对齐
同一字体下参考字母 `H` 的可见高度中心。
透明画布不参与锚点定位，但完整图像盒仍参与行高计算。
图标画布保持 16×16，行边距继续上下 2、左右 4，图标与文字 gap 为 4。

三个 SVG 恢复到首轮修改前的字节。可见范围作为独立资源元数据放在
`zoom_typographic_row.rs`，并由密度 4／8／16 的栅格测试验证；
没有按文字位置再次平移 SVG。

每行是外部 `Row` Figure，通过已有 `CONTAINER`／`PREPARATION` capability
测量、绘制文字，并为普通 `ImageFigure` 子节点提交位置。
图像身份、revision、DPI 密度替换继续使用原资源链路。
这是样例组合，不改变默认 Label 的对齐语义，也不创建 Core 内置特权分支。

Row 的根绘制使用 node-local 坐标，子图像位置使用 client-local 坐标；
Runtime 会为子节点应用 Insets。两者的锚点转换到 surface 后应相等，
不能在图像位置中重复加 Insets。
真实窗口的首轮检查发现重复偏移与边距未进入外框测量的问题，均已修复；
新增发布后检查同时约束实际坐标、完整图像盒与文字测量宽度。

## 字体与准备成本

普通 shaping 的 `ink_bounds` 未准备，无法直接读取 `H` 的实际可见高度。
Zoom 宿主因此启用现有 `OutlineTextEngine` 与默认轮廓 provider，
沿用同一 Runtime 字体集合。文字与 `H` 使用同一 resolved descriptor，
通过已有、按字体 revision 失效的有界 styled shaping 缓存复用测量。
字体或字号变更后重新准备，不按固定像素偏移模拟 baseline。

轮廓准备会增加 CPU 工作及缓存，不是无成本的布局调整。
本机 release 采样：首次稳定化 4.545 ms；60 次循环切换成员字号后，
整场景稳定化中位数 3.724 ms、P95 4.115 ms、最大 4.538 ms。
计时不包含 fixture 构造、GPU 绘制和 surface present，
不等于端到端交互延迟，也没有与最初 Label 版本做同配置性能比较。
Debug 下接入既有缓存前后 P95 为 79.770／41.401 ms，日志保留；
这只比较本次实现的缓存接入，不代表对原始样例的性能提升。

本轮未增加依赖、字体或图像资源。没有测发布产物体积增量，
不以源码规模代替产物结论。跨平台字体、系统 DPI 切换和上游像素验收仍保留。

## 验证与画面

八项定向测试通过，覆盖多档字体的锚点关系、字体变更后的实际发布坐标、
完整行外框、原图元数据、图像 density／revision 和原缩放交互。
GPU 回归单独执行通过，覆盖 DPI 1／1.5／2 与缩放范围阶段。
成本采样也已单独执行，普通测试中的两个 ignored 均有本轮执行证据。

默认和最大缩放已在 PID 76505 的实际窗口检查；
[前后对照](../evidence/zoom-typographic-alignment-2026-10-30/comparison.png)以首轮
留白校正后的画面为 before。
[收据与日志](../evidence/zoom-typographic-alignment-2026-10-30/receipt.json)
保留原始截图散列和采样范围。窗口截图使用 debug 二进制，
性能采样使用 release 测试二进制；不宣称截图整体逐像素相同。

Strict Clippy 仍被既有 graph fixture 的三处 Rust 1.99 分块 lint 阻断；
本轮没有弱化检查。Zoom 保持 `in_progress`，未提交 Git。
报告交付前使用技术文档清晰度与好好说话技能复核，
明确坐标空间、计时范围及原证据与当前实现的区别。
