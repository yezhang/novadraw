# ADR-001: 使用 Rust + WebGPU 实现图形框架

类型：`architecture-decision`

## 状态

已通过

## 背景

项目目标是在 WebGPU 平台上实现一个高性能图形框架，参考 Eclipse Draw2D/GEF 的设计理念。需要选择合适的技术栈来实现跨平台渲染能力。

## 决策

- **渲染后端**: 使用 vello (WebGPU)
- **窗口/事件**: 使用 winit（仅作为技术验证）
- **文本渲染**: 使用 cosmic-text
- **主要语言**: Rust

## 后果

### 正面

- WebGPU 提供高性能 GPU 渲染能力
- Rust 提供内存安全和并发安全
- vello 支持 GPU 加速渲染
- 便于后续扩展到其他平台（Metal, Vulkan, DirectX）

### 负面

- WebGPU 目前浏览器支持有限
- winit 功能有限，可能需要更换

## 参考

- vello: <https://github.com/linebender/vello>
- winit: <https://github.com/rust-windowing/winit>
- cosmic-text: <https://github.com/linebender/cosmic-text>

## 日期

2025-01-13
