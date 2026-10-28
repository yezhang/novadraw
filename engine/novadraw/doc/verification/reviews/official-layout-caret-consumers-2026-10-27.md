# 布局与 Caret 消费者核验

类型：`implementation-verification`

日期：2026-10-27

范围：用户批准的 K02/K05 消费者核验。先对照只读 Draw2D 源契约，再运行真实
文字反例和官方 Canvas 工厂，发现通用问题后修复 Core。本批不关闭完整入口，
不启动另一条长文本实施线。执行状态由 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

`api_semantics`：`layout.manager`、`text.interaction`、`text.flow`、`damage.repaint`。

## Caret 选区不能截断组合字素

官方 `CaretExample.showCaret` 同时消费 offset 和 trailing；`TextFlow.paintSelection`
实际分支中的 end 处理不能只由 inclusive 注释推断。Core 保留 UTF-8、grapheme
和 affinity，Java UTF-16 的坐标单位不进入 Core。

原样例用 `chars().next()` 计算选区终点。官方文本中的 `םֱ`、`מֱ`、`صً`、`ثُ`、
`سٍَ` 带组合附加符；反例在 `hover_selection` 取得 `InvalidTextPosition`，
绘制随后跳过选区。普通 trailing 命中也会从边界开始错误选择后一个字素。

`TextInteractionMap::grapheme_range_at` 复用既有 Unicode 分段，从合法 caret
position 查询完整逻辑字素。Downstream 取后一个，Upstream 取前一个；对应源边缘
返回 None。返回端点携带同一版本并接受可见范围校验。样例先通过快照验证来源和
版本，再在公开 map 中查询并映射回来源，不自行实现字素算法。

外部 provider 和内置 provider 共用这条查询。已有测试覆盖 ASCII、组合附加符、
多字节 emoji、ZWJ emoji、阿拉伯文、源边缘、非法端点和过期版本。
样例保留透明蓝色 alpha 高亮；这与上游 white/XOR 是明确的绘制变体。
本批没有恢复 XOR，没有宣称全部 UTF-16、字体和原生栅格逐像素等价。
契约补充见 [ADR-042](../../adr/adr-042-structured-flow-layout.md)。

## Grid 必须按分配后的列宽重新测量

只读 Draw2D `GridLayout` 在分配列宽后，对没有 height hint 且宽度变窄或 Fill
宽度改变的 child 再调用 `computeSize`，然后求解行高。
原 Core 只按初始自然尺寸求解行高。补充 Label/FlowPage 消费者后，窄宽反例
出现文字快照已重排、父级仍提交旧高度的情况。

Core 现在共用 `allocated_tracks`：列宽分配、受约束 child 重测、行高求解。
公开 preferred/minimum measurement 与实际 layout 读取同一阶段结果。
显式 height hint 保持权威；span、indent、grab、等宽整数分配和 minimum=preferred
保留既有语义。计算只产出待校验 LayoutOutput，测量失败不提前写入 child bounds。

补充文字消费者覆盖 Flow、Toolbar、Grid 的自然宽度、100/40/0 可用宽度、
恢复宽度、Label minimum 和 FlowPage 重排。Grid 还覆盖双列 span、indent、
显式 height hint 与窄宽下 parent measurement 包含完整 child 高度。
这些是真实文字类型的额外消费者，不能冒充原官方 Shape 矩阵本身。

## GPU 证据必须记录真实提交模式

共享 `GpuCanvas` 使用 `VelloTextureRenderer`，该后端声明 FULL_FRAME_ONLY。
不主动请求 Full 也会提交 Full；其连续图像比较不能证明 retained-partial。
本批增加实际 damage mode 观测，并保留该工具的 full-frame 比较口径。

`layout-consumer-proof` 复用现有 tree-animation-proof 的 winit 启动模式，
通过 `VelloRenderer` 的真实 retained target 验证官方 Native panel Canvas，
避免测试配置下 Flow/Toolbar 的旧 mock 控件工厂。
同一 Runtime/renderer 连续提交；每个更新帧与随后强制 Full 的 RGBA 图像逐像素相等。

| 场景 | 注入缩放 | 更新帧 | 实际模式 |
|---|---|---:|---|
| Flow/Grid/Toolbar 视口 820→760→620→440→300→620→820 | 1/2 | 42 | Full |
| Flow/Grid/Toolbar child 隐藏和恢复 | 1/2 | 12 | Partial |
| Caret 三次真实鼠标移动 | 1/2 | 6 | Partial |

60 组对照通过，其中 18 组实际 Partial。所有场景都断言至少发生一次 Partial，
不能用 backend capability 声明代替实际提交模式。
`frames.csv` 保留全部 60 次比较，每个场景和注入缩放保留最后一对 actual/full 图像。
证据已从本机临时目录保存到 `target/verification/official-layout-caret-consumers/`，
同目录 manifest 记录提交模式计数与文件散列。
通过 NOVADRAW_CONSUMER_EVIDENCE 可指定后续输出目录。

该入口调用 `render_for_screenshot`，不获取窗口 drawable。证据证明实际 retained
纹理的重绘与擦除；连续窗口 resize 事件、surface present、实际跨显示器 DPI、
Windows/Linux 和浏览器像素仍未验收。1/2 为注入 scale，不代表系统 DPI 切换。

```sh
cargo test -p novadraw --test p2_t02_text_interaction_contract --test p2_grid_measurement_contract --test m5_layout_contract
cargo test -p draw2d-official-examples --lib layout_consumer_contracts
cargo test -p draw2d-official-examples --lib text_interaction::tests
cargo run -p draw2d-official-examples --bin layout-consumer-proof
```

## 成本与后续边界

没有新增依赖、字体或常驻 GPU 资源。字素查询沿既有准入机制线性扫描文本，
Grid 只在分配宽度要求重新测量且无 height hint 时执行额外测量。
本批是正确性整改，没有采集新的性能前后对比或发布产物差额，不宣称性能提升。
长文本追加、可见输出和生产后端的预算继续由原 LT 批次维护。

入口计数仍为 16 complete、9 in_progress；字体、主题、连续窗口和平台验收保留。
历史 K02 与官方功能审计保持原始快照，本页记录新增证据。

## 本批门禁结果

| 门禁 | 结果 |
|---|---|
| workspace fmt / check | 通过 |
| Core 与样例 `--lib` 严格 Clippy | 通过 |
| `cargo test -p novadraw` | 1,178 passed、0 failed、3 ignored，含 doctest |
| 官方样例库级回归 | 97 passed、0 failed、7 ignored |
| 补充离屏布局 GPU 测试 | 独立执行通过，明确为 full-frame |
| Native retained 验证入口 | 60 组对照通过，18 组实际 Partial |
| Native/Wasm API 与架构边界 | 通过，Runtime methods 28/28 |
| 文档引用门禁 | 410 个 Markdown 与当前命令引用通过 |
| 中文记录机械扫描及回读 | 半角标点、破折号、空泛元话语和编号标题命中均为 0 |

忽略项没有自动计入通过；本批离屏布局 GPU 用例和 Native retained 入口已分别执行。
已查看 Caret 的实际图像；选区范围、边界和版本由契约断言验证。
上述结果对应本批收尾版本，不覆盖之后的长文本或其他并行改动。
