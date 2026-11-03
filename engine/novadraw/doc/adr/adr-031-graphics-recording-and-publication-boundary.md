# ADR-031：Graphics 录制与帧出版边界

类型：`architecture-decision`

能力归属：Core Graphics/Runtime，platform 宿主提交。

状态：已接受，2026-10-09。

## 问题

独立 painter 与 Figure 若使用不同绘图语言，或录制器能伪造 Runtime submission，
扩展者会获得不同失败语义，资源因果与帧确认也会被绕过。

## 决策

只保留一套 Graphics 绘图语言及失败合同，内置与外部 painter 同模型。
独立录制可产生拥有数据的结果；只有 Runtime 可以出版包含资源与版本权威的帧。
录制权与出版权分离。

## 替代与代价

不保留第二套公开 Canvas/RenderContext，也不把自由录制的 IR 直接当成已提交帧。
直接收口调用面有迁移成本，但无需永久维护兼容分支与双重绘图协议。

## 关联设计

[录制与出版协议](../design/rendering/graphics-recording-and-publication.md)
定义签名、失败和独立录制；
[文字与图形整合](../design/rendering/text-graphics-integration.md)定义共同绘图能力。
