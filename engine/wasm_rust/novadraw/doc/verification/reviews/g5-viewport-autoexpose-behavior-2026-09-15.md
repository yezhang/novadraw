# G5.5 Viewport Feedback 与 Auto-expose 行为验证

类型：`verification`

日期：2026-09-15

结论：`behavior_verified`。G5.5 自动契约与 headless 产品重放闭合；G5 整体仍为
`in_progress`，等待检查点 C 人工验收。

## GEF 对标

实现依据限定为：

- `org.eclipse.gef.AutoexposeHelper`；
- `org.eclipse.gef.editparts.ViewportAutoexposeHelper`；
- `org.eclipse.gef.tools.TargetingTool`；
- `org.eclipse.gef.tools.DragEditPartsTracker`；
- `org.eclipse.gef.tools.AbstractConnectionCreationTool`；
- `org.eclipse.gef.tools.ConnectionEndpointTracker`；
- `org.eclipse.draw2d.Viewport` 与 zoom scroll policy。

保留了边缘带检测、重复 step、随时取消以及 viewport 变化后重新计算 target/feedback 的
核心语义。未引入 SWT timer、全局时钟或 Zest 产品逻辑。

## 已验证行为

- Viewer 根层包含真实 Viewport；scalable layers 位于 viewport 内，unscaled feedback 与
  handles 位于 viewport 外；
- surface 到 model/routing 的转换统一读取 FigureTree 当前 transform；
- move/resize 在 scroll 与 anchor zoom 后保持内容坐标和 grab offset；
- bendpoint 在 zoom 下提交 routing-domain point，不把 surface point写入模型；
- connection create 在 auto-expose 后保持 source 锁定并重建 feedback；
- host 注入 elapsed time，单 step 有上限，RangeModel clamp 后不会空转；
- auto-expose 不修改模型 revision 或 CommandStack，release 只提交一个 Command；
- Native 事件循环支持 wheel scroll、Command/Control-wheel anchor zoom 和连续
  auto-expose 调度。

## 自动证据

- suite：`g5.5.viewport-autoexpose`
- Editor G3/G4/G5 contract：PASS
- M8 viewport contract：PASS
- Native editor headless replay：PASS
- manifest/document validation：PASS
- workspace full gate（fmt/check/clippy/test）：PASS

## 剩余门禁

- 按 `doc/verification/manual/g5-viewport-autoexpose.md` 完成 Native 体验验证；
- 与 G5.2-G5.4 人工用例合并执行检查点 C。

## 2026-09-16 人工复查修订

- 增加真实 viewport 右下角双轴 auto-expose 契约，两个 range 均可滚动时 x/y origin
  必须在同一 step 内变化；
- pointer leave 现在明确取消 Tool、停止调度并清理 feedback；
- Native window resize 在 viewport validation 与 range clamp 后重新计算 active Tool
  feedback，并重投影 unscaled selection/endpoint/bendpoint handles；
- scaled feedback layer 对齐 GEF `FeedbackLayer extends FreeformLayer`：transient bounds
  参与临时 freeform extent，不再被 layer 初始 bounds 截断；
- 人工文档补充各生命周期动作、前置条件和可观察结果。
