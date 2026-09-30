# Group 4：目标、文档治理与语义分母

审计范围为现行文档和关联实现；归档未作为当前契约读取。
基线 Draw2D HEAD：4463d9d0ce13c19d10fbe769d29f28b7345a8cba。

## 确认的事实

1. overview 明确 Web/macOS/Windows/Linux、大规模 Figure、可替换 host/backend；
   ADR-023 的六 package 边界与这些目标一致。strategy 三份材料属于输入，不应取代目标。
2. Draw2D 方法级表按其五列表格语法统计为 verified 83、partial 8、deferred 1。
   不可换算为能力覆盖率：一行包含多个 API，graph layout/printing 等只在高层表中出现，
   verified 的 text.flow/accessibility 又含明确的子能力例外。
3. partial 包含 gradient、path clip、custom dash/miter、atomic indexed add、
   多矩形 clipping；高层还含 graph layout/print/widgets。当前 P2 backlog 并未逐项承接。
   已接受的限制不属于实现 bug，但用户长期对等目标仍缺剩余工作和退出条件。
4. org.eclipse.draw2d.graph.DirectedGraphLayout/CompoundDirectedGraphLayout 属于允许
   对标的 Draw2D 本体；不能把图自动布局一概当成 Zest 上层而排除。
5. Draw2D ViewportAwareConnectionLayerClippingStrategy.getEdgeClippingRectangle
   实现 nearest common viewport 的跨 viewport 可见性；当前账本的 strict topology 是
   明确收窄，不是完整对等。Figure.remove 解除 parent 后对象仍由调用者持有；
   ADR-014 的 dispose/rebuild 是有意迁移策略，同域保活需单独评估，不应恢复已撤回的
   自动跨 Runtime 迁移承诺。

## 文档漂移

- `doc/design/00-index.md:48-54` 把 component-update 列为非规范提案；
  `component-update.md:3-7` 为 normative-design / accepted / initial-contract-implemented。
- `doc/design/architecture/figure-inspector.md` 要求后续独立 protocol crate；
  ADR-023 §2 明确只有真实 wire version 需求才拆。专题应引用新决策并区分 target/deferred。
- `doc/parity/draw2d/api-coverage.md:202,211` 引用 novadraw_render/novadraw_geometry；
  :225 把 FigureTree::set_visible/set_enabled 写成实际公开 API，
  `graph/mod.rs:4152,4209` 实为 pub(crate)，运行期使用 FigureEditor。
- `api-coverage.md:304-309` 仍列 ViewportHandle mutator；viewport.rs:226,271,290,299
  已为 pub(crate)，真实可变入口是 Runtime scoped editor。
- `product-deliverables.md` 仍称 ShortestPath Year 2 才做；P2-C02 已 complete。
- `roadmap/00-index.md` 当前执行方向仍写从 ADR-020 继续，同时描述其后 P2 已完成。
- `book/src/09-verification-and-extension.md:77` 使用不存在的 workspace.quality；
  调用 xtask 返回 `unknown command`。Book 只修读者可执行入口，不加入审计历史。

## 公开 API 探针

README :38-40 的 compile_fail 一次导入 FigureNode、RenderCommand、UpdateManager。
Core lib.rs :93-98/:115-130 仍公开 FigureNode/NodeState/UpdateManager 等。
独立 rustc import probe 显示 FigureNode/UpdateManager/PendingMutations/EventDispatcher
全部可导入，仅 RenderCommand 被拒绝。现有 doctest 5 个普通 + 1 个 compile_fail 均通过。
因此 compile_fail 的一项缺失遮蔽其他项边界回归；这是一项确定的测试可靠性缺陷，
不声称根层导出本身已绕过 Runtime 可变权限。

## 门禁范围

xtask Manifest::validate 检查 suite 引用路径、命令 ID、相对路径、parity 状态词。
validate_parity_statuses 只扫描五列且首列 backtick 的行，不解析公开 Rust 符号、
设计文档元数据、API family 分母或能力完成度。当前 docs PASS 为
47 commands / 2 profiles / 23 suites，不能推出语义一致。
manifest 平台标签：host 11、native-macos 15、web 7、headless 16、wasm 1；
没有 Windows/Linux 专属 suite 标签。这里只报告标签事实，是否存在平台运行证据由组2复核。

## 实际阅读入口

- AGENTS.md、CLAUDE.md、cloud project/user memory；
- doc/00-index.md、design/00-index.md、architecture/{00-index,overview,static-architecture,
  directory-structure,component-update,figure-inspector}.md、editor/architecture.md；
- ADR-001、014、017—023、adr/README.md；
- roadmap/{00-index,p2-delta-backlog,product-deliverables,demo-matrix}.md；
- parity/draw2d/api-coverage.md；
- strategy/{00-index,commercial-value-analysis,ai-graphical-editor-generator}.md；
- reviews/{00-index,engine-capability-assessment-2026-09-28}.md、plans/00-index.md；
- verification/README.md、suites.toml、benchmarks/README.md；
- novadraw/{README.md,src/lib.rs,src/prelude.rs,src/advanced.rs,tests/facade_contract.rs}；
- tools/xtask/src/main.rs 的 manifest 校验、执行与命令分支；
- scripts/check_public_api_dependencies.sh、check_facade_dependencies.sh；
- book/src/09-verification-and-extension.md 前半与相关符号定向检索；
- Draw2D Figure.remove/removeAll、ViewportAwareConnectionLayerClippingStrategy、
  DirectedGraphLayout，以及 Graphics/SWTGraphics/Shape 的能力声明。

其他文件仅作为 inventory 或定向搜索范围；未声明逐行穷尽审查。
