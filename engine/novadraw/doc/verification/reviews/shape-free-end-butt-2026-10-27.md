# 矩形分边描边的平齐自由端

类型：`implementation-verification`

状态：实现与最终自动门禁完成；macOS Native 样例及 GPU 像素复核通过。

契约：[ADR-052](../../adr/adr-052-independent-corners-and-edge-borders.md) 的自由端修订。

## 行为

相邻两边都开启时，保留已有接角和颜色分区。相邻边关闭时，开启边在直线切点
采用 Butt 平齐截面；零圆角时切点就是矩形顶点。因此样例中缺底边的矩形左右
描边完整延伸到底部，末端为水平截面，不向 bounds 外延长。

有圆角的自由端不再绕向关闭边，填充轮廓仍保留圆角。此修订不改变 Figure 命中、
固有尺寸、原始 bounds 或顶点。边存在性由解析后的 Paint 是否为 None 决定；
显式透明颜色仍是已配置的边。

规则由 Core 的 RectangleBorderGeometry 实现，RoundedRectangleFigure 和
RectangleShapeBorder 共用；样例自动得到平齐端点，不增加标题容器或覆盖补丁。
四边均开启的路径保持原有分区。regions() 继续返回四边相接时的基础区域；
paint() 根据实际边开关生成并复用自由端区域。

## 证据

- 新增几何测试遍历 16 种边开关组合，逐点检查开启边全线宽覆盖且无重叠。
- 圆角自由端用例检查直线切点截断，以及填充轮廓保留圆角。
- 对象缓存用例检查相同 mask 复用同一 Arc；变化后只保留最近一种 mask。
- 11 组 native GPU 场景通过。DPI 1/1.5/2 的不等半径矩形逐像素检查两侧
  末行的完整线宽，末行 alpha=128，下一行 alpha=0；接角半透明接缝及边关闭
  后的清除、稳定重绘检查同时通过。
- Native 样例已启动、截图、正常退出，并目视检查了左右下端及标题区分隔线。

[完整 Native 样例画面](../evidence/independent-shape-corners/composite-shapes-butt-native.png)
与[GPU 回读图](../evidence/independent-shape-corners/independent-corner-butt-gpu.png)
保留为本次证据。其他平台像素验收不由 macOS 结果外推。

## 成本与边界

几何对象新增一个对象所有的派生模型，以及最近一次 mask 的不可变路径缓存。
使用 Arc 与 Mutex，不使用全局状态；不预建全部 16 种组合。稳定 mask 不重建
路径；旧 Arc 在缓存替换后仍有效。新增数据不进入任何公开的后端类型签名。

release 测量：100 次新对象的自由端路径首次构建 p95 为 0.875 µs；
同一对象 100000 次缓存访问平均 11.92 ns/op。派生模型内联部分 632 bytes，
路径容器 192 bytes；这些数值不包含 Arc 控制块和 Path 自有堆存储，不是整棵树
的内存测量。几何构建为固定四边的 O(1) 工作，不增加渲染热路径日志。

实际 release GPU prepare/submit 计时保留在测试输出，GPU 回读不计入 submit。
该测试每组只含首次、关闭边后和稳定重绘三个样本，不能作为完整帧 p95。
当前分边自由端只提供 Butt 规则，统一描边已有 LineCap 行为保持原语义。

沿用 shape-artifact 消费者的同配置 release 链接测量：

| 产物 | 普通 Figure 消费 | 独立圆角/分边消费 | 差值 |
|---|---:|---:|---:|
| Native dylib | 352096 bytes | 352112 bytes | +16 bytes |
| WASM | 180121 bytes | 182882 bytes | +2761 bytes |
| WASM gzip level 9，mtime=0 | 69635 bytes | 70720 bytes | +1085 bytes |

这是同一代码快照的消费方式差值，不是 Core 修订前后的总体差值；
普通 Figure 路径也已经包含分边实现。Native 两种模式在半径 0/10/30 下均
成功执行，录制三个形状命令。WASM 只计链接和压缩体积，不代表浏览器采证。

## 复现

```bash
cargo xtask verify shape.independent-corners
cargo test -p novadraw --release --lib free_end_path_cost -- --ignored --nocapture
cargo run -p composite-shapes-app -- --screenshot
```

最终 workspace fmt、Clippy（`-D warnings`）和 test 通过：1561 项通过，
17 项按各自条件忽略；所需 native GPU 项已通过 shape.independent-corners
单独执行。该 suite 全部步骤、401 个 Markdown 文档检查，以及 Native/WASM
公开 API 边界检查均通过。
