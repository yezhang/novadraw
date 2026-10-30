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
