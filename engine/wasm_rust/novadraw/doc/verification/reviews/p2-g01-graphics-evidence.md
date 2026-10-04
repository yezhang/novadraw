# P2-G01 Graphics 实现与像素证据

类型：`verification-record`

日期：2026-10-05

规范：[Graphics 扩展契约](../../design/rendering/p2-g01-graphics-extension.md)、
[ADR-024](../../adr/adr-024-graphics-paint-stroke-and-clipping.md)。
交付状态：[P2 backlog](../../roadmap/p2-delta-backlog.md)。

## 实现边界

Core 的 Paint、StrokeStyle、ClipPath 与多能力预检已贯通 Runtime、Vello、
Canvas2D fallback；Canvas2D 对新增能力显式拒绝。内置 PointList/Connection
使用真实 stroke envelope；未修改递归渲染主循环、shaping 或字体 fallback。

外部消费者位于 `examples/scenes/src/graphics.rs`，仅使用公开 API，
Native 和 Web 通过同一 `ndcanvas/graphics-extension` 场景构建。
反馈使用专用 LayerFigure 的普通 Figure 添加、移动和删除，没有 XOR 合成。

Vello 的 glyph cache 不消费 dash；有轮廓的虚线字形由 backend 私有 Skrifa
轮廓接入路径描边。无轮廓彩色/位图 glyph 保留 Vello 的既有处理，不承诺虚线。

## 验证矩阵

| 入口 | 覆盖 | 结果 |
|---|---|---|
| `core.p2-g01-graphics` | 受检值、状态栈、多能力消融、失败原子性、miter bounds/damage、Vello lowering | 23 项 Core、21 项 Vello 通过 |
| `graphics.p2-g01-visual` | Native GPU 离屏 DPI 1/2，反馈与 miter 连续帧、拒绝提交后的像素 | 通过，含曲线裁剪与 reset/restore 补充断言 |
| `graphics.p2-g01-present` | Native surface 提交、局部 damage 与完整重绘逐像素比较 | 未通过：重绘事件后首帧仍返回 Skipped，保留未完成 |
| `graphics.p2-g01-web-pixels` | WebGPU 画布导出 DPI 1/2，复用 Native 断言 | 最终场景两种 DPI 均通过 |
| workspace full gate | 格式、API/依赖边界、Native/Web 编译、Clippy、workspace 单测/集成/doctest | 各入口通过；Clippy 等价整改后续跑剩余入口 |

首次 full 执行停在 Clippy 的 `manual_is_multiple_of` 告警。将奇数判断改为
`!lengths.len().is_multiple_of(2)` 后，继续执行 `workspace.clippy` 与
`workspace.test`，均通过；没有重复已通过的边界检查。`cargo xtask docs`
及 `git diff --check` 也通过。

像素断言采用固定逻辑坐标与最多 5/255 的通道容差；底图恢复使用整张 RGBA
逐字节相等。覆盖透明渐变的 premultiplied sRGB、全局 alpha 恰好一次、
重复 stop 硬边、glyph 原点与画刷坐标、dash offset 与非均匀缩放、旋转、
EvenOdd 洞、曲线裁剪、图像/文字交集、空 clip、reset/restore、兄弟 Figure
隔离、miter 增大/还原、深浅及重叠背景中的反馈显示/移动/取消。

`submit` 和 `render_for_screenshot` 都接收派生溢出和 glyph skew 非有限的非法帧；
均须返回 InvalidGraphicsInput，上一张纹理须保持不变，原合法帧随后仍可成功。

截图回读修复了行对齐：GPU buffer 按 COPY_BYTES_PER_ROW_ALIGNMENT 填充，
保存 PNG 时剔除 padding。800×600（DPI 1）和 1600×1200（DPI 2）同时覆盖
需要与不需要 padding 的情况。

## 复现与证据归属

验证命令以 `verification/suites.toml` 为唯一入口。

- 离屏 PNG：`target/verification/p2-g01/offscreen/`。
- surface PNG：`target/verification/p2-g01/present/`。
- Web PNG：`target/verification/p2-g01/web-1.png` 与 `web-2.png`。
- 本轮 suite 执行日志：`target/verification/p2-g01/` 下的 `*.log`。
- 18 张已验证 PNG 的校验和：同目录 `pixel-sha256.json`。

Web 先运行 `web.build`，通过该 suite 的 manual 入口启动服务器。
打开 <http://127.0.0.1:4173/?backend=vello&theme=ndcanvas&scene=graphics-extension&capture=png>，
确认 READY 和 Vello WebGPU，通过 Toggle 1x / 2x DPR 切换两种分辨率，
验证页在成功提交的同一帧中导出 PNG，保存在 canvas 的 `data-frame-png` 属性；
`data-captured-frame` 标记该快照帧号。读取该 data URL、解码到上述 Web 路径，
再运行 `graphics.p2-g01-web-pixels`。未开启 capture 参数时不进行 PNG 编码。
不能在呈现后的任意时刻调用 canvas.toDataURL 取代这个步骤：浏览器可能已
清空交换缓冲区，返回透明图像。页面 READY 或编译通过也不代替像素断言。

离屏 suite 不证明局部 retained repair 或窗口呈现；present suite 使用真实 submit，
但不测量 WindowServer 合成延迟。Windows/Linux 原生 runner 与端到端性能采证
仍按项目既定安排后置，未借本次像素结果关闭。

本轮没有给出性能提升结论。虚线 glyph 需要 CPU 轮廓提取，其成本随字形和
路径复杂度变化；本记录不把小型像素样例外推为大文本或复杂路径的性能证据。
