# DisplayList 中间层实现计划

类型：`normative-design`

> **方案文档，不是 Draw2D 源码说明，也不是当前实现清单。**
> Draw2D 本身没有本文所设计的二进制 DisplayList 协议。下文中的组件映射仅用于
> 说明职责类比；当前渲染主线与归档边界以
> [`../../archive/render-iterative-poc.md`](../../archive/render-iterative-poc.md)、
> [`../../verification/reference/draw2d-source-audit.md`](../../verification/reference/draw2d-source-audit.md)
> 和代码为准。

## crate 设计决策

### 方案 A：独立 crate `novadraw-displaylist`

**优势**：
- 可独立发布到 crates.io
- 二进制协议定义与渲染逻辑完全解耦
- 其他项目可独立依赖使用
- 版本管理更灵活（可独立升级协议版本）

**劣势**：
- 增加项目复杂度（workspace 管理）
- 依赖共享问题（需复制或共享 novadraw-geometry 类型）
- 发布流程更复杂

**依赖关系**：

```text
novadraw-displaylist (独立发布)
├── bytemuck
├── hashbrown
└── novadraw-geometry

novadraw-render (可选依赖)
├── novadraw-displaylist
└── novadraw-scene
```

## 框架总览图

draw2d 框架与 Novadraw 的组件映射关系：

```text
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                              draw2d 框架架构                                         │
├─────────────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    │
│  │   SWT GC     │───▶│ Lightweight  │───▶│   Update     │───▶│  Figure Tree  │    │
│  │ (渲染目标)   │    │   System     │    │   Manager    │    │  (渲染层)    │    │
│  └──────────────┘    └──────────────┘    └──────────────┘    └──────────────┘    │
│                            │                                              │         │
│                      事件路由                                          绘制遍历     │
│                      EventDispatcher                                    paint()      │
└─────────────────────────────────────────────────────────────────────────────────────┘
                                          ↓
                        ┌───────────────────────────────────────────┐
                        │            draw2d Figure 系统              │
                        ├───────────────────────────────────────────┤
                        │  IFigure (接口)                           │
                        │      │                                    │
                        │      └── Figure (基类实现)                 │
                        │              ├── Shape (描边/填充)        │
                        │              │     ├── RectangleFigure     │
                        │              │     ├── Ellipse             │
                        │              │     └── Polygon/Polyline    │
                        │              └── 容器 Figure               │
                        │                    ├── FlowPage            │
                        │                    ├── ScrollPane          │
                        │                    └── FreeformLayer     │
                        └───────────────────────────────────────────┘
                                          ↓
                        ┌───────────────────────────────────────────┐
                        │          draw2d 布局系统                   │
                        ├───────────────────────────────────────────┤
                        │  LayoutManager (接口)                     │
                        │      ├── FlowLayout                       │
                        │      ├── BorderLayout                     │
                        │      ├── XYLayout                         │
                        │      └── DelegatingLayout                 │
                        └───────────────────────────────────────────┘
```

```text
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                             Novadraw 框架架构                                        │
├─────────────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    │
│  │    Vello     │◀───│   SceneHost  │◀───│  FigureGraph  │◀───│  NdCanvas    │    │
│  │ (渲染后端)   │    │ (主循环)     │    │ (场景图)      │    │ (绘制 API)   │    │
│  └──────────────┘    └──────────────┘    └──────────────┘    └──────────────┘    │
│                            │                   │                   │              │
│                      事件分发              树遍历              绘制命令           │
│                      InputEvent            SlotMap             RenderCommand      │
└─────────────────────────────────────────────────────────────────────────────────────┘
                                          ↓
                        ┌───────────────────────────────────────────┐
                        │           Novadraw Figure Trait          │
                        ├───────────────────────────────────────────┤
                        │  Bounded (边界)                           │
                        │      │                                    │
                        │      └── Figure (渲染接口)                │
                        │              │                            │
                        │              └── Shape (描边/填充)         │
                        │                    ├── RectangleFigure     │
                        │                    ├── EllipseFigure      │
                        │                    └── PolygonFigure       │
                        │                                                │
                        │  Updatable (验证)                          │
                        │      └── TriangleFigure (validate 缓存)     │
                        └───────────────────────────────────────────┘
                                          ↓
                        ┌───────────────────────────────────────────┐
                        │           Novadraw 布局系统                 │
                        ├───────────────────────────────────────────┤
                        │  LayoutManager (trait)                    │
                        │      ├── FlowLayout                        │
                        │      ├── BorderLayout                      │
                        │      └── FillLayout                        │
                        └───────────────────────────────────────────┘
```

### 框架映射表

| draw2d 组件 | Novadraw 对应 | 说明 |
|---|---|---|
| SWT GC | NdCanvas | 底层绘制 API |
| LightweightSystem | NovadrawSystem + SceneHost | 前者是组合根，后者承担渲染入口协调 |
| UpdateManager | SceneUpdateManager | 延迟批处理 + 两阶段更新 |
| Figure Tree | FigureGraph + SlotMap | 树结构 + O(1) 查找 |
| IFigure | Figure + Bounded | Trait 体系 |
| Figure (基类) | Shape + 具体实现 | 描边/填充 |
| Graphics | NdCanvas | 状态管理 + 变换栈 |
| LayoutManager | LayoutManager | 布局接口 |
| Border | Border trait | 装饰器 |

### 方案 B：内嵌模块 `novadraw-render::display_list`

**优势**：
- 共享 novadraw-geometry 类型更便捷
- 项目结构简单
- 无需处理跨 crate 依赖

**劣势**：
- 无法独立发布
- 与渲染逻辑耦合

### 推荐：方案 A（独立 crate）

理由：
1. 你明确提到"希望单独发布"
2. 工业级协议应保持技术独立性
3. 支持跨项目复用（如独立工具链）

## 实现范围

**完整功能（本次交付）**：

1. **基础二进制协议**
   - 扁平二进制 Buffer (Vec<u8>)
   - u32 OpCode + 8-byte Aligned Payload
   - bytemuck::Pod 实现零拷贝解析
   - 核心指令：PushState, PopState, SaveLayer, FillRect, StrokeRect, Path, Image, GlyphRun, SetClip

2. **多缓冲区设计**
   - Command Stream (固定大小指令)
   - Data Pool (变长数据：Path 顶点、Dash 阵列)

3. **增量更新**
   - Chunk 分块机制
   - Patch 协议 (Replace, Delete, UpdateIndex)
   - 差分数据传输

4. **资源系统**
   - ResourceHandle (u64) ID 化
   - Manifest 声明式资源清单
   - ResourceResolver 接口
   - 加载中状态降级渲染

5. **高级特性**
   - Save/Restore 栈
   - SaveLayer (离屏缓冲)
   - 混合模式 (Blend Modes)
   - CustomEffect 着色器扩展
   - GlyphRun 文本流

## 关键文件

```
novadraw-displaylist/              # 独立 crate（可单独发布）
├── Cargo.toml
├── src/
│   ├── lib.rs                     # 模块入口
│   ├── header.rs                  # DisplayListHeader (repr(C), bytemuck::Pod)
│   ├── opcode.rs                  # OpCode 枚举 (u32)
│   ├── command.rs                 # 二进制指令结构体
│   ├── chunk.rs                   # Chunk 结构
│   ├── pool.rs                    # DataPool (变长数据)
│   ├── dispatcher.rs              # Dispatcher 回放器 trait
│   ├── recorder.rs                # Recorder 录制器
│   ├── manifest.rs                # Resource Manifest
│   ├── patch.rs                   # Patch 结构 (增量更新)
│   ├── resource.rs                # ResourceHandle, ResourceResolver
│   ├── effect.rs                  # CustomEffect, BlendMode
│   └── glyph_run.rs               # GlyphRun 文本流
└── tests/
    ├── abi_test.c                 # C ABI 兼容性测试
    └── protocol_test.rs           # 协议正确性测试

novadraw-render/                   # 渲染层（可选依赖）
└── src/
    ├── lib.rs                     # 添加 display_list feature
    ├── adapter/
    │   └── mod.rs                 # DisplayList -> RenderCommand 适配器
    └── backend/vello/mod.rs       # 添加 display_list 回放支持
```

## 详细实现步骤

### 步骤 1：基础类型定义 (display_list/opcode.rs)

```rust
// OpCode 枚举 (u32)
pub enum OpCode {
    Nop = 0,
    PushState = 1,
    PopState = 2,
    SaveLayer = 3,
    RestoreLayer = 4,
    FillRect = 5,
    StrokeRect = 6,
    FillPath = 7,
    StrokePath = 8,
    DrawImage = 9,
    DrawGlyphRun = 10,
    SetClip = 11,
    // ... 更多指令
}
```

### 步骤 2：指令结构体 (display_list/command.rs)

```rust
// 8-byte aligned 结构体
#[repr(C, align(8))]
pub struct FillRectCmd {
    pub opcode: u32,           // OpCode::FillRect
    pub rect: [f64; 4],        // x, y, width, height
    pub color: u32,            // ABGR packed color
    pub transform_id: u32,     // 资源句柄
}

unsafe impl bytemuck::Zeroable for FillRectCmd {}
unsafe impl bytemuck::Pod for FillRectCmd {}
```

### 步骤 3：DataPool (display_list/pool.rs)

```rust
pub struct DataPool {
    data: Vec<u8>,
    offsets: Vec<usize>,
}

impl DataPool {
    pub fn add_path(&mut self, path: &Path) -> usize { ... }
    pub fn add_text(&mut self, text: &str) -> usize { ... }
    pub fn get(&self, offset: usize) -> &[u8] { ... }
}
```

### 步骤 4：Header (display_list/header.rs)

```rust
#[repr(C)]
pub struct DisplayListHeader {
    pub magic: u32,           // 0x444C4953 ('DLIS')
    pub version: u32,         // 版本号
    pub total_size: u64,      // 总大小
    pub command_count: u32,   // 指令数量
    pub data_pool_offset: u64,// DataPool 起始偏移
    pub manifest_offset: u64, // Manifest 起始偏移
    pub viewport: [f64; 4],   // 视口 AABB
}

unsafe impl bytemuck::Pod for DisplayListHeader {}
```

### 步骤 5：Recorder (display_list/recorder.rs)

```rust
pub struct Recorder {
    commands: Vec<u8>,
    pool: DataPool,
    manifest: Manifest,
}

impl Recorder {
    pub fn fill_rect(&mut self, rect: Rect, color: Color) { ... }
    pub fn draw_image(&mut self, handle: ResourceHandle, dest: Rect) { ... }
    pub fn save_layer(&mut self, blend_mode: BlendMode) -> LayerId { ... }
    pub fn finish(self) -> DisplayList { ... }
}
```

### 步骤 6：Dispatcher (display_list/dispatcher.rs)

```rust
pub trait Dispatcher {
    fn fill_rect(&mut self, cmd: &FillRectCmd);
    fn draw_image(&mut self, cmd: &DrawImageCmd);
    // ... 其他方法
}

pub struct DisplayList<'a> {
    header: &'a DisplayListHeader,
    commands: &'a [u8],
    pool: &'a DataPool,
}

impl<'a> DisplayList<'a> {
    pub fn replay<D: Dispatcher>(&self, dispatcher: &mut D) { ... }
}
```

### 步骤 7：与现有系统集成

修改 `NdCanvas` 支持双模式：
```rust
impl NdCanvas {
    // 现有 Vec<RenderCommand> 模式
    pub fn commands(&self) -> &Vec<RenderCommand> { ... }

    // 新增：录制到 DisplayList
    pub fn record_display_list(&self) -> DisplayList { ... }
}
```

修改 `VelloRenderer`：
```rust
impl VelloRenderer {
    pub fn render_dl(&mut self, dl: &DisplayList) { ... }
}
```

## ABI 兼容设计

确保所有结构体使用 `repr(C)`，便于 C++/Swift 直接读取：
- u32 OpCode
- 8-byte 对齐
- 固定大小指令 + 变长数据池

## 版本控制

```rust
const DISPLAYLIST_VERSION: u32 = 1;
const MAGIC_NUMBER: u32 = 0x444C4953; // 'DLIS'
```

## 验证计划

1. **协议正确性**：
   - 单元测试：指令编码/解码、零拷贝解析
   - 验证 bytemuck::Pod 正确实现
   - Patch 生成与应用测试

2. **ABI 兼容**：
   - 编写 C 测试程序验证二进制布局
   - 验证跨语言读取能力

3. **功能测试**：
   - 测试所有指令类型（PushState, FillRect, SaveLayer 等）
   - 测试增量更新（Chunk, Patch）
   - 测试资源系统（Manifest, Handle, Resolver）

4. **性能基准**：
   - 编码/解码性能
   - 内存占用对比

## 交付物

1. `novadraw-displaylist/` 独立 crate（完整功能）
2. 单元测试覆盖
3. 协议文档（Rustdoc + ASCII 架构图）
4. C FFI 兼容性测试
5. 与 novadraw-render 的集成适配器

## Cargo.toml 配置

```toml
# novadraw-displaylist/Cargo.toml
[package]
name = "novadraw-displaylist"
version = "0.1.0"
edition = "2021"
description = "Industrial-grade DisplayList binary protocol for 2D graphics"
repository = "https://github.com/your-org/novadraw"

[features]
default = ["std"]
std = ["bytemuck/std"]
wasm = ["bytemuck/wasm32"]

[dependencies]
bytemuck = { version = "1.14", features = ["derive", "wasm32-size-t-is-usize"] }
hashbrown = "0.14"

# 可选依赖（用于测试）
[dev-dependencies]
criterion = "0.5"
```

## 发布流程

1. 在 `Cargo.toml` workspace 中添加新 crate
2. 配置 crates.io 发布信息
3. 创建 Git tag（如 `displaylist-v0.1.0`）
4. `cargo publish --package novadraw-displaylist`
