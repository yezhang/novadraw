# DisplayList 候选协议

类型：`proposal`

本文讨论可选的序列化 DisplayList。它不是 Novadraw 核心架构前提，不是 Draw2D
语义，也不代表已经承诺二进制 ABI。

核心规范只要求：

```text
Figure paint
→ RecordingCanvas
→ in-process CommandStream
→ RenderSubmission
→ RenderBackend
```

是否把 CommandStream 固化为可持久化、跨语言或可增量传输的 DisplayList，必须通过
本文的验证门禁后由 ADR 决定。

## 1. 候选用例

DisplayList 只在以下用例有明确价值时成立：

- 跨进程或远程渲染；
- 确定性录制与回放；
- 多后端共享稳定命令格式；
- 大型静态子树的 retained command cache；
- 跨语言 consumer；
- frame capture 和离线分析。

如果唯一消费者是同进程 Vello 后端，普通 Rust enum command stream 通常更简单、
类型更安全，也更容易演进。

## 2. 与核心架构的边界

```text
Runtime
→ RenderSubmission
   ├── CommandStream
   ├── Damage
   ├── ResourceDelta
   └── SurfaceInfo
        │
        ├── direct backend adapter
        └── optional DisplayList encoder
```

DisplayList 不能反向定义：

- Figure trait；
- FigureTree 存储；
- layout 或 input 协议；
- damage 的逻辑语义；
- RenderBackend 的平台 surface 生命周期。

## 3. 语义模型

候选协议应优先定义语义，再定义二进制布局。

最低命令族：

```text
State
├── Push / Pop
├── Transform2D
├── Clip
├── Alpha
└── Blend

Draw
├── Fill / Stroke Path
├── Image
├── GlyphRun
└── Layer

Resource
├── Declare
├── Reference
└── Release
```

2.5D projective composition 应作为显式 capability/version extension，不能把所有二维
命令无条件扩展成 4x4 payload。

## 4. 状态模型

候选协议可以采用 stateful command stream，但必须满足：

- chunk 入口状态完整或显式声明继承来源；
- 任意独立回放单元不依赖不可见的前序状态；
- Push/Pop 深度可验证；
- malformed stream 不导致越界读取；
- temporary paint override 的语义与显式 Push/Set/Draw/Pop 等价。

“子 chunk 自动继承父状态”只有在 chunk 拓扑和回放顺序被协议化后才成立，不能作为
未定义假设。

## 5. 资源

资源必须使用 generation-aware handle：

```text
ResourceHandle
├── namespace
├── index
└── generation
```

协议需要明确：

- 图像、字体、glyph atlas 和 shader 的声明；
- 资源缺失时的确定性 fallback；
- 上传完成和释放时机；
- patch 对旧 generation 的引用处理；
- producer/consumer 的资源配额；
- 不可信输入的大小限制。

## 6. Chunk 与增量 Patch

Chunk 不是默认要求。引入前必须证明：

- 场景中存在足够稳定的可缓存边界；
- patch 编码和索引维护成本低于重录制；
- chunk 粒度不会破坏绘制状态和 clip；
- resource dependencies 可独立计算；
- replacement/delete/reorder 具有确定语义。

候选操作：

```text
InsertChunk
ReplaceChunk
DeleteChunk
MoveChunk
UpdateResource
```

每个 patch 必须引用 base frame/version。base 不匹配时拒绝 patch 或请求完整 snapshot。

## 7. 二进制格式要求

只有 ADR 接受二进制协议后，才定义精确 `repr(C)` 或 wire layout。届时必须覆盖：

- magic、major/minor version；
- endian；
- alignment 和 padding；
- 总长度与各 section 边界；
- checked integer arithmetic；
- unknown opcode 跳过规则；
- feature/capability negotiation；
- checksum 的覆盖范围和算法；
- resource/string/path 大小限制；
- fuzzing 和恶意输入模型。

Rust `repr(C)`、`bytemuck::Pod` 和“零拷贝”不能单独构成跨语言 ABI 保证。包含 enum、
padding、指针、平台相关 usize 或未初始化字节的结构都不能直接作为稳定 wire format。

## 8. 版本兼容

建议：

```text
major mismatch → reject
minor extension → skip length-delimited unknown records
required capability missing → reject with reason
optional capability missing → defined fallback
```

协议升级必须同时说明 producer 和 consumer 的兼容矩阵。

## 9. 性能假设

以下结论都需要 benchmark，不能直接写成设计事实：

- binary 一定比 Rust enum 更快；
- zero-copy 一定减少总成本；
- stateful command 一定更小；
- chunk patch 一定优于重录制；
- 多 region replay 一定减少 GPU 工作；
- 跨后端统一指令不会损失特定后端能力。

测试至少比较：

- encode/decode 时间；
- allocation 和 peak memory；
- full frame 与 patch 大小；
- Vello direct adapter 与 DisplayList playback；
- 静态/动态混合场景；
- text/image/resource-heavy 场景；
- Web/WASM 上的拷贝成本。

## 10. 安全

从文件、网络或其他进程读取时，DisplayList 是不可信输入：

- 所有 offset/length checked；
- 限制递归、state stack、chunk 和 resource 数量；
- 验证浮点数；
- 禁止任意 shader 或明确 sandbox；
- checksum 只检测损坏，不替代真实性验证；
- decoder 失败不得产生部分 GPU 提交。

## 11. 晋级条件

本 proposal 只有同时满足以下条件才能通过 ADR 升级为 normative design：

1. 至少两个真实 consumer，或一个明确跨进程/持久化用例；
2. 与直接 CommandStream 的 benchmark；
3. 完整资源生命周期；
4. fuzz 和 malformed input 测试；
5. 版本兼容策略；
6. 2D 和可选 Projective3D capability 设计；
7. 与 Damage/RenderSubmission 的边界验证；
8. 明确维护和发布责任。

在此之前，核心架构不得依赖固定二进制布局。
