# Shapes 编辑器剩余功能实施与验证

类型：`implementation-verification`

2026-10-28。依据已批准的[官方样例对等方案](../../design/editor/gef-official-examples-parity-proposal.md)
和用户“继续完成 Shapes 样例的剩余功能实现”的授权。
本轮完成 Shapes 功能实现及自动验证，完整 Native 人工验收仍保留未完成状态。
执行队列只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 功能与所有权

Editor 新增 `CreationTemplate<A>`、CreationTool、MarqueeTool、typed drop、
批量受检 selection 和应用命令/保存点入口。内置与外部能力使用同一 typed
identity、RequestProtocol、policy registry 和 CommandStack，不新增业务枚举。
契约见 [ADR-054](../../adr/adr-054-template-creation-and-marquee.md)。

Masonry 增加 typed 模板源、稳定 action 去重、应用键盘回调、滚轮/中键平移和
auto-expose 调度。预览保留在 widget/session，不因 pointer move 重建外层 UI；
新文档身份即使快照值相同也会发布。命令错误不会因成功绘制而消失。
失焦、禁用和取消按上游实际 capture target 清理，不依赖延迟更新的 active 标记。

Shapes 应用拥有节点/边模型、几何策略、删除恢复命令、JSON schema、文件服务、
Palette、Outline 与属性表单。主容器采用公开 FreeformLayer，超出视口的节点参与 extent；
Router 的障碍身份在节点集合变化时替换，几何变化走现有 Runtime 失效。
连接重连时恢复 locator 绑定，目标箭头和连接身份沿用原 visual。

文件写入先创建同目录临时文件、写入并 sync，再 rename；成功后标记保存点。
打开新模型重建会话和历史，失败保留原文档。拒绝重复 JSON ID、重复顺序、
模型/边身份冲突、自连接、重复有向边、缺失端点、非法几何和未知版本。
应用文档不序列化 Runtime/Figure/EditPart ID。

## SH-01～SH-10

分母来自[官方样例分析](../../reference/gef/official-examples-analysis.md)；
Java 仅作静态源码参考，不编译或运行。

| 场景 | 本轮自动证据 |
|---|---|
| SH-01 工厂与投影 | 两种模板生成对应模型与 Figure；Chopbox/Ellipse Anchor、目标箭头及 GPU 图元 |
| SH-02 约束编辑 | Widget/headless 拖动一致；多选缩放一次 undo/redo；DPI/resize 期间 source/history 保留 |
| SH-03 模板拖放 | 实际 Xilem click/drop、跨 widget capture；独立外部 RequestProtocol 的创建工具与 drop 共享历史 |
| SH-04 连接创建与拒绝 | 实线/虚线、拒绝自连接与重复有向边，线型不能绕过拒绝；未完成连接取消 |
| SH-05 锚点与路由 | 障碍物创建、移动、删除后路径更新；端点移动/缩放后椭圆边界断言与矩形边界断言 |
| SH-06 重连 | 源和目标重连、undo、端点集合、稳定连接身份与目标箭头位置恢复 |
| SH-07 删除恢复 | 多节点与边同批删除、关联边清理、undo 恢复模型身份、节点 Z 顺序和关系；redo |
| SH-08 生命周期 | rebuild 保留会话，teardown 移除 action source；禁用/失焦/取消不提交；重新打开获得新 namespace |
| SH-09 Outline/Geometry | 双向选择投影，几何经命令提交，错误与历史保留；初始和新文档稳定投影 |
| SH-10 文件和保存点 | Save/Save as/Open、编辑/撤销保存点、unsaved 拒绝、显式丢弃打开与文件失败恢复 |

补充检查包括 100 次 hover 不发布外层 action、拖动反馈不发布而提交发布、
双 DPI 滚轮/中键平移和焦点内 Delete/Undo/Redo、模板取消与禁用键盘不创建。
GPU harness 复用上游输入/布局，绘制使用真实 WGPU imaging_vello，
没有用上游 CPU 截图替代 GPU 证据。

## 验证

受影响 crate 的完整测试通过：Shapes 最终 21 项通过、2 项性能采样按设计忽略；
Editor 的 G1～G5、Direct Edit 和外部 template/marquee 消费者通过；
Masonry 4 项通过。Shapes 包含滚动期间坐标一致性与创建/框选 auto-expose。
workspace fmt、严格 Clippy 和 check 通过。

`cargo test --workspace --exclude xtask` 通过。完整 workspace test 已执行的代码测试
也全部通过，最后在 xtask 文档测试处停止。
失败来自本任务外的长文本报告：该报告引用
`target/verification/long-text/scroll-resize-shared-20261028/hot-comparison.json`，
而 `target/` 被外部进程清理。没有为通过门禁伪造历史证据，也没有改动该无关报告。

构建期间常规 `target/` 被外部进程清理两次。本轮最终构建与采样改用
`/private/tmp/novadraw-shapes-final-20261028`，没有清理或复用其他会话缓存。

```sh
cargo +1.96.0 test -p novadraw-editor -p novadraw-platform-masonry -p gef-editor-demo -- --test-threads=1
cargo +1.96.0 clippy -p gef-editor-demo -p novadraw-platform-masonry -p novadraw-editor --lib -- -D warnings
cargo +1.96.0 check --workspace
cargo +1.96.0 test --release --locked -p gef-editor-demo --lib cost_sample -- --ignored --nocapture --test-threads=1
```

## 成本

依赖仍固定到未修改上游
`66c8893f12232b9d5cdf4296f773aa3dc07ee776`，Rust 1.96.0。
新增序列化依赖复用 workspace 的 serde/serde_json，不带新字体或图片资源。
macOS arm64、Rust 1.96.0、同一 release 配置与锁文件下：

| 产物 | 原始字节 | gzip-9 字节 |
|---|---:|---:|
| UI shell | 9,975,088 | 3,923,670 |
| Shapes editor | 15,129,728 | 6,137,660 |
| 增量 | 5,154,640 | 2,213,990 |

640×480、DPI=1。每项执行 1,020 次，丢弃前 20 次，统计 1,000 次：

| CPU 路径 | p50 ms | p95 ms | max ms |
|---|---:|---:|---:|
| 四次 pointer event | 0.073 | 0.119 | 0.704 |
| Scene preparation | 0.043 | 0.066 | 0.216 |
| 障碍命令 + shortest-path 路由 + Scene | 0.147 | 0.197 | 1.286 |

第一项包含按下、移动、释放和一次额外定位，循环后执行 undo；第二项是其后的
recording 准备。路由场景有两个端点、一个矩形障碍和一条连接，每轮修改障碍、
重算路径、准备 Scene，再 undo。采样不包含 GPU 光栅化、surface present、
窗口交付、RSS 或 GPU memory，也不能外推为 Web 传输成本或正式性能验收。

## 待验收边界

本轮 SH 分母的自动契约不能代替原生完整人工交互。当前 release 二进制已从
`/private/tmp/novadraw-shapes-final-20261028` 启动，进程和前台激活成功；
当前环境没有桌面操作工具，交互未核验。跨显示器 DPI、窗口外 capture、完整 IME、
系统 AT 和其他平台 runner 保持独立未完成范围。普通模板转移使用窗口逻辑坐标，
不宣称系统级跨窗口 DnD。

文件交互采用可编辑路径框，不宣称系统文件对话框；格式是应用 JSON，
不兼容原 Java 对象序列化。Shapes 不加入官方样例没有的手动折点。
imaging 目标的逐操作 AA / 原生 glyph-run 限制沿用[迁移记录](masonry-upstream-migration-2026-10-28.md)。
