# DisplayList 探索计划

类型：`proposal`

本文是 [display-list-protocol.md](display-list-protocol.md) 的实验计划，不是当前交付
承诺。只有实验结果通过 ADR 审查，相关能力才进入规范架构。

## 1. 前置条件

开始二进制协议实现前，核心渲染边界必须稳定：

```text
RecordingCanvas
→ CommandStream
→ RenderSubmission
→ RenderBackend
```

其中：

- CommandStream 已能表达现有 Figure paint；
- ResourceHandle 生命周期闭合；
- Damage 与 surface 恢复语义明确；
- Vello direct adapter 可作为性能基线；
- 项目已有第二 consumer 或明确的跨进程需求。

条件不满足时，只完善进程内 RenderSubmission，不创建独立协议 crate。

## 2. 探索阶段

### E1：语义覆盖

目标：证明一个最小 DisplayList 能无损表达当前 command stream。

范围：

- state push/pop；
- Affine2D；
- rectangular/path clip；
- fill/stroke path；
- image；
- glyph run；
- alpha/blend；
- resource declare/reference/release。

输出：

- 命令语义表；
- direct 与 playback 像素对比；
- unsupported capability 清单。

### E2：受检编码

目标：定义内存安全、可 fuzz 的 length-delimited 编码。

要求：

- 不直接把 Rust enum 或 `repr(C)` 当作 wire format；
- 明确 endian、alignment、version 和 limits；
- 所有 offset/length 使用 checked arithmetic；
- unknown optional record 可跳过；
- decoder 不产生未验证引用。

输出：

- encoder/decoder prototype；
- property tests；
- fuzz target；
- malformed corpus。

### E3：资源协议

目标：验证图像、字体和 glyph 资源的 generation 与生命周期。

场景：

- resource pending；
- out-of-order completion；
- replacement；
- stale generation；
- consumer cache eviction；
- producer release；
- missing resource fallback。

### E4：Chunk/Patch

目标：验证增量传输是否有实际收益。

先用普通 immutable chunk，不预设文件格式：

```text
Frame N snapshot
→ identify stable subtrees
→ encode changed chunks
→ apply against exact base version
→ compare final playback
```

必须测试 insert、replace、delete、reorder 和 base mismatch。

### E5：跨后端

至少验证两个 consumer：

- Vello adapter；
- software/reference renderer、remote consumer 或独立 inspection tool。

只有“同一个 Vello 后端的另一条路径”不构成充分的跨后端价值证明。

### E6：2.5D Capability

Projective3D 作为可选扩展：

- 单独 capability bit/version；
- 明确 transform precision；
- projected clip；
- unsupported fallback；
- 不改变二维命令的基础 payload。

真正 Scene3D 不通过扩展二维 DisplayList 指令临时拼接，应另行设计 3D command
protocol。

## 3. Benchmark

固定比较：

| 指标 | Baseline | Candidate |
|---|---|---|
| CPU | direct CommandStream adapter | encode + decode + playback |
| Memory | Rust command storage | binary buffer + indexes |
| Frame size | full command stream | full DL / patch |
| Latency | direct submit | producer to consumer |
| Cache | no retained chunks | retained chunks |

场景至少包括：

- 大量简单 shape；
- 深层 transform/clip；
- 文本密集；
- 图像密集；
- 90% 静态、10% 动态；
- 全场变化；
- Web/WASM memory boundary。

不只报告平均值，还要报告 p95、allocation、峰值内存和 patch miss。

## 4. Crate 决策

在协议未稳定前：

```text
novadraw-render/
└── experimental/display_list/
```

满足晋级条件后，才考虑：

```text
novadraw-display-list       # protocol + checked codec
novadraw-render-vello       # consumer
```

独立发布必须额外具备：

- semver/version policy；
- compatibility tests；
- security policy；
- format specification；
- release ownership。

## 5. ADR Gate

ADR 必须回答：

1. 哪个真实问题不能由进程内 CommandStream 解决？
2. consumer 是谁？
3. benchmark 是否证明收益？
4. 协议是否必须跨语言或跨进程？
5. 资源和 patch 生命周期是否闭合？
6. 如何处理未知命令和版本？
7. 是否会限制 RenderBackend 的原生能力？
8. 维护成本由谁承担？

未通过 ADR 时，实验代码不得成为 Runtime 或 Figure 的依赖。

## 6. 非目标

- 不在探索阶段承诺 crates.io 发布；
- 不承诺 C/C++/Swift ABI；
- 不把零拷贝作为先验目标；
- 不同时设计完整远程渲染协议；
- 不把 Scene3D 塞入二维 opcode；
- 不以已有实验代码反推核心架构。
