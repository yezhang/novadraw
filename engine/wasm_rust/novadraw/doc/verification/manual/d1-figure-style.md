# D1.4a/b FigureStyle 手工验收

类型：`verification`

状态：`approved`

本流程验证 FigureStyle 继承、局部覆盖和平台 cursor effect。

## 1. 启动

```bash
cargo run -p style-app
```

切换到 `Inherited FigureStyle` 场景。

## 2. 绘制与继承

1. 外层区域显示蓝色背景、黑色轮廓和文本，整体为半透明。
2. 左侧子节点继承蓝色背景、黑色前景、字体和 alpha。
3. 右侧子节点覆盖为红色背景、白色前景和不透明 alpha。
4. 两个 sibling 的局部样式互不污染。

## 3. Cursor

1. 光标移动到左侧子节点，平台 cursor 显示 Pointer。
2. 光标移动到右侧子节点，平台 cursor 显示 Crosshair。
3. 光标移出场景内容，平台 cursor 恢复 Default。

## 4. 回归

依次切换原有七个 style 场景，确认填充、透明度、描边宽度、描边颜色、line cap、
line join 和 border 结果无变化。

## 5. 通过条件

- 继承与局部覆盖符合上述规则；
- cursor 跟随当前命中 Figure；
- 场景切换和 resize 后无状态泄漏或残影。

验收后记录：

```text
D1.4a/b: PASS
平台: macOS
失败项: 无
```

## 6. 验收记录

- 日期：2026-09-03
- 平台：macOS
- 结果：PASS
- 失败项：无
