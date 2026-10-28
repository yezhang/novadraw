# 项目代码质量评审

类型：`architecture-review`

日期：2026-10-26

基线：`bf6409d0cb9ef65aa47091d8d4b3d07b3a1cd973` 加评审时工作区改动。

## 结论

项目的领域分层、类型化身份和失败隔离已有清楚的实现，但输入校验仍不一致。最应优先处理的是合法缩放输入触发 panic、旧 SVG 解码绕过预算，以及无效图像被提交为 Ready。这些问题无法靠格式检查和 Clippy 排除。

注释并非普遍不足。坐标域、扩展协议、预算和命令历史的关键边界有解释；薄弱处集中在公共方法的失败条件、少量过时注释，以及几个承载多种职责的大文件。建议补充不变量和失败契约，避免只增加复述代码的注释。

本次记录 8 项发现：3 项 P1、4 项 P2、1 项 P3。P1 应在面向外部输入的交付前处理；P2 按消费场景和改动边界排期；P3 属于维护改进。未发现可确认的任意代码执行、越权或内存破坏漏洞。依赖漏洞扫描未执行，不能据此认定依赖安全。

## 范围与证据边界

本次允许扫描现状实现，以 [CLAUDE.md](../../../CLAUDE.md) 为项目规则依据，结合已提供的项目记忆核对术语和边界。

对七个一方库的 `src/` 做了文件规模和危险模式检索，深入抽查 Core 缩放、图像、资源、图布局、递归渲染，Editor 直接编辑和命令历史，Vello 缓存与截图，以及 Winit/Web 宿主。另查看了示例宿主和官方样例的工作区 diff。未逐函数审阅全部代码，未扫描第三方源码、归档实现和依赖源码。

| 库 | Rust 文件 | 总行数 |
|---|---:|---:|
| Core | 171 | 91,374 |
| Editor | 16 | 13,935 |
| Inspector | 1 | 203 |
| Vello 后端 | 7 | 5,671 |
| Winit adapter | 7 | 1,925 |
| Web adapter | 7 | 1,974 |
| winit-canvas | 5 | 1,199 |

共 214 个文件、116,281 行，包含内联测试。行数和注释数量只用于定位审查重点，不作为质量评分或注释充分性的证明。

评审期间，其他工作继续修改官方样例与文档。因此这里记录的是检查时的结果；交付或提交前仍需对最终快照运行相应门禁。本次没有修改生产代码或创建 Git 提交。

## 发现清单

| 编号 | 优先级 | 发现 | 证据强度 |
|---|---|---|---|
| CQ-01 | P1 | 合法缩放值导致测量 panic | 最小 Rust 探针复现 |
| CQ-02 | P1 | 旧 SVG 解码入口绕过像素预算 | 对照探针和调用链确认 |
| CQ-03 | P1 | 图像完成入口接受无效像素和 scale | Runtime 公共入口复现 |
| CQ-04 | P2 | 截图吞掉 GPU 等待错误并对映射失败 unwrap | 静态调用链确认，未注入设备故障 |
| CQ-05 | P2 | 资源缓存同步重复扫描，并复制图像像素 | 静态复杂度确认，未测量延迟 |
| CQ-06 | P2 | Web 输入状态先变更，DOM 操作失败无反馈 | 静态调用链确认，未做浏览器故障复现 |
| CQ-07 | P2 | 公开身份和逆变换仍保留旧失败表达 | 公共编译探针及签名确认 |
| CQ-08 | P3 | 注释门禁大范围豁免，部分注释与签名矛盾 | 源码确认 |

### CQ-01：正数缩放不保证测量运算有限

位置：[scalable.rs](../../../crates/novadraw/src/container/scalable.rs)，18–19、219–225、381–387 行。

`valid_scale` 接受所有有限正数。两个缩放容器随后把测量上限除以 scale，并断言结果仍然有效。`100.0 / f64::MIN_POSITIVE` 溢出为正无穷，`MeasureConstraints::new` 拒绝它，接着 `expect` 触发 panic。

最小探针使用 `ScalableLayeredPaneFigure::with_scale(f64::MIN_POSITIVE)`，再调用 `Bounded::layout_constraints(MeasureConstraints::width(100.0))`，得到：

```text
InvalidMaximumWidth(inf)
accepted_scale_causes_panic=true
```

调用者的两个输入都通过了公开校验。只修改 `expect` 文案或捕获 panic 不能修复输入域与运算域的矛盾。应明确缩放后测量溢出的契约，并让相关运算返回可识别的失败；若采用准入限制，需要根据几何和测量域推导，不能随意设置最小 scale。两种缩放容器应共用同一规则。

建议回归覆盖极小 scale、极大约束、双轴约束，以及缩放后尺寸和 baseline 的溢出。无需改动递归渲染主循环。

### CQ-02：有界 SVG API 没有覆盖通用解码入口

位置：[command.rs](../../../crates/novadraw/src/render/command.rs)，791–817、834–839 行；[svg.rs](../../../crates/novadraw/src/render/svg.rs)，78–115 行。

`SvgImageSource::rasterize` 在分配前检查像素预算和字节可寻址性。然而 `ImageData::decode_svg` 调用 `rasterize_intrinsic`，直接按源尺寸进入 Pixmap 分配；通用 `ImageData::decode` 也会进入这条路径。

探针用一个 64×64 SVG 做安全的小规模对照：

```text
bounded_svg=RasterPixelBudgetExceeded { required: 4096, budget: 1 }
legacy_svg_pixels=16384
```

这证明两个入口的预算契约不同。这里没有声称旧入口忽略调用者传入的预算，因为它根本不接受预算参数。若宿主接收外部 SVG 并使用通用解码入口，大尺寸源可能消耗大量内存或使进程终止；本次没有执行巨量分配攻击。

建议统一预算入口，让兼容 API 也进入明确的有限预算路径。还应区分源字节、SVG 解析与内嵌图像成本、最终栅格像素预算，避免把最终像素上限误当成整个解码过程的内存上限。当前禁用 SVG 外部路径读取的防护应保留。

### CQ-03：资源身份通过校验，图像内容却未校验

位置：[command.rs](../../../crates/novadraw/src/render/command.rs)，674–682、782–788 行；[resource.rs](../../../crates/novadraw/src/runtime/resource.rs)，254–264、390–399、513–544 行。

`ImageData` 字段公开，`from_rgba` 不检查像素长度或 scale。Runtime 的请求完成入口校验 token、资源种类和版本，但没有检查图像内容，随后将其置为 Ready。

探针通过 `Runtime::resources()` 注册图像并取得请求 token，提交 `ImageData::from_rgba(2, 2, vec![0], 0.0)`，结果为：

```text
invalid_image_accepted=Ok(())
```

2×2 RGBA 图像应有 16 字节，探针只提交 1 字节，且 scale 为零。[ImageResourceRef::logical_size](../../../crates/novadraw/src/render/command.rs) 会用尺寸除以 scale；[ImageFigure](../../../crates/novadraw/src/figure/image.rs) 的测量和绘制继续消费该尺寸。这使无效源数据进入布局和后端资源链路。尚未证明会造成 GPU 内存破坏。

建议在资源提交前集中校验字节长度、尺寸运算可寻址性、scale 和派生逻辑尺寸。可以增加可失败构造，但只加构造器不足以防止公开字段绕过，提交边界仍须验证。失败应保留原资源 revision、payload 和请求状态，不能先发布 Ready 再发现错误。

该问题在 [旧语义审计](../evidence/draw2d-gef-semantic-audit-2026-09-15/group/group_3.md) 中已有防御性观察，本次增加了当前 Runtime 公共入口接受无效数据的直接证据。

### CQ-04：截图的 Result 契约没有覆盖 GPU 失败

位置：[Vello screenshot](../../../crates/novadraw-backend-vello/src/lib.rs)，1905–2000 行，重点为 1973–1984 行。

公开 `screenshot` 返回 `io::Result<()>`，但忽略 `device.poll` 的结果，并对通道接收和 GPU 映射结果连续 `unwrap`。映射失败会通过 panic 退出；等待错误也没有进入调用者的错误处理路径。使用 `wait_indefinitely` 还让这个同步 API 没有自身的等待上限。

这是失败处理缺口，不是已复现的设备挂起或死锁。正常截图成功不能覆盖设备丢失、映射拒绝和等待失败。

建议把等待、映射和通道错误转换为返回错误，并明确截图是否允许阻塞事件循环。需要根据宿主需求决定等待期限或异步读取方式，并检查退出时映射资源的释放。只有内部不变量成立的 `expect` 才适合保留，GPU 返回值不是此类不变量。

### CQ-05：缓存替换正确，但批量更新成本偏高

位置：[Vello 资源缓存同步](../../../crates/novadraw-backend-vello/src/lib.rs)，2009–2075 行。

每个字体或图像 Upsert/Remove 都调用一次 `HashMap::retain`，扫描整个缓存。U 个更新作用于容量 C 的缓存，扫描成本约为 O(U×C)；全量 snapshot 从空表逐项插入时也会重复扫描，具有二次增长趋势。每次图像 Upsert 还执行 `image.pixels.clone()`，完整复制 Core 保留的像素。

例如，4096×4096 RGBA 图像的一份原始像素为 64 MiB。这是尺寸计算，未在本次评审中测量实际 RSS 或上传成本。

当前实现有版本替换和移除测试，未发现缓存失效错误。建议以 ResourceId 为索引保存当前 revision 和 payload，或维护可直接删除旧键的索引，保留 revision 校验；不可通过跳过版本检查换取性能。图像共享存储需要继续满足不可变资源和生命周期契约。

应测量大批 snapshot、替换、移除的时间和峰值内存，记录冷同步与稳态 delta 的差别。每帧没有资源变化时，不应把上述成本归因到所有帧。

### CQ-06：Web 输入失败会失去状态同步证据

位置：[dom_text_input.rs](../../../crates/novadraw-platform-web/src/dom_text_input.rs)，206–239、293–321 行。

`apply_effect` 先改变 bridge 状态及本地逻辑区域，再执行 DOM action。`focus`、`blur`、属性和样式写入的错误全部被忽略；公开方法返回 `()`，宿主无法知道 DOM 操作失败。`set_surface_origin` 遇到非有限坐标也直接返回。

因此，DOM 操作一旦报错，bridge 的会话记录可能已变化，而宿主没有错误或降级通知。这里没有断言正常浏览器一定会失败，也没有把清理阶段忽略 `remove_event_listener` 错误等同于业务错误吞没。

建议定义可观察的宿主操作结果，区分样式提示失败、焦点获取失败和会话释放失败。严重失败应触发明确的会话终止或降级，并保持因果顺序。不要把所有 DOM 副作用包装成无法兑现的完整事务回滚。

这部分代码受 Wasm/feature 条件控制，本次 Native 默认 feature 的 Clippy 通过不证明它已编译或通过浏览器验收。

### CQ-07：公开表面仍有旧失败表达

位置：[identity.rs](../../../crates/novadraw/src/identity.rs)，40–54 行；[ChildTransform](../../../crates/novadraw/src/figure/mod.rs)，140–146 行。

身份宏为 FigureId 等类型公开生成 `null()` 和 `is_null()`。外部 Rust 探针成功编译并返回 `public_null_id=true`。测试专用 `add_child` 会使用这个值，但该辅助方法有 `cfg(test)`，不能据此断言生产树修改会吞错返回 null。

另外，公开 `ChildTransform::apply_inverse_to` 在不可逆时返回 `false`。这与项目要求的可失败 API 使用 `Result` 的方向不一致，并使调用者缺少不可逆原因。

建议将测试占位身份限制在测试域，清理公开 sentinel 构造；为逆变换定义可识别的失败通道。此项主要影响 API 一致性和维护，不是已发现的越权漏洞。Runtime arena 的 namespace 校验仍会拒绝其他 Runtime 的身份，不应删除这层校验。

### CQ-08：注释门禁覆盖不完整，少量注释已经误导

位置：[Core lib.rs](../../../crates/novadraw/src/lib.rs)，1–45 行；[Bounded](../../../crates/novadraw/src/figure/mod.rs)，183–202 行；[ScaleError](../../../crates/novadraw/src/container/scalable.rs)，30–36 行。

Core 开启 `deny(missing_docs)`，但 connection、container、figure、host、layout、runtime 等整个领域模块被 `allow(missing_docs)` 豁免。因此 lint 通过不能证明公共 API 文档充分。测量约束和部分公共构造器仍缺少输入域与失败条件的说明。

`Bounded::bounds` 注释写着“默认实现返回零矩形”，实际签名没有默认方法体。ScaleError 的公开错误文案还使用旧 `block` 术语。这些是可确认的文档和风格问题。

规模方面，`runtime/runtime.rs` 为 5,990 行，`viewer/mod.rs` 为 3,847 行；两者有测试，不能只按总行数判定过度复杂。但 Viewer 直接编辑中多次出现取出 active state、更新 feedback、失败放回状态的流程，审查者必须跨多处验证所有权恢复路径。

建议逐领域补齐公共契约并缩小 lint 豁免范围；优先写坐标域、附着后 authority、变更通知、失败后的状态。大文件应按独立职责和重复生命周期流程整理，避免仅按行数切分，或为了复用引入庞大泛化事务。

## 性能与安全的补充判断

### 已有防护值得保留

抽查中确认了 Runtime namespace 和 arena 身份隔离；图布局的共享工作预算与非有限算术检查；渲染树深度限制及递归栈增长；Editor 命令故障锁定；SVG 外部文件解析禁用；逐操作 AA 的可失败扩容及工作预算。

这些机制比统一捕获 panic 或使用全局状态更符合项目边界。没有理由以性能优化为名去掉身份、预算和版本校验，也没有证据要求恢复迭代渲染主线。

### 图布局和连续 AA 更新需要规模数据

[ranking.rs](../../../crates/novadraw/src/layout/graph/ranking.rs) 的初始排名多轮遍历节点并扫描边，逆序链会放大重复工作；树 component 搜索也为访问节点扫描整个边集。[候选验证](../../../crates/novadraw/src/layout/graph/validation.rs) 做两两重叠检查，并在检查中遍历 ancestor。工作预算能限制总操作，不能证明大场景响应时间可接受。

这些是静态复杂度观察，尚未给出实际性能退化数据。建议用节点顺序不同的 DAG、深层 compound 和大批连接测量，再决定是否引入邻接索引、祖先查询缓存或空间索引。

逐操作 AA 的 [既有成本记录](per-operation-antialias-implementation-2026-10-26.md) 已报告：Apple M1 Pro/Metal，1024×1024、25 单元持续变换、DPI 2 的帧等待 P95 为 25,010 µs，超过 60 fps 的 16.7 ms。该数字来自既有 release 实验，本次没有重测，且不含窗口 present 或输入响应。不能把静态缓存收益推广到持续变换。

### 外部资源加载需要宿主明确预算

[FileResourceProvider](../../../crates/novadraw-platform-winit/src/resource.rs) 直接整文件读取；[FetchResourceProvider](../../../crates/novadraw-platform-web/src/resource.rs) 等待完整 ArrayBuffer 后再复制成 Vec，没有暴露大小、取消或期限控制。对可信本地资源可接受；接收外部大资源的宿主需要限制策略，否则在解码前就可能积累大量字节。

这不构成文件越权或服务端 SSRF 的证据。文件路径和 URL 来自调用者，不能仅因支持任意路径就断言安全漏洞。

Core 无条件依赖 Parley/resvg，并提供内置字体字节；SVG 解码会建立字体数据库。后续优化应测量相同消费者和配置下的最终 Wasm、资源与初始化成本。当前用户偏好高质量内置字体，不能仅按源码体积建议删除字体或拆空 crate。

## 本次验证

| 检查 | 结果 | 能证明的范围 |
|---|---|---|
| `cargo build -p novadraw --lib` | 通过 | 当前 Core 构建及最小探针链接基础 |
| 外部 Rust 探针 | 编译、运行通过 | 复现 CQ-01/02/03，并确认公开 null 身份 |
| 六个库 `--lib` 严格 Clippy | 通过 | Core、Editor、Vello、Winit、Web、winit-canvas 的 Native 默认 feature 库目标 |
| `cargo fmt --all -- --check` | 未通过 | 检查时 ShapeStyles/UMLClassDiagram 的并行修改存在 rustfmt 差异 |
| `cargo audit --version` | 不可用 | 本机未安装 cargo-audit，未执行 RustSec 依赖漏洞扫描 |
| `cargo xtask docs` | 通过 | 检查时 383 个 Markdown 文件及当前 xtask 引用 |
| `git diff --check` | 通过 | 已跟踪改动的空白检查 |

Clippy 使用：

```bash
cargo clippy -p novadraw -p novadraw-editor -p novadraw-backend-vello -p novadraw-platform-winit -p novadraw-platform-web -p winit-canvas --lib -- -D warnings
```

探针只使用小 SVG 和小像素数据，并用 `catch_unwind` 记录预期 panic；临时执行文件已自动删除。未新增生产代码、测试文件或依赖。未运行 workspace 全量测试、浏览器/GPU 故障注入、Miri、fuzz 或完整 feature 矩阵，也未把历史 1,501 项测试通过表述成本次验证。

## 建议执行顺序

先为 CQ-01/02/03 定义失败契约并加入针对边界输入的回归，再逐项修复。涉及公共签名或资源边界时更新相应 ADR 和 API 检查；本报告不直接扩大现有路线图批次。

随后处理截图和 Web 输入的外部失败反馈，再用代表性规模测量缓存同步和图布局。最后整理公开 sentinel、逆变换失败表达及文档豁免。阶段验证按受影响 crate 和 suite 选择，最终交付再运行项目规定的全量门禁。

报告交付前按“好好说话”复核技术术语、条件、数字和证据强度。机械扫描的编号标题、对比句式、元话语、中文半角标点与破折号均命中 0；修订只补充验证记录和证据链接，未改变发现结论。新增报告另做本地空白与链接检查，避免只依赖尚未覆盖未跟踪文件的 `git diff --check`。
