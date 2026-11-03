# B1：ASCII 从右向左塑形准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：补丁重放、完整矩阵、消费者、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存保持默认关闭。

本片接续[配对定位](text-stages-b1-gpos-pair-2026-11-02.md)，
把已映射 ASCII 的 RTL 方向处理接入共享准入策略。
保留默认坐标、全局 feature、零 lookup flags、无 GDEF／AAT，
以及既有 GSUB／GPOS 算法限制。复杂脚本与非 ASCII 仍提前返回未支持。

## 保存目标方向，再处理原生方向

普通入口用原始目标方向决定字符镜像；准入入口此前把该方向固定成 LTR。
本片在进入 pipeline 前保存 buffer 的真实方向，
普通与准入仍调用同一个 shaping 内核，不改变普通输出算法。

Latin、Common 与 unknown 的原生方向是 LTR。
RTL 输入在 Unicode 属性初始化后按 grapheme 反转，再把当前方向改成 LTR。
已支持 ASCII 没有 continuation，所有 grapheme 均为单项，
即使多个字符使用相同 cluster 身份，也不会合并成多项组。
准入先覆盖相邻项检查、单项组的端点检查和完整 info 反转。
如果当时已有 positions，还覆盖其反转；此时只处理逻辑长度，
不把备用容量当作反转范围。

未指定 script 时，原生方向为 Invalid，不在这个阶段反转。
该路径保留当前 RTL 方向；定位完成后反转 info 与 positions，
费用在 Position 阶段、定位执行前支付。
GPOS 不增加输出长度，前置 GSUB 的增长仍由既有 lookup 预留协议处理。

## 镜像查询与正常映射分别计费

RTL 目标方向在替换阶段触发 `rotate_chars`。
每个 ASCII 字符的 `mirrored()` 会读取四个生成表元素；
存在镜像时，`has_glyph` 再做一次字体映射查询。
预检先收费镜像表访问，确认镜像仍在 ASCII 范围内，
再收费并执行字体映射。不能用 ASCII 映射上界执行非 ASCII 镜像查询。

镜像不存在于字体时，内核保留已经映射的原字符，并添加 `rtlm` mask。
因此镜像缺字可以准入；原字符缺字仍在 pipeline 前返回未支持。
执行阶段单独预付镜像扫描、潜在字体映射和 info 读写，
随后 normalizer 的重复映射继续独立收费。
固定缓存被预检预热不免除后续费用。

## 定向消费者

新增消费者逐字段比较 info、positions、cluster 与 flags，
覆盖 LTR／RTL、Latin／Common／unknown／未指定 script、
镜像存在与缺字、三种 cluster level、顺序／等值／倒序 cluster，
以及备用容量、前置替换和 SinglePos／PairPos。

缺字 fixture 使用 format 12 的缺失区间。
format 0 的零号 glyph 仍产生 `Some(GlyphId(0))`，
不能用该记录证明 `has_glyph` 返回 false。
单字符断言直接检查括号变为镜像或保留原字符；
费用断言分别核对 Unicode、Mapping、Information 与 Positions 的新增访问。

冷／热路径在每个准入点拒绝，检查原始错误 source、精确费用前缀，
以及预检保留输入、pipeline 恢复空可用 buffer 和后续普通调用。
复杂脚本、原字符缺字和中文输入各有 pipeline 前拒绝反例。

## 可复验入口

```bash
python3 tools/scripts/check_harfrust_rtl_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_rtl_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind rtl \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_rtl_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

二十一份补丁依次重放后，全部 81 份源码与生产逐字节一致。
本片只修改准入模块，其余 80 份保持不变。第二十一份补丁 SHA-256：
`06e8d5c4a745f2509ee46896bc8f7f6f5dd0d304a2bc242c7ca70443843ceeb6`。

十四矩阵每配置 404,976 份完整输出，四配置共 1,619,904 份观察一致。
新增方向矩阵每配置 28,800 份，既有十三份 oracle 与签名保持。
输出以无损 gzip 保存，散列针对解压后的字节。
Native／portable 各通过 203 项依赖与 75 项消费者测试，本片新增四项。
SinglePos 旧负例原先把 ASCII RTL 视为未支持；
本片改为 RTL 非 ASCII 反例，生产代码和冻结普通 oracle 不因此改变。
修正前失败日志保存在 `rtl-before-negative-test-update/`。

## 成本

Native release 完成 64,000 个准入成本样本，每组 100 次预热、
1,000 个正式样本。输入为重复的 `ABC(`，共 3,072 字符；
普通、无操作回调、计费和末尾拒绝四种模式按样本轮换。
计时包含 shaping、缓存、预留和结果／拒绝清理，
输入、data、plan 与热缓存准备在计时外。
下表使用无备用容量的数据，单位为微秒。

| script 与路径 | 普通 P95 | 计费 P95 | work 上界 | allocation | 准入次数 |
|---|---:|---:|---:|---:|---:|
| Latin 纯定位，冷 | 150.917 | 169.417 | 2,731,378 | 14 | 7,196 |
| Latin 纯定位，热 | 151.708 | 170.458 | 2,728,800 | 0 | 7,120 |
| Latin 前置扩张，冷 | 308.834 | 332.666 | 5,077,971 | 18,450 | 7,307 |
| Latin 前置扩张，热 | 302.500 | 331.416 | 5,075,280 | 18,432 | 7,204 |
| 未指定纯定位，冷 | 148.375 | 166.292 | 2,719,092 | 14 | 7,195 |
| 未指定纯定位，热 | 152.625 | 171.875 | 2,716,514 | 0 | 7,119 |
| 未指定前置扩张，冷 | 330.959 | 355.750 | 5,074,901 | 18,450 | 7,306 |
| 未指定前置扩张，热 | 328.750 | 351.083 | 5,072,210 | 18,432 | 7,203 |

纯定位输出 3,072 glyph，前置扩张后输出 5,376 glyph。
定位与反转本身不增长存储。冷 allocation 的十四项来自完整 lookup 缓存；
前置 GSUB 的保守输出上界为 9,216 glyph，
两份数组的新容量共 18,432 slots，冷路径另有十八项缓存存储。
备用容量 16,384 覆盖前置扩张，所有热路径 allocation 为零，
冷路径只收十四／十八项缓存存储。完整已初始化数组的既有遍历仍收费，
方向反转只收费逻辑长度，二者范围不同。
末尾拒绝的调用次数与 allocation 等于完整路径，
work 少最后一次 glyph 数费用；逐样本核对输出长度与拒绝状态。

原始版与生产版另完成三轮交替顺序、48,000 个普通入口对照样本。
两版使用相同 source 与 feature，计时排除成功结果释放。
下表给出三轮 P95 范围，单位为微秒。

| script 与路径 | 原始版 P95 范围 | 生产版 P95 范围 |
|---|---:|---:|
| Latin 纯定位，冷 | 151.417–157.000 | 150.125–164.750 |
| Latin 纯定位，热 | 146.959–149.667 | 147.583–151.459 |
| Latin 前置扩张，冷 | 295.334–299.917 | 293.792–307.667 |
| Latin 前置扩张，热 | 294.000–295.041 | 294.417–297.125 |
| 未指定纯定位，冷 | 144.292–146.375 | 146.042–152.125 |
| 未指定纯定位，热 | 144.375–144.667 | 145.833–150.292 |
| 未指定前置扩张，冷 | 321.792–325.917 | 318.541–325.833 |
| 未指定前置扩张，热 | 322.750–324.916 | 322.000–329.208 |

系统负载未受控，部分普通入口尾延迟上升，部分下降。
这些数据不能证明稳定提速或零开销；准入计费模式的费用检查本身也有成本。
本片补齐方向路径的费用与失败行为，尚未改变 Core 的默认宽度复用行为。

生产准入模块与第二十片比较，Native／portable 均保留六项既有 Clippy 诊断，
新增诊断为零。消费者与两份成本源码的严格 Clippy 通过；
Core Native／Wasm check 和 portable 消费者 Wasm 构建通过。
Wasm 未执行测试，构建结果不代表 32 位运行验收。

最终收据为 `rtl-verification.json`、`rtl-cost-summary.json`、
`rtl-ordinary-cost-summary.json` 与 `rtl-gates.json`，保存在
[B 切片证据目录](../evidence/text-stages-b-2026-11-02/README.md)。
这些定向证据不代替 B2–B4、C 的 Core 全输出等价与 D 的默认启用门禁。

报告按技术结构和好好说话做自审，分别核对方向顺序、镜像缺字、
费用与证据范围，再检查中文措辞和节奏；不作为独立评审结论。
