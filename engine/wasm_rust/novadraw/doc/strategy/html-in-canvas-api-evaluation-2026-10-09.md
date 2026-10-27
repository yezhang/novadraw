# HTML-in-Canvas API 技术评估

类型：`product-strategy`

日期：2026-10-09

证据截止：2026-10-09

## 结论

HTML-in-Canvas 对 Novadraw **有中长期战略价值，但当前不应进入 Core、默认 Web
渲染路径或正式兼容承诺**。

它最有价值的场景不是替代已有文字渲染、Tooltip、属性面板或 Accessibility Bridge，
而是让少量真实 HTML 控件或富内容作为画布内对象参与 GPU 合成，同时保留浏览器原生
文本选择、表单、查找、翻译和无障碍语义。该能力与专业图形编辑器的富节点、局部表单、
未来 3D 视口具有方向性匹配。[cite:1][cite:2]

当前建议为：

1. 将其登记为 Web 平台实验能力，不建立 P2 交付项。
2. 等待真实“画布内 HTML 合成”产品用例后，再做独立 POC。
3. POC 只允许落在 Web 平台、Vello backend 和应用 composition root 的协作边界，
   不向 `novadraw` Core 暴露 DOM、`ElementImage`、`GPUTexture` 或浏览器专有类型。
4. 继续保留现有 Parley/Vello 文本、Figure-native widget、DOM Tooltip、
   EditContext 和 Accessibility Snapshot/Delta 主路径。

## 1. 技术定义

HTML-in-Canvas 是 WICG 正在孵化的 Web API 提案。它让浏览器为 Canvas 后代 DOM
生成可绘制快照，再将快照绘入 Canvas 2D，或写入 WebGL/WebGPU 纹理。原始 DOM
仍负责语义、布局、表单行为和浏览器集成；Canvas/GPU 负责最终视觉合成。[cite:1]

截至证据截止日，当前 explainer 使用五组核心能力：

| 能力 | 当前接口 | 作用 |
|---|---|---|
| 布局启用 | `canvas content="drawable"` | 让 Canvas 后代参与布局和无障碍树 |
| 可绘制边界 | 元素 `drawable` 属性 | 建立独立可绘制子树和包含边界 |
| 失效通知 | `paint` / `requestPaint()` | DOM 快照变化时通知 Canvas 重绘 |
| 图像输出 | `drawElementImage`、`texElementSubImage2D`、`drawElementImageToTexture` | 输出到 2D、WebGL 或 WebGPU |
| 几何回写 | `updateElementGeometry` / `clearElementGeometry` | 同步命中、裁剪和无障碍几何 |

`captureElementImage()` 还能生成可转移到 Worker 的 `ElementImage`。Worker 可以消费
快照，但 DOM 布局和快照捕获仍起源于主线程，几何更新最终也要异步回到主线程。
[cite:1]

### 1.1 一帧内的因果顺序

```text
DOM/CSS/表单状态变化
-> 浏览器 layout + paint，锁定 drawable 快照
-> canvas paint 事件
-> Canvas2D 绘制，或把快照写入 WebGL/WebGPU texture
-> 更新元素 canvas geometry
-> 浏览器据此执行 hit-test 与 accessibility geometry
-> paint 回调内发生的 DOM 变化进入下一帧
```

这不是 DOM 节点真正迁入 GPU 场景树，而是“DOM 语义与布局 + 浏览器快照 +
Canvas/GPU 合成 + 几何回写”的双向桥接。`paint` 每个 rendering update 最多执行一次；
回调中的 DOM 变更不会递归重画当前帧。[cite:1]

## 2. 与现有替代方案的区别

| 方案 | 浏览器原生 DOM 行为 | Canvas 内任意合成 | 跨浏览器成熟度 | 主要用途 |
|---|---|---|---|---|
| 普通 DOM overlay | 完整 | 否，只能层叠在 Canvas 上下 | 高 | 面板、菜单、Tooltip、文本输入 |
| `html2canvas` / SVG `foreignObject` | 输出后丢失交互 | 得到静态位图 | 中 | 截图、导出、缩略图 |
| Canvas 自绘 + 独立 AX DOM | 需应用同步两套树 | 是 | 高 | 跨平台专业画布 |
| HTML-in-Canvas | 保留 DOM 语义和交互 | 是 | 低，仍在演进 | 富 HTML 参与 2D/3D GPU 合成 |

HTML-in-Canvas 的不可替代点只有一个：**真实 DOM 内容需要处在 Canvas/GPU 的视觉
合成链内部**。若只需要在画布上方显示控件，DOM overlay 更简单、更成熟，也不会引入
纹理更新和几何双向同步。

## 3. 标准与浏览器成熟度

### 3.1 当前状态

- `[CONFIRMED]` WICG explainer 仍标记为 living explainer；对应 WHATWG HTML
  规范 PR 仍是 open draft。[cite:1][cite:3]
- `[CONFIRMED]` Chromium Platform Status 将能力标记为“Prepare to ship”，Origin
  Trial 从 Chrome 148 延长到 160，并把 Chrome 157 列为预计 shipping milestone。
  这是计划状态，不等于截至 2026-10-09 已成为稳定跨浏览器能力。[cite:2]
- `[CONFIRMED]` WebKit 给出 positive 标准立场并开始实现；已落地的早期变更明确以
  IDL 和可编译骨架为主，功能实现后续补齐。[cite:4][cite:5]
- `[CONFIRMED]` Mozilla 标准立场请求仍未给出 positive 或 negative signal。
  [cite:6]
- `[CONFIRMED]` TAG review 已启动，但公开记录未给出完成结论。[cite:7]

因此，不能把“Chromium 计划发布”和“WebKit 正向信号”解释为当前可依赖的
Chrome/Safari/Firefox 互操作性。

### 3.2 API 仍在变化

2026 年 5 月的 Chrome Origin Trial 文章使用 `layoutsubtree`、
`copyElementImageToTexture` 和返回变换的 `drawElementImage()`。[cite:8]
当前 WICG explainer 已改为：

- `content="drawable"` 加元素级 `drawable`；
- `drawElementImage()` 返回 `void`；
- 显式 `updateElementGeometry()` / `clearElementGeometry()`；
- WebGPU 方法改名为 `drawElementImageToTexture()`。

WebKit 在 2026 年 9 月专门同步了这一轮 IDL 变化。[cite:5] 这说明 API 方向已经比
早期 retained-mode 方案稳定，但绑定层和应用代码仍应预期 breaking change。

## 4. 能力收益与限制

### 4.1 真实收益

1. **浏览器原生排版与控件**：CSS 富文本、双向文本、复杂脚本、表单控件和选择行为
   不再由 Canvas 应用重复实现。[cite:1][cite:8]
2. **浏览器语义能力**：DOM 内容可进入无障碍树，并可继续参与 find-in-page、复制、
   翻译、扩展和 DevTools。[cite:1][cite:8]
3. **GPU 合成**：DOM 快照可作为 WebGL/WebGPU 纹理参与 shader、变换和 3D 场景。
   [cite:1]
4. **Worker 路径**：`ElementImage` 可转移给 Worker，在 `OffscreenCanvas` 中消费；
   这有利于把部分 Canvas 工作移出主线程。[cite:1]

### 4.2 关键限制

1. **主线程仍是布局源头**：DOM layout、paint 和快照捕获不能消失。当前设计不能让
   Canvas 内滚动和动画像普通合成器滚动一样脱离 JavaScript 更新；自动更新模式仍是
   future consideration。[cite:1][cite:8]
2. **几何同步是新的一致性责任**：绘制位置、命中顺序、裁剪和无障碍 bounds 必须
   与 GPU 输出同步。Worker 路径还增加异步几何确认。[cite:1]
3. **任意 shader 形变无法由单个矩阵和矩形 clip 完整反演**：当前几何协议表达
   `DOMMatrix` 和 `DOMRect`。由此可推断，非仿射扭曲、复杂遮挡或像素级形变只能提供
   近似命中，不能自动获得与最终像素完全一致的交互区域。
4. **隐私安全绘制不等于页面原样截图**：跨域内容、系统主题、visited 状态、拼写标记、
   autofill 预览、IME popup 和部分平台视觉不会进入可读回快照。[cite:1][cite:9]
5. **失效信息仍不完整**：提案 issue 曾专门讨论移除元素没有直接出现在
   `changedElements` 中，应用需要额外维护旧集合才能做粒度更新。[cite:10]
6. **浏览器和绑定层不成熟**：API 名称与 IDL 近期仍变化，WebKit 功能未完成，
   Firefox 未表态，Rust `web-sys` / `wgpu` 生态尚不能视为稳定支持。

## 5. 对 Novadraw 的价值判断

### 5.1 场景适配

| Novadraw 场景 | 价值 | 判断 |
|---|---|---|
| 富 HTML 节点嵌入画布并参与遮挡、裁剪、缩放 | 高 | 这是 API 的核心差异化价值 |
| 未来 3D 视口中的表单、标签和浏览器内容 | 高但远期 | 符合 texture use case，但项目尚无真实 3D consumer |
| 复杂文本标签 | 低 | 已有 Parley/Vello 跨平台文本链路，替换会破坏 Native/Web 等价 |
| Direct text edit / IME | 低 | EditContext 已提供更窄、更合适的输入桥 |
| Tooltip、菜单、属性面板、Palette | 低 | 普通 DOM overlay 更简单，且符合 Host UI 边界 |
| Accessibility | 中，适合作为 Web 投影实现 | 不能替代 Core Snapshot/Delta，否则 Native/Headless 语义分裂 |
| 截图与确定性导出 | 低到中 | 受隐私过滤、浏览器差异和实验性实现影响，不宜作为权威导出 |
| 大规模节点默认渲染 | 未证明 | 缺少本项目工作负载下的 CPU、GPU、内存和交互延迟证据 |

### 5.2 与项目边界的契合点

项目现有边界已经为实验留出正确位置：

- `novadraw` Core 不依赖 DOM、WebSys、Vello 或 Winit；
- `novadraw-platform-web` 负责 DOM、Canvas、输入和浏览器调度；
- `novadraw-backend-vello` 拥有 WGPU device/queue、surface 和 backend-local cache；
- 应用 composition root 负责组合平台 adapter 与 backend。

这些约束见 [ADR-023](../adr/adr-023-crate-consolidation-and-extension-boundaries.md)。
HTML-in-Canvas 不应改变该依赖方向。

现有 Accessibility Bridge 把可访问性定义为 stable scene 的派生语义投影，不建立
第二棵业务真值树，见
[Tooltip 与 Accessibility Bridge 契约](../design/architecture/tooltip-accessibility.md)。
HTML DOM 只能成为 Web provider，不能反向成为 Figure 或 Editor 的状态源。

Host UI 已明确由平台 UI 技术承载，只有必须位于画布坐标域的交互才使用
Figure-native widget，见
[AI 图形编辑器生成器设计](ai-graphical-editor-generator.md#11-host-ui-边界)。
因此 HTML-in-Canvas 不能成为“把所有 UI 搬进 Canvas”的理由。

### 5.3 当前实现阻力

当前 Core 图像资源是不可变 RGBA：

- [`ImageData`](../../crates/novadraw/src/render/command.rs) 持有 `Vec<u8>`；
- [`ResourcePayload`](../../crates/novadraw/src/render/submission.rs) 只包含 Image/Font；
- Vello backend 将 RGBA 复制为 `peniko::ImageData` 并交给 `Scene::draw_image`；
- Vello renderer 自己拥有 WGPU device、queue、retained texture 和 scratch texture。

HTML-in-Canvas 的 WebGPU 快路径要求在浏览器原生 `GPUQueue` 上把 DOM 快照写入目标
`GPUTexture`。当前锁定的 `wgpu 29.0.4` 公开了既有
`copy_external_image_to_texture`，但没有新提案的 `drawElementImageToTexture`；
其 Web backend 的原生 queue/texture 字段也不是公共接口。于是存在两个阻力：

1. **句柄互操作**：在不 fork `wgpu`、不依赖私有字段的前提下，暂时没有稳定类型化
   入口调用新浏览器方法。
2. **场景内排序**：Vello `Scene` 当前消费 CPU `peniko::ImageData`，不是外部
   `GPUTexture`。即使完成纹理写入，也还需解决 HTML 与任意 Figure 的 Z-order、
   clip、opacity、damage 和 retained partial 合成，不能只在整帧前后追加一次 blit。

把 DOM 先画到辅助 2D Canvas，再转成 RGBA 或普通 external image 可以做非交互 POC，
但会增加中间光栅化/复制，并使命中几何落在错误 Canvas 上。它不能证明正式集成可行。

## 6. 推荐架构位置

### 6.1 当前阶段

不新增 Core 类型，不修改 `RenderCommand`，不新增 `HtmlFigure`，不修改
Accessibility Snapshot 真值来源。

若产品出现明确用例，建立一个不进入发布承诺的 Web POC：

```text
Application composition root
├── Web DOM provider
│   ├── drawable element ownership
│   ├── paint / removal invalidation
│   └── hit-test / accessibility geometry update
└── Vello Web interop experiment
    ├── same-device destination texture
    ├── browser queue extension call
    └── texture composition / damage measurement
```

平台 adapter 与 backend 仍不互相依赖；composition root 只协调窄能力。DOM 事件必须
转换为 typed Request/Command 或已有 Runtime 输入，不能直接修改模型。

### 6.2 未来正式化条件

只有 POC 证明真实需求后，才评审 backend-neutral 的“动态外部表面”语义。该契约应
表达稳定身份、revision、logical size、alpha、色彩空间、damage、session loss 和
可用性，不表达 `Element`、`ElementImage` 或 `GPUTexture`。

Web 实现可以把该资源绑定到 DOM 快照；Native 实现可以拒绝或绑定其他平台 surface。
若没有第二个真实 consumer，不应提前抽象公共 trait 或扩张 `ResourcePayload`。

## 7. POC 的最低验收

POC 必须针对一个真实富节点，而不是独立 API demo，并同时验证：

1. 当前 API feature detection 和无支持浏览器的明确降级。
2. 同一 DOM 节点的视觉、pointer、keyboard、selection、IME 和 accessibility
   几何一致。
3. zoom、scroll、nested viewport、DPR、resize、clip、opacity 和部分 damage。
4. HTML 位于两个 Vello Figure 之间时的正确 Z-order，而非只覆盖在整张画布顶部。
5. DOM 内容变化、元素移除、隐藏、重新挂载和 backend session loss。
6. 普通 DOM overlay、HTML-in-Canvas 和原生 Novadraw Figure 三条路径的
   frame time、主线程时间、纹理内存、上传字节和 input-to-present 对照。
7. Chrome 与至少一个非 Chromium 引擎的真实运行；若仍无实现，明确记录
   `unsupported_environment`。
8. 禁止通过 CPU readback 或每帧 PNG 编码掩盖直接纹理互操作缺口。

## 8. 决策门槛

满足以下条件前，不进入正式 roadmap：

- 有一个 DOM overlay 无法正确实现的真实产品用例；
- 至少两个浏览器引擎提供可运行实现，或产品明确只承诺 Chromium；
- WHATWG API 形状不再发生关键生命周期和命名变化；
- `wgpu` 提供受支持的互操作入口，或项目有经过评审的稳定适配层；
- POC 证明浏览器原生排版收益大于 DOM layout、快照、纹理合成和同步成本；
- Accessibility Bridge、Editor Command 和 Runtime submission 仍保持单一真值；
- Native 与 Headless 的能力差异通过显式 capability 和结构化错误表达。

## 9. 最终建议

**决策：观察并保留扩展位置；暂不实现。**

HTML-in-Canvas 很可能成为 Web 图形应用的重要能力，尤其适合富内容节点和未来
2D/3D 混合场景。但对当前 Novadraw，它不是基础渲染能力，也不是现有文本、输入或
无障碍系统的替代品。现在集成会同时承受标准变化、单引擎依赖、Rust WebGPU 互操作
缺口和双树一致性成本，收益尚无真实 consumer 支撑。

当产品第一次提出“HTML 必须参与 Canvas 内部 Z-order、clip 或 shader”的需求时，
再启动窄 POC；在此之前，继续投入当前 Core API、Vello 渲染、Editor 事务和平台等价，
边际收益更高。

## Sources

<a id="cite-1"></a>[cite:1] WICG, “HTML-in-Canvas” living explainer:
<https://github.com/WICG/html-in-canvas>

<a id="cite-2"></a>[cite:2] Chrome Platform Status, “HTML-in-canvas”:
<https://chromestatus.com/feature/5172548013916160>

<a id="cite-3"></a>[cite:3] WHATWG HTML pull request #11588, “Add HTML-in-Canvas APIs”:
<https://github.com/whatwg/html/pull/11588>

<a id="cite-4"></a>[cite:4] WebKit Standards Position #630, “html-in-canvas”:
<https://github.com/WebKit/standards-positions/issues/630>

<a id="cite-5"></a>[cite:5] WebKit bug #325295, current HTML-in-Canvas IDL update:
<https://bugs.webkit.org/show_bug.cgi?id=325295>

<a id="cite-6"></a>[cite:6] Mozilla Standards Position #1076, “HTML-in-Canvas”:
<https://github.com/mozilla/standards-positions/issues/1076>

<a id="cite-7"></a>[cite:7] W3C TAG design review #1204:
<https://github.com/w3ctag/design-reviews/issues/1204>

<a id="cite-8"></a>[cite:8] Chrome for Developers, “Introducing the HTML-in-Canvas API origin
trial”:
<https://developer.chrome.google.cn/blog/html-in-canvas-origin-trial?hl=en>

<a id="cite-9"></a>[cite:9] WICG HTML-in-Canvas security and privacy questionnaire:
<https://github.com/WICG/html-in-canvas/blob/main/security-privacy-questionnaire.md>

<a id="cite-10"></a>[cite:10] WICG issue #85, removed-element invalidation:
<https://github.com/WICG/html-in-canvas/issues/85>
