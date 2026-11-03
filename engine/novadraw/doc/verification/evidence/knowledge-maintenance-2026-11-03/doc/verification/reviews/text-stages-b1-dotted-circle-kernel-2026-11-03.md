# B1：dotted-circle 输出与同步准入

类型：`implementation-verification`

日期：2026-11-03。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：实现、Native／portable 测试、完整输出矩阵、最终源码成本采样和跨目标门禁通过。
B1 保持未关闭，生产宽度复用缓存继续默认关闭。

[前片](text-stages-b1-normalizer-kernel-2026-11-03.md)已为规范化中的
分解、排序和重组建立操作准入。输入以组合标记（mark）开头时，
Harfrust 可在 flags 和上下文允许、字体映射命中的条件下插入
U+25CC 虚线圆圈（dotted-circle）。插入时输入索引仍为零，
`sync` 会复制全部原始输入；规范化成功结束时没有剩余输入的证明不适用于它。
本片补齐这条输出与同步路径，公共组合字符预检仍返回未支持。

## 共享内核与费用依据

普通入口和内部准入入口共用 `output_info_with`、`sync_with`，
以及 `insert_dotted_circle_with`。普通策略为无操作策略，
flags、上下文、首字符类别与字体映射的短路顺序保持。
仅在 U+25CC 映射命中后创建圆圈记录；缺字不插入。

| 顺序 | 操作前准入 | 成本依据 |
|---|---|---|
| 判定 | flag／context 与首记录属性读取 | 固定标量与存储类别，不查询 Unicode 表 |
| 映射 | `has_glyph(U+25CC)` | 使用已证明的 charmap 查询界，实际查询另行计费 |
| 构造 | 首记录 mask／cluster 与圆圈 Unicode 属性 | U+25CC 为非 ASCII 的 So；四次 category 表读取和一次 default-ignorable 读取，不执行 mark CCC 查询 |
| 输出 | `output_info_with` 先走受准入的 `make_room_for_with` | 长度检查、两数组预留、潜在前缀迁移与单条记录写入分别计费 |
| 同步 | `sync_with` 先准入，再复制未消费输入 | `next_glyphs_with(len - idx)` 按当时长度支付源读取与目标写入，增长前预留两数组 |
| 转移 | 两数组交换与输出状态收口 | 仅交换存储所有权及固定标量，不复制数组元素 |

`sync_with` 保留原普通入口的失败顺序：进入时 `successful == false`
只清理输出状态并返回 false；不复制后缀。普通 `ensure` 失败后的既有行为
同样保持，不借本片调整上游算法。内部准入拒绝立即返回原始错误，
不完成后续同步或通过普通入口重做。

新增内部测试分别覆盖 flags／context／字体缺字抑制、完整后缀字节保持、
已预留容量与增长、每个准入点拒绝后的存储复用、增长上限和失败同步。
每个成功插入保留首记录 mask／cluster，并将全部旧输入放在新增记录之后。
使用精确输入容量，使两份数组新增整份容量的费用可观察；
已预留 128 slots 的对照不新增容量。

## 独立原版输出与成本验证

内部普通／准入比较共享生产算法，不能独立证明普通语义保持。
因此新矩阵另以未修改的 Harfrust 0.3.2 归档运行相同消费者源码。
此前二十一矩阵的 fixture、oracle、成本输入与二十八份补丁保持冻结。
原版既有配置只在全部解压字节、签名和分母核对后复用；
生产配置与新增原版配置重新执行。

新矩阵每配置 497,664 份观察：
3 内核 × 2 GDEF × 3 mapping × 2 圆圈映射 × 2 轮次 × 3 script
× 2 方向 × 6 输入 × 4 flags × 2 context × 3 cluster 模式
× 4 cluster level × 2 容量。
包含普通 mark、canonical 分解、variation selector、圆圈 glyph 存在／缺失，
以及 None、Latin、Arabic 普通脚本输出。
这些普通路径对照不扩大公共准入支持范围。

二十九份补丁已精确重放并核对全部 83 份源码；
本片三份生产文件与一个新增内部测试模块之外的 79 份源码保持原字节。
增量补丁 SHA-256：
`8a35a12f2beeb9eea9d93150882be93bfae2948c66f5e512d7f66aeaaafb45fe`。
二十二矩阵四配置共 10,786,752 份完整观察一致，
新增原版／生产 Native／portable 四份配置签名均为
`f8236698add257bf21cb4a5afcde60616899881a94a3f8b8525b74b3ccbfb392`。
旧原版 42 份配置逐字节核对后复用，生产 42 份及新增四份配置重新执行。
额外 invisible 参考的 160 份观察保留原收据，并再次核对源与输出身份。
Native／portable 各 214 项内部测试与 102 项消费者测试通过。

成本验证使用冻结的三组输入，并另加真实圆圈插入的普通采样。
新增输入为 1,408 个字符，以 `BEGINNING_OF_TEXT` 开始，
比较圆圈映射存在／缺失及 precomposed 映射存在／缺失。
后者在 mark 映射保留时执行实际分解增长，补充前片增长未进入计时范围的场景。
全部成本程序在计时前通过 Clippy，再使用交替顺序采集原版／生产三轮数据。
字体构造、计划构造、输入准备和结果释放不在计时范围内；
系统负载未受控，结果不能单独证明 Core 布局提速或回退。

最终源码共采集 768,000 条成本样本。准入组包含普通、
无操作回调、计数回调和最后准入点拒绝四种路径，逐样本费用稳定，
最后拒绝保留预期费用前缀。以下是各配置的分位数范围，单位为微秒。

| 准入组路径 | P50 范围 | P95 范围 |
|---|---:|---:|
| 普通 | 49.792–121.666 | 52.917–129.708 |
| 无操作回调 | 91.542–165.583 | 98.584–174.792 |
| 计数回调 | 98.583–173.500 | 105.916–183.375 |
| 最后准入点拒绝 | 99.792–175.291 | 106.709–185.084 |

| 普通路径组 | 样本数 | 原版 P95 范围 | 生产 P95 范围 | 同配置、同轮次生产／原版 P95 |
|---|---:|---:|---:|---:|
| singleton | 144,000 | 52.542–109.042 | 50.458–108.125 | 0.949–1.025 |
| mark，保留 precomposed 映射 | 144,000 | 72.792–192.625 | 75.334–196.000 | 0.916–1.169 |
| 圆圈与真实分解增长 | 288,000 | 94.791–397.833 | 98.167–405.875 | 0.923–1.179 |

准入组另有 192,000 条样本。七个成本程序均在采样前通过 Clippy；
最终门禁再次核对其日志身份和全部十九份原始采样文件。
Native／portable 库 Clippy 与冻结前片的六项既有诊断一致，新增诊断为零。
消费者两配置 Clippy、Core Native／Wasm check 和消费者 Wasm build 通过，
合计十一项边界检查。Wasm 只构建，未执行。
成本与门禁收据见证据目录中的 `dotted-circle-kernel-*-cost-summary.json`
和 `dotted-circle-kernel-gates.json`。

工作区格式、本片五个 Rust 模块格式、三个 Python 脚本语法、
七份状态文档空白检查及 527 份 Markdown 文档门禁通过。
成稿已按事实、逻辑和措辞分别回读；中文四类机械扫描均零命中，
该结果只说明未发现字面候选。文字复核由作者自审，没有独立读者复述验证。

本片冻结快照的复验入口如下；源码进入后续片后，
应使用对应冻结源码，不能用旧脚本改写新的生产补丁。

```bash
python3 -B tools/scripts/check_harfrust_dotted_circle_kernel_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_dotted_circle_kernel_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_dotted_circle_kernel_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-wasm --linker rust-lld
```

## 当前限制与后续执行

本片只完成输出与同步的内部准入。原始 mark 的 Unicode CCC／continuation、
grapheme 合并、反向分组、fallback 类别和 mark 定位仍须逐项补齐。
合并可能重复扫描输入后缀与输出前缀，不能按互斥分组长度付一次费用。
fallback mark extent 的固定字体表访问界也不能代替重复查询与整组扫描费用。

公共 raw mark、variation selector 和未知脚本回调仍在预检拒绝，
不进入未覆盖的阶段。之后继续 B1 复杂 GPOS／脚本、B2 Fontique／Parley
分析编排、B3 导出、B4 backing 与 C／D；本片通过不等于 B1 或 ADR-059 完成。
