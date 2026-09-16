# G5.5 Viewport Feedback 与 Auto-expose 人工验证

类型：`manual-verification`

状态：`pending`

入口：

```bash
cargo xtask manual g5.5
```

## A. Scroll 与 Zoom

1. 启动节点编辑器，确认 Viewport 显示浅灰背景和灰色虚线边界。
2. 按住 Command/Control 使用滚轮放大画布。
3. 松开修饰键后使用滚轮滚动画布。
4. 确认鼠标下的模型位置在缩放前后保持稳定。
5. 确认节点、连接和 scaled feedback 同步滚动、缩放，handles 保持固定屏幕尺寸。
6. 确认浅灰背景和虚线边界固定标示 Viewport client rectangle，不随内容滚动。

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

## C. Connection Auto-expose

1. 分别开始 connection create、endpoint reconnect 和 bendpoint move。
2. 将指针保持在 viewport 边缘并等待自动滚动。
3. 确认 source、connection、endpoint 或 bendpoint index 在滚动期间保持锁定。
4. 确认 target 与 feedback 按当前 viewport 重新计算，没有跳变或双重缩放。
5. 完成操作后验证 undo/redo；Escape 取消时不得改变模型。

## D. 生命周期

1. **Escape**：开始节点拖拽并在边缘等待 viewport 已滚动，保持鼠标左键按下并按
   Escape；确认滚动立即停止、feedback 消失，随后释放鼠标也不产生 Command。
2. **Pointer leave**：重新开始边缘拖拽，保持左键按下并把指针移出窗口；确认手势取消、
   feedback 消失，移回窗口后不会自行恢复滚动。
3. **Focus loss**：重新开始边缘拖拽，然后用 Command-Tab 切换应用；确认滚动与 feedback
   清理，切回后没有残留 active gesture。
4. **Range boundary**：放大并滚动到最右下边界，再在右下角保持拖拽至少 2 秒；确认
   viewport 不越界，窗口不持续请求无变化的重绘，释放只产生一个 Command。
5. **Window resize**：选中节点或连接，放大并滚到右下区域；拖动窗口右下角扩大窗口，
   直到 viewport origin 因 range clamp 向左上移动。
6. 在 resize 全程确认内容、selection/endpoint/bendpoint handles 和活动 transient
   feedback 使用同一新 transform 同步移动；handle 屏幕尺寸保持不变，不留旧位置残影。
7. resize 完成后再次执行 B、C，确认 edge band 仍按新窗口 logical surface 边界检测。

## 结果模板

```text
G5.5-A scroll/zoom: PASS / FAIL
G5.5-B bounds auto-expose: PASS / FAIL
G5.5-C connection auto-expose: PASS / FAIL
G5.5-D lifecycle: PASS / FAIL
```
