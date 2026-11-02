# Editor E03 平台输入验证消费者

能力归属：Editor 依赖的独立平台输入验证；不新增 Core 或 Editor 公共 API。

消费者复用 `p2_e03_container_editing_contract.rs` 的模型、Policy、移动和创建 Tool。
桥接层只转发输入并输出模型父关系、历史、命令构造次数与反馈数量。
Web 使用 SVG、Native 使用 egui 画出查询到的 bounds，帮助定位输入；
这些示意图不验证 Core paint 或 Vello 像素。

```bash
cargo test --manifest-path tests/consumers/editor-container-input/Cargo.toml --lib
cargo run --manifest-path tests/consumers/editor-container-input/Cargo.toml --features native --bin editor-container-input-native
cargo build --manifest-path tests/consumers/editor-container-input/Cargo.toml --features web --lib --target wasm32-unknown-unknown
```

Web 构建后用与锁文件相同版本的 wasm-bindgen 生成 `web/pkg`，
通过本地 HTTP 服务打开 `web/index.html`。
默认 `container-cost` feature 仅让复用的消费者夹具注册容器 Policy，
不是产品 Editor 的 feature。Native 的 egui 默认字体只用于验证宿主。

真实输入的坐标以宿主逻辑单位传入。Web 的 client 坐标减去 SVG 左上角；
Native 将窗口坐标减去内容区域左上角，DPI 只用于宿主显示。
本消费者不处理产品文件或用户业务数据。

私有 revision 耗尽验证在隔离源码副本执行，
不通过本消费者或产品 API 注入；复现范围和证据由 E03 补验记录说明。

## 组合输入与成本续验

Native 和 Web 支持 `N` 多选、`O` 有序目标、`E` 外部模型 revision、
`I` 使目标 policy 失效、`T` 换 Tool、`Z` 缩放、`S` 滚动。
Web 用实际窗口尺寸触发 ResizeObserver，同步逻辑 viewport；
SVG 的输入坐标扣除边框，与绘制的 content 域一致。

Web 的 “Drag E03 template” 使用浏览器原生 HTML drag/drop，
再调用相同的 `EditorDomain::drop_into_container`。它不代表操作系统文件 DnD。
Native 验证宿主提供 `e03-template.txt` 文件接入，使用相同模板和父域 grab offset；
是否取得真实系统输入证据以续验报告为准。

安装或复用 Playwright 后，对本地页面执行：

```bash
node tests/consumers/editor-container-input/verify_browser.cjs \
  http://127.0.0.1:8743 /tmp/e03-browser-receipt.json
```

驱动器分别验证 DPR 1／2 的 23 个检查点。只通过真实鼠标、
键盘、窗口 resize 和 HTML drag/drop 触发编辑，读取模型与历史核对结果。

成本复现：

```bash
python3 tests/consumers/editor-container-input/run_cost_fixture.py \
  --target-dir /tmp/e03-phase-cost-target \
  --output-dir /tmp/e03-phase-cost-results
```

脚本复制 Core／Editor 到隔离目录，插入无分配的线程局部计时器，
输出源码指纹、差异、原始日志与逐样本数据；计时器不进入产品构建。
模型复制、命令构造／执行、snapshot、投影、反馈和稳定化分别记录。
部分计时范围包含其他阶段，不能直接相加；统计口径见 E03 续验报告。
