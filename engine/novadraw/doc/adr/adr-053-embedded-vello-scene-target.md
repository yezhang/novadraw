# ADR-053：嵌入式 Vello Scene 输出目标

类型：`architecture-decision`

## 状态与范围

2026-10-28，依据用户批准的
[GEF 对等样例方案](../design/editor/gef-official-examples-parity-proposal.md)实施首个前置切片。
本 ADR 细化方案 A 的 backend 出口，不授权改动第三方 UI 框架或 Core 渲染主循环。
完成状态与证据见 [P2 backlog](../roadmap/p2-delta-backlog.md)。

`api_semantics`：`paint.protocol`、`frame.submission`、`resource.lifecycle`、
`coordinate.conversion`、`clipping.strategy`。

## 决策

在具体 Vello backend 中增加 `scene::VelloSceneRenderer` 与 scoped `SceneBackend`。
调用者持有画布局部 Vello Scene；backend 不创建窗口、Device、Queue、renderer 或 timer，
只复用现有 SceneEncoder、资源缓存、session gate 与抗锯齿预处理。
Vello 类型留在具体 backend，Core/Editor 的 RenderBackend 协议保持不变。

Scene 输出坐标为逻辑画布坐标。既有编码器按 surface DPI 生成设备空间描边与抗锯齿，
输出时附加逆 DPI transform，宿主最终按同一 DPI 合成一次。
宿主平移由宿主控制；非 DPI 的缩放/旋转下设备像素 OFF 语义尚未认证，
不能把本切片证据外推为任意宿主 transform 的像素一致性。

目标采用 full frame，不申报 retained partial damage。
显式 clear color 以画布局部背景矩形表达；缺省背景透明。
候选 Scene 构造完成后整体替换目标；失败、Retry 或 Skipped 不替换旧目标。
资源缺失时 Retry，session 更换必须带 snapshot，旧 session 结构化拒绝。
backend cache 可以接纳资源，但目标不能发布半帧。

`Presented` 表示目标 Scene 已发布，不表示整窗 GPU submit、present 或扫描输出成功。
Runtime 可以确认该目标；重复 Widget paint 只合成已发布 Scene，不重复确认 frame。
宿主 device/window 失效后必须重建资源 session，并请求完整快照。
Runtime 与 Scene renderer 一对一，防止不同文档的缓存、字体和 session 混用。

## 外部消费者与验证

外部 integration test 仅通过公开 Runtime/RenderBackend/scene API 构造输入。
覆盖成功替换、partial damage 拒绝、无效 DPI、旧 session、资源缺失 Retry、
空尺寸 Skipped 和目标保持。GPU 比较同一 submission 的 Scene 输出与现有
纹理后端，覆盖 DPI、偏移、裁剪、背景透明/不透明与旧像素擦除。
此证据不证明 Masonry capture、IME、accessibility 或 Xilem rebuild lifecycle。

## 成本

复用既有编码实现，不引入新依赖、内置字体或第二套 lowering。
为原子发布同时保留旧目标与候选 Scene，存在短期编码内存峰值；
调用者不应额外缓存每一帧 Scene。本切片测同配置消费产物的大小与 encode 成本，
完整 UI 的首次加载、RSS、整窗 GPU 成本待 Widget/View 消费者完成后测量。
不声称零拷贝、零分配或完整 UI 性能已达标。
