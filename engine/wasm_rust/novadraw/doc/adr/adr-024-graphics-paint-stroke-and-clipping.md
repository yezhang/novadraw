# ADR-024: Graphics Paint、Stroke 与路径裁剪

类型：`architecture-decision`

## 状态

已接受，2026-10-02 用户批准实施。交付状态见
[P2-G01](../roadmap/p2-delta-backlog.md)。

## 决策

接受 [Graphics 扩展契约](../design/rendering/p2-g01-graphics-extension.md)：

1. Core 使用自有、受检、不可变的 Paint、StrokeStyle、ClipPath 值；
   矢量图元与 GlyphPaint 共用 paint/stroke 协议。
2. 路径裁剪支持 NonZero/EvenOdd，保存调用时 transform，与矩形裁剪共同参与
   push/restore/pop/reset。空路径是空裁剪，不是 no-op。
3. custom dash/offset 使用逻辑长度；可配置 miter 同时进入绘制与实际 visual bounds，
   Runtime 受控 mutation 原子更新新旧 damage。
4. Render IR 根据实际内容要求多项能力；Runtime 发布前与 backend 接收前均预检。
   非法 Graphics 输入使用独立结构化结果，不部分接受 session、资源或像素。
5. 拒绝设备像素 XOR；以 retained feedback Figure 的显示、更新、删除承载交互用途，
   不承诺逐像素反色等价。

公共值从 graphics 领域导出，backend-neutral 签名不暴露第三方类型。
保持六个公开 package、Runtime 单一提交权威和现有递归 traversal。

## 取舍与迁移

统一值替代各图元重复的 stroke 字段，不新增只有单一图元可用的渐变 opcode。
公共 Render IR 构造、Result setter 与 RenderOutcome 穷尽匹配同步迁移；
不保留行为不同的双入口。顺序为 stroke → clip → gradient → 组合及视觉验收。

每个切片必须同时交付 producer、backend consumer、失败契约和外部消费者。
Native/Web 像素验证不能由编译或 lowering 测试替代；未验证项继续保留未完成。

## 关系

- 扩展 [ADR-020](adr-020-engine-value-and-render-contract.md) 的可执行 IR 与受检值原则。
- 保持 [ADR-022](adr-022-third-party-type-and-render-dependency-boundary.md) 的第三方类型边界。
- 不修改 Figure hit-test、child clipping strategy 或文本 shaping 契约。
- `api_semantics` 与验证矩阵以 Graphics 专题为准。
