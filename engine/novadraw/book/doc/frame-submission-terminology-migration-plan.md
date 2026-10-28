# “帧出版”术语拆分与迁移方案

类型：`book-maintenance-plan`

状态：`completed`

完成日期：2026-10-14

适用范围：

- `book/user/`：《Novadraw 应用开发与扩展指南》
- `book/internals/`：《深入理解 Novadraw》
- `book/contributor/`：《Novadraw 贡献者指南》
- `book/WRITING_PROFILE.md` 和三本书的维护规则

## 1. 协作顺序

当前工作区已有 Agent 修改“投影”和“场景事实”相关术语。本方案会触及部分相同文件。
执行前应等待这些修改完成或暂停，再读取最新工作区。不要按照本文引用的旧句子或旧行号
覆盖并行改动。

推荐顺序：

```text
完成正在进行的术语迁移
-> 重新统计“出版”和 publication
-> 按语境分类
-> 更新公共术语规则
-> 修改三本书
-> 构建和抽查
```

## 2. 问题

“帧出版”来自 `frame publication` 的直译。它试图表达一条重要边界：录制结果经过
Runtime 的稳定化、资源校验和身份分配后，才会成为正式的 `RenderSubmission`。

架构含义成立，但“出版”在中文里主要用于书籍、刊物和面向读者的内容发布。放在帧协议
中会产生几种误解：

- 读者可能把它理解成书稿出版；
- “帧已出版”看不出 Host 是否已经调用后端；
- “帧已发布”又容易被理解成像素已经呈现；
- “出版权威”像组织职责，不像一个明确的 API 边界；
- `publication` 在通知、拓扑和稳定观察中还有其他含义，不能统一翻成“出版”。

读者正文应直接写清当前阶段产生了什么对象、谁持有它、下一步由谁执行。

## 3. 协议事实

`runtime.frames().prepare_submission(...)` 成功时，Runtime 已经完成：

1. 派生状态收敛；
2. 表现快照与重绘区域计算；
3. 绘制命令录制和校验；
4. backend capability 与设备输入校验；
5. 资源快照或增量冻结；
6. `session_id` 和 `frame_id` 分配；
7. in-flight 状态建立。

返回值是：

```text
FramePreparation::Ready(RenderSubmission)
```

此时尚未发生：

- Host 调用 `RenderBackend::submit`；
- Backend 返回 `RenderResult`；
- Runtime 执行 `complete_submission`；
- Runtime 返回 `SubmissionAcknowledgement`；
- 后端确认 `Presented`；
- 像素完成呈现。

所以，“帧已提交”和“帧已呈现”都不能替代当前的“帧已出版”。

## 4. 术语裁决

### 4.1 帧提交准备

“帧提交准备”指 Runtime 从稳定场景状态生成 `RenderSubmission` 的完整过程。

适用语境：

- `prepare_submission` 的职责；
- Runtime 与独立 `CommandRecorder` 的边界；
- 录制错误阻止生成正式提交包；
- Runtime 分配 session/frame 并建立 in-flight 状态。

章节标题和机制总称优先使用“帧提交准备”。

### 4.2 提交包已就绪

“提交包已就绪”对应：

```text
FramePreparation::Ready(RenderSubmission)
```

它是“帧已出版”的直接替代词。该状态表示 Host 已经可以取得完整提交包，但还没有调用
后端。

需要写完整名称时使用“渲染提交包已就绪”。

### 4.3 提交后端

“提交后端”指 Host 调用：

```text
RenderBackend::submit(&RenderSubmission)
```

只有越过这一边界，才可以写：

- Host 已向后端提交；
- 提交已经发起；
- 后端正在处理或已经返回结果。

不要把 Runtime 返回 `FramePreparation::Ready` 写成“已经提交后端”。

### 4.4 帧已呈现

“帧已呈现”只对应：

```text
RenderResult = Ok(RenderDisposition::Presented)
```

`Skipped`、`Retry` 和所有 `RenderFailure` 都不能写成“已呈现”。

### 4.5 完成已确认

“完成已确认”指 Host 把完整 `RenderResult` 交回 Runtime，Runtime 完成：

```text
complete_submission(...)
-> SubmissionAcknowledgement
```

该阶段确认资源基线、完整重绘需求和 backend session 恢复动作。

## 5. 推荐生命周期

正文统一采用下列顺序：

```text
场景状态稳定
-> 表现快照形成
-> 绘制命令录制完成
-> 渲染提交包已就绪
-> Host 提交后端
-> 后端返回 RenderResult
-> Runtime 确认完成结果
```

每个阶段回答不同问题：

| 阶段 | 已经成立 | 尚未成立 |
|---|---|---|
| 录制完成 | 命令语法和状态栈有效 | 没有 frame/session 身份 |
| 提交包已就绪 | `RenderSubmission` 完整，已建立 in-flight | 后端尚未处理 |
| 提交后端 | Host 已调用 backend | 不保证 Presented |
| 后端返回结果 | disposition 或 failure 已知 | Runtime 尚未确认资源状态 |
| 完成已确认 | acknowledgement 已形成 | 是否继续调度由 Host 决定 |

## 6. 替换表

| 当前表达 | 建议表达 | 使用条件 |
|---|---|---|
| 帧出版 | 帧提交准备 | 指 Runtime 的完整准备过程 |
| 帧已出版 | 提交包已就绪 | 对应 `FramePreparation::Ready` |
| 出版帧 | 生成渲染提交包 | 使用动词时 |
| 出版提交 | 生成提交包；准备提交包 | 根据句子选择 |
| 帧出版入口 | 帧提交准备入口 | 指 `prepare_submission` |
| Runtime 出版权威 | 只有 Runtime 可以生成正式 `RenderSubmission` | 避免角色名词 |
| 出版前 | 提交包就绪前；准备完成前 | 写明阶段 |
| 本帧不得出版 | 本次不能生成提交包 | 录制或校验失败 |
| 未出版命令容器 | 未形成提交包的命令记录 | `RecordedDrawing` 等 |
| 录制不等于帧出版 | 录制完成不等于提交包就绪 | 章节结论 |
| 帧出版与恢复闭环 | 帧提交与恢复闭环 | 包含 Host/backend/ack |
| frame publication | 帧提交准备 | 读者正文 |

不要把所有 `publish` 机械替换为“帧提交准备”。不同语境按第 7 节处理。

## 7. publication 的其他语境

### 7.1 通知与稳定观察

事务完成后向观察者公开稳定结果，使用：

- 发布通知；
- 发布稳定结果；
- 稳定状态可见；
- 对外发布。

这里的 `publication` 可以翻译为“发布”，不使用“出版”。

### 7.2 拓扑和结构候选

候选节点、约束或关系尚未进入可见拓扑时，使用：

- 尚未发布到拓扑；
- 拒绝后不产生可见节点；
- 原子发布结构变化。

### 7.3 书稿和读者内容

下列语境继续使用“出版”：

- 书籍出版；
- 读者正文的出版边界；
- 哪些内容进入成书；
- 出版前的语言和版式检查。

`book/README.md`、`book/WRITING_PROFILE.md` 和各书 `UPDATE_PROMPT.md` 中讨论书稿边界的
“出版”不属于本次迁移对象。

### 7.4 规范与源码原词

以下内容保持原样：

- ADR 标题和既有规范名称；
- Rust 标识符、枚举和函数名；
- 代码注释中的 `publish` 或 `publication`；
- 历史审计证据；
- 引用原文时的英文术语。

读者正文引用 ADR-031 时，可以写：

> ADR-031 中的 frame publication，在本书正文中称为“帧提交准备”。

## 8. 文件级修改建议

### 8.1 公共写作规则

修改 `book/WRITING_PROFILE.md`：

- 在推荐译法中增加“帧提交准备”“提交包已就绪”“帧已呈现”“完成已确认”；
- 规定 Runtime、Host、Backend 三个阶段必须分开写；
- 把读者正文中的“帧出版”列为高风险直译词；
- 明确书稿出版语境继续使用“出版”。

“事实与出版边界”一节讨论的是成书边界，原则上保留原名。若该节同时讨论运行时协议，
应拆成“事实来源”和“成书边界”两个小节。

### 8.2 《深入理解 Novadraw》

#### `book/internals/src/README.md`

- “场景运行时”定义改为“场景事务、派生收敛、帧提交准备和完成确认的权威”；
- `RenderSubmission` 定义改为“Runtime 生成并交给 Host 的一帧拥有数据”；
- 不再写“Runtime 出版给后端”，因为 Runtime 不直接调用 Backend。

#### `book/internals/src/01-graphical-framework.md`

建议把四个边界写为：

```text
场景状态已稳定
表现快照已形成
渲染提交包已就绪
完成结果已确认
```

“帧已出版”改为“提交包已就绪”。同时保留 Host 调用 `submit` 的独立步骤。

#### `book/internals/src/07-rendering-and-frame.md`

重点修改：

- 小节“录制不等于帧出版”改为“录制完成不等于提交包就绪”；
- “本帧不能出版”改为“本次不能生成提交包”；
- “出版前”改为“提交包就绪前”；
- “Runtime 可以出版提交”改为“Runtime 可以生成带身份的 `RenderSubmission`”；
- “第二个帧出版入口”改为“绕过 Runtime 的提交包生成路径”。

保持现有边界：

- `RecordedDrawing` 不带 surface/session/frame 身份；
- `RenderSubmission` 已带完整身份和资源同步；
- Host 才调用 Backend；
- Runtime 负责 completion acknowledgement。

#### `book/internals/src/appendix-concept-map.md`

- “Runtime 帧出版”改为“Runtime 准备提交包”；
- “session 内一次出版提交”改为“session 内一次待完成提交”；
- “帧出版与恢复闭环”改为“帧提交与恢复闭环”；
- “录制不等于出版”改为“录制完成不等于提交包就绪”；
- Mermaid 图中保留 `FramePreparation::Ready(RenderSubmission)` 作为精确锚点。

#### 其他章节

检查：

- `03-pipeline-and-extension.md` 中泛化的 publish；
- `13-capability-model.md` 中属性变化发布；
- `14-correctness-and-evolution.md` 中 publication phase；
- `OUTLINE.md` 和思考题。

泛化的事务阶段使用“发布”，不改成“帧提交准备”。

### 8.3 《Novadraw 应用开发与扩展指南》

用户指南多数示例直接使用 `FramePreparation::Ready(submission)`，代码无需修改。

正文应统一说明：

- `Ready` 表示提交包已就绪；
- Host 随后调用 Backend；
- `Presented` 才表示帧已呈现；
- acknowledgement 决定恢复和后续调度。

不要把应用书中的“出版边界扫描”改掉。该词描述哪些材料进入读者正文。

### 8.4 《Novadraw 贡献者指南》

贡献者指南应使用真实代码名，同时采用新的中文说明：

```text
FramePreparation::Ready：渲染提交包已就绪
RenderBackend::submit：Host 提交后端
RenderDisposition::Presented：帧已呈现
SubmissionAcknowledgement：完成结果已确认
```

涉及 prepare、validate、commit、publish 的通用事务说明继续使用 `publish` 或“发布”。
只有 frame publication 改为“帧提交准备”。

## 9. 非目标

本方案不修改：

- `FramePreparation`、`RenderSubmission`、`RenderResult` 等 Rust 标识符；
- Runtime、Host 和 RenderBackend 的所有权边界；
- in-flight、资源同步和 session/frame 语义；
- ADR-031 的标题和正文；
- 历史审计报告；
- 代码注释；
- 书稿出版和读者内容边界中的“出版”。

若 ADR-031 未来需要改名，应单独发起架构文档任务。书稿 Agent 不得借术语清理改写已
接受 ADR。

## 10. 执行批次

### Batch A：建立术语入口

1. 更新 `book/WRITING_PROFILE.md`；
2. 更新三本书 README 和术语表；
3. 定义“出版”的保留语境；
4. 重新统计 `出版`、`publish` 和 `publication`。

完成条件：读者可以区分提交包就绪、提交后端、帧已呈现和完成已确认。

### Batch B：核心原理书

1. 修改第 1 章的帧生命周期；
2. 修改第 7 章的录制与提交边界；
3. 修改核心概念附录；
4. 修改第 14 章的通用 publication 表达；
5. 更新提纲、术语表、图示和交叉引用。

完成条件：核心原理书不再使用“帧出版”。

### Batch C：用户与贡献者指南

1. 检查用户指南帧准备示例的解释文字；
2. 更新贡献者指南 Runtime/Frame 章节；
3. 保留真实代码标识符；
4. 检查书稿出版语境没有被误改。

完成条件：三本书对 Runtime、Host 和 Backend 的阶段使用相同中文名称。

### Batch D：构建与人工复核

1. 构建三本书；
2. 检查 Mermaid 图中的阶段标签；
3. 抽查 PDF 中的表格和长标识符换行；
4. 审核剩余命中及保留理由。

## 11. 禁止的修改方式

- 禁止把所有“出版”替换成“发布”；
- 禁止把 `FramePreparation::Ready` 写成“帧已提交”；
- 禁止把 `Ready` 写成“帧已呈现”；
- 禁止省略 Host 调用 Backend 的步骤；
- 禁止把 `RecordedDrawing` 描述为正式帧；
- 禁止修改 ADR-031 和源码标识符；
- 禁止按本文旧行号覆盖并行 Agent 修改。

每个命中先归入一种语境：

```text
书稿出版
通用事务发布
结构或通知发布
帧提交准备
提交包已就绪
提交后端
帧已呈现
代码或规范原词
无法判定
```

“无法判定”需要检查对象类型、调用者、session/frame 身份和 in-flight 状态。

## 12. 验收

### 12.1 定向搜索

```bash
rg -n '帧出版|出版帧|出版提交|出版权威|未出版|frame publication' \
  book/user book/internals book/contributor book/WRITING_PROFILE.md
```

读者正文中的帧协议命中应清零。剩余“出版”只允许用于：

- 书稿和读者内容边界；
- ADR/规范名称；
- 历史原文；
- 本术语迁移计划。

继续搜索：

```bash
rg -n 'publish|publication|帧发布|发布帧' \
  book/user book/internals book/contributor
```

每个命中都要说明属于通用发布还是帧提交准备。

### 12.2 语义检查

- `RecordedDrawing` 不带 session/frame 身份；
- `FramePreparation::Ready` 表示提交包已就绪；
- Runtime 在返回 `Ready` 前建立 in-flight；
- Host 调用 `RenderBackend::submit`；
- `Presented` 才表示帧已呈现；
- Runtime 根据完整 `RenderResult` 返回 acknowledgement；
- 后端失败不回滚已提交场景状态；
- 书稿出版语境保持原意。

### 12.3 自动验证

```bash
cargo xtask docs
git diff --check
./tools/scripts/build_book.sh
```

构建后抽查：

- 用户指南系统模型和帧准备示例；
- 核心原理书第 1、7、14 章；
- 核心概念附录；
- 贡献者指南 Runtime/Frame 章节；
- 三本 PDF 中的 Mermaid 标签。

## 13. 交付要求

执行 Agent 应报告：

1. 最终术语表；
2. 帧协议相关替换数量；
3. 书稿出版语境的保留位置；
4. 保留 `publish/publication` 的位置和理由；
5. 无法判定而未修改的位置；
6. 三本书的构建结果。

不要把本方案加入任一本书的 `SUMMARY.md`。它是维护文档，不属于读者正文。

## 14. 执行结果

本次迁移完成以下修改：

1. 公共术语真源增加“帧提交准备”“提交包已就绪”“提交后端”“帧已呈现”和
   “完成已确认”；
2. 三本书及其维护规则统一 Runtime、Host 和 Backend 的阶段名称；
3. 迁移 16 处帧协议旧表达；
4. 把 12 处通用事务 `publish/publication` 改为“发布”，避免与帧提交准备混淆；
5. 用户指南的帧驱动伪代码改为通过 `runtime.frames()` 取得 facade，并显式处理
   `SubmissionAcknowledgement`；
6. 读者正文中的“帧出版”“出版帧”“出版提交”“出版权威”和
   `frame publication` 命中清零。

保留项：

- `book/WRITING_PROFILE.md` 和三本书维护规则中的“出版”继续表示书稿出版、成书边界或
  读者内容筛选；
- 本方案保留旧词，用于说明迁移对象和验收搜索；
- `book/doc/projection-terminology-migration-plan.md` 中的 `publish` 属于另一份维护
  方案对旧事务阶段的记录，不属于读者正文；
- ADR、源码标识符、代码注释和历史审计证据均未修改。

无法判定而未修改的位置：0。

验证结果：

- `cargo xtask docs` 通过；
- `git diff --check` 通过；
- `./tools/scripts/build_book.sh` 通过，三本书的 HTML 与 PDF 均已生成；
- PDF 文本抽查未发现旧运行时术语；
- 原理书第 1、7 章和概念附录、用户指南第 4 章、贡献者指南 Runtime 章节的图表与长
  标识符显示正常。
