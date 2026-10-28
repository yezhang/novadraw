# 设备像素游程抗锯齿实验

类型：`verification`

日期：2026-10-26

结论：现有 Vello Area 能将整数设备像素游程绘成二值覆盖，并保留透明度、
渐变、曲线裁剪和绘制顺序。该结果支持继续评审
[逐图形 AA 方案修订 2](../../design/rendering/shape-antialias-proposal.md)，
不表示 Core、生产 OFF 后端或 ShapeStylesExample 已完成。

## 运行环境与复现

实际依赖为 `vello 0.10.0`、`kurbo 0.13.1`，dev 构建。
适配器为 Apple M1 Pro，Metal，IntegratedGpu；目标为 64×64 RGBA8Unorm，
透明背景，Vello `AaConfig::Area`。没有窗口或 Java 运行步骤。

实验源码：
[shape_aa_feasibility.rs](../../../crates/novadraw-backend-vello/examples/shape_aa_feasibility.rs)。

```sh
cargo run -p novadraw-backend-vello --example shape_aa_feasibility
```

产物为 `target/verification/shape-aa-feasibility/report.json` 和 8 份 `.rgba`
原始图像。每份图像按行排列，宽高均为 64，4 通道，每行 256 字节。
重复执行覆盖同名证据；断言失败即非零退出。

## 实际判据与结果

实验先在包含剪切、非均匀缩放和亚像素平移的圆形轮廓上，按像素中心求 winding，
再把连续内部像素合并成整数矩形路径。正常圆形与游程路径分别通过同一 Vello
Area 配置渲染。二值覆盖判据使用透明背景上的不透明白色；这里没有 clip 或其他
alpha，最终 alpha 可以直接检验几何覆盖。

| 判据 | 实际结果 |
|---|---|
| ON 边缘有部分覆盖 | 156 个 alpha 在 0 与 255 之间的像素 |
| OFF 与中心采样逐像素一致 | 全部 4,096 个像素符合，alpha 仅为 0 或 255 |
| 游程规模 | 圆形 37 条水平游程 |
| OFF 保留半透明 paint | 覆盖像素 alpha 为 128，未覆盖像素为 0 |
| 渐变坐标保持 | 1,190 个共同完全覆盖的内部像素，最大通道差不超过 1 |
| 曲线描边二值覆盖 | 带 Round cap、dash 和非整数变换的轮廓，4,096 个像素符合；54 条游程 |
| 曲线 clip、layer alpha、ON/OFF 交错顺序 | 单路径游程与逐整数像素参考最大通道差为 0 |

渐变检查使用独立 brush transform，比较正常几何和整数游程在共同内部像素的颜色；
描边检查先以既有 Kurbo stroke 展开轮廓再变换。它证明这种轮廓能够被二值绘制，
没有独立裁定 Kurbo CPU 展开与 Vello GPU stroke 的全部边缘差异。

混合画面按顺序绘制半透明蓝色 ON 圆形、曲线 clip/layer 内的红色 OFF、
最后的半透明绿色 ON 矩形。参考画面将游程拆成逐整数像素矩形；
它检验 Vello 层合成是否随几何打包方式改变，不作为独立软件合成器的正确性证明。

当前报告记录参考 mask 的单次 CPU 时间为 14,111 微秒。
这是 dev 构建的逐像素 winding 时间，存在运行波动，不能作为生产预算或优化结论。
实验代码没有进入 Core、Runtime 或生产 backend lowering，不增加默认依赖和资源。

## 未验证范围与后续门禁

未覆盖 EvenOdd、自交边界与精确顶点归属、全部五种 Shape、全部 cap/join/miter、
文字 OFF、Surface DPI/resize/reset、retained-partial、WebGPU runner 与 release 性能。
ShapeStyles 的设备 hairline 和低 miter 仍为独立待评审契约。

生产方案必须采用有预算的扫描线准备、完整缓存身份和资源同步前的整帧检查。
需比较启用/禁用 OFF 的相同 release 产物、Wasm 传输体积、冷/热缓存、连续变换、
高 DPI、复杂路径的 P95 帧耗时及峰值内存。通过此实验不免除这些门禁。
只读上游证据和原有 AA 语义继续由方案正文维护。
