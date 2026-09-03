# 资源身份与异步完成契约

类型：`normative-design`

本文定义图像、字体等异步资源从 Runtime 注册到 RenderSubmission 的生命周期。

## 1. 身份

资源使用稳定且防陈旧引用的身份：

```text
ResourceId
├── runtime namespace
└── slot key (index + generation)

ImageId(ResourceId)
FontId(ResourceId)
```

namespace 隔离不同 Runtime；slot generation 防止删除后复用槽位时旧 handle 指向新资源。
Figure 只持有 typed ID，不持有解码对象、GPU texture 或字体后端对象。

## 2. 状态机

```text
register → Pending
Pending  → Ready(revision)
Pending  → Failed(reason)
Ready    → Ready(revision + 1)
Failed   → Ready(revision + 1)
any      → Removed
```

- Pending/Failed 使用确定性 fallback；
- Ready transition 产生 resource add/update delta；
- Removed 产生 release delta；
- unknown、namespace mismatch 和 kind mismatch 返回结构化错误；
- worker 不直接修改 Registry 或 FigureTree。

## 3. 依赖

ResourceRegistry 维护：

```text
ResourceId → dependent FigureId set
```

依赖 Figure 删除后，Runtime 在稳定事务边界清理失效引用。资源 Ready、Failed、更新或
Removed 时，所有仍附着的依赖 Figure 均 invalidate + repaint；字体和图像都可能改变
内在尺寸，因此不做只 repaint 的猜测。

## 4. 完成事务

平台 worker 将解码结果作为消息交回 Runtime：

```text
worker result
→ Runtime::complete_image / complete_font / fail_resource
→ ResourceRegistry transition
→ dependency invalidation
→ ResourceDelta queued
→ prepare_submission
→ backend submit
```

Runtime 同一时刻最多允许一个 in-flight submission。若提交失败或要求 retry，该帧
携带的 ResourceDelta 必须恢复到新 delta 之前，保持因果顺序。

## 5. Submission

`ResourceDelta`：

- `added`: `ResourceUpdate { id, revision, payload }`；
- `removed`: `ResourceId`。

payload 是进程内数据，不承诺序列化 ABI。Image payload 使用 RGBA 数据；Font payload
使用字体字节。后端可以缓存或上传，但不得把 GPU 对象泄漏回 Figure。

## 6. 边界

- 本阶段不实现 ImageFigure、Label 或字体 shaping；
- 本阶段不创建解码线程池；
- 平台 adapter 决定如何读取文件、网络或浏览器资源；
- Runtime API 是唯一允许提交完成消息和改变资源状态的入口；
- ResourceRegistry 不使用全局状态或 singleton。

## 7. 验证

- 不同 Runtime 的 ID 不冲突；
- remove 后旧 generation 无法访问新资源；
- kind mismatch 和重复 remove 无副作用；
- Ready/Failed/Removed 都只更新仍有效的依赖 Figure；
- Retry 后 resource delta 顺序和 payload 不丢失；
- Presented 后 delta 不重复提交。
