# Shape App - 形状展示验证

## 功能说明

验证 Draw2D Figure 系统的各种基础图形渲染能力。

## 运行方式

```bash
cargo run -p shape-app
```

## 场景说明

| 场景 | 名称 | 验证内容 |
|------|------|----------|
| 0 | Rectangle 基本属性 | 矩形的基本渲染、填充色 |
| 1 | Ellipse 椭圆 | 椭圆的渲染、颜色设置 |
| 2 | Line 直线 | 细长矩形的线条效果 |

## 操作说明

- 按数字键 `0`-`2` 切换场景
- 按 `ESC` 退出程序

## 依赖模块

- `novadraw-scene`: 场景图和 Figure 接口
- `novadraw-render`: Vello 渲染后端
- `winit`: 窗口和事件处理
