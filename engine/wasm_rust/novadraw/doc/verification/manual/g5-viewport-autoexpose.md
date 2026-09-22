# G5.5 Viewport Feedback 与 Auto-expose 人工验证

类型：`manual-verification`

状态：`passed`

验收日期：2026-09-22

入口：

```bash
cargo xtask manual g5.5
```

## A. Scroll 与 Zoom

1. 启动节点编辑器，确认 Viewport 显示浅灰背景和 2 logical px 灰色虚线边界。该 demo
   的根 Viewport 铺满窗口客户区，因此虚线贴近窗口内缘，resize 时应随窗口边界扩张。
2. 按住 Command/Control 使用滚轮放大画布。
3. 松开修饰键后使用滚轮滚动画布。
4. 确认鼠标下的模型位置在缩放前后保持稳定。
5. 确认节点、连接和 scaled feedback 同步滚动、缩放，handles 保持固定屏幕尺寸。
6. 确认浅灰背景和虚线边界固定标示 Viewport client rectangle，不随内容滚动。
7. 保持节点选中并连续缩放，确认每一帧四角 handles 都与节点边界贴合，不出现需要下一次
   pointer event 或 release 才恢复的错位。

## B. Bounds Auto-expose

1. 选中节点并开始 move 或 resize。
2. 将指针保持在 viewport 内侧边缘带。
3. 确认无额外 pointer move 时 viewport 仍连续滚动，feedback 不闪断。
4. 先放大到内容宽、高都超过窗口，并确保两个滚动轴都未到边界；把指针保持在距右边、
   下边都小于约 18 logical units 的右下角带。
5. 确认一次停留会同时增加水平和垂直 origin；若某一轴已经到达 range 边界，则只允许
   另一轴继续滚动。
6. 把矩形拖到原模型内容范围之外但仍可通过 auto-expose 滚动到的位置；确认高亮轮廓不会
   在旧内容边界处被截断，只有真正位于 viewport 窗口外的像素会被裁剪。
7. 释放后确认只提交一个 Command，undo/redo 恢复同一模型结果。

Viewport 以图元完整的 surface bounds 判断是否完整可见。仅鼠标、节点中心或左上角位于
虚线范围内，不代表整个节点位于 clip 内；超出右侧或下侧的像素应被 Viewport 裁剪。

## C. Connection Auto-expose

每组从新窗口开始。Connection create/reconnect 的点状 feedback 会提供透明的临时 range
reserve，因此在 100% 初始画布也必须能从右下角连续推进两个轴。Bendpoint 若已有路径
范围不足，可先放大到约 200% 并用普通滚轮返回左上。

1. **Connection create**：
   - 按 `C`，单击蓝色节点固定 source；
   - 不按鼠标，将指针移到虚线右边或下边内侧约 5-10 logical px 并保持；
   - 确认 Viewport 连续滚动，蓝色 source 不变，橙色反馈端点保持在当前指针；
   - 移到绿色节点并单击，确认只创建一条连接，undo/redo 正常。
2. **Endpoint reconnect**：
   - 先创建并选中蓝到绿连接；
   - 按住一个黄色 endpoint handle，拖到距右、下虚线边界均约 5-10 logical px 的交叠
     边缘带并保持；
   - 确认两个轴连续同时滚动，固定端、ConnectionPart 和被移动 endpoint 不变，反馈无
     跳跃或双重缩放；
   - 在滚动期间直接释放到空白区，确认 Viewport clamp 后连接端点和黄色 handles 同帧
     对齐，不需要额外移动鼠标触发刷新；
   - 拖到有效节点释放，确认只提交一个 reconnect Command，undo/redo 正常。
3. **Bendpoint move**：
   - 选中连接，按住青色 create handle 创建折点，或按住橙色 move handle；
   - 拖到仍有剩余 range 的边缘带并保持；
   - 确认 connection、operation 和 bendpoint index 不变；
   - 释放后只提交一个 create/move Command，undo/redo 正常。
4. 上述三类操作分别用 `Escape` 取消一次，确认模型与 history 不变，feedback 清除。

## D. 生命周期

1. **Escape**：在 100% zoom 开始节点拖拽并在边缘等待 viewport 已滚动，保持鼠标左键
   按下并按 Escape；确认滚动立即停止、feedback 消失，随后释放鼠标也不产生 Command。
   若 range 只由 transient feedback 临时扩展，取消后 origin 回到拖拽前位置属于正确
   clamp；节点和 handles 必须保持对齐。
2. **Pointer return**：记录拖拽起点；等待 auto-expose 已改变 origin 后，将指针快速移到
   一个明确不同于起点的内部位置并继续按住。确认 feedback 对应该内部位置，而不是回到
   拖拽起点；永久内容 range 先完成 clamp，feedback 随后按稳定 transform 重建，应用
   不崩溃且 handles 不错位。
3. **Pointer leave**：重新开始边缘拖拽，保持左键按下并把指针移出窗口；确认手势取消、
   feedback 消失，移回窗口后不会自行恢复滚动。若临时 range 消失，Viewport 可回到拖拽
   前 origin；下一稳定帧四个 selection handles 必须继续贴合节点四角，不得向左偏移。
4. **Focus loss**：重新开始边缘拖拽，然后用 Command-Tab 切换应用；确认滚动与 feedback
   清理，切回后没有残留 active gesture。取消导致的临时 range 收缩可以使 Viewport
   回到操作前 origin，但 handles 必须与节点对齐。
5. **Range boundary 与提交**：在 100% zoom 把节点拖到右下 edge band，等待 transient
   feedback 推进 Viewport origin，再保持指针位于窗口内释放。确认只产生一个 Command；
   提交后的节点成为永久内容，Viewport 必须停留在可见目标附近，不得跳回左上角。执行
   undo 后永久 extent 消失，此时 origin 被 clamp 回拖拽前位置属于正确行为。
6. **Window resize**：选中节点或连接，放大并滚到右下区域；拖动窗口右下角扩大窗口，
   直到 viewport origin 因 range clamp 向左上移动。
7. 在 resize 全程确认内容、selection/endpoint/bendpoint handles 和活动 transient
   feedback 使用同一新 transform 同步移动；handle 屏幕尺寸保持不变，不留旧位置残影。
8. 确认浅灰背景和虚线边界扩展到新的窗口客户区，证明 Viewport clip rectangle 已更新。
9. resize 完成后再次执行 B、C，确认 edge band 仍按新窗口 logical surface 边界检测。

## 结果模板

```text
G5.5-A scroll/zoom: PASS / FAIL
G5.5-B bounds auto-expose: PASS / FAIL
G5.5-C connection auto-expose: PASS / FAIL
G5.5-D lifecycle: PASS / FAIL
```

验收结果：

```text
G5.5-A scroll/zoom: PASS
G5.5-B bounds auto-expose: PASS
G5.5-C connection auto-expose: PASS
G5.5-D lifecycle: PASS
```

人工验收期间关闭了三类边界问题：pointer leave 后 handles 错位、bounds 提交后 Figure 与
feedback 坐标漂移，以及点状 connection feedback 无法持续扩展双轴 range。最终复验确认
create/reconnect 右下角连续双轴滚动，release/cancel 后 Viewport、连接端点和 handles
同帧稳定。
