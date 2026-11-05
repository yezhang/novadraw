# 长文本原生字形裁剪的证书边界

类型：`proposal`

能力归属：backend 原生字形覆盖证书候选；Core 提供字体与布局身份，宿主提供设备变换输入。

日期：2026-10-27。修订日期：2026-11-05。状态：已补齐 SR-R01 设计响应及锁定 lowerer 核验，生产证书合同待复核。

本文承接[长文本方案](long-text-performance-proposal.md)的 LT-03，
解释默认原生字形为何仍完整录制，以及生产裁剪需要补齐的合同。
当前实现与测量状态见[实施记录](../../verification/reviews/long-text-performance-implementation-2026-10-27.md)。
本文没有授予新的后端能力，也不表示生产裁剪或呈现验收通过。

## 已核验的事实

Vello 后端的 `append_glyph_run` 明确设置 `.hint(false)`。
因此，当前默认原生路径与未 hint 的轮廓之间，hinting 不是主要未决项。
仍须处理字体 face、变量坐标、合成斜体、设备尺寸、数值转换和非轮廓字形。
后端在虚线字形描边的轮廓不可用时继续提交原生字形，
不能假定任意合法字体都只含可提取的单色曲线。

当前原生路径在缩放前把 origin 转成 f32，再与 glyph 的 f32 位置相加，
并用 f32 缩放 font size 与 glyph position。
Core 的轮廓定位和 clip 换算则包含 f64 几何运算。
两条路径的坐标精度与运算顺序不同；字体资源相同不等于设备包络相同。
例如，数值达到 2²⁶ 时，相邻 f32 的间距已达到 8 个逻辑单位。
大 origin 配合反向平移可以把文字放回屏幕可见区域，
所以不能仅凭屏幕尺寸排除这类输入。
这是数值边界判断，尚未作为 GPU 像素反例验收。

Vello 的 fine shader 使用像素内的解析面积积分或 MSAA 采样。
局部采样支持范围有界，但曲线 flatten、f32 编码、变换和 clip 的误差仍在它之前。
“像素内积分”不能单独证明所有合法变换都满足统一的两像素包络。
现有测试只在有限输入下声明两像素范围。

源码定位：

- 本仓库 `crates/novadraw-backend-vello/src/lib.rs`：`append_glyph_run`、
  `SceneEncoder::transform_to_affine`、`VelloRenderer::capabilities`。
- 本仓库 `crates/novadraw-backend-vello/src/glyph_outline.rs`：未 hint 的曲线提取及不可用路径。
- Vello 参考源码 `vello_encoding/src/glyph_cache.rs`：字体、size、变量坐标和 hint policy。
- Vello 参考源码 `vello_shaders/shader/fine.wgsl`：像素内面积积分与采样。

参考仓库源码用于解释机制；锁定依赖版本的实际行为仍须以对应构建产物核验。

## 锁定 lowerer 核验

2026-11-05 管线删减核对实际 workspace 依赖：生产 Vello、vello_encoding、
vello_shaders 为 0.10.0，字形提取使用 skrifa 0.44；锁文件中的其他 Vello 版本
不能作为本次原生路径的覆盖依据。

锁定的 `vello_shaders-0.10.0/shader/flatten.wgsl` 使用 `flatten_euler`：
先由端点与导数拟合 Euler spiral，再以多项式积分近似生成线段。
`tol = 0.25` 参与误差估计，但 `dt <= 1 / 65536` 时强制接受该段，
每段的线段数量另外限制在 1～100。最终 path bbox 来自实际发出的线段。
所以，控制点包络和该容差尚不能直接组成实际栅格覆盖证明；
不能假定所有发出的顶点都位于原 Bezier 凸包加 0.25 像素之内。

`es_params_eval` 还包含 chord 近似值的除法，非圆弧分支包含积分的逆近似。
有限坐标或可逆 affine 本身不能证明这些中间量远离退化值。
候选有限域必须覆盖实际接受的全部细分段、f32 运算、近零导数调整、
Euler 参数和逆参数范围，随后才能推导线段顶点及像素支持范围。
只测试常用字体的曲线包络，不能证明任意合法字体满足该域。
对应 CPU 实现在 `vello_shaders-0.10.0/src/cpu/{flatten,euler}.rs`，
其采样可以用于差分诊断，不能自动代替 GPU 运算的边界证明。

本次尚未得到通过复核的有限域证书，未改变生产能力声明或原生提交。
后续先验证静态曲线摘要能否低成本证明操作落入有效域；
未知格式、曲线退化和无法证明的变换保留完整输出。
证明若只能在逐帧遍历全文后取得，仍不满足录制前索引的成本目标。

### 可先核验的空墨迹分支

锁定的 Vello `scene.rs::DrawGlyphs::draw` 会先分流 COLR／CPAL 和 bitmap，
普通轮廓则进入 `vello_encoding::GlyphCacheSession::get_or_insert`。
后者在 `n_path_segments == 0` 时明确 reset 本次 encoding。
因此可以先核验一个不依赖 Euler 顶点误差的分支：在对应 face、变量坐标、
未 hint policy 下，成功提取后没有任何路径操作的单色字形。
这只是待验证的有限域，不能按字符是空格、advance 为零或逻辑行盒为空决定。
颜色／bitmap 分流必须排除；提取失败、未知格式或有路径操作时保留原生输出。

若该分支证明成立，可先建立统一 provider、policy 和帧绑定通道，
删除已证明没有墨迹的填充输出。完整布局、advance、来源、selection 和 caret 保留；
字体依赖仍验证。空墨迹不会产生可见范围索引所需的曲线包络，
也不能关闭一般屏幕外 glyph 的裁剪验收。留存、首次构造、碎片数量、
GPU 像素和拖拽／resize 的净成本均须单独验证。

进一步核对发现，现有 `SkrifaOutlineProvider` 在 font units 中未缩放提取，
Vello GlyphCache 则以实际 f32 size 提取；现有 `GlyphOutline` 也没有原生分流
的 provenance。不能把现有 `path().operations().is_empty()` 当作已受检原生证书。
一个可实施的受限查询必须采用实际尺寸和变量实例的成功提取，并排除字体层面的
COLR／CPAL 与 bitmap 分流；否则返回未知。还需证明提取设置与实际 lowering
相同、空 encoding 不产生绘制支持，以及删除部分 glyph 后的命令片段顺序。
本次静态核验尚未补齐这些证据，没有据此添加能力或跳过输出。

另一个更小的待核验域是实际选中 `Glyf` 格式且
`loca.get_glyf(glyph, glyf)` 成功返回空记录的字形。skrifa 的 glyf loader 在该
分支只设置 phantom points；gvar 只调整这些度量点，不增加轮廓点或 contour。
这有机会证明零路径操作不依赖 ppem，避免按每帧 DPI 再次提取。准入仍须核对
实际格式优先级（VARC 高于 glyf）、glyph 可获取、颜色／bitmap 分流和完整资源版本。
零 contour 的非空记录、复合字形、CFF 和未知格式不由这个域自动涵盖。
该静态分支的首个 GPU 夹具对照已通过，公开消费合同尚未建立，
不属于已实现优化。`novadraw-backend-vello` 的
`empty_native_glyf_domain_preserves_gpu_pixels_without_fragmenting_runs` 只在测试中
使用上述准入，排除缺失 glyph，核对空记录在四个尺寸、三个变量坐标下不产生
路径操作。Inter 混合空／非空 run 经压紧后，三个设备缩放、斜体与平移组合、
Area／MSAA8／MSAA16 共 36 对 GPU readback 逐字节一致，且参考输出确有可见像素。
该证据只覆盖此字体夹具；全部空 run、颜色／bitmap、资源替换、policy/session
失效、公开外部消费者、留存计费和整体性能仍待补齐。生产代码没有使用该测试 helper。

删除 run 中间的空字形会增加有序片段。候选可以在资产构造时生成保持原位置的
紧凑绘制 backing，避免逐帧新增一条命令对应一个片段；原始 glyph backing 继续
服务布局和交互。新 backing 是输出算法资产，必须计入构造、留存和淘汰费用，
不能声称这一方案全链路零复制。两种表示均需用实际空字形比例比较净成本，
通过之后才选定生产表示。

### 后端空记录成本试验

公开证书通道实施前，先独立测量后端删除空记录的净成本。试验只改变 Vello
单色填充的 glyph iterator，不改变 Core IR、布局、资源校验或原生曲线 lowering；
这不是录制前裁剪的替代实现，也不关闭步骤 4。它用于决定是否值得增加静态资产。
后端按完整资源 ID、revision 和 collection index 缓存空记录位图；
准入采用上面的实际 Glyf 格式与无颜色／bitmap 条件，不查询字符或 advance。
未知字体、glyph、格式及位图分配失败保留原生输出。描边继续完整输出。

位图按字体 glyph count 有界，最多留存 16 个 face，满容量时新 face 使用完整路径；
资源移除、替换和 session snapshot 清除相关记录。统计构造时间、位图容量、
Vello 实际 glyph 数和 Core 命令数；不得把 Core IR 未减少写成已减少。
混合／全部空 run、变量坐标、AA、资源替换与 session 恢复先通过定向验证，
再用同配置拖拽、滚轮、resize、首次与追加对照决定保留或退役。
这项内部试验不增加公开可替换能力；公开 provider／policy 合同仍按下节实施。

## 生产合同必须包含什么

原生墨迹与设备覆盖应分别证明，不能用逻辑行高度替代二者。

| 证据 | 必须对应的输入 | 缺失时的行为 |
|---|---|---|
| 原生墨迹包络 | 字体资源及 revision、face、glyph、变量坐标、尺寸、合成变换和字形格式 | 保留该字形 |
| 设备覆盖包络 | 实际 lowerer、AA 模式、数值精度、设备缩放、绘制和 clip 变换 | 保留该绘制片段 |
| 来源一致性 | 同份受检布局、来源映射、墨迹资产及发布版本 | 拒绝候选或使用完整旧发布 |
| 可剪枝性 | 有限数值域、可逆 clip 换算、误差界与实际绘制操作 | 退化为完整录制 |

证明应绑定实际操作和变换的数值域。当前
`filled_path_coverage_outset_pixels` 是后端的填充路径声明，
没有原生 glyph 的 provenance，也不能表达坐标相关的转换误差。
不能把它直接扩充成原生 glyph 的通用证书。

## 建议的下一项设计

先定义可替换的原生墨迹 provider 与操作级覆盖检查角色。
provider 产出不可变、带来源依据的已知、空或未知包络，
不把 GPU 格式放进 Core 布局。覆盖检查使用该帧实际设备缩放、
变换和 lowerer policy；不满足受检数值域时返回未知。
内置与外部实现共用同一准入、预算与失败通道，
不能按 Parley、Vello 或某个内置 Figure 类型建立特权分支。

以下准入与查询通道补齐
[滚动与 resize 评审](../../verification/reviews/long-text-scroll-resize-review-2026-10-28.md)
SR-R01 的设计响应，尚未证明接口可实现或满足性能预算。
只在后端收到全文 IR 后计算包络，无法减少已经发生的 Core 录制成本；
backend 状态不写入 source 或布局缓存。
本文定义待复核角色与调用顺序，不冻结尚未验证的公开类型名。

可替换轮廓引擎可继续用于验证已知几何的索引，
但把默认原生输出切换成曲线路径会改变渲染路径和成本，
需要单独取得像素与性能证据，不能作为默认 Large 提速的替代证明。

## 准入与查询通道

静态墨迹与设备相关证据分别持有，绑定关系由 Runtime 本帧创建：

| 角色 | 所有者与内容 | 生命周期 |
|---|---|---|
| 静态墨迹资产与索引 | 文字墨迹 provider；绑定字体资源版本、face、变量坐标、glyph 与受检布局，提供已知／空／未知包络和保守摘要 | 不可变资产，随文字、字体或 provider 依赖失效 |
| 操作覆盖 policy | 实际 backend 的可选能力；描述实际 lowerer、AA、数值格式、转换顺序和有效域，提供只读检查入口 | backend/session 与 policy 版本内有效，不存入文字布局缓存 |
| 本帧覆盖上下文 | Runtime；绑定资源版本、backend/session、policy 身份、SurfaceInfo，供 Recorder 查询 | 本次 frame preparation 与 submission，不能跨帧隐式复用 |
| 操作查询 | Recorder；提供当前填充／描边等操作、字体实例、clip／变换和静态摘要 | 生成对应 IR 之前，每次使用实际绘制状态 |
| 提交核验 | backend；核对绑定并采用对应 policy，拒绝不兼容输入 | 发出该帧绘制 GPU 工作之前 |

可选能力使用统一 typed identity、payload trait 和实例 registry。
默认缺失或未知；内置和外部 provider/backend 共用同一入口与受检结果。
不能用诊断字符串作行为身份，也不能用填充路径 outset 字段冒充原生覆盖能力。
检查入口只读取已冻结的 policy 和资源描述，不反向访问或修改附着的 Runtime。

拟议调用顺序为：

1. 文字准备阶段从已验证的字体资源和布局生成静态墨迹资产与摘要，
   在受检候选发布时安装；backend policy 尚未参与，也不生成设备证书。
2. Runtime 在帧准备前获取实际 backend 可选能力，冻结 session／policy 和资源版本。
   能力声明须能追溯到实际 lowerer；只有接口名或通用像素常量不足以准入。
3. Runtime 将本帧只读上下文下传 Recorder。Recorder 在生成原生 IR 前，
   用当前操作、变换、clip 和摘要进行覆盖查询，获得可保守筛选的范围或未知。
4. Recorder 保留原序中的合法片段，并将必要资源依赖及 policy 绑定带入 submission。
   保留 glyph 的原位置与绘制语义；省去输出不免除来源或资源依赖。
5. backend 在资源同步后、实际编码前核对 session、policy 与资源绑定。
   若实际 lowering 已变化，拒绝该裁剪提交；Runtime 按既有恢复和重新录制合同
   获取当前有效输入，或在能力未知时完整录制。

步骤 3 不能依赖步骤 4 的全文 IR 才生成证明。
步骤 5 不能以保守回退为名直接使用已删除 glyph 的 IR 与新 policy；
需要重新录制完整内容或重新取得有效证明。
资源更新导致静态资产失效时，按准备和发布合同重验，不绕过布局版本校验。
任何 source mutation 已提交后，拒绝候选或提交不表示源已回滚。

## 每帧检查的成本边界

字体墨迹、包络和索引按不可变资产版本创建，留存预算包含旧 lease。
本帧创建上下文可以按实际唯一字体依赖受检去重，
不先遍历全文 glyph 生成或刷新全部设备证书。
绘制操作仍分别检查当前 transform、clip、AA、paint 类型和数值域；
相同实例的检查只有完整本帧输入与依赖相同才可复用。

先对段落／子树的静态包络摘要取得设备范围证明，
能证明不相交的已知摘要直接剪枝；否则下降到必要候选或未知后代。
未知摘要只约束对应分支，不能使已知 sibling 丧失剪枝资格。
未知叶子保留完整原生输出，不能按 line box、glyph advance 或逻辑段落位置删除。
跨行墨迹、高度重叠、复杂变换或数值域失败允许全扫描和完整输出。

H 计入访问的段落／子树摘要，C 计入需下降的候选，
G 保留总 glyph 分母，V 记录实际提交 glyph。
另记录设备检查次数、资源重验、未知分支、完整退化和静态资产构建量。
区分静态构建与本帧检查时间，已知／未知／退化分别记录；
失败前工作计入预算，不宣称所有输入都满足可见量复杂度。
CPU 可见查询时间包括上述检查，不能把证书工作藏到计时边界之外。

## 验收分母

必须同时覆盖普通与远处墨迹、负 bearing、合成斜体、变量字体、
非轮廓字形退化、非均匀缩放、嵌套 clip、大 origin 与抵消平移、
奇异或不满足数值域的变换，以及旧新 damage 修复。
GPU 对照使用实际后端与原始提交，裁剪结果逐像素对比完整结果。
失败、未知包络和退化次数单独记录。
另覆盖 session 更换、同 session 的 policy 更新、字体替换、
填充／描边或 AA 操作变化、录制到提交之间绑定失效，
以及混合已知／未知分支和每帧摘要检查成本。

原 Large 的滚动验收另外记录段落检查 H、候选 C、总 glyph G、
实际提交 V、IR 耗时及 GPU/呈现终点。
有限轮廓夹具通过或 CPU 稳定化提速，都不能关闭这个验收。
在合同与证据补齐前，生产 Vello 保持未声明通用覆盖范围，
默认原生 glyph 保持完整录制。
