# Web 平台手工验证

类型：`verification`

本文用于验证 Novadraw 在浏览器中的完整链路：

```text
DOM Input -> WebInputAdapter -> Runtime -> RenderSubmission
          -> Vello WebGPU -> HTMLCanvasElement
```

Vello WebGPU 是正式验证后端。Canvas2D 仅作为诊断基线，用于区分 Runtime /
输入问题与 WebGPU / Vello 问题。

## 1. 验证范围

本流程覆盖：

- Wasm 编译与 wasm-bindgen 产物生成；
- JavaScript、Wasm 和静态资源服务；
- Vello WebGPU adapter、device 和 canvas surface 初始化；
- Runtime 首帧、Partial damage 和 Full damage 提交；
- Pointer、Wheel、Keyboard 事件；
- CSS logical units、physical pixels 和 DPR；
- 浏览器 resize；
- Canvas2D 对照路径。

本流程不替代 Windows/Linux 原生窗口验收，也不证明 Firefox 或 Safari 已兼容。

## 2. 环境准备

以下命令均在 workspace 根目录执行：

```bash
cd /Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw
```

需要：

- 当前稳定版 Chrome/Chromium，且 WebGPU 可用；
- Rust 工具链；
- Python 3；
- `wasm32-unknown-unknown` target；
- 与项目 `wasm-bindgen` crate 版本一致的 `wasm-bindgen-cli`。

安装 wasm target 和仓库本地 CLI：

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli \
  --version 0.2.127 \
  --locked \
  --root target/wasm-tools
```

检查环境：

```bash
rustup target list --installed
target/wasm-tools/bin/wasm-bindgen --version
python3 --version
```

预期：

- target 列表包含 `wasm32-unknown-unknown`；
- CLI 输出 `wasm-bindgen 0.2.127`；
- Python 3 可以正常启动。

在浏览器 DevTools Console 中执行：

```javascript
Boolean(navigator.gpu)
```

必须返回 `true`。WebGPU 需要安全上下文，`http://127.0.0.1` 和
`http://localhost` 可用于本地验证；不要直接使用 `file://` 打开 HTML。

## 3. 编译

执行：

```bash
./scripts/build_web_validation.sh
```

预期退出码为 0，并生成：

```text
apps/web-validation/dist/
├── index.html
├── styles.css
└── pkg/
    ├── web_validation.js
    └── web_validation_bg.wasm
```

构建脚本默认使用：

```text
target/wasm-tools/bin/wasm-bindgen
```

如需使用其他位置的同版本 CLI：

```bash
WASM_BINDGEN="$(command -v wasm-bindgen)" ./scripts/build_web_validation.sh
```

## 4. 启动资源服务

在独立终端执行：

```bash
./scripts/serve_web_validation.sh
```

默认监听 `127.0.0.1:4173`。端口占用时指定其他端口：

```bash
PORT=4174 ./scripts/serve_web_validation.sh
```

在另一个终端检查静态资源：

```bash
curl -I http://127.0.0.1:4173/pkg/web_validation_bg.wasm
```

预期：

- HTTP 状态为 `200`；
- `Content-Type` 为 `application/wasm`。

## 5. Vello WebGPU 验证

访问：

```text
http://127.0.0.1:4173/?backend=vello
```

不带查询参数的根 URL 也必须默认使用 Vello WebGPU。

| 步骤 | 操作 | 通过标准 |
|---|---|---|
| 1 | 等待初始化完成 | 顶部显示 `READY · Vello WebGPU`，Backend 显示 `Vello WebGPU` |
| 2 | 检查初始画面 | Canvas 有浅灰背景、边框和蓝色矩形，无空白、黑屏或透明帧 |
| 3 | 将指针移入、移出矩形 | 矩形在绿色和蓝色间切换，pointer 计数递增，交互帧为 `Partial` |
| 4 | 在矩形内按下并释放 | 按下时为红色；释放后为紫色并显示 `Focused` |
| 5 | 保持 Canvas 焦点并按 `A` | Keyboard 显示 `Character('a')`，key 计数至少增加 2 |
| 6 | 指针停在矩形上滚动滚轮或触控板 | 页面不滚动，wheel 计数递增，画面无异常 |
| 7 | 点击 `Toggle 1x / 2x DPR` | logical size 不变，physical size 按 DPR 更新，当前帧为 `Full` |
| 8 | 连续切换 DPR 五次 | 每次都完整重绘，无缩放累积、残影、黑帧或 surface 错误 |
| 9 | 调整浏览器窗口大小 | surface 尺寸跟随 Canvas 更新，首帧为 `Full`，内容位置和比例正确 |

打开 DevTools 并检查：

- Console 不得出现 panic、WebGPU validation error 或 Wasm 异常；
- Network 中 `web_validation.js` 和 `web_validation_bg.wasm` 必须成功；
- 页面不得静默切换到 Canvas2D。

## 6. Canvas2D 对照验证

访问：

```text
http://127.0.0.1:4173/?backend=canvas2d
```

重复第 5 节步骤 1-9，并确认 Backend 显示 `Canvas2D`。

判断原则：

- 仅 Canvas2D 正常：优先检查 WebGPU adapter/device、surface 配置和 Vello 提交；
- 两种后端均异常：优先检查 Runtime、输入适配、surface 信息或资源加载；
- 仅某种输入异常：检查对应 DOM 事件到 `WebInputAdapter` 的映射。

## 7. 自动化状态

页面在 `body` 上提供以下属性：

| 属性 | 预期 |
|---|---|
| `data-ready` | 初始化完成为 `true`，失败为 `error` |
| `data-backend` | `vello-webgpu` 或 `canvas2d` |
| `data-frame-count` | 成功提交的帧数，大于 0 |
| `data-pointer-events` | Pointer 事件计数 |
| `data-wheel-events` | Wheel 事件计数 |
| `data-key-events` | Keyboard 事件计数 |

自动化验证不能只检查 DOM 状态；Canvas 是 GPU 输出时，还必须保留截图并确认画面非空。

## 8. 常见失败

| 现象 | 优先检查 |
|---|---|
| `wasm32-unknown-unknown` 未安装 | 执行 `rustup target add wasm32-unknown-unknown` |
| wasm-bindgen schema/version 不匹配 | 重新安装文档指定版本并清理 `dist/` 后构建 |
| Wasm 请求失败或 MIME 错误 | 必须通过脚本启动 HTTP 服务，不使用 `file://` |
| 页面显示 `ERROR` | 查看 Console 中的 Vello WebGPU 初始化错误 |
| `navigator.gpu` 为 `false` | 浏览器版本、硬件加速、WebGPU 开关和运行环境 |
| Vello 黑屏但 Canvas2D 正常 | WebGPU surface、Vello render target 和提交链路 |
| 两种后端均黑屏 | Runtime、RenderSubmission、surface 尺寸和资源加载 |
| DPR/resize 后残影 | Full damage、surface reconfigure 和 retained texture 重建 |

## 9. 验收记录

```text
Commit:
浏览器及版本:
操作系统:
GPU / WebGPU adapter:

[ ] wasm32 release 构建通过
[ ] Wasm MIME 为 application/wasm
[ ] Vello WebGPU 初始化并绘制首帧
[ ] Pointer hover/press/release/focus 通过
[ ] Keyboard keydown/keyup 通过
[ ] Wheel / touchpad 通过
[ ] 1x/2x DPR 切换通过
[ ] 浏览器 resize 通过
[ ] Canvas2D 对照路径通过
[ ] Console 与 Network 检查通过

失败步骤与复现:
结论: PASS / FAIL
```

验证结束后，在资源服务终端按 `Ctrl+C` 停止服务。
