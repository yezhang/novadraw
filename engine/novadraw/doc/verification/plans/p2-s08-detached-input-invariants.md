# S08 输入不变量验证矩阵

类型：`verification-plan`

状态：草案，待评审；2026-10-29。以下用例均为待执行。

契约见 [ADR-056](../../adr/adr-056-detached-input-invariants.md)及
[输入不变量专题](../../design/architecture/detached-input-invariants.md)。
本矩阵定义验收方法，不记录独立实施状态。

## 值、构造与样式

| ID | 场景 | 必须观察的结果 |
|---|---|---|
| S08-V01 | Opacity 输入 NaN、正负无穷、负数、大于 1、端点 0／1 | 非法输入结构化拒绝；端点合法；graphics／animation 消费同一个值 |
| S08-V02 | 字体正字号、weight 端点、Unicode family、`700 12px Inter Variable` | 原值保持；完整 token 解析；未知 family 进入正常 fallback |
| S08-V03 | 字体零／负／非有限字号、空 family、越界 weight、重复 token、非法前缀与多字节文本 | 无 panic、无截断、无默认伪成功；返回 TextError |
| S08-V04 | 父 alpha 0.5、子覆盖 0.8、另一个 child 未覆盖；字体覆盖／移除 | 依次为 0.5／0.8／0.5；不相乘；字体测量与绘制使用相同 descriptor |
| S08-V05 | 直接构造和 builder 的默认／显式 inset、stroke 更新 | 默认四侧为宽度；自动值随宽度，显式值保留；两条入口等价 |
| S08-V06 | Line／Rectangle 宽度零、负、非有限、有限却导致 stroke extent 溢出 | 构造与更新均拒绝；Graphics 零宽无绘制基线仍通过 |
| S08-V07 | Border 四侧逐一输入非法值、横纵合计溢出、TitleBar 二倍 padding 溢出 | 带字段／side 的结构化错误；失败不返回新有效对象 |
| S08-V08 | 默认 Margin／Compound／TitleBar／Etched／Scheme，零边距、不对称边距 | 默认不 panic；Margin 无可见绘制；optional 和空色带语义保持 |
| S08-V09 | Color 饱和、动画采样、viewport range、小 owner 截空、圆角适配 | 保留原先合法行为；非法输入校验发生在算法适配之前 |
| S08-V10 | Detached Figure 原始 bounds／stroke／corner 构造，translate 溢出和数值 setter | 非法输入构造／更新拒绝；Shape 零宽、负坐标、旧 corner dimensions 单位保持；各同类入口有覆盖清单 |

## 发布、外部实现与恢复

| ID | 场景 | 必须观察的结果 |
|---|---|---|
| S08-P01 | 外部 Figure 返回非法 initial bounds／insets；builder、Runtime add、subtree 候选 | 统一准入拒绝；无新增节点／登记／源 revision；已有状态保持 |
| S08-P02 | 外部 Border 返回 NaN／负 inset／非法 preferred；内置和外部嵌套 Compound | 无类型特判；相同字段校验与阶段错误；outer／inner source 可追溯 |
| S08-P03 | 各侧有限但组合溢出；TitleBar 文字指标加 padding 溢出 | 构造静态组合或 owner 准备时拒绝；不靠 clamp 产生成功 |
| S08-P04 | 外部 initial 属性回调计数；检查与提交间试图更换值 | prepare 捕获一次，同一候选值用于检查和发布 |
| S08-P05 | 已挂载 Figure 的 style／border／typed component 更新拒绝，再提交有效候选 | 旧值、源 revision、通知与 damage 保持；有效重试可恢复 |
| S08-P06 | 同值 style 和 border 配置更新、Deferred mutation 拒绝 | 同值无变化；延迟拒绝可从现有 Runtime 通道观察 |
| S08-P07 | 共享 TitleBar／Compound 的两个 owner 字体与尺寸不同，一侧准备失败 | owner 快照隔离；失败不污染另一侧，不安装过期成功快照 |
| S08-P08 | 合法负坐标、零尺寸、大 inset 和不可逆／设备不可表示变换 | 合法几何保持；S07 数值失败和后端精度失败按独立通道报告 |
| S08-P09 | 外部回调 panic | 使用既有 fault 语义；不报告为普通输入拒绝或完整外部副作用回滚 |

独立消费者使用公开领域导入、第三方 Figure／Border、普通 builder 和 typed
component/capability mutation。禁止测试靠 crate 内私有字段、concrete 分支或专属
扩展 hook 获得成功。至少一个完整场景包含构造、挂载、更新、失败和恢复。
构造 unit tests 与外部准入测试分别证明不同边界，不相互替代。

## 渲染与公共边界

| ID | 验证 | 证据 |
|---|---|---|
| S08-R01 | 原 raw alpha／font 类型、原数值字段 literal、旧 Opacity alias、panic constructor 不再可用 | 分别 compile-fail；API snapshot 核对唯一路径与新返回值 |
| S08-R02 | Border／Figure style 的真实测量、client area、命中、旧新 damage 与通知 | 既有 Border／文字／轮廓契约回归，加独立行为断言 |
| S08-R03 | 实线／dash、分边圆角、透明描边、小 owner 与共享标题 | Native retained GPU 像素与 Partial／Full 对照；公开 Graphics 消费 |
| S08-R04 | 无默认 feature、默认动画消费者、Native／Wasm API | 对应编译及消费者；Opacity 迁移不强制启用动画 |
| S08-R05 | 稳定帧、连续 resize、style 更新和嵌套 Border 指标准备 | 同环境前后 P50／P95、分配、复制、缓存及数据规模 |
| S08-R06 | 相同 release 配置与消费者的 Native／Wasm 产物 | 原始／压缩字节与首次资源；新增依赖、默认 feature 均核对 |

## 执行门禁

设计期只运行文档与链接检查。接受后内环按受影响 crate 的 `--lib` 或单个
integration target 运行 check、Clippy 和定向测试；切片完成使用当前
[suite 入口](../../../tests/README.md)中的对应契约门禁，不提前虚构 suite ID。

同类整改批次运行 workspace fmt／check，最终运行 workspace fmt、严格 Clippy
和 test，补齐 docs 与 diff 检查。既有环境或证据缺失须注明，不能勾选未运行用例。
平台发布验收可以保留，受影响的 Core 数值和 Native GPU 回归仍须完成。

## 关闭依据

完成记录逐项关联 V／P／R 的测试、命令、环境和证据，列出永久限制；
更新公开 API 与对应 accepted 文档后，才关闭 R03／TC-03／API-F04／API-10。
2026-10-29 的颜色文本回归仅证明 ADR-020 修复，不替代本矩阵。
