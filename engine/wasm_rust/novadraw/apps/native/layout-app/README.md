# Layout App - 布局管理器验证

## 功能说明

验证 Draw2D 布局管理系统，包括 FlowLayout、BorderLayout、StackLayout、XYLayout、GridLayout 等布局类型。

## 运行方式

```bash
cargo run -p layout-app
```

## 场景说明

| 场景 | 名称 | 验证内容 |
|------|------|----------|
| 0 | XYLayout + Constraints | 绝对定位与显式尺寸约束 |
| 1 | FillLayout | 第一个子元素填充 client area |
| 2 | FlowLayout | 水平流式排列和自动换行 |
| 3 | Nested Layouts | 嵌套布局与约束传播 |
| 4 | Constraint Update | 约束变化后的重新布局 |
| 5 | GridLayout | 三列等宽网格与填充约束 |
| 6 | ToolbarLayout | 单行排列、压缩和 minor-axis stretch |
| 7 | StackLayout | 所有子元素覆盖同一 client area |
| 8 | No Layout | 无布局管理器的对照场景 |
| 9 | BorderLayout | 北、南、东、西、中五区域 |

## 操作说明

- 按数字键 `0`-`9` 切换场景
- 按 `ESC` 退出程序

## 布局类型说明

### FlowLayout

元素按顺序排列，支持水平或垂直方向，超出容器时自动换行。

### BorderLayout

将容器划分为五个区域：

- **North**: 顶部区域
- **South**: 底部区域
- **West**: 左侧区域
- **East**: 右侧区域
- **Center**: 中央区域

### StackLayout

元素堆叠显示，后添加的元素在上层。

### XYLayout

绝对定位，每个元素指定精确的 X/Y 坐标。

### GridLayout

将容器划分为规则的网格，元素按网格单元排列。

## 依赖模块

- `novadraw-scene`: 场景图和 Figure 接口
