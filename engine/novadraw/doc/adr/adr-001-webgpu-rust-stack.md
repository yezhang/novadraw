# ADR-001：Rust 与 WebGPU 技术栈

类型：`architecture-decision`

能力归属：Core 技术栈；backend／platform 管理具体渲染与宿主实现。

状态：已接受，2025-01-13；2026-09-10 校准，文本选型以关联设计为准。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

跨 Native、Web 与 Headless 的图形框架需要一致的核心语言和可移植 GPU 路径，
同时允许替换平台宿主与文字服务。

## 决策

采用 Rust；默认 GPU renderer 使用 Vello／WebGPU。Native 默认由 Winit adapter
适配输入与窗口，Core 不依赖 Winit。Parley 是默认可替换文字 adapter；
布局与字体轮廓不受默认 renderer 选型绑定。

## 替代与代价

Rust 所有权与 WebGPU 的跨平台后端适合统一 Core；以单个平台原生图形 API
作为核心合同会把平台限制带入消费者。默认选型不意味着后端唯一或保证各浏览器能力。
代价是 GPU／浏览器能力仍需分别验证，默认第三方库升级需要独立适配。

## 关联设计

[文字布局](../design/architecture/text-layout.md)、
[图文整合](../design/rendering/text-graphics-integration.md)；
包依赖边界见 [ADR-023](adr-023-crate-consolidation-and-extension-boundaries.md)。

原批准范围和修订依据见[原稿 001](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-001-webgpu-rust-stack.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
