# S08-A 受检样式与公共边界实施记录

类型：`implementation-verification`

本记录对应用户批准实施的 [ADR-056](../../adr/adr-056-detached-input-invariants.md)
S08-A。Opacity 领域迁移、受检样式、字体 token 解析和 Graphics 输入迁移已完成。
本切片的公共消费者、API、非法输入与继承门禁通过；S08 整批保持 `in_progress`。
Border 构造和 builder 属于 S08-B，Detached 几何与外部指标准入属于 S08-C，
完整场景成本与整批关闭属于 S08-D。

## 实现与失败位置

`graphics::Opacity` 保存私有 f64，大小与 f64 相同。`try_new` 在构造时区分
`GraphicsInputError::NonFinite` 与 `OpacityOutOfRange`；0 和 1 合法。
删除 `animation::Opacity` 公开路径，动画继续为同一个类型实现 AnimationValue。
AnimationError 的 GraphicsInput 包装通过 `source` 保留原始原因。
采样范围收敛仍属于动画算法，样式和录制层的隐藏 clamp 已删除。

FigureStyle 的 alpha 和 font 分别为 `Option<Opacity>` 和
`Option<FontDescriptor>`，ResolvedStyle 使用同一类型。父 alpha 0.5、
子覆盖 0.8、移除覆盖后恢复 0.5；字体也按最近祖先覆盖。
普通 mutation、Deferred mutation、动画 presentation、Scene capture 和文字准备
共用受检值。递归主循环仅迁移三处 alpha 调用，没有改变遍历或裁剪顺序。

字体解析按完整的空白分隔 token 查找 `sizepx`，保留数值 weight、负号和 Unicode，
拒绝零、负或非有限字号、空 family、越界 weight、重复 style／weight 和未知前缀。
未知 family 仍允许通过正常字体匹配与 fallback 测量、绘制。
FontDescriptor 的 Display 提供规范诊断文本；准备缓存以 descriptor 为键，
查询和绘制不再从样式文本重新解析字体。

Graphics／PaintContext 的 `set_opacity` 安装 Opacity，原始标量不再进入该入口。
GraphicsError 仍用于绘制和录制失败，输入原因在受检值构造时返回。
此前用非法 opacity 制造绘制失败的场景改用非法描边，继续验证发布拒绝与恢复，
避免把构造失败误记为绘制阶段失败。

## 验证矩阵对应

矩阵入口为 [S08 验证矩阵](../plans/p2-s08-detached-input-invariants.md)。
下列结果只覆盖 S08-A；跨切片的矩阵项不因此整体关闭。

| 矩阵项 | 本切片证据 | 覆盖范围 |
|---|---|---|
| V01 | `p2_s08_style_input_contract` | 非有限、越界、端点、类型大小、动画共用类型与 source |
| V02、V03 | 同上 | 数值 weight 端点、完整 token、Unicode、重复与非法 token、构造负测、Display 解析回归 |
| V04 | 同上、`p2_deferred_style`、`p2_scene_capture` | 第三方 Figure 继承／覆盖／移除、同值不变、冻结字体与 opacity |
| V09 | `p2_animation_contract`、`p2_graphics_contract` | 动画范围收敛与合法透明绘制；其他适配由后续切片复核 |
| P05、P06 | 样式契约与 Deferred 契约 | 非法值在样式提交前构造拒绝；有效更新恢复、同值不变；Border 部分待执行 |
| R01 | Opacity、FontDescriptor、Graphics 的 compile-fail doctest；公共 API snapshot | raw alpha／font、私有标量、旧动画路径、raw setter 不可用；Border 部分待执行 |
| R02、R03 | 文字 fallback 测量／绘制、现有契约与 Native GPU | 本切片影响的样式与 Graphics 回归；完整 Border 场景待执行 |
| R04 | 独立默认／无默认消费者、Native／Wasm 公共 API 检查 | 公开领域路径与同一个动画值类型 |
| R05 | 独立 style-input 成本消费者 | 两节点连续样式更新；稳定帧、resize、嵌套 Border 尚未测量 |
| R06 | 同配置 Native／Wasm release 产物 | 本消费者可达样式代码；完整产品与首次字体资源成本待 S08-D |

独立消费者位于 `tests/consumers/style-input`，拥有自己的 workspace 和依赖解析，
只使用公开领域导入。五个契约包含第三方 Figure 的构造、挂载、覆盖、失败与恢复，
以及未知字体的真实测量和 glyph 绘制。默认和无默认配置分别通过五项测试。
Core 当前 `default = []`，动画常驻；不存在单独的 animation feature。

## 自动门禁

Native 使用 Rust 1.96.0；Wasm 使用 stable 的 Rust 1.99.1。
内环日志与最终 workspace 门禁分开保存：

- [workspace check](../evidence/p2-s08-a-workspace-check-final-2026-10-29.log)。
- [Core lib](../evidence/p2-s08-a-core-lib-2026-10-29.log)：427 通过，1 忽略。
- [六组契约](../evidence/p2-s08-a-core-contracts-complete-2026-10-29.log)：
  animation 29、Deferred 4、Graphics 24、新样式 5、capture 13、TC-01 2，共 77 项。
- [默认消费者](../evidence/p2-s08-a-external-default-2026-10-29.log)与
  [无默认消费者](../evidence/p2-s08-a-external-no-default-2026-10-29.log)。
- [Native／Wasm API snapshot 和架构检查](../evidence/p2-s08-a-api-snapshot-2026-10-29.log)。
- [最终严格 workspace Clippy](../evidence/p2-s08-a-workspace-clippy-2026-10-29.log)与
  [workspace tests](../evidence/p2-s08-a-workspace-tests-final-2026-10-29.log)：
  170 个测试目标／doctest 汇总，1,667 通过、0 失败、21 忽略。
  五项新增 compile-fail 均通过。

命令使用 `cargo fmt --all -- --check`、`cargo clippy --workspace -- -D warnings`
和 `cargo test --workspace`。独立消费者使用 `--manifest-path` 和 `--lib`，
Wasm 使用 `--release --lib --target wasm32-unknown-unknown`。
[文档检查](../evidence/p2-s08-a-docs-final-2026-10-29.log)通过 444 个 Markdown 文件。
首轮全量编译发现动画 behavior 测试仍导入旧 Opacity 路径，已迁到 graphics；
首轮文档检查拒绝未登记的报告类型，已改用 `implementation-verification`。
原始尝试分别保存在 workspace-tests 与 docs 日志中。

## Native GPU 与既有像素断言

[离屏 GPU 日志](../evidence/p2-s08-a-native-gpu-offscreen-2026-10-29.log)通过
DPI 1／2 的 retained 像素断言，覆盖 gradient、opacity、路径裁剪、状态恢复、
dash、miter 与文字。输出图位于 `target/verification/p2-g01/offscreen/`。

首次检查暴露了旧断言错误：`restore_state` 移除内层空 clip，仍保留外层路径 clip，
而旧断言要求外层路径之外的像素为红色。此次加入路径内的红色矩形作为正例，
同时要求路径外保持白色。这样同时检查空 clip 已解除和祖先 clip 仍有效。
该修正不改变 Core 裁剪协议，原失败日志保存在
[首次 GPU 检查](../evidence/p2-s08-a-native-gpu-2026-10-29.log)。

[`--present` 复跑](../evidence/p2-s08-a-native-gpu-final-2026-10-29.log)在 10 秒内
未收到窗口 redraw，按既有 timeout 退出。窗口呈现和浏览器像素仍待平台验收，
离屏通过不替代这两项证据。

## 成本与产物范围

消费者预热一次后运行 101 个样本，每个样本构造两个挂载节点，执行 120 次父样式
更新及子继承查询。字号在 12px／13px 间切换，alpha 为 0.5，包括 Runtime 初始化
与 teardown。计数 allocator 记录分配／realloc 请求次数及请求字节，
不代表存活内存、GPU 帧延迟或首次字体加载。

迁移前后均为 `opt-level=s`、LTO、单 codegen unit、abort panic 和 strip。
样式文本仍在每次应用更新时解析一次；迁移后存入 descriptor，
迁移前存入文本并在查询后解析。
基线日志为[默认](../evidence/p2-s08-a-cost-before-2026-10-29.log)和
[无默认](../evidence/p2-s08-a-cost-before-no-default-2026-10-29.log)。
初次迁移成本与诊断格式调整后的数据也保留，不以最终数据覆盖失败尝试。

迁移前总分配次数为 200,283，请求字节为 52,373,651。
诊断格式调整后的复测为 224,422 次与 54,632,112 字节，分别增加约 12.05% 和 4.31%。
额外 24,139 次与 `101 × (120 × 2 − 1)` 一致：
首次字体更新的旧值为空，其余旧／新 FontDescriptor 都需要生成属性诊断文本；
原 String 值可以直接移入日志。规范 Display 与单缓冲区写入已消除初版 Debug
格式的多次扩容，但仍保留这项可解释的迁移成本。

编译与 workspace 测试全部结束后，
[默认单次复测](../evidence/p2-s08-a-cost-after-quiescent-2026-10-29.log)的
P50／P95 为 157,625／309,459 ns，
[无默认单次复测](../evidence/p2-s08-a-cost-after-no-default-2026-10-29.log)为
153,959／210,458 ns。由于尾延迟波动，随后各配置连续运行五次，
[全部重复记录](../evidence/p2-s08-a-cost-repeat-2026-10-29.log)保持原样：

| 配置 | 迁移前单次 P50／P95，ns | 迁移后五次 P50／P95 的中位数，ns | 迁移后五次 P95 范围，ns |
|---|---|---|---|
| 默认 | 155,250／171,834 | 153,834／188,375 | 166,166～204,375 |
| 无默认 | 128,958／141,292 | 150,166／179,708 | 169,958～198,167 |

重复测量的分配次数与请求字节完全一致。P95 中位数均高于原单次基线，
但基线没有同环境重复配对，两个等价 feature 配置的基线计时也有明显差异，
因此只能记录观察值，不能把全部计时差异归因于此次类型迁移。
后续性能验收需补成对、多轮测量；这项不确定性仍归 R05／S08-D。

最终运行数据和原始／gzip 字节、SHA-256 见
[产物收据](../evidence/p2-s08-a-artifacts-2026-10-29.json)。

| 产物 | 迁移前原始字节 | 迁移后原始字节 | 迁移前 gzip 字节 | 迁移后 gzip 字节 |
|---|---|---|---|---|
| Native 默认 | 2,095,296 | 1,959,216 | 940,843 | 945,019 |
| Native 无默认 | 2,095,296 | 1,959,200 | 940,668 | 944,613 |
| Wasm 默认 | 2,119,035 | 2,121,452 | 708,136 | 708,464 |
| Wasm 无默认 | 未单独采样 | 2,121,470 | 未单独采样 | 708,474 |

Native 默认 gzip 增加 4,176 字节，约 0.44%；Wasm 默认原始／gzip
分别增加 2,417 和 328 字节。
Wasm 无默认配置只补测迁移后产物，缺少独立迁移前配对基线。

此次没有新增依赖、crate、内置字体或默认 feature；测量产物只包含消费者可达代码，
不包含 Native 窗口、GPU 宿主或完整产品资源。计时样本很小，
前后测量期间的系统负载也不同，不据此宣称总体性能改善。
稳定帧、连续 resize、嵌套 Border 准备、复制量和缓存命中仍在 R05／S08-D 内。

## 后续边界

下一切片为 S08-B：Border 私有受检配置、正宽与 inset 规则、builder 等价迁移、
Margin 历史兼容字段移除和描边复用。S08-A 不关闭 R03、TC-03、API-F04 或 API-10
整项；其余 V／P／R 用例继续按矩阵执行。

成稿复核保留上述命令、数字、失败位置和验收范围。字面扫描的章节编号、
对比句模板、空泛元话语、中文正文半角标点均为 0 处，无豁免项。
