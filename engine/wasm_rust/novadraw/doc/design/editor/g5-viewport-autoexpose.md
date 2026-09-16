# G5.5 Viewport Feedback 与 Auto-expose 契约

类型：`normative-design`

状态：`implemented`

适用范围：Editor 拖拽手势在 scroll/zoom 下的 request、target、feedback 与 viewport
auto-expose。

## 1. 坐标真源

平台输入进入 Editor 时只携带 logical surface point。Viewer 必须通过 Runtime 当前有效
transform 将其转换到模型、routing 或 feedback 所需坐标域；Tool、Policy 和应用不得复制
`(surface / scale) + origin` 公式。

- pointer 的 logical surface point 在没有新输入事件时保持不变；
- viewport origin 以 RangeModel 为唯一真源；
- scale 以 ScalablePane 为唯一真源；
- scaled feedback 使用内容 transform，unscaled feedback 使用 logical surface；
- scaled feedback layer 是 FreeformLayer，transient feedback 必须参与临时 extent，不能被
  layer 的初始固定 bounds 裁剪；
- request 中提交给模型的 location/delta 必须由当前 transform 计算；
- transform 不可逆、viewport 已退休或目标坐标非有限时，拒绝本次更新且不产生半反馈。

scroll、zoom 或 window resize 导致 viewport transform 改变后，即使 pointer 没有移动，
活动手势也必须重新计算 request、target、command preview 和 feedback；unscaled handles
也必须在新 transform 提交后重投影。已提交模型与稳定 Figure identity 不因临时 viewport
变化而改变。

## 2. Auto-expose 状态

Auto-expose 是活动 Editor 手势的临时状态，不是 Viewport 的业务状态。首批覆盖：

- node move/resize；
- connection create；
- connection endpoint reconnect；
- connection bendpoint create/move。

检测区域是 viewport logical surface client rectangle 内侧的边缘带。边缘阈值使用
logical surface units，因而不随内容 zoom 改变。指针必须位于 client rectangle 内且位于
收缩后的安全区域外；角区允许同时产生水平和垂直分量。

状态至少保存：

- 活动手势身份；
- 最近一次 logical surface pointer；
- 当前 edge direction；
- 可继续调度提示。

不得保存第二份 viewport origin、scale、ModelId 或活 Runtime 句柄。

## 3. Step 与调度

平台事件循环负责提供单调时间并请求下一次 step；Editor 不读取全局时钟、不创建线程或
全局 timer。每个 step：

1. 重新读取 viewport client rectangle、origin、range 和 scale；
2. 重新检测 pointer 是否仍在有效边缘带；
3. 根据 host 提供的 elapsed time 和 edge direction 计算 surface-domain scroll delta；
4. 通过当前 transform 转换为 Viewport range domain delta，并由 RangeModel 原子 clamp；
5. 若 origin 实际变化，使用同一 surface pointer 重新运行活动 Tool 更新链；
6. 先清理旧 feedback，再安装按新 transform 计算的 feedback；
7. 返回 changed、continue 和下一次调度提示。

elapsed time 必须有受检上限，避免窗口阻塞后单次跳跃过大。无法继续向当前方向滚动时不应
产生空转更新；若另一轴仍可滚动，则保留该轴。

## 4. 手势不变量

- press 时锁定的 source、operation、handle role 和 model revision 不因 scroll/zoom 改变；
- target 必须在每个有效 step 后按当前 surface hit-test 重新解析；
- move/resize 的 grab offset 保持在内容坐标中，不能因 origin 改变发生跳跃；
- connection creation 的 source 保持锁定，pointer/target 端按当前 viewport 重算；
- reconnect 固定 ConnectionPart 与被移动 endpoint；
- bendpoint 固定 connection、operation 和 index；
- auto-expose 不执行模型 Command，不进入 undo/redo history；
- release 前清理 transient feedback，再构造并执行唯一模型 Command；
- Escape、pointer leave、focus loss、tool switch、history transition、source retirement
  和 Viewer drop 必须停止调度并清理状态。

## 5. 与 GEF 的对应与差异

保留 GEF：

- `TargetingTool` 在活动 drag/connection 手势中查找并驱动 `AutoexposeHelper`；
- `ViewportAutoexposeHelper` 只在 viewport 内侧边缘带生效；
- helper 以可重复 step 推进，可由 release/cancel 随时终止；
- viewport 变化后模拟一次 pointer 更新，重新计算 target 与 feedback；
- move drag 保持相对 source 的起点语义。

Novadraw 差异：

- 使用显式 typed step/result，不返回含糊的 helper boolean；
- monotonic time 由 host 注入，Editor 不依赖 SWT event thread 或全局 clock；
- surface/content 转换统一由 Runtime 坐标协议完成；
- RangeModel clamp 后只有实际 origin 变化才重建 feedback；
- 首版只支持 Viewer 根 viewport，不开放嵌套 viewport helper 搜索；跨 nested viewport
  topology 继续遵循 Draw2D Core 的结构化拒绝。

## 6. 验证门禁

至少覆盖：

1. edge-band detect 的内侧、中心、外侧与四个角；
2. threshold 在 50%、100%、200% zoom 下保持相同 logical surface 宽度；
3. elapsed clamp、对角 step、range clamp 与无效方向停止；
4. scroll 后使用固定 surface pointer 重算 content location；
5. zoom 后 create/reconnect/bendpoint feedback 与 target 不漂移；
6. move/resize 在 auto-expose 后保持 grab offset；
7. auto-expose 不改变模型 revision 和 CommandStack；
8. release 只提交一个 Command，undo/redo 与无 scroll 场景一致；
9. cancel、focus loss、history transition、retirement 和 drop 清理调度及 feedback；
10. transform 不可逆与非有限输入在提交前拒绝；
11. Native 连续边缘拖拽可滚动且反馈不闪断；
12. corner step 在两个 range 均可滚动时同时改变水平和垂直 origin；
13. window resize clamp origin 后，unscaled handles 与 active feedback 重投影；
14. 超出原模型 extent 的 scaled feedback 扩展临时 range，边缘滚动后轮廓保持完整；
15. workspace fmt/check/clippy/test。
