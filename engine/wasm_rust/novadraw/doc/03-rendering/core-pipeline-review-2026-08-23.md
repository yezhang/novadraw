# 核心渲染管线审查报告（2026-08-23）

## 范围与结论

本次审查覆盖 Figure/递归渲染、Validation/UpdateManager、事件与延迟变更、
SceneHost 以及 Vello 提交链路，共 19 个核心文件。

结论：基础调用链已经形成，但尚未达到“核心通用机制完整”的标准。M5 仍为
`in_progress`，事件与通知仍分别属于 M6/M7 的前置原型。

## 修复状态

| ID | 严重度 | 问题 | 状态 |
|---|---|---|---|
| CP-01 | P0 | 空 damage 与全量重绘语义混淆 | `verified` |
| CP-02 | P1 | 10,000 层合法树可能耗尽递归渲染栈 | `verified` |
| CP-03 | P1 | Figure 回调 invalidate 未同步图状态 | `verified` |
| CP-04 | P1 | `DoNotClipChildBounds` 分支泄漏绘制状态 | `verified` |
| CP-05 | P1 | `reset_clip` 后无法恢复外层裁剪 | `verified` |

## 缺陷详情

### CP-01 空 damage 与全量重绘语义混淆

`SceneHost` 无条件提交 `FigureGraph::perform_update()` 返回的 Canvas。启动、切换场景、
直接平移和 resize 只请求宿主更新，没有加入 dirty region，因此可能得到空命令和空
`DamageSet`。Vello 后端又把空 damage 回退为整窗区域，最终以透明 scratch 覆盖
retained texture。

修复方向：

- 提交协议显式区分 `NoOp`、`Full` 和 `Partial`。
- `SceneHost` 不向后端提交 `NoOp`。
- 首帧、切换场景、直接变换和 resize 显式请求 `Full`。

### CP-02 递归渲染深度与树深度契约不一致

`FigureGraph` 接受深度不超过 10,000 的树，但递归渲染每层会保留多个 Rust 栈帧。
现有深度边界测试只构树，不执行渲染，未证明 10,000 层可安全绘制。

修复方向：

- 保持当前递归渲染主线，不恢复已归档的迭代渲染入口。
- 使用按需分段栈承载深树递归，不改变递归渲染协议。
- 增加 `MAX_TREE_DEPTH` 渲染进程级契约测试。

### CP-03 回调 invalidate 不满足原子契约

`SceneNovadrawContext::invalidate()` 只调用 `UpdateManager::add_invalid_figure()`，
没有把目标节点及 validation 祖先链标为无效。队列消费后会因节点仍为 valid 而直接
返回，Figure validate/layout 不会执行。

修复方向：

- 回调期只记录 invalidation 请求。
- Figure 借用释放后统一调用 `FigureGraph::mark_invalid()`。

### CP-04 无 child bounds 裁剪时状态泄漏

Figure 在 `init_properties()` 之后才 `push_state()`。子 Figure 的 `pop_state()` 只能
恢复到自己的本地属性。`DoNotClipChildBounds` 分支没有恢复父保存点，后续兄弟和父
border 会继承前一个 child 的颜色、字体或透明度。

修复方向：

- 每个 child paint 返回后统一恢复父 client-area 保存点。
- 增加兄弟 Figure 属性隔离测试。

### CP-05 reset_clip 无法恢复外层裁剪

Vello 的 `ResetClip` 会弹出全部实际裁剪层，而 `GraphicsState` 只保存
`clip_depth`。后续 `restore_state/pop_state` 只能恢复数字，无法重建已删除的裁剪
几何。

修复方向：

- Vello `RenderState` 中保存可重放的裁剪描述。
- 恢复状态时由后端重建目标裁剪栈。
- 增加 `outer clip -> push -> set/reset clip -> pop -> draw` 回归测试。

## 非阻断但尚未完整的能力

- M5 尚缺 `GridLayout`、`ToolbarLayout`、`StackLayout`、六布局反向等价测试和
  1k+ Figure stress。
- M6 尚缺键盘、滚轮、drag、hover、double-click、完整 focus/capture 生命周期。
- M7 尚缺六类 listener 的注册解绑和严格因果顺序。
- Vello surface error、零尺寸 resize 和恢复后的全量重绘仍需独立增量收口。

## 验收门禁

```bash
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test
```

定向回归覆盖：

- `damage_mode_distinguishes_none_full_and_partial`
- `first_command_promotes_unspecified_damage_to_full`
- `host_only_request_forces_full_render`
- `manager_work_uses_incremental_update`
- `no_request_is_a_noop`
- `test_scene_dispatch_context_defers_structure_mutation_until_after_callback`
- `test_unclipped_children_restore_parent_graphics_state_between_siblings`
- `clip_restore_plan_replays_saved_outer_clip_after_reset`
- `test_tree_depth_limit_accepts_boundary_and_rejects_next_level_atomically`
