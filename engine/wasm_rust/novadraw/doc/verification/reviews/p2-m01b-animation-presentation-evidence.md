# P2-M01B Presentation / Damage 实现记录

类型：`implementation-verification`

日期：2026-10-06

状态：`complete`

## 1. 范围

本切片实现 ADR-026 的表现平面基础：

- Figure opacity 与 node-local transform override；
- committed state 与 presentation state 分离；
- old/new visual envelope damage；
- Runtime-owned、NonInteractive temporary visual；
- hidden、surface suspend、dispose、cancel、completion 与 fault 清理；
- provider sample 非法值与 panic 边界；
- 无 active presentation 时的 O(1) 快路径。

Layout bounds capture、Connection route、Viewport、dash flow 与 moving pulse 属于 M01-C。

## 2. 公开合同

- `FigurePresentationChannels` 为 attached Figure 提供 typed opacity/transform channel；
- `Opacity` 在类型边界保持 `[0, 1]`；
- `InteractionGeometryPolicy::Committed` 是 M01-B 唯一接受的 Figure 策略；
- `TemporaryVisual` 持有 generational Runtime identity，并复用相同 Timeline；
- `SuspensionPolicy::{Advance, Pause, Finish}` 控制 hidden/surface suspend；
- `AnimationState::{Paused, Failed}` 暴露暂停和 provider sample 失败；
- faulted Runtime 拒绝新的动画 mutation。

## 3. 状态与录制

- Figure bounds、style、topology、通知与命中仍读取 committed tree；
- animation sample 只写 `AnimationService` 的 presentation override；
- Runtime 在帧冻结后生成 immutable `PresentationSnapshot`；
- Figure renderer 只读取 snapshot，未改变递归顺序、深度保护或 source state；
- transform override 放宽对应 child 的 committed-bounds 包装裁剪，但保留 parent
  client-area 裁剪；
- temporary visual 在树录制后绘制，不进入 FigureTree、layout、hit-test 或
  accessibility。

无 active timeline 时 `AnimationService::snapshot()` 直接返回 `None`；damage 与
snapshot 都只遍历 active tracks 推导出的 subject，不扫描 FigureTree 或全部已注册
channel。

## 4. Damage 与生命周期

- Figure envelope 汇总目标 subtree 的 committed visual bounds；
- presentation transform 作用后投影到 logical surface；
- 每次视觉变化写入 old/new envelope；
- temporary visual 的激活、移动、完成与取消均覆盖旧新区域；
- dispose 在 subtree 提取前取消相关 owner 并移除 managed channel；
- Disabled/ReducedMotion、hidden Finish、cancel 与自然完成均清除 override；
- surface/hidden Pause 平移 local timeline，不累计不可见期间时间。

## 5. 失败与重试

- prepared track 在 `0 / 0.5 / 1` 采样点执行 admission 预检；
- 运行期非法 sample 将 owner 标记为 `Failed` 并清除所有 override；
- extension panic 进入 Runtime fault boundary，清理 active presentation 与 temporary
  visual 后继续传播 panic；
- backend `Retry` 不回退 animation clock，下一帧重放同一 sampled presentation。

## 6. 合同覆盖

`novadraw/tests/p2_animation_contract.rs` 当前 26 项，其中 M01-B 新增 12 项，覆盖：

1. Figure opacity/transform 只改变录制表现；
2. old/new surface envelope；
3. nested child transform 与 committed clipping；
4. temporary visual 激活、取消与自然完成；
5. Figure dispose 清理；
6. surface Pause；
7. hidden Pause/Finish；
8. Committed interaction 与不支持策略拒绝；
9. provider admission/runtime failure；
10. provider panic fault cleanup；
11. backend Retry 保持当前表现。

## 7. 验证工作流

本切片同时修正验证内环：

- 不在单 target 内环使用 `--tests` 或 `--all-targets`；
- 使用 `cargo check -p novadraw --lib`；
- 使用 `cargo clippy -p novadraw --lib -- -D warnings`；
- 使用 `cargo clippy -p novadraw --test p2_animation_contract -- -D warnings`；
- 功能收口只运行一次 `cargo xtask verify core.p2-m01-animation`；
- `cargo fmt --all` 只在连贯编辑批次结束运行。

本会话发现历史 `novadraw` 包缓存达到 255,153 个文件、44.5 GiB；包级清理前，
单 crate check 曾耗时 50–78 秒且 CPU 大部分时间空闲。清理后的完整依赖冷构建约
40 秒，随后热 `cargo check -p novadraw --lib` 为 1.4–4.4 秒。`cargo clean`
本身删除海量小文件耗时数分钟，因此仅作为异常缓存维护手段，不进入常规门禁。

本切片验证结果：

- `cargo xtask verify core.p2-m01-animation`：26 项通过；
- `cargo clippy -p novadraw --lib -- -D warnings`：通过；
- `cargo clippy -p novadraw --test p2_animation_contract -- -D warnings`：通过；
- `cargo test -p novadraw`：356 项 unit test、全部 integration test 与 doctest 通过。

## 8. 结论

M01-B 完成。M01-C 可以在同一 presentation plane 上增加 capture、bounds、route、
viewport 与 continuous effect，不需要领域私有 clock、damage 或取消语义。
