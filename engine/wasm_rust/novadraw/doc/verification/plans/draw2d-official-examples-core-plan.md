# 官方示例 Core 实施计划

类型：`verification`

日期：2026-10-19

状态：已获准按计划实施，2026-10-19；按 K00 → K01 的顺序逐切片推进

## 目标与边界

补齐 [全25入口矩阵](draw2d-official-examples-remaining-contracts.md#全部入口逐项矩阵)
中的通用平台无关契约，并逐项关闭官方 Figure 构成和功能差额。K00–K09/H01–H04
只作为本计划任务ID，不创建新的M/G编号；正式delta归入
[P2 backlog](../../roadmap/p2-delta-backlog.md)，已有完成态不改写。

Core拥有Figure树、几何、测量/布局、输入分发、资源、route/range、稳定更新和
presentation/输出协议。算法可替换实现、widgets、border样式使用相同公开扩展模型；
Winit/Web/native control、Vello栅格和Printer留在对应adapter。
官方样例的选中目标、配对关系、caret 高亮和 fake typing 状态由示例控制器持有，
消费 Core 的输入、图形更新、文字几何和绘制契约。原生 Text/Combo 的编辑状态由
对应控件拥有。本计划不新增 Editor 会话、Command 或 Viewer 消费要求，也不把
业务编辑状态放入 Core TextFlow。

优先级按“共享契约先于单例移植、基础输入/测量先于派生、source先于presentation”
排列。S08/S09当前未启动，先核对它们与本计划输入/错误范围，接受对应设计后推进，
避免另开并行整改主链。以下各步必须做到：源码契约→现状定向核对→最小ADR/差额→
外部消费者测试→实现→相应suite→文档/入口状态更新。

## 实施步骤与退出条件

2026-10-19 实施进度：K00 已提交 25 入口源码/helper/resource 清单及固定基线验证，
全事件/platform manifest 与 Java 运行取证尚未完成。K01 已提交延迟 style/cursor；
本切片增加 owned subtree、完整准入与成功/失败回调，并接入 TreeExample More Children
和选中 Label 白字/取消继承；已验证拒绝消费、FIFO 和结果回调 panic。
资源准入已覆盖内置与外部 Figure 的 typed 声明，拒绝 foreign/retired 并允许 pending；
Runtime / Viewport 的同步与事件期完整 contents 替换已接入同一子树预检和回执，
单 Figure contents 同步复用资源准入，非法新输入保留旧场景。
配对 Connection binding 已接入整树预检和发布，保留构造身份与成功回执分离，
同一路径支持外部 Anchor/Router，并接入 ConnectionAndAnchor 两组端点。
其他旧单节点资源准入及重入测试
仍待推进，K00/K01 均保留未完成。契约见
[ADR-009 增量](../../adr/adr-009-runtime-dynamic-mutation-contract.md#k01-完整子树与挂载回执增量)。

### K00 全入口基线与预检

交付：验证manifest记录25个main、helper、初始树、全部控件/fixture、资源、
事件顺序、source调用和平台依赖。将历史blocked拆成“已确认契约缺口、移植工作、
平台未支持、仅证据不足”，不将四者折叠。

动作：

1. 从固定Java基线提取每个入口的feature清单，包含inactive/unused方法的排除说明。
2. 捕获结构/name/order/layout/border/constraint及字体/DPI，取证mouse/key/focus/
   pack/resize/close，不以33项Rust测试当25入口全feature验收。
3. GraphTests固定16个、CompoundGraphTests固定10个菜单fixture，两方向均登记；
   反射默认顺序随Java运行捕获，避免只凭源文件排序。
4. 验证器将source/factory/status绑定，不把没有实现的入口绑定替代factory。

验收：25/25入口均映射K/H或“已有能力待核验”；每个缺口有可观测失败反例。
没有实现改变，执行docs/diff即可；源码manifest验证命令注册后再执行。

### K01 Runtime 写入、结果与完整构造

消费：Tree新增子树；Grid Add/Clear/属性；Obstruction障碍和配对端点；两graph与
ShapeStyles整场景替换；TextFlowLarge fragment/reparent；InternalFrame cursor。

先复用：S01 indexed+constraint、F02 typed capability mutation、S03 scoped facade、
S06整批准入/失败清理、S07受检query。现有批量append到已存在父级不等于挂载描述中
尚未分配ID的完整新子树。先验证public API与host事件结果处理是否已足够，再选
typed owned request/receipt或construction-only subtree，禁止先为Tree发明专用接口。

交付契约：

- 请求固定所属Runtime、目标、顺序、payload与失败通道；Figure回调不取得完整可变
  Runtime，事件结束后在稳定边界执行，nested dispatch顺序可复放。
- subtree内部暂存身份不能冒充FigureId；ID/binding/资源映射须完整预检。成功后的
  receipt使外部model更新，拒绝时model不发布虚假的children。
- 同步和延迟写入复用同一typed机制。通用style/text/cursor更新保留继承和清除语义，
  例如selected Label白字/取消后恢复inherit、mouseExited清除cursor。
- 非法geometry/style/constraint/resource预检拒绝零写入；callback panic可能保留
  真实已挂载前缀并fault，沿S06清理报告处理，**不承诺任意扩展副作用全回滚**。
- 能以app事件消费实现的业务意图留在app；需复用的事件提交机制位于Core。

验收：外部crate构造有Label/Border/Layout的至少两层subtree、配对节点binding、
整场景替换与目标style/cursor；foreign/stale、child limit、10,000深度、末项拒绝、
重入、panic、提交与model receipt时序；无运行时日志进入渲染热路径。

### K02 测量、布局、边框与坐标

消费：Flow/Toolbar/Grid/EllipseHitting、SimpleTree/Tree、ScrollPane、两Locator示例、
UML/Zoom、MultiLineLabel。

动作：

1. 以Java尺寸oracle核对preferred/minimum/hints、insets、background/foreground/font
   inherit、child-local坐标与damage新旧包络，区分算法偏差和host固定尺寸偏差。
2. Flow测horizontal/vertical、wrap边界、主次对齐、stretch；Toolbar测shrink、
   minimum与stretch/reset；Grid测span/grab/等宽/indent/margins/hint及零/小容器。
3. 保留官方setSize→resetShapes语义，不能用永久preferred-size覆盖可变size而改变
   layout。GroupBox、TitleBar、Scheme、FoldedPage、DogEar保持owner font metrics、
   绘制顺序和bounds/insets；不因样式缺失增加假容器。
4. 按下→拖动→release→resize/scroll/zoom后核对坐标，尤其官方绝对坐标迁移与
   scaled client area。事件点转换继续由Core处理。

验收：相同输入生成尺寸与局部/绝对bounds oracle；嵌套层级/partial damage和runtime
replacement无陈旧测量；叶子/隐藏/空children、不可逆变换、失效位置结构化拒绝。
只修已证明缺口，不重写受保护递归主循环。

### K03 Locator 与官方绕障路由

Locator：扩展公开placement输入，使外部locator可读取owner几何、被定位child受检
measurement及统一坐标域。复用connection direct-child binding和route batch；
避免为内置EndpointLocator专设owner/size入口。
默认u=14/v=4、四象限、无owner fallback、v<0/0/>0、切线tan限制、整数舍入，
均对照ConnectionEndpointLocator.relocate。ConnectionAndAnchor和UML必须同时验。

路由：另立官方visibility graph实现身份，现有正交ShortestPath保持不变；移植
Path/Obstacle/Vertex/Segment对应算法，不使用全局static，不用GraphLayout代替路径
绕障。共享拐角路径顺序与spacing=4、dirty path/obstacle增删更新均是目标。
算法数据/identity与FigureId分离；adapter消费稳定batch snapshot并通过现有registry
返回route，不直接paint或写Connection真值。

验收：Java/Rust单障碍双路径oracle，包括
`(0,50)→(36,83)→(63,83)→(100,50)` 与
`(0,55)→(32,87)→(67,87)→(100,55)`；
多障碍、多路径共享角、路径增删、随机初值、输入顺序及不可达/退化恢复；
binding admission失败无陈旧路由，算法实例隔离。
Obstruction的parent mismatch需原版执行取证；不得无依据增加成对删除。

### K04 控件组合、repeat 和 range

消费：Zoom/Path/ScrollPane、三layout/Tree、Bidi、MultiLineLabel，复用P2-W01规划。
Core只承担通用input/action/capture/clock/range，具体widget行为与样式在可替换组件
层；原生Text/Combo/Group的platform实现归H01。

动作：

1. ScrollBar用up/down/pageUp/pageDown/thumb五个真实child，外部替换thumb/按钮仍
   使用同一role identity与layout约束；不以单Figure命中区域代替官方构成。
2. 建立repeat scheduler与runtime monotonic clock共享调度，press/drag-out/back/
   release/disable/hide/dispose/focus-loss/capture-loss取消。repeat不是animation，
   禁用动画不能禁用按住连发。
3. range显式owner/共享binding/订阅释放，clamp规则、extent与value写入原子闭合。
   两view共享vertical range要裁决extent/max冲突和循环；不要事件互相复制value。
4. integer Scale、键盘increment/page、radio group、Text/Combo editable/read-only/
   default-selection/focus-out明确角色；Text编辑状态不放在TextFlow图元。
5. MultiLineLabel作为readonly复合widget保留focus ring、Margin2、mirror keyboard、
   自动scrollbar及readonly accessibility；不强制创建EditorDomain。

验收：fixed-time repeat、单次/连发/取消action数量、无活动零wakeup、五children替换
与真实拖动；shared range删view释放；DefaultSelection/FocusOut按源码事件oracle；
readonly text控件无Command。未知platform key不能静默视为已支持。

### K05 styled Flow、inline Figure 与文字绘制

消费：TextFlow/Bidi/Caret/MultiLineLabel/TextFlowLarge/TextSurrounding、Path；
Label/字体链路也服务UML/Zoom/Print。归P2-T03，复用T01/T02/G02。

动作：

1. 冻结稳定fragment/paragraph/inline identity与嵌套表示，区分logical text模型、
   flow composition与真实Figure children；保留多font/color inherit和baseline。
2. block base direction与LEFT/RIGHT相对语义、ALWAYS_LEFT/RIGHT绝对语义、Canvas
   mirror分别建模；hard newline/no-wrap与soft wrap独立。
3. FlowBorder读取每行fragment box、lead/trail及owner font metrics；Caret的
   TestBorder不得退化成整个TextFlow外框。
4. FlowAdapter接入真实child subtree测量、baseline/行内占位/换行、绘制/clip/hit/
   damage与children生命周期；第二proxy内RTL FlowPage仍走同一Flow协议。
5. fragment mutation/reparent保留位置身份及局部失效；TextFlowLarge只追加A–Z/a–z/
   空格到target，不改其他fragment。tooltip走标准宿主协议。
6. immutable TextLayout/InteractionMap提供UTF-8边界、affinity与revision查询；
   上游UTF-16闭区间适配明确，错误不clamp。静态TextLayout selection/background/
   underline范围作为绘制输入，支持Path两次drawTextLayout，**不拥有可变selection**。

验收：六段TextFlow结构；20组large与400字体fragment、首target reparent；
两个FlowAdapter树；Bidi两侧十段；不同字号baseline/颜色inherit/fragment border；
Path文字partial underline与rotation；stale position、grapheme、emoji、跨line/inline
hit/selection；Caret 的临时高亮与 TextFlowLarge 追加由示例控制器驱动，原生
Text/Combo 的完整输入行为归 K04/H01，不依赖 Editor direct-edit session。
外部TextLayout provider和inline Figure必须走相同受检输出及失败通道。

### K06 图布局与树动画

graph归P2-L01：定义不依赖FigureTree/Editor的有向/compound模型输入和几何输出。
保留node/edge/containment身份、尺寸/padding/insets/weight/delta/direction；
输出rank/feedback/vNodes/edge points/group bounds。算法与Figure投影adapter分开。
依次移植transpose、break cycles、rank、virtual nodes、vertical、cross-min、
horizontal以及compound对应步骤；用全fixture和不变量核验，不以通用DAG简化替代。
官方compound子图视觉平铺，adapter保持source composition。

tree的normal/hanging/rows/contour是可替换Layout实现，不属于Editor TreeViewer。
核验compressed深层5px clearance、negative overlap correction、远兄弟slack、
mixed rows、collapse空pane、LEFT/CENTER与transpose。

动画归M01窄增量：沿ADR-026使用source final与presentation，实例级210ms、linear/
整数sample、中心reset、reverse collapse、viewport tracking和中间连线几何一致。
先比较已有bounds/route/viewport transaction能否组合；仅为缺失的显式capture、
temporary subtree/依赖绘制增加通用contract。不能每帧写source布局、copy singleton
或另建private timer。同步host与事件期调用适配统一由K01承担。

验收：Directed32/Compound20 fixture组合；cycle/孤立/无边/空图/非法ID/尺寸/超深
containment；第三方算法输出原子拒绝；两tree同size oracle；动画disable/reduced/
cancel/retarget/hide/dispose/重入/viewport变更，line/title共享presentation epoch。

### K07 实时快照、共享视图与背景源

消费：Thumbnail、Bidi共享range、InheritBackground、Tree tracking。
依赖K01/K04/K05，复用G02 immutable presentation与render/resource协议。

动作：

1. source Runtime是唯一树/资源owner；只读recorded subtree或snapshot描述携带
   source identity、stable epoch、bounds、resource references、damage与失效状态。
2. Thumbnail adapter可用缩放栅格快照/tiled cache，保aspect ratio并响应source变化；
   不复制业务model或再次mount source Figure；source replacement/unsubscribe有
   有界资源释放，多surface完成独立确认，不能共享单个in-flight完成句柄。
3. Bidi两棵独立内容共享range，不与Thumbnail同source合并；共同基础只在稳定
   view identity、range绑定、snapshot/host生命周期有真实复用时抽取。
4. 继承背景以source frame/phase/DPI/宿主region描述接入合成；native parent背景由
   H01产生，Core不保存SWT Composite。透明root不允许错误清除父级背景。

验收：source drag两个窗口下一有效epoch均更新、target resize保持aspect、hidden
target无无效wakeup、source disposal/late completion/namespace reset；两个range
view不同extent与关闭；背景scroll/resize相位/alpha/partial damage oracle。
平台中断不放宽Core identity或复制scene当实时投影。

### K08 Graphics、合法值与资源

与H02先做capability评审，避免后期发现Vello无法表达而返工。消费ShapeStyles/
Path/Zoom/Print，复用G01/G02/R02。

动作：

- 逐shape AA On/Off/Default明确Graphics状态、IR、backend capability及fallback；
  不支持必须在frame前结构化拒绝，并保留示例未完成。
- 定义SWT width0 hairline的设备像素语义和miter0/0.5/1等合法值映射，兼容现有受检
  stroke，不把画笔语义偷偷clamp成常量；fractional dash offset/alpha/enabled也核验。
- SVG原资源通过明确image provider处理，scale/DPI选择与raster cache/ResourceId
  生命周期统一；保留原asset内容，PNG替代不自动证明SVG缩放等价。
- Glyph装饰/selection绘制、rotated bounds与clip/measure使用同一layout快照；
  字体fallback和system theme由H01显式提供。

验收：八axis两两矩阵与五shape在合法取值下的IR及同平台栅格；X再Y覆盖顺序、
zero/large/fractional width与clip；SVG多DPI；拒绝无partial frame/resource泄漏。

### K09 输出目标与打印

消费Print，依赖K02/K08、O01；示例按钮直接调用输出适配入口。

交付：稳定source/output snapshot、logical/page/device域、DPI、printable region、
margin、tile/fit策略、clip/scale/text/image和cancel/error/completion。官方main只要求
默认TILE和零margin；其他模式源于PrintFigureOperation基础contract，不能把它们
说成main已提供控件。

执行：先纯page规划与受检输出协议，再实际printer/backend adapter，再PrintDialog
host与真实job/page。打印source白色背景用输出作用域/覆盖描述保证完成恢复，
不永久改屏幕source。拒绝或用户取消无job、失效source无陈旧输出。

验收：多page tile、DPI/非零bounds、font/image/connection及取消/error resource
释放；Print的三个字体Label/11个直接children和native Print it按钮保持。只有预览
或PDF不关闭真实打印交付。

## 平台与验收步骤

| 步骤 | 交付 | 依赖与退出 |
|---|---|---|
| H01 | Host composition含shell sizing/pack、native Group/Button/Scale/Combo/Text、Canvas/mirror、Caret、font/theme、focus/input/IME、多窗口退出 | K00定义native元素清单后先设计；Winit/Web缺控件bridge时单独明确adapter，不用Figure替代native对照 |
| H02 | Vello/backend AA/合法stroke/SVG、透明背景/parent合成、surface repair | K08/K07明确IR和资源边界；同平台oracle、device loss与late completion |
| H03 | Printer/dialog/job/page、取消、资源和输出设备 | K09 page协议；真实平台作业证据 |
| H04 | 每入口source、结构、算法、事件、platform视觉与失败manifest | K00，按各切片增量执行；25/25均闭合才宣称全样例对等 |

Native默认按当前macOS环境验证，Windows/Linux/Web按能力矩阵逐项登记；
没有测试的runner保留未完成。打印设备/原生控件不可用应单列平台状态，不能把它
转换为Core算法缺失。全部官方入口使用Core、扩展实现和平台适配，不增加Editor依赖。

## 交付顺序与门禁

| 轮次 | 顺序 | 可交付结果 |
|---|---|---|
| 准备 | K00 → S08/S09范围对齐 → K01/H01/K08 ADR评审 | 全量manifest、边界和失败模型，无实现空包 |
| 基础 | K01 → K02 → K03 | 真实动态构造/继承/Locator/绕障；先关闭多入口共享问题 |
| 组件 | K04 → K05 | ScrollBar/range/repeat，styled flow/inline，readonly/native对照职责完整 |
| 算法 | K06 | Directed/Compound全fixture，tree最终布局与presentation动画 |
| 投影 | K07 | live snapshot、多surface/shared-range/背景继承 |
| 输出 | K08验收 → K09 → H03 | Graphics合法值/backend与真实分页打印 |
| 全量 | H01/H02/H04逐入口最终验收 | 25入口feature闭合、证据可复放，catalog与parity同步 |

K08/H01的**设计与采证提前**，实现按依赖串行；不将所有功能绑成一次大改。
新suite先注册真实命令与consumer；复用现有text/layout/connection/animation suites，
不能编造已存在的新suite ID。小修改crate --lib/单target，切片对应suite，批次
workspace fmt/check，最终fmt/Clippy/test及Native/Web证据。docs改动只docs/diff。

每个新ADR包含外部消费者证据、调用域、状态owner、身份寿命、失败分类、通知/damage、
性能测量与退出路径。源码事实链接统一见
[全量方案证据入口](draw2d-official-examples-remaining-contracts.md#源码证据入口)；
本计划的步骤是设计提案，不能当作已存在API或已通过测试。
