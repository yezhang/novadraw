# ADR-025：图文协议、独立录制与后端预处理边界

类型：`architecture-decision`

能力归属：Core 图文语义与录制；backend 轮廓预处理／GPU 格式；Runtime 帧出版。

状态：已接受，2026-10-05；沿用原编号 007／031 的有效图文和出版边界。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

调用者需要统一图文 API；默认布局和 GPU 字体实现不应要求消费者协调
两套状态，也不能封死独立轮廓、预处理与自研 renderer。

## 决策

Graphics 是组合借用的统一图文语言，Measure／Paint 按阶段限制权限。
Figure 在布局阶段准备不可变文字结果，paint 不重新 shaping。
布局决定精确字体、glyph 位置与交互几何；轮廓 provider 可独立替换；
布局后的曲线预处理、GPU 格式与设备缓存由 backend 拥有。

可复用 glyph 几何与每次绘制的位置／状态分离，Core 保留中立语义。
独立 CommandRecorder 只产生资源闭合的 owned RecordedDrawing；
Runtime 唯一分配场景 frame／session 并管理出版确认。
图文共用有序 paint、clip、damage 和 submission，失败不部分接受帧。

## 替代与代价

把布局、轮廓和 GPU 资产混进同一文字结果会绑定后端；公开内部 recorder
或另设图文提交入口会产生第二出版权威。代价是字体／资源版本与实际缓存依赖
必须明确，替换轮廓不能假定 Vello 默认 draw_glyphs 自动使用外部 provider。
当前不冻结未知 shader ABI 或实现尚无消费者的片段算法。

## 关联设计

[图文整合](../design/rendering/text-graphics-integration.md)、
[文字布局](../design/architecture/text-layout.md)、
[录制与出版](../design/rendering/graphics-recording-and-publication.md)、
[后端失败](../design/rendering/backend-failure-and-recovery.md)。

原批准范围和修订依据见[原稿 025](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-025-unified-graphics-and-glyph-preparation.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
