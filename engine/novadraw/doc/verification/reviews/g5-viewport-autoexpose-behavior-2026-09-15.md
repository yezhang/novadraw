# G5.5 Viewport Feedback 与 Auto-expose 行为验证

类型：`verification`

日期：2026-09-15

结论：`complete`。G5.5 自动契约、headless 产品重放与 Native 人工验证闭合；
G5 检查点 C 于 2026-09-22 通过。

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
- connection create/reconnect 使用透明 range reserve，使点状 endpoint feedback 在
  初始内容范围边缘也能持续推进双轴；
- host 注入 elapsed time，单 step 有上限，RangeModel clamp 后不会空转；
- auto-expose 不修改模型 revision 或 CommandStack，release 只提交一个 Command；
- release/cancel 在宿主重建 unscaled handles 前稳定永久 extent 与 viewport origin；
- Native 事件循环支持 wheel scroll、Command/Control-wheel anchor zoom 和连续
  auto-expose 调度。

## 自动证据

- suite：`g5.5.viewport-autoexpose`
- Editor G3/G4/G5 contract：PASS
- M8 viewport contract：PASS
- Native editor headless replay：PASS
- manifest/document validation：PASS
- workspace full gate（fmt/check/clippy/test）：PASS

## 人工门禁

- `doc/verification/manual/g5-connection-creation.md`：PASS；
- `doc/verification/manual/g5-connection-reconnect.md`：PASS；
- `doc/verification/manual/g5-connection-bendpoint.md`：PASS；
- `doc/verification/manual/g5-viewport-autoexpose.md`：PASS；
- 检查点 C：PASS。

## 2026-09-16 人工复查修订

- 增加真实 viewport 右下角双轴 auto-expose 契约，两个 range 均可滚动时 x/y origin
  必须在同一 step 内变化；
- pointer leave 现在明确取消 Tool、停止调度并清理 feedback；
- Native window resize 在 viewport validation 与 range clamp 后重新计算 active Tool
  feedback，并重投影 unscaled selection/endpoint/bendpoint handles；
- scaled feedback layer 对齐 GEF `FeedbackLayer extends FreeformLayer`：transient bounds
  参与临时 freeform extent，不再被 layer 初始 bounds 截断；
- 人工文档补充各生命周期动作、前置条件和可观察结果。

## 2026-09-22 检查点 C 收口

- bounds feedback 在替代源图元时按 prospective extent 提前 clamp，避免永久 range
  收缩后 Figure 偏离 release surface；
- connection create/reconnect 的透明 range reserve 覆盖 edge threshold 与一个最大
  auto-expose step，右下角可连续同时推进水平和垂直 origin；
- release 与显式 cancel 在返回宿主前稳定 Runtime，随后重建的 selection/endpoint
  handles 与最终 viewport transform 同帧一致；
- Native 人工复验确认 scroll/zoom、bounds、connection、生命周期和 resize 全部通过。
