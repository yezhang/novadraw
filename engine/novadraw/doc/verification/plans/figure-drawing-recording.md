# Figure 局部绘制录制验证矩阵

类型：`verification-plan`

能力归属：Core 负责 typed provider、Graphics 状态、证明、命令与留存；
Vello backend 负责实际命令消费和像素；测试 host 负责呈现／拒绝反馈。
Editor 和应用功能不计为本片新增能力。

日期：2026-11-06。状态：验证计划候选，未执行，待与
[具体录制协议](../../design/architecture/figure-drawing-recording.md)独立复核。
[既有混合像素证据](../reviews/figure-tree-projection-implementation-2026-11-05.md#混合-headless-像素与路由恢复)
只覆盖投影，不满足本矩阵的录制与复制要求。
执行状态只由 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

`api_semantics`：`paint.protocol`、`graphics.context`、`clipping.strategy`、
`border.protocol`、`damage.repaint`、`frame.preparation`、`render.backend_session`。

## 正确性与外部消费者

独立消费者 crate 只依赖公共 Core API，提供自定义 typed payload 和 reader。
混合容器含 shape、image、text、child、border，以及读取状态／产生错误的未知 painter。
对照使用同一输入、依赖和 backend 的 live callback，不借旧 cache 构造 oracle。

| 编号 | 触发与反例 | 必须观察的结果 |
|---|---|---|
| DR01 | 同一外部 binding 在相同状态下 cold／warm 绘制 | warm 省略 opt-in callback，未知 callback 仍调用；全部像素与 live 相等 |
| DR02 | 改 transform、clip path、opacity、paint、stroke、AA、font 或局部 bounds | 对应完整进入 proof miss；重录后的状态与像素相等；相同 clip 包络不能掩盖不同 clip |
| DR03 | 平移、嵌套 pan、有限奇异变换、段内 SetTransform | 不重复应用进入映射或 opacity；平移后允许 miss，不以强制命中作为首片门禁 |
| DR04 | self／child／border 与未知 painter 交错、clip 越界墨迹 | 顺序、外层隔离和 client-area clip 一致；未知 overflow 仍 Full |
| DR05 | owned payload 代次变化、条件输入 A→B、嵌套输入变化但外层值相等 | 正确 miss／传递检查，新读取集原子替换；旧依赖随旧消费者退出 |
| DR06 | 返回前或最终验证时共享首次值改变、reader 退休、忽略错误、输入环 | 首错拒绝帧，不以重读替换首次值；新操作可在修正后恢复 |
| DR07 | 图像／字体相同 id 不同 revision、资源删除与再准入 | 当前闭包验证；非法资源显式失败，不使用旧 lease 绕过 ready 状态 |
| DR08 | 段内多 Pop、越 floor Restore、未平衡 Push、先 Graphics 错再 cleanup 错 | 保留最先原因，不安装部分段；后续环境与恢复前缀正确 |
| DR09 | 未知 painter 读取累计文字统计；注册 payload 产生非零统计增量 | 非零增量段不取得回调省略资格；只执行一次，统计仍代表实际 CPU selection |
| DR10 | registry 替换、绑定 clone、卸载／reparent／退休及槽位复用 | 完整身份检查；旧 binding 不变成新注册，lease 不提前释放 |
| DR11 | A Presented，B 录制失败或 Skipped，C 修正／重新 Present | 保留 A，C 修复旧位置及 overflow；未呈现 B 不晋升显示前缀 |
| DR12 | backend session 更换／device 恢复、丢失资源 Snapshot | 明确接收或拒绝；资源基线与 Full 恢复仍执行，Core cache 不省略恢复 |
| DR13 | shared chunk 包含 backend 不支持的操作或缺资源 | Native、离屏、测试 backend 在修改输出前统一拒绝，不漏扫 shared chunk |
| DR14 | 独立 CommandRecorder 使用同一 binding，无 Runtime scope | 执行真实 callback；需要 unavailable reader 时显式失败，不访问 Runtime 旧 cache |

DR01–DR04 使用全部 RGBA 对照，并验证真实非空 glyph 与 image 输出。
文字允许保持 live，不把混合场景存在文字解释为文字 callback 已获复用。
DR06、DR08、DR11 核对错误 source、旧输出和下一次成功恢复，不只断言返回 Err。
Native／Web 的 backend 消费与恢复入口分别检查；没有平台证据时保留待验。

## 留存、复制与成本

每项在改变行为的边界计数，而不是事后按 callback 数估算。
计数关闭后的净成本另采样，不用 instrumentation 耗时证明生产收益。

| 编号 | 场景 | 证据与通过条件 |
|---|---|---|
| DR15 | 克隆 stream／submission、多帧命中 | command／path／glyph backing 身份保持；深复制字节为零，引用复制与分配单独记录 |
| DR16 | command capacity、共享完整 glyph buffer、gradient／dash、资源与证明 | 容量账本包含完整 backing；共享 slice 小不能降低实际 pinned storage 计费 |
| DR17 | 接近 cache／in-flight／scratch／条目上限，同时保留旧、新与恢复帧 | 先预留峰值；超限显式失败或仅放弃 cache entry，不无限展开；释放最后 owner 才归还容量 |
| DR18 | 淘汰、reader 退休、失败候选与 submission clone 依次释放 | 无泄漏与提前释放；旧输出仍可消费；独立留存责任与去重 owner 明确 |
| DR19 | cold、warm、每类 state miss、淘汰重建、设备恢复 | 记录 paint 调用、command／glyph 生成、深复制字节、段引用、分配、retained／peak bytes |
| DR20 | 小树、混合大树、长文字、连续 pan／拖动／resize | 关闭 tracing 的相邻版本 active／frame／latency P50／P95／P99，保留回退；不承诺全树恒定成本 |

采样分别记录 preparation、proof、录制、组装、backend 编码与呈现反馈。
backend 仍遍历命令时应如实报告，不把 Core 零复制写成 GPU 或整帧零成本。
长文字统计限制导致 callback 保持 live 时，也必须报告实际生成成本。

源码、Cargo.lock、构建配置、二进制、操作摘要、像素、原始样本与 SHA-256
收据放在 `target/verification/figure-drawing-recording-<date>/`。
对照两侧除录制切片外保持一致；无法冻结相同依赖时拒绝作独立归因。
原 workset 基线缺失时不替代总体成本门禁，单独记录当前切片的比较范围。

## 分片门禁

先通过容器及所有 backend 消费／拒绝路径，再接入 provider 缓存。
精确测试和受影响 crate 的 lib／单 test Clippy 覆盖内环；
公开容器迁移完成后运行 workspace quick gate，最终批次才运行完整边界门禁。
文档候选本身只执行文档检查，不将本矩阵标为已通过。
独立复核接受合同不代表上述反例已执行，也不代表 FR-02 或生产开关关闭。
