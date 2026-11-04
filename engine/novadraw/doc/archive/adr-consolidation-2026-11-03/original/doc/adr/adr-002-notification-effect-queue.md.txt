# ADR-002: 分层通知与 Runtime effect queue

类型：`architecture-decision`

状态：已通过；2026-09-10 按 [ADR-014](adr-014-extensibility-and-lifecycle-boundaries.md)
修订。原日期 2026-05-06；旧版推导见
[历史快照](../archive/adr-audit-2026-09-10/original/doc/adr/adr-002-notification-effect-queue.md.txt)。

## 背景

Draw2D 提供 Figure、Coordinate、Property、Ancestor、Layout 和 Update 等不同语义；
Zed 的 effect queue 提供延迟执行与借用释放的参考。两者不处于同一抽象层。
Novadraw 保留领域分层，不复制 Java 同步 listener 网络，也不引入完整 Zed Runtime。

## 当前决策

1. Runtime 是唯一 effect/mutation 提交与外部通知 flush owner；UpdateManager
   负责 validation、damage 和 frame preparation 的阶段协议，Host 只做平台适配。
2. 内部 dirty work、damage 与外部 notification journal 分离。内部依赖更新和必要
   lifecycle 不依赖外部 listener 回流。
3. FigureMoved、CoordinateSystemChanged、PropertyChanged、Ancestor、Action、
   Layout 和 Update 按语义分型，不压成一个无类型 event bus。
4. 输入 callback 使用短生命周期 snapshot 和 effect recording，不重入 Runtime。
5. 外部通知在 stable scene 发布后 FIFO flush；相同事件的 listener 保持注册顺序。
   self-removal 和 scope 遵循 ADR-010 及 ADR-014。
6. journal 保存事件发生时的 source revision/epoch、sequence 和必要 old/new 数据。
   flush 查询只读最新 stable scene；历史事件不对应一份可任意查询的历史全树。
7. 延迟的 Validating/Painting 是阶段记录，不是事前拦截 hook。需要影响当前布局的
   行为属于 Layout/组件协议，不能注册 observer 来修改当前事务。
8. 非收敛错误走独立诊断出口；不能等待成功通知 flush 才让 Host 得知失败。

## 时序

```text
source/structural commit and required lifecycle
-> typed derived work
-> stable scene publication
-> damage/recording and preparation metadata
-> external observation flush
-> backend completion produces separate presentation fact
```

场景稳定、录制完成、实际呈现不是同一个事件。无像素变化但有源状态事实时，
不应要求产生 GPU 帧才允许观察已稳定事实。

## 后果

保留 Draw2D 的语义分层，同时拒绝重入和半稳定查询。代价是与同步 listener 的
时机不等价，必须提供历史 payload 和明确的查询 epoch。不能将所有生命周期或
内部一致性代码机械转换成提交后通知。

## 参考

- [通知语义映射](../parity/draw2d/notification-mapping.md)
- [UpdateManager](../design/rendering/update-manager.md)
- [ADR-003](adr-003-rust-runtime-and-geometry-boundaries.md)
- [ADR-010](adr-010-runtime-listener-lifecycle.md)
