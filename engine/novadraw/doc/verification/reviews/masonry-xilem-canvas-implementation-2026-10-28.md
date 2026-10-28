# Masonry / Xilem 画布与 Shapes 首批实施

类型：`implementation-verification`

2026-10-28。授权：“好的，记录这个方案，然后开始实施”及“继续实施吧”。
契约：[GEF 对等方案](../../design/editor/gef-official-examples-parity-proposal.md)、
[画布接入](../../design/editor/masonry-xilem-canvas-integration.md)、
[ADR-053 Scene 目标](../../adr/adr-053-embedded-vello-scene-target.md)。
这是实际消费者的阶段记录，完整 Widget bridge 和官方 Shapes 保持未完成。

## 实施

[`novadraw-platform-masonry`](../../../crates/novadraw-platform-masonry/README.md)提供：

- EditorSession：Viewer、EditorDomain、VelloSceneRenderer 与缓存 Scene；
- EditorCanvas：content-box 坐标输入、capture、取消、layout clip、动画回调驱动；
- CanvasView：Send+Sync 描述及 build/sync/action 回调，rebuild 不重建会话。

[`gef-editor-demo`](../../../examples/native/gef-editor-demo/README.md)的业务模型、
Figure projection、typed ChangeBounds/CreateChild/DeleteParts policy 与 Command
留在样例。Xilem toolbar 只入队动作，sync 消费一次；main 是 composition root。
矩形/椭圆、选择/拖动、创建/删除、undo/redo 的底层机制均走公开 Editor API。
没有修改 Core/Editor 来区分内置与外部消费者。

两包使用独立 workspace/lockfile；默认引擎 workspace 中 UI 包数量为 0。
依赖树中只有 Vello 0.10.0、WGPU 29.0.4 各一个版本。
固定 Xilem 源码与局部变更见 [原兼容副本记录](../evidence/masonry-vendor-baseline-2026-10-28.txt)，
原 Xilem、GEF 和 Java 参考源码没有写入或运行。

paint 只合成缓存 Scene。输入、layout、rebuild、DPI 更新及 Retry 请求一次动画
回调，回调 prepare → backend submit → complete，读取 Runtime recovery decision。
编码失败保留旧 Scene并发送诊断；Scene Presented 不代表窗口 present 成功。
普通画布不接受 IME，不声称已有节点级 accessibility。

## 消费验证

六项测试通过，入口为
[`src/tests.rs`](../../../examples/native/gef-editor-demo/src/tests.rs)。
[原始日志与截图](../evidence/masonry-xilem-2026-10-28/)包含成功与失败证据。

| 范围 | 结果 |
|---|---|
| Widget 与 headless 输入 | 相同拖动经同一 policy/command 更新到相同 bounds；仅一个 undo entry |
| Xilem rebuild | Undo/Redo 后业务 bounds、Widget ID、Editor namespace、Runtime namespace 和 history 保留；空队列不重复提交 |
| 消息与 teardown | 正确 typed action 路由，错误类型返回 Stale；teardown 移除 action source，重插入分配新 Widget ID |
| capture / lifecycle | 主指针捕获和释放通过；禁用、窗口失焦取消反馈，不修改模型或 history |
| 创建/删除 | CreateChild / DeleteParts 经同一 CommandStack，可依次 undo |
| 实际 toolbar 消费 | 对真实 Button action 做 message/rebuild，再点击 Undo，模型恢复且 toolbar 资格更新，无反馈循环 |
| GPU | DPI=1/2 下初始、移动与 undo 的内部像素及旧位置擦除通过；真实 toolbar 全场景合成通过 |

GPU harness 使用真实 WGPU 纹理及 Vello，沿用上游 `use_cpu=true` 的混合路径；
不将这些结果外推为原生 surface present 或全 GPU compute 验收。
上游 harness 原本没有 Native runner 的最终 DPI 变换。已在兼容副本补齐该变换，
其 window_size、mouse_move 仍为物理像素；测试据此显式输入物理尺寸和坐标。
失败用例没有通过降低 DPI 或像素标准绕过。

检查入口：

```sh
cargo test --manifest-path examples/native/gef-editor-demo/Cargo.toml --lib -- --test-threads=1
cargo clippy --manifest-path examples/native/gef-editor-demo/Cargo.toml --lib --bin gef-editor-demo --bin ui-shell-baseline -- -D warnings
cargo clippy --manifest-path crates/novadraw-platform-masonry/Cargo.toml --lib -- -D warnings
```

consumer 与 adapter 严格 Clippy 通过。未运行无关 workspace 全量测试。
文档全局门禁在并发长文本报告
`long-text-scroll-resize-implementation-2026-10-28.md` 的 `review-report` 类型处失败，
不改写该其他会话的文件，不宣称全局门禁通过。

## 有限成本测量

本地 macOS / arm64，rustc 1.94.1，release 默认 opt-level=3、strip=symbols，
同一 demo manifest、lockfile 与 target 构建 editor 和 ui-shell-baseline。
baseline 只保留 Xilem window/label，editor 增加 Core/Editor/Scene 与样例链路。
这是 Native 应用产物增量，不能作为 Web 下载成本或仅 Widget API 的边际成本。

| 产物 | 原始字节 | gzip-9 字节 |
|---|---:|---:|
| UI shell | 9,773,728 | 3,865,656 |
| Shapes editor | 13,381,824 | 5,399,111 |
| 增量 | 3,608,096 | 1,533,455 |

1020 次固定拖动/撤销循环，丢弃 20 次预热，保留 1000 次。画布 640×480、
两形状、DPI=1，不包含 GPU、surface present、窗口事件交付或首次启动时间。
计时分别覆盖四次 pointer event 与一次动画回调 Scene 准备，未把两项 p95 相加。
后台构建可能影响尾延迟；单机单次结果不作为正式性能 SLA。

| CPU 路径 | p50 ms | p95 ms | max ms |
|---|---:|---:|---:|
| 四次 pointer event | 0.089 | 0.173 | 0.871 |
| Scene 准备 | 0.055 | 0.094 | 0.659 |

采样入口：

```sh
cargo test --release --manifest-path examples/native/gef-editor-demo/Cargo.toml --lib input_and_scene_cost_sample -- --ignored --nocapture
```

没有 RSS/GPU memory、Web 产物或冷启动分布证据。
测试字体只属于 dev dependency，Native UI 正常运行使用宿主字体。

## Native 首帧仍未验收

实际应用两次启动均建立了屏幕上的 `Novadraw — Shapes editor` 窗口，
但窗口截图为白色内容区；不能称为完整启动成功或用户可用画面。
离屏成功截图与 Native 失败截图分别保存，不能互相替代。

代码证据显示 WGPU 29 Metal 会在 NSWindow 尚未 visible 或被遮挡时返回 Occluded，
上游宿主在显示前尝试首帧。第一轮后补了 show 后的 redraw，第二轮仍空白；
最终又补了 Occluded(false) 后重新请求 redraw。该最终改动已进入编译检查，
尚没有它的实际窗口复验，不认定根因闭环。

本轮已使用两次 startup 尝试，结束了自己的失败诊断进程，没有保存未经健康首帧
验证的 Run action，原 `.trae/environments/run.json` 保持原有设置。
下一次 Native 验证先确认该 exposure 回调的 surface publication，再验证鼠标拖动、
undo 与 resize/DPI；无需修改其他会话的应用或停止它们的进程。

## 剩余范围

完整 IME/text-input bridge、系统 AT、窗口外 capture、自动 expose、
CreationTool、marquee、模板拖放和连接交互继续按批准方案实施。
本次不调整完整官方样例的能力 denominator，也不升级 GEF parity ledger。
状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
