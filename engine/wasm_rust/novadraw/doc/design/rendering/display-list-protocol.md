# Novadraw DisplayList 协议架构设计

类型：`normative-design`

> **独立方案文档。** 本文定义的是 Novadraw 候选协议，不是 Eclipse Draw2D
> 的原生架构或 API，也不代表仓库当前已经实现全部所列能力。

## 0. 设计说明：Skia 混合模式

本协议采用 **Skia 混合模式**，同时支持两种指令类型：

### 指令类型

| 类型 | 说明 | 示例 |
|------|------|------|
| **状态指令** | 设置/修改当前渲染状态 | `SetFillColor`, `SetStrokeWidth`, `SetTransform` |
| **绘制指令** | 执行具体绘制操作（使用当前状态） | `FillRect`, `DrawImage`, `DrawGlyphRun` |

### 使用示例

```rust
// 方式 1：状态指令 + 绘制指令（推荐，体积更小）
SetFillColor(0xFF0000FF)     // 设置红色
SetStrokeWidth(2.0)          // 设置线宽
FillRect(rect)               // 使用当前状态绘制

// 方式 2：临时覆盖状态（适合一次性特殊绘制）
FillRectWithPaint(rect, Paint { color: blue, .. }) // 临时蓝色
// 等价于：save() → set blue → fillRect() → restore()
```

### 状态管理

- **Save/Restore**：支持状态栈保存与恢复
- **临时覆盖**：绘制指令可带内嵌状态，临时覆盖当前状态
- **状态继承**：子 Chunk 继承父 Chunk 的状态

### 优势

1. **灵活性**：按需选择状态复用或临时覆盖
2. **体积效率**：重复绘制时复用状态，减小体积
3. **性能**：减少状态切换开销
4. **兼容性**：支持两种使用习惯

---

## 1. 概述

设计一套工业级、纯协议层的 DisplayList (DL) 框架，作为 UI 逻辑层与多样化渲染后端（Skia, Vello, Wgpu, 远程流渲染）之间的通用语言。

**核心目标**：
- 零拷贝解析，支持内存直接映射
- 增量更新，支持分块 Patch
- 资源解耦，ID 化管理
- ABI 兼容，C++/Swift 可直接读取

---

## 2. 架构总览

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           DisplayList 架构                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────┐     ┌─────────────────┐     ┌─────────────────────┐  │
│   │   Recorder  │────▶│  DisplayList    │────▶│      Dispatcher     │  │
│   │  (Figure)   │     │  (Binary Buf)   │     │  (Playback Engine)  │  │
│   └─────────────┘     └─────────────────┘     └─────────────────────┘  │
│                              │                           │               │
│                              ▼                           ▼               │
│                        ┌───────────┐            ┌──────────────────┐   │
│                        │ Manifest  │            │   RenderBackend   │   │
│                        │ (Headers) │            │ (Vello/Skia/...) │   │
│                        └───────────┘            └──────────────────┘   │
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                      Binary Buffer Layout                        │   │
│   ├─────────────────────────────────────────────────────────────────┤   │
│   │  [Header][Chunk Index][Chunk 1][Chunk 2]...[Chunk N][Data Pool] │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. 内存布局设计

### 3.1 二进制 Buffer 结构

```
┌──────────────────────────────────────────────────────────────────────────┐
│                              DisplayList                                  │
├──────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐                                                    │
│  │   DL Header     │  64 bytes - 协议元信息                              │
│  ├─────────────────┤                                                    │
│  │ Chunk Index     │  N * 16 bytes - Chunk 索引表                        │
│  ├─────────────────┤                                                    │
│  │   Chunk 1       │  可变大小 - 绘图指令流                               │
│  │   Chunk 2       │                                                    │
│  │   ...           │                                                    │
│  │   Chunk M       │                                                    │
│  ├─────────────────┤                                                    │
│  │   Data Pool     │  变长数据：Path 顶点、Dash 数组、Text Buffer 等      │
│  └─────────────────┘                                                    │
└──────────────────────────────────────────────────────────────────────────┘
```

### 3.2 核心数据结构（repr(C)）

```rust
// ========== 文件: novadraw-displaylist/src/lib.rs ==========

use bytemuck::{Pod, Zeroable};
use glam::Vec2;

// ============================================================================
// 头部定义
// ============================================================================

/// 协议魔数
pub const DL_MAGIC: u32 = 0x444C_0100; // "DL01"

/// 协议版本
pub const DL_VERSION: u16 = 0x0001;

/// DisplayList 头部 (64 bytes, 8-byte aligned)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DisplayListHeader {
    /// 协议魔数: 0x444C0100
    pub magic: u32,
    /// 协议版本: 0x0001
    pub version: u16,
    /// 标志位 (详见 HeaderFlags)
    pub flags: u16,
    /// 头部大小 (用于兼容性扩展)
    pub header_size: u32,
    /// 总 Buffer 大小 (bytes)
    pub total_size: u64,
    /// Chunk 数量
    pub chunk_count: u32,
    /// 资源清单数量
    pub manifest_count: u32,
    /// Chunk Index 起始偏移 (从 Buffer 起始)
    pub chunk_index_offset: u64,
    /// Data Pool 起始偏移
    pub data_pool_offset: u64,
    /// CRC32 校验码 (不含 header)
    pub crc32: u32,
    /// 保留字段 (填充至 64 bytes)
    _reserved: [u8; 32],
}

/// 头部标志位
#[repr(u16)]
pub enum HeaderFlags {
    /// 包含 AABB 索引 (用于空间裁剪)
    HasAabbIndex = 1 << 0,
    /// 启用压缩
    Compressed = 1 << 1,
    /// 包含时间戳
    HasTimestamp = 1 << 2,
    /// 是增量 Patch
    IsPatch = 1 << 3,
    /// 包含资源清单
    HasManifest = 1 << 4,
}

// ============================================================================
// Chunk 结构
// ============================================================================

/// Chunk 索引项 (16 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ChunkIndex {
    /// Chunk ID (用于 Patch 定位)
    pub chunk_id: u32,
    /// Chunk 类型 (详见 ChunkType)
    pub chunk_type: u16,
    /// Chunk 标志
    pub flags: u16,
    /// AABB 空间索引 (如果启用)
    pub aabb: Aabb,
    /// Chunk 在 Command Stream 中的偏移
    pub offset: u32,
    /// Chunk 大小 (bytes)
    pub size: u32,
}

/// Chunk 类型
#[repr(u16)]
pub enum ChunkType {
    /// 根 Chunk (包含完整场景)
    Root = 0,
    /// 静态绘制内容
    Static = 1,
    /// 动态内容 (高频更新)
    Dynamic = 2,
    /// 背景层
    Background = 3,
    /// 前景层 (UI 覆盖层)
    Foreground = 4,
    /// 文本层
    Text = 5,
    /// 图像层
    Image = 6,
    /// 视频层
    Video = 7,
}

/// 边界盒子 (32 bytes, 8-byte aligned)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, Default)]
pub struct Aabb {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Aabb {
    /// 检查与另一个 AABB 是否相交
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min_x <= other.max_x && self.max_x >= other.min_x &&
        self.min_y <= other.max_y && self.max_y >= other.min_y
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.min_x >= self.max_x || self.min_y >= self.max_y
    }
}

// ============================================================================
// 指令操作码
// ============================================================================

/// 指令操作码 (OpCode)
#[repr(u32)]
pub enum Opcode {
    /// 空指令
    Nop = 0,

    // === 状态管理 (1-19) ===
    /// 保存当前状态
    SaveState = 1,
    /// 恢复状态
    RestoreState = 2,
    /// 保存图层 (带混合模式)
    SaveLayer = 3,
    /// 恢复图层
    RestoreLayer = 4,
    /// 重置状态栈
    ResetState = 5,

    // === 变换与裁剪 (20-39) ===
    /// 设置变换矩阵
    SetTransform = 20,
    /// 重置变换为单位矩阵
    ResetTransform = 21,
    /// 设置裁剪矩形
    SetClipRect = 22,
    /// 重置裁剪
    ResetClip = 23,
    /// 设置混合模式
    SetBlendMode = 30,
    /// 设置不透明度
    SetAlpha = 31,

    // === 基本绘图 (40-79) ===
    /// 填充矩形
    FillRect = 40,
    /// 描边矩形
    StrokeRect = 41,
    /// 绘制圆角矩形
    FillRoundedRect = 42,
    /// 绘制圆形
    FillCircle = 43,
    /// 描边圆形
    StrokeCircle = 44,
    /// 绘制椭圆
    FillEllipse = 45,

    // === 路径绘制 (80-119) ===
    /// 开始路径
    BeginPath = 80,
    /// 移动到 (相对坐标)
    MoveTo = 81,
    /// 直线到 (相对坐标)
    LineTo = 82,
    /// 水平线到 (相对坐标)
    HorizontalLineTo = 83,
    /// 垂直线到 (相对坐标)
    VerticalLineTo = 84,
    /// 三次贝塞尔曲线
    CubicBezierTo = 85,
    /// 二次贝塞尔曲线
    QuadraticBezierTo = 86,
    /// 圆弧
    ArcTo = 87,
    /// 闭合路径
    ClosePath = 88,
    /// 填充路径
    FillPath = 89,
    /// 描边路径
    StrokePath = 90,

    // === 图像绘制 (120-149) ===
    /// 绘制图像
    DrawImage = 120,
    /// 绘制图像子区域
    DrawImageRect = 121,
    /// 绘制 9-patch 图像
    DrawImage9Patch = 122,

    // === 文本绘制 (150-189) ===
    /// 绘制字形序列 (核心文本指令)
    DrawGlyphRun = 150,
    /// 设置字体
    SetFont = 151,
    /// 设置字号
    SetFontSize = 152,
    /// 设置字体特征 (Weight, Stretch, Style)
    SetFontFeatures = 153,

    // === 高级效果 (190-199) ===
    /// 设置阴影
    SetShadow = 190,
    /// 设置滤镜
    SetFilter = 191,
    /// 自定义效果
    CustomEffect = 192,
}

// ============================================================================
// 指令结构 (Fixed-size Payloads)
// ============================================================================

/// 指令头 (所有指令的前缀)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OpHeader {
    /// 操作码
    pub opcode: u32,
    /// 指令大小 (包括 header, bytes)
    pub size: u32,
    /// 保留
    _reserved: u32,
}

/// 状态保存指令
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OpSaveState {
    pub header: OpHeader,
    /// 状态标记 (StateFlags)
    pub flags: u32,
}

/// 变换指令 (24 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OpSetTransform {
    pub header: OpHeader,
    /// 2D 仿射变换矩阵 (列主序)
    /// [a, b, c, d, tx, ty]
    /// Transform = | a  c  tx |
    ///             | b  d  ty |
    ///             | 0  0  1  |
    pub matrix: [f32; 6],
}

/// 填充矩形指令 (24 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OpFillRect {
    pub header: OpHeader,
    pub rect: Rect,
    /// 颜色 (sRGBA, 预乘 alpha)
    pub color: ColorU,
}

/// 描边矩形指令 (28 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OpStrokeRect {
    pub header: OpHeader,
    pub rect: Rect,
    pub stroke_width: f32,
    /// 颜色 (sRGBA, 预乘 alpha)
    pub color: ColorU,
}

/// 混合模式枚举
#[repr(u32)]
pub enum BlendMode {
    SrcOver = 0,
    Src = 1,
    DstOver = 2,
    Dst = 3,
    SrcIn = 4,
    DstIn = 5,
    Plus = 6,
    Multiply = 7,
    Screen = 8,
    Overlay = 9,
    Darken = 10,
    Lighten = 11,
    ColorDodge = 12,
    ColorBurn = 13,
    HardLight = 14,
    SoftLight = 15,
    Difference = 16,
    Exclusion = 17,
}

/// 设置混合模式指令
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OpSetBlendMode {
    pub header: OpHeader,
    pub blend_mode: u32, // BlendMode
}

/// 矩形 (16 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 无符号颜色 (32-bit ABGR, 预乘 alpha)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ColorU(pub u32);

impl ColorU {
    /// 从 RGBA 创建 (非预乘)
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        let premul_r = (r as u32 * a as u32 / 255) as u8;
        let premul_g = (g as u32 * a as u32 / 255) as u8;
        let premul_b = (b as u32 * a as u32 / 255) as u8;
        ColorU((a as u32) | (premul_b as u32) << 8 | (premul_g as u32) << 16 | (premul_r as u32) << 24)
    }

    /// 从预乘 RGBA 创建
    pub fn from_premul_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        ColorU((a as u32) | (b as u32) << 8 | (g as u32) << 16 | (r as u32) << 24)
    }

    pub fn r(&self) -> u8 { (self.0 >> 24) as u8 }
    pub fn g(&self) -> u8 { (self.0 >> 16) as u8 }
    pub fn b(&self) -> u8 { (self.0 >> 8) as u8 }
    pub fn a(&self) -> u8 { self.0 as u8 }
}

/// 资源句柄 (u64)
pub type ResourceHandle = u64;

// ============================================================================
// 图像指令
// ============================================================================

/// 绘制图像指令 (32 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OpDrawImage {
    pub header: OpHeader,
    /// 图像资源句柄
    pub image_handle: ResourceHandle,
    /// 目标矩形
    pub dst_rect: Rect,
    /// 源矩形 (如果为空, 使用完整图像)
    pub src_rect: Rect,
    /// 图像滤镜 (ImageFilter)
    pub filter: u32,
}

/// 采样滤镜
#[repr(u32)]
pub enum ImageFilter {
    Nearest = 0,
    Linear = 1,
    Cubic = 2,
    Gaussian = 3,
}

// ============================================================================
// 字形运行指令 (核心文本指令)
// ============================================================================

/// 字形运行 (变长, 存储在 Data Pool)
/// 注意: GlyphRun 的数据存储在 Data Pool, 此处仅包含引用
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OpDrawGlyphRun {
    pub header: OpHeader,
    /// 字形运行在 Data Pool 中的偏移
    pub glyph_run_offset: u32,
    /// 字形运行大小 (bytes)
    pub glyph_run_size: u32,
    /// 字体资源句柄
    pub font_handle: ResourceHandle,
    /// 颜色 (sRGBA, 预乘 alpha)
    pub color: ColorU,
}

/// 字形运行数据 (存储在 Data Pool, repr(C))
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GlyphRunData {
    /// 字形数量
    pub glyph_count: u32,
    /// 保留对齐
    _reserved: u32,
    /// 字形 ID 数组偏移 (在 Data Pool 中)
    pub glyph_ids_offset: u32,
    /// 字形位置数组偏移 (在 Data Pool 中)
    pub positions_offset: u32,
    /// 渲染模式
    pub render_mode: u8,
    /// 保留
    _reserveg2: [u8; 3],
    /// 保留字段
    _reserved3: u32,
}

// ============================================================================
// 路径指令 (简化版)
// ============================================================================

/// 绘制路径指令 (变长)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OpFillPath {
    pub header: OpHeader,
    /// 路径数据在 Data Pool 中的偏移
    pub path_offset: u32,
    /// 路径数据大小 (bytes)
    pub path_size: u32,
    /// 填充规则 (FillRule)
    pub fill_rule: u32,
    /// 颜色 (sRGBA, 预乘 alpha)
    pub color: ColorU,
}

/// 填充规则
#[repr(u32)]
pub enum FillRule {
    NonZero = 0,
    EvenOdd = 1,
}

/// 路径数据 (存储在 Data Pool)
/// 变长结构: [PathCommand, PathCommand, ...]
#[repr(C)]
pub enum PathCommand {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    HorizontalLineTo(f32),
    VerticalLineTo(f32),
    CubicBezierTo(f32, f32, f32, f32, f32, f32),
    QuadraticBezierTo(f32, f32, f32, f32),
    ArcTo(f32, f32, f32, f32, f32, bool, bool),
    ClosePath,
}

// ============================================================================
// 资源清单 (Manifest)
// ============================================================================

/// 资源清单项 (存储在头部或独立区域)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ResourceManifest {
    /// 资源句柄
    pub handle: ResourceHandle,
    /// 资源类型 (ResourceType)
    pub resource_type: u16,
    /// 资源标志
    pub flags: u16,
    /// 资源哈希 (SHA-256 前 8 bytes)
    pub hash: u64,
    /// 资源大小 (bytes)
    pub size: u64,
    /// 资源名称或路径 (UTF-8, 偏移量)
    pub name_offset: u32,
    /// 保留
    _reserved: u32,
}

/// 资源类型
#[repr(u16)]
pub enum ResourceType {
    Image = 0,
    Font = 1,
    Gradient = 2,
    Shader = 3,
    Mesh = 4,
}

/// 资源加载状态
#[repr(u8)]
pub enum ResourceLoadState {
    NotLoaded = 0,
    Loading = 1,
    Loaded = 2,
    Error = 3,
    /// 降级为占位颜色
    Placeholder = 4,
}

// ============================================================================
// Patch 结构 (增量更新)
// ============================================================================

/// Patch 头部
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PatchHeader {
    /// 基础 DisplayList 版本号
    pub base_version: u64,
    /// Patch ID
    pub patch_id: u32,
    /// Patch 操作数量
    pub operation_count: u32,
    /// Patch 总大小 (bytes)
    pub patch_size: u64,
}

/// Patch 操作类型
#[repr(u32)]
pub enum PatchOp {
    /// 替换 Chunk
    ReplaceChunk = 0,
    /// 删除 Chunk
    DeleteChunk = 1,
    /// 新增 Chunk
    InsertChunk = 2,
    /// 更新资源清单
    UpdateManifest = 3,
    /// 全量替换
    FullReplace = 4,
}

/// Patch 操作
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PatchOperation {
    /// 操作类型
    pub op_type: u32,
    /// 目标 Chunk ID (如果是 Replace/Delete)
    pub target_chunk_id: u32,
    /// 插入位置 Chunk ID (如果是 Insert)
    pub insert_after_chunk_id: u32,
    /// 新 Chunk 数据偏移 (在 Patch Buffer 中)
    pub new_chunk_offset: u32,
    /// 新 Chunk 数据大小
    pub new_chunk_size: u32,
}

// ============================================================================
// 资源解析器接口
// ============================================================================

/// 资源解析器 trait
pub trait ResourceResolver {
    /// 解析图像资源
    fn resolve_image(&self, handle: ResourceHandle) -> Result<ImageHandle, ResourceError>;
    /// 解析字体资源
    fn resolve_font(&self, handle: ResourceHandle) -> Result<FontHandle, ResourceError>;
    /// 获取资源加载状态
    fn get_load_state(&self, handle: ResourceHandle) -> ResourceLoadState;
}

/// 资源错误
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("Resource not found: {0}")]
    NotFound(ResourceHandle),
    #[error("Resource load failed: {0}")]
    LoadFailed(ResourceHandle),
    #[error("Invalid resource format")]
    InvalidFormat,
}

// ============================================================================
// Dispatcher (分发器)
// ============================================================================

/// DisplayList 分发器 - 零拷贝解析与回放
pub struct DisplayListDispatcher<'a> {
    /// 指向 Buffer 的指针
    data: &'a [u8],
    /// 头部引用
    header: &'a DisplayListHeader,
    /// Chunk 索引表
    chunk_indices: &'a [ChunkIndex],
    /// Data Pool 起始位置
    data_pool: &'a [u8],
    /// 资源解析器
    resolver: &'a dyn ResourceResolver,
}

impl<'a> DisplayListDispatcher<'a> {
    /// 从 Buffer 创建 Dispatcher (零拷贝)
    pub fn new(data: &'a [u8], resolver: &'a dyn ResourceResolver) -> Result<Self, DisplayListError> {
        // 验证头部
        let header = unsafe {
            &*(data.as_ptr() as *const DisplayListHeader)
        };

        if header.magic != DL_MAGIC {
            return Err(DisplayListError::InvalidMagic);
        }
        if header.version != DL_VERSION {
            return Err(DisplayListError::VersionMismatch(header.version));
        }

        // 验证大小
        if data.len() < header.header_size as usize {
            return Err(DisplayListError::BufferTooSmall);
        }

        // 解析 Chunk 索引表
        let chunk_indices_size = header.chunk_count as usize * std::mem::size_of::<ChunkIndex>();
        let chunk_indices = unsafe {
            std::slice::from_raw_parts(
                data[header.chunk_index_offset as usize..].as_ptr() as *const ChunkIndex,
                header.chunk_count as usize,
            )
        };

        // Data Pool
        let data_pool = &data[header.data_pool_offset as usize..];

        Ok(Self {
            data,
            header,
            chunk_indices,
            data_pool,
            resolver,
        })
    }

    /// 迭代执行所有指令
    pub fn dispatch(&self, ctx: &mut dyn RenderContext) {
        let mut op_ptr = self.data[64..].as_ptr(); // 从 header 之后开始
        let op_end = self.data[self.header.data_pool_offset as usize..].as_ptr();

        while op_ptr < op_end {
            let header = unsafe { &*(op_ptr as *const OpHeader) };
            let opcode = header.opcode;

            // 跳过头部
            op_ptr = unsafe { op_ptr.add(std::mem::size_of::<OpHeader>()) };

            match opcode {
                Opcode::FillRect as u32 => {
                    let op = unsafe { &*(op_ptr as *const OpFillRect) };
                    ctx.fill_rect(&op.rect, ColorU(op.color.0));
                    op_ptr = unsafe { op_ptr.add(std::mem::size_of::<OpFillRect>()) };
                }
                Opcode::StrokeRect as u32 => {
                    let op = unsafe { &*(op_ptr as *const OpStrokeRect) };
                    ctx.stroke_rect(&op.rect, op.stroke_width, ColorU(op.color.0));
                    op_ptr = unsafe { op_ptr.add(std::mem::size_of::<OpStrokeRect>()) };
                }
                Opcode::SetTransform as u32 => {
                    let op = unsafe { &*(op_ptr as *const OpSetTransform) };
                    ctx.set_transform(op.matrix);
                    op_ptr = unsafe { op_ptr.add(std::mem::size_of::<OpSetTransform>()) };
                }
                Opcode::SetBlendMode as u32 => {
                    let op = unsafe { &*(op_ptr as *const OpSetBlendMode) };
                    ctx.set_blend_mode(op.blend_mode);
                    op_ptr = unsafe { op_ptr.add(std::mem::size_of::<OpSetBlendMode>()) };
                }
                Opcode::DrawImage as u32 => {
                    let op = unsafe { &*(op_ptr as *const OpDrawImage) };
                    ctx.draw_image(op.image_handle, &op.dst_rect, &op.src_rect, op.filter);
                    op_ptr = unsafe { op_ptr.add(std::mem::size_of::<OpDrawImage>()) };
                }
                Opcode::DrawGlyphRun as u32 => {
                    let op = unsafe { &*(op_ptr as *const OpDrawGlyphRun) };
                    // 从 Data Pool 读取 GlyphRunData
                    let glyph_run_data = unsafe {
                        &*(self.data_pool[op.glyph_run_offset as usize..].as_ptr()
                            as *const GlyphRunData)
                    };
                    self.dispatch_glyph_run(ctx, glyph_run_data, op.font_handle, op.color);
                    op_ptr = unsafe { op_ptr.add(std::mem::size_of::<OpDrawGlyphRun>()) };
                }
                Opcode::SaveState as u32 => {
                    ctx.save_state();
                    // 无额外数据
                }
                Opcode::RestoreState as u32 => {
                    ctx.restore_state();
                    // 无额外数据
                }
                Opcode::Nop as u32 => {
                    // 跳过
                }
                _ => {
                    // 未知指令，跳过
                    eprintln!("Unknown opcode: {}", opcode);
                    return;
                }
            }
        }
    }

    /// 分发字形运行
    fn dispatch_glyph_run(
        &self,
        ctx: &mut dyn RenderContext,
        glyph_run: &GlyphRunData,
        font_handle: ResourceHandle,
        color: ColorU,
    ) {
        // 检查资源状态
        match self.resolver.get_load_state(font_handle) {
            ResourceLoadState::Loaded => {
                // 正常渲染
                let glyph_ids = self.read_glyph_ids(glyph_run);
                let positions = self.read_positions(glyph_run);
                ctx.draw_glyphs(font_handle, glyph_run.render_mode, &glyph_ids, &positions, color);
            }
            ResourceLoadState::Error | ResourceLoadState::NotLoaded => {
                // 降级: 绘制占位矩形
                let bounds = self.calculate_glyph_bounds(glyph_run);
                ctx.fill_rect(&bounds, self.get_placeholder_color(font_handle));
            }
            ResourceLoadState::Placeholder | ResourceLoadState::Loading => {
                // 降级: 绘制占位符
                let bounds = self.calculate_glyph_bounds(glyph_run);
                ctx.fill_rect(&bounds, ColorU(0x80808080)); // 灰色半透明
            }
        }
    }

    /// 读取字形 ID 数组
    fn read_glyph_ids(&self, glyph_run: &GlyphRunData) -> &[u32] {
        unsafe {
            std::slice::from_raw_parts(
                self.data_pool[glyph_run.glyph_ids_offset as usize..].as_ptr() as *const u32,
                glyph_run.glyph_count as usize,
            )
        }
    }

    /// 读取字形位置数组
    fn read_positions(&self, glyph_run: &GlyphRunData) -> &[Vec2] {
        unsafe {
            std::slice::from_raw_parts(
                self.data_pool[glyph_run.positions_offset as usize..].as_ptr() as *const Vec2,
                glyph_run.glyph_count as usize,
            )
        }
    }

    /// 计算字形边界
    fn calculate_glyph_bounds(&self, glyph_run: &GlyphRunData) -> Rect {
        let positions = self.read_positions(glyph_run);
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);

        for pos in positions {
            min = min.min(*pos);
            max = max.max(*pos);
        }

        Rect {
            x: min.x,
            y: min.y,
            width: max.x - min.x,
            height: max.y - min.y,
        }
    }

    /// 获取占位颜色
    fn get_placeholder_color(&self, _handle: ResourceHandle) -> ColorU {
        ColorU(0xFF808080) // 默认灰色
    }

    /// 空间裁剪: 获取可见的 Chunk 列表
    pub fn get_visible_chunks(&self, view_rect: &Rect) -> Vec<&ChunkIndex> {
        if !self.header.flags.contains(HeaderFlags::HasAabbIndex) {
            // 没有 AABB 索引，返回所有 Chunk
            return self.chunk_indices.iter().collect();
        }

        let view_aabb = Aabb {
            min_x: view_rect.x,
            min_y: view_rect.y,
            max_x: view_rect.x + view_rect.width,
            max_y: view_rect.y + view_rect.height,
        };

        self.chunk_indices
            .iter()
            .filter(|chunk| !chunk.aabb.is_empty() && chunk.aabb.intersects(&view_aabb))
            .collect()
    }
}

/// 渲染上下文 trait
pub trait RenderContext {
    fn fill_rect(&mut self, rect: &Rect, color: ColorU);
    fn stroke_rect(&mut self, rect: &Rect, width: f32, color: ColorU);
    fn set_transform(&mut self, matrix: [f32; 6]);
    fn set_blend_mode(&mut self, mode: u32);
    fn draw_image(&mut self, handle: ResourceHandle, dst_rect: &Rect, src_rect: &Rect, filter: u32);
    fn draw_glyphs(&mut self, font_handle: ResourceHandle, render_mode: u8, glyph_ids: &[u32], positions: &[Vec2], color: ColorU);
    fn save_state(&mut self);
    fn restore_state(&mut self);
}

/// DisplayList 错误
#[derive(Debug, thiserror::Error)]
pub enum DisplayListError {
    #[error("Invalid magic number")]
    InvalidMagic,
    #[error("Version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u16, got: u16 },
    #[error("Buffer too small")]
    BufferTooSmall,
    #[error("Invalid chunk index")]
    InvalidChunkIndex,
    #[error("Invalid data offset")]
    InvalidDataOffset,
}
```

---

## 4. 增量更新逻辑

### 4.1 Patch 生成与应用流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           增量更新流程                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   原始 DL                   Patch 生成                 新 DL                  │
│   ┌─────────┐            ┌─────────────────┐       ┌─────────────────────┐  │
│   │ Chunk 1 │            │ PatchHeader     │       │ Chunk 1 (不变)      │  │
│   │ Chunk 2 │───diff────▶│ - base_version  │──────▶│ Chunk 2 (修改)      │  │
│   │ Chunk 3 │            │ - op_count      │       │ Chunk 3 (不变)      │  │
│   └─────────┘            │ Op1: Replace    │       │ Chunk 4 (新增)      │
│                          │   chunk_id=2    │       │ Chunk 5 (新增)      │
│                          │ Op2: Insert     │       └─────────────────────┘  │
│                          │   after=3       │                                │
│                          │ Op3: Insert     │                                │
│                          │   after=4       │                                │
│                          └─────────────────┘                                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Patch 应用示例

```rust
// ========== 文件: novadraw-displaylist/src/patch.rs ==========

/// 应用 Patch 到 DisplayList
pub struct PatchApplier {
    base_display_list: Vec<u8>,
    patch_data: Vec<u8>,
}

impl PatchApplier {
    /// 创建 PatchApplier
    pub fn new(base: Vec<u8>, patch: Vec<u8>) -> Self {
        Self {
            base_display_list: base,
            patch_data: patch,
        }
    }

    /// 应用 Patch 并生成新的 DisplayList
    pub fn apply(self) -> Result<Vec<u8>, PatchError> {
        let patch_header = unsafe {
            &*(self.patch_data.as_ptr() as *const PatchHeader)
        };

        // 验证基础版本
        let base_header = unsafe {
            &*(self.base_display_list.as_ptr() as *const DisplayListHeader)
        };

        if base_header.header_size != patch_header.base_version {
            return Err(PatchError::VersionMismatch);
        }

        // 解析 Patch 操作
        let ops = self.parse_operations(patch_header)?;

        // 收集所有修改的 Chunk ID
        let mut modified_chunks: Vec<u32> = Vec::new();
        let mut deleted_chunks: Vec<u32> = Vec::new();
        let mut new_chunks: Vec<(u32, Vec<u8>)> = Vec::new();

        for op in &ops {
            match op.op_type {
                PatchOp::ReplaceChunk => {
                    modified_chunks.push(op.target_chunk_id);
                    let chunk_data = self.extract_chunk_data(&op)?;
                    new_chunks.push((op.target_chunk_id, chunk_data));
                }
                PatchOp::DeleteChunk => {
                    deleted_chunks.push(op.target_chunk_id);
                }
                PatchOp::InsertChunk => {
                    let chunk_data = self.extract_chunk_data(&op)?;
                    new_chunks.push((op.insert_after_chunk_id + 1, chunk_data));
                }
                PatchOp::FullReplace => {
                    // 全量替换，直接返回 Patch 中的数据
                    return Ok(self.extract_full_replacement(patch_header));
                }
                _ => {}
            }
        }

        // 复制不变的 Chunk
        let mut result = Vec::new();
        result.extend_from_slice(&self.base_display_list[..64]); // 复制头部

        // 复制 Chunk 索引和内容
        // ... (实现 Chunk 复制逻辑)

        // 应用修改
        // ... (实现修改应用逻辑)

        Ok(result)
    }

    /// 解析 Patch 操作
    fn parse_operations(&self, header: &PatchHeader) -> Result<&[PatchOperation], PatchError> {
        let ops_size = header.operation_count as usize * std::mem::size_of::<PatchOperation>();
        unsafe {
            Ok(std::slice::from_raw_parts(
                self.patch_data[std::mem::size_of::<PatchHeader>()..].as_ptr() as *const PatchOperation,
                header.operation_count as usize,
            ))
        }
    }

    /// 从 Patch 提取 Chunk 数据
    fn extract_chunk_data(&self, op: &PatchOperation) -> Result<Vec<u8>, PatchError> {
        let start = op.new_chunk_offset as usize;
        let end = start + op.new_chunk_size as usize;
        if end > self.patch_data.len() {
            return Err(PatchError::InvalidOffset);
        }
        Ok(self.patch_data[start..end].to_vec())
    }

    /// 提取全量替换数据
    fn extract_full_replacement(&self, header: &PatchHeader) -> Vec<u8> {
        self.patch_data[std::mem::size_of::<PatchHeader>()..].to_vec()
    }
}

/// Patch 错误
#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    #[error("Version mismatch")]
    VersionMismatch,
    #[error("Invalid offset")]
    InvalidOffset,
    #[error("Invalid operation")]
    InvalidOperation,
}
```

---

## 5. Recorder (录制器)

```rust
// ========== 文件: novadraw-displaylist/src/recorder.rs ==========

/// DisplayList 录制器
pub struct DisplayListRecorder {
    chunks: Vec<ChunkRecorder>,
    chunk_index: Vec<ChunkIndex>,
    data_pool: Vec<u8>,
    data_offsets: Vec<u32>,
    current_chunk_id: u32,
}

impl DisplayListRecorder {
    /// 创建新的 Recorder
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            chunk_index: Vec::new(),
            data_pool: Vec::new(),
            data_offsets: Vec::new(),
            current_chunk_id: 0,
        }
    }

    /// 开始新的 Chunk
    pub fn begin_chunk(&mut self, chunk_type: ChunkType) -> &mut ChunkRecorder {
        let chunk_id = self.current_chunk_id;
        self.current_chunk_id += 1;

        let chunk = ChunkRecorder::new(chunk_id, chunk_type);
        self.chunks.push(chunk);
        self.chunks.last_mut().unwrap()
    }

    /// 结束当前 Chunk
    pub fn end_chunk(&mut self, aabb: Option<Aabb>) {
        if let Some(chunk) = self.chunks.last_mut() {
            let (offset, size) = chunk.finalize();
            let chunk_index = ChunkIndex {
                chunk_id: chunk.chunk_id,
                chunk_type: chunk.chunk_type as u16,
                flags: 0,
                aabb: aabb.unwrap_or_default(),
                offset,
                size,
            };
            self.chunk_index.push(chunk_index);
        }
    }

    /// 完成录制并生成 DisplayList
    pub fn finish(self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // 预留头部空间
        buffer.resize(64, 0);

        // 写入 Chunk 索引
        let chunk_index_offset = buffer.len() as u64;
        for index in &self.chunk_index {
            unsafe {
                let index_bytes = std::slice::from_raw_parts(
                    index as *const ChunkIndex as *const u8,
                    std::mem::size_of::<ChunkIndex>(),
                );
                buffer.extend_from_slice(index_bytes);
            }
        }

        // 写入 Chunk 数据
        for chunk in &self.chunks {
            let chunk_data = chunk.data();
            buffer.extend_from_slice(chunk_data);
        }

        // 写入 Data Pool
        let data_pool_offset = buffer.len() as u64;
        buffer.extend_from_slice(&self.data_pool);

        // 填充头部
        let header = DisplayListHeader {
            magic: DL_MAGIC,
            version: DL_VERSION,
            flags: if !self.chunk_index.is_empty() && self.chunk_index.iter().any(|c| !c.aabb.is_empty()) {
                HeaderFlags::HasAabbIndex as u16
            } else {
                0
            },
            header_size: 64,
            total_size: buffer.len() as u64,
            chunk_count: self.chunk_index.len() as u32,
            manifest_count: 0,
            chunk_index_offset,
            data_pool_offset,
            crc32: 0, // 计算 CRC32
            _reserved: [0; 32],
        };

        // 重新写入头部
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const DisplayListHeader as *const u8,
                std::mem::size_of::<DisplayListHeader>(),
            );
            buffer[..64].copy_from_slice(header_bytes);
        }

        buffer
    }

    /// 添加数据到 Data Pool，返回偏移量
    pub fn add_to_data_pool<T: Pod>(&mut self, data: &T) -> u32 {
        let offset = self.data_pool.len() as u32;
        unsafe {
            let bytes = std::slice::from_raw_parts(
                data as *const T as *const u8,
                std::mem::size_of::<T>(),
            );
            self.data_pool.extend_from_slice(bytes);
        }
        offset
    }
}

/// 单个 Chunk 的录制器
pub struct ChunkRecorder {
    chunk_id: u32,
    chunk_type: ChunkType,
    data: Vec<u8>,
    aabb: Aabb,
}

impl ChunkRecorder {
    pub fn new(chunk_id: u32, chunk_type: ChunkType) -> Self {
        Self {
            chunk_id,
            chunk_type,
            data: Vec::new(),
            aabb: Aabb::default(),
        }
    }

    /// 写入指令头
    fn write_header(&mut self, opcode: u32, size: u32) {
        let header = OpHeader {
            opcode,
            size,
            _reserved: 0,
        };
        unsafe {
            let bytes = std::slice::from_raw_parts(
                &header as *const OpHeader as *const u8,
                std::mem::size_of::<OpHeader>(),
            );
            self.data.extend_from_slice(bytes);
        }
    }

    /// 添加 FillRect 指令
    pub fn fill_rect(&mut self, rect: &Rect, color: ColorU) {
        let size = std::mem::size_of::<OpFillRect>() as u32;
        self.write_header(Opcode::FillRect as u32, size);

        let op = OpFillRect {
            header: OpHeader {
                opcode: Opcode::FillRect as u32,
                size,
                _reserved: 0,
            },
            rect: *rect,
            color,
        };

        unsafe {
            let bytes = std::slice::from_raw_parts(
                &op as *const OpFillRect as *const u8,
                std::mem::size_of::<OpFillRect>(),
            );
            self.data.extend_from_slice(bytes);
        }

        // 更新 AABB
        self.update_aabb(rect.x, rect.y, rect.width, rect.height);
    }

    /// 添加 StrokeRect 指令
    pub fn stroke_rect(&mut self, rect: &Rect, width: f32, color: ColorU) {
        let size = std::mem::size_of::<OpStrokeRect>() as u32;
        self.write_header(Opcode::StrokeRect as u32, size);

        let op = OpStrokeRect {
            header: OpHeader {
                opcode: Opcode::StrokeRect as u32,
                size,
                _reserved: 0,
            },
            rect: *rect,
            stroke_width: width,
            color,
        };

        unsafe {
            let bytes = std::slice::from_raw_parts(
                &op as *const OpStrokeRect as *const u8,
                std::mem::size_of::<OpStrokeRect>(),
            );
            self.data.extend_from_slice(bytes);
        }

        // 更新 AABB (包括描边宽度)
        let half_width = width / 2.0;
        self.update_aabb(
            rect.x - half_width,
            rect.y - half_width,
            rect.width + width,
            rect.height + width,
        );
    }

    /// 设置变换
    pub fn set_transform(&mut self, matrix: [f32; 6]) {
        let size = std::mem::size_of::<OpSetTransform>() as u32;
        self.write_header(Opcode::SetTransform as u32, size);

        let op = OpSetTransform {
            header: OpHeader {
                opcode: Opcode::SetTransform as u32,
                size,
                _reserved: 0,
            },
            matrix,
        };

        unsafe {
            let bytes = std::slice::from_raw_parts(
                &op as *const OpSetTransform as *const u8,
                std::mem::size_of::<OpSetTransform>(),
            );
            self.data.extend_from_slice(bytes);
        }
    }

    /// 设置混合模式
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        let size = std::mem::size_of::<OpSetBlendMode>() as u32;
        self.write_header(Opcode::SetBlendMode as u32, size);

        let op = OpSetBlendMode {
            header: OpHeader {
                opcode: Opcode::SetBlendMode as u32,
                size,
                _reserved: 0,
            },
            blend_mode: mode as u32,
        };

        unsafe {
            let bytes = std::slice::from_raw_parts(
                &op as *const OpSetBlendMode as *const u8,
                std::mem::size_of::<OpSetBlendMode>(),
            );
            self.data.extend_from_slice(bytes);
        }
    }

    /// 绘制图像
    pub fn draw_image(&mut self, handle: ResourceHandle, dst_rect: &Rect, src_rect: &Rect, filter: ImageFilter) {
        let size = std::mem::size_of::<OpDrawImage>() as u32;
        self.write_header(Opcode::DrawImage as u32, size);

        let op = OpDrawImage {
            header: OpHeader {
                opcode: Opcode::DrawImage as u32,
                size,
                _reserved: 0,
            },
            image_handle: handle,
            dst_rect: *dst_rect,
            src_rect: *src_rect,
            filter: filter as u32,
        };

        unsafe {
            let bytes = std::slice::from_raw_parts(
                &op as *const OpDrawImage as *const u8,
                std::mem::size_of::<OpDrawImage>(),
            );
            self.data.extend_from_slice(bytes);
        }

        self.update_aabb(dst_rect.x, dst_rect.y, dst_rect.width, dst_rect.height);
    }

    /// 绘制字形运行
    pub fn draw_glyph_run(&mut self, glyph_run: GlyphRunBuilder, font_handle: ResourceHandle, color: ColorU) {
        let size = std::mem::size_of::<OpDrawGlyphRun>() as u32;
        self.write_header(Opcode::DrawGlyphRun as u32, size);

        // ... (实现字形运行数据写入 Data Pool)
    }

    /// 更新 AABB
    fn update_aabb(&mut self, x: f32, y: f32, width: f32, height: f32) {
        let min_x = x.min(self.aabb.min_x);
        let min_y = y.min(self.aabb.min_y);
        let max_x = (x + width).max(self.aabb.max_x);
        let max_y = (y + height).max(self.aabb.max_y);
        self.aabb = Aabb { min_x, min_y, max_x, max_y };
    }

    /// 完成录制，返回 (offset, size)
    pub fn finalize(&mut self) -> (u32, u32) {
        (0, self.data.len() as u32) // offset 由外部计算
    }

    /// 获取 Chunk 数据
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 获取 Chunk ID
    pub fn chunk_id(&self) -> u32 {
        self.chunk_id
    }

    /// 获取 Chunk 类型
    pub fn chunk_type(&self) -> ChunkType {
        self.chunk_type
    }
}

/// 字形运行构建器
pub struct GlyphRunBuilder {
    glyph_ids: Vec<u32>,
    positions: Vec<Vec2>,
    render_mode: u8,
}

impl GlyphRunBuilder {
    pub fn new(render_mode: u8) -> Self {
        Self {
            glyph_ids: Vec::new(),
            positions: Vec::new(),
            render_mode,
        }
    }

    pub fn add_glyph(&mut self, glyph_id: u32, position: Vec2) {
        self.glyph_ids.push(glyph_id);
        self.positions.push(position);
    }
}
```

---

## 6. 实现计划

### 6.1 文件结构

```
novadraw-displaylist/
├── Cargo.toml
└── src/
    ├── lib.rs                    # 主入口，导出公共 API
    ├── header.rs                 # DisplayListHeader, ChunkIndex
    ├── opcode.rs                 # Opcode 枚举
    ├── ops/
    │   ├── mod.rs               # 指令模块
    │   ├── state_ops.rs         # SaveState, RestoreState
    │   ├── transform_ops.rs     # SetTransform, SetClipRect
    │   ├── shape_ops.rs         # FillRect, StrokeRect, FillPath
    │   ├── image_ops.rs         # DrawImage
    │   └── text_ops.rs          # DrawGlyphRun
    ├── dispatcher.rs             # DisplayListDispatcher
    ├── recorder.rs              # DisplayListRecorder
    ├── patch.rs                 # Patch 结构与应用
    ├── manifest.rs              # ResourceManifest
    ├── resource.rs              # ResourceResolver trait
    └── error.rs                 # 错误类型
```

### 6.2 Cargo.toml 依赖

```toml
[package]
name = "novadraw-displaylist"
version = "0.1.0"
edition = "2021"

[dependencies]
bytemuck = { version = "1.14", features = ["derive"] }
glam = "0.30"
thiserror = "1.0"
crc32fast = "1.3"
```

### 6.3 实施步骤

1. **Phase 1**: 基础协议结构 (Header, ChunkIndex, 基础 OpCode)
2. **Phase 2**: 指令实现 (FillRect, StrokeRect, Transform, BlendMode)
3. **Phase 3**: Dispatcher 实现 (零拷贝解析, Send+Sync)
4. **Phase 4**: 资源清单 (独立 JSON/YAML 文件)
5. **Phase 5**: 增量更新 Patch 机制
6. **Phase 6**: 与 novadraw-render 集成

---

## 7. 性能分析

### 7.1 内存布局优化

| 优化点 | 说明 | 效果 |
|--------|------|------|
| 扁平 Buffer | 连续内存，无指针间接 | 提高缓存命中率 |
| 固定大小指令 | 8-byte aligned payload | SIMD 优化友好 |
| Data Pool | 变长数据集中存储 | 减少碎片化 |
| bytemuck::Pod | 零拷贝直接映射 | 消除反序列化开销 |

### 7.2 CPU 缓存优化

```
Cache Line: 64 bytes

┌─────────────────────────────────────────┐
│  OpHeader (16 bytes) + Payload          │  ← 一次缓存加载覆盖完整指令
├─────────────────────────────────────────┤
│  指令跨缓存行? → 保持 8-byte aligned     │
├─────────────────────────────────────────┤
│  连续指令 → 预取器可预测加载             │
├─────────────────────────────────────────┤
│  Chunk 边界 → 可独立预取                 │
└─────────────────────────────────────────┘
```

### 7.3 增量更新优化

| 场景 | 优化策略 | 效果 |
|------|----------|------|
| 局部修改 | Patch 仅包含差异 | 减少传输量 |
| 空间裁剪 | AABB 索引跳过不可见 Chunk | 减少渲染量 |
| 资源预取 | Manifest 提前加载 | 降低卡顿 |

---

## 8. 与现有架构集成

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Novadraw 架构集成                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  novadraw-scene                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  FigureGraph.render()                                            │   │
│  │      │                                                          │   │
│  │      ▼                                                          │   │
│  │  FigureRenderer ──▶ DisplayListRecorder                        │   │
│  │      │               │                                          │   │
│  │      │               ▼                                          │   │
│  │      │         ┌─────────────────┐                              │   │
│  │      │         │   DisplayList   │◀─────── novadraw-displaylist │   │
│  │      │         └─────────────────┘                              │   │
│  │      │               │                                          │   │
│  │      ▼               ▼                                          │   │
│  │  novadraw-render     Dispatcher                                 │   │
│  │  - NdCanvas          - 零拷贝解析                               │   │
│  │  - RenderCommand     - 空间裁剪                                 │   │
│  │                      - 资源解析                                 │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 9. 验证计划

### 9.1 单元测试
- 协议头部解析/序列化
- 指令编码/解码
- AABB 交集计算
- Patch 生成/应用

### 9.2 集成测试
- Figure → DisplayList → RenderBackend 完整链路
- 增量更新正确性
- 空间裁剪正确性

### 9.3 性能测试
- 录制吞吐量 (ops/sec)
- 回放吞吐量 (ops/sec)
- 内存占用对比

---

## 10. 待确认问题

1. **Data Pool 编码格式**: Path 数据是否需要压缩? (lz4/zstd)
2. **资源清单位置**: 存储在头部后还是独立文件?
3. **多线程支持**: Dispatcher 是否需要 Send + Sync?
4. **加密签名**: 是否需要数字签名?
