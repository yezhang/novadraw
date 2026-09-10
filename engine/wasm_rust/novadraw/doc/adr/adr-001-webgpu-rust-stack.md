# ADR-001: 使用 Rust + WebGPU 实现图形框架

类型：`architecture-decision`

## 状态

已通过

2026-09-10 校准：文本选型已由 ADR-007 替代。原选型完整保存在
[历史快照](../archive/adr-audit-2026-09-10/original/doc/adr/adr-001-webgpu-rust-stack.md.txt)。

## 背景

项目目标是在 WebGPU 平台上实现一个高性能图形框架，参考 Eclipse Draw2D/GEF 的设计理念。需要选择合适的技术栈来实现跨平台渲染能力。

## 决策

- **渲染后端**: 使用 vello (WebGPU)
- **窗口/事件**: Native adapter 使用 winit；引擎接口不依赖 winit
- **文本布局**: 使用 Parley 默认 adapter，边界遵循 ADR-007
- **文本渲染**: 后端消费 Novadraw Glyph IR，默认由 Vello 绘制
- **主要语言**: Rust

## 后果

### 正面

- WebGPU 提供高性能 GPU 渲染能力
- Rust 提供内存安全和并发安全
- vello 支持 GPU 加速渲染
- 便于后续扩展到其他平台（Metal, Vulkan, DirectX）

### 负面

- WebGPU 目前浏览器支持有限
- 平台能力与浏览器支持需分别验证，winit 不成为引擎公共 API

## 参考

- vello: <https://github.com/linebender/vello>
- winit: <https://github.com/rust-windowing/winit>
- [ADR-007](adr-007-parley-text-layout.md)

## 日期

2025-01-13
