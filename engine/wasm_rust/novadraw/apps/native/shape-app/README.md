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

- `novadraw`: 场景图、Figure 与渲染协议
- `novadraw-backend-vello`: Vello 渲染后端
- `winit`: 窗口和事件处理
