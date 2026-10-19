# 仓库结构与线程边界

本文解释顶层 `client/`、`engine/` 的来源，以及它们与当前 Novadraw 架构的关系。

## 历史来源

2024 年 6 月的目录重构把仓库设想为三部分：

- `client`：图形编辑器 UI；
- `engine`：绘图引擎；
- `backend`：按需实现的后端服务。

当时的 `client/` 只是一个依赖 CanvasKit 的前端脚手架。它在该次重构后没有继续演进，
也不是当前 Rust workspace 的构建目标。

Web Worker 样例在 2024 年 8 月加入旧 C++/Wasm engine，用于验证主线程与绘制线程间的
消息通信。时间顺序表明：`client/engine` 分目录先于 Web Worker 样例，因而这组目录
最初表达的是 UI 与绘图核心的产品职责分离，不是专门为 Web Worker 建立的线程边界。

## 当前有效目录

| 路径 | 状态 | 用途 |
|---|---|---|
| `engine/wasm_rust/novadraw/` | 当前主线 | Novadraw Rust workspace、示例、书籍、规范与验证 |
| `client/` | 历史脚手架 | 不属于当前构建和公共 API |
| `engine/wasm_cpp/` | 历史实验 | 早期 C++、Wasm、WebGPU 与 Worker 验证 |
| `tests/` | 研究材料 | 字体和图形技术实验，不属于产品 workspace |
| `docs/third_party/` | 第三方参考 | Eclipse Draw2D 上游源码快照，用于行为核验 |

代码、Issue 和 Pull Request 默认应以 Rust workspace 为目标。历史目录不能定义现行
架构，也不应被新功能继续依赖。

## 当前逻辑边界

Novadraw 当前采用的是按所有权和替换点划分的多层架构：

```text
应用模型与产品逻辑
        |
        +-- novadraw-editor（可选）
        |
        v
novadraw Core
FigureTree / Runtime / Layout / Input / RenderSubmission
        |
        +-- RenderBackend -> Vello 或其他后端
        |
        +-- PlatformHost  -> Winit / Web / Headless
```

这些边界解决的是依赖方向和状态所有权问题：

- Core 保持平台无关，不持有 DOM、窗口或具体 GPU 类型；
- Editor 通过模型、EditPart、Tool、Request、Policy 和 Command 组织编辑语义；
- PlatformHost 管理窗口或 Canvas、输入、IME、DPI 与帧调度；
- RenderBackend 消费稳定的 `RenderSubmission`；
- 异步资源任务只能把结果提交回 Runtime 边界，不能从其他线程直接修改 FigureTree。

## Web Worker 的定位

Web Worker 是 Web 平台的一种宿主部署方案，不是 Core 与 Editor 的定义边界。

当前 Core 默认由单个 UI/runtime 线程独占；它不要求所有领域对象实现
`Send + Sync`。字体解析、图像解码、网络和 GPU 工作可以在其他线程或 Worker 中
执行，但跨线程通信应使用消息、资源句柄或不可变提交包。

若后续正式支持 Worker 渲染，需要在 Web 平台层明确：

1. DOM 输入、IME 和 accessibility 仍由主线程桥接；
2. `OffscreenCanvas`、资源和提交包的传输协议；
3. Runtime 所在线程及其唯一写入权；
4. surface 丢失、恢复、取消和迟到消息的失败语义；
5. 主线程模式与 Worker 模式共享同一 Core 公共 API。

因此应保留“Core / Platform / Backend 解耦”，但没有必要保留“所有 client 代码在
主线程、所有 engine 代码在 Worker”这一目录或运行时假设。

## 后续整理原则

在进行顶层目录迁移前，应先保证历史实验没有仍被验证脚本引用。最终目标是让仓库
物理结构直接表达当前 Rust workspace，而不是继续扩展历史 `client/engine` 二分。
