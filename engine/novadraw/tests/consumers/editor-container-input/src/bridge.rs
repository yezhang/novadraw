#[derive(serde::Serialize, Clone)]
pub struct VisualRect {
    pub model: u32,
    pub parent: Option<u32>,
    pub bounds: [f64; 4],
}

#[derive(serde::Serialize, Clone)]
pub struct ProbeSnapshot {
    pub mode: &'static str,
    pub active: bool,
    pub faulted: bool,
    pub revision: u64,
    pub history: usize,
    pub builds: usize,
    pub feedback: usize,
    pub feedback_bounds: Vec<[f64; 4]>,
    pub scale: f64,
    pub visuals: Vec<VisualRect>,
    pub nodes: Vec<VisualRect>,
    pub child_orders: Vec<(u32, Vec<u32>)>,
}

pub struct InputProbe {
    viewer: GraphicalViewer<Model, Factory>,
    domain: EditorDomain<Model>,
    creating: bool,
}

impl InputProbe {
    pub fn new(creating: bool) -> Self {
        let (mut viewer, mut domain) = editor();
        if creating {
            domain
                .set_active_tool(&mut viewer, ContainerCreationTool::new(Template(20)))
                .unwrap();
        } else {
            select(&mut viewer, &[FIRST]);
        }
        Self {
            viewer,
            domain,
            creating,
        }
    }

    pub fn press(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.domain
            .pointer_pressed(
                &mut self.viewer,
                Point::new(x, y),
                MouseButton::Left,
                Default::default(),
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn select_batch(&mut self) {
        select(&mut self.viewer, &[FIRST, SECOND]);
    }

    pub fn ordered_target(&mut self) {
        self.viewer.model().trace.borrow_mut().ordered = true;
    }

    pub fn external_revision(&mut self) -> Result<(), String> {
        self.viewer
            .synchronize_model(Model::publish)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn invalidate_target(&mut self) -> Result<(), String> {
        let target = self.viewer.part_for_model(RIGHT).unwrap();
        self.viewer
            .invalidate_container_policy(target)
            .map_err(|e| e.to_string())
    }

    pub fn switch_tool(&mut self) -> Result<(), String> {
        self.domain
            .set_active_tool(&mut self.viewer, novadraw_editor::SelectionTool::new())
            .map_err(|e| e.to_string())
    }

    pub fn drop_template(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.domain
            .drop_into_container(
                &mut self.viewer,
                &Template(20),
                Point::new(x, y),
                Point::new(5.0, 4.0),
                Default::default(),
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn move_pointer(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.domain
            .pointer_moved(&mut self.viewer, Point::new(x, y))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn release(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.domain
            .pointer_released(&mut self.viewer, Point::new(x, y), MouseButton::Left)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn cancel(&mut self) -> Result<(), String> {
        self.domain
            .cancel_tool(&mut self.viewer)
            .map_err(|e| e.to_string())
    }

    pub fn undo(&mut self) -> Result<(), String> {
        self.domain
            .undo(&mut self.viewer)
            .map_err(|e| e.to_string())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        self.domain
            .redo(&mut self.viewer)
            .map_err(|e| e.to_string())
    }

    pub fn zoom(&mut self, scale: f64) -> Result<(), String> {
        self.domain
            .set_viewport_scale_at(&mut self.viewer, scale, None)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn scroll(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.domain
            .set_viewport_origin(&mut self.viewer, Point::new(x, y))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn resize(&mut self, width: f64, height: f64) -> Result<(), String> {
        let changed = self
            .viewer
            .drive()
            .resize_logical_viewport(width, height)
            .map_err(|e| e.to_string())?;
        if changed {
            self.domain
                .refresh_after_viewport_change(&mut self.viewer)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn snapshot(&mut self) -> Result<ProbeSnapshot, String> {
        self.viewer
            .stabilize_for_query()
            .map_err(|e| e.to_string())?;
        let mut ids: Vec<_> = self.viewer.model().nodes.keys().copied().collect();
        ids.sort_unstable();
        let parent = |id| {
            self.viewer
                .model()
                .nodes
                .iter()
                .find_map(|(p, node)| node.children.contains(&id).then_some(*p))
        };
        let nodes = ids
            .iter()
            .map(|id| {
                let b = self.viewer.model().nodes[id].bounds;
                VisualRect {
                    model: *id,
                    parent: parent(*id),
                    bounds: [b.x, b.y, b.width, b.height],
                }
            })
            .collect();
        let visuals = ids
            .iter()
            .filter_map(|id| {
                let part = self.viewer.part_for_model(*id)?;
                let b = self.viewer.part_bounds_in_surface(part)?;
                Some(VisualRect {
                    model: *id,
                    parent: parent(*id),
                    bounds: [b.x, b.y, b.width, b.height],
                })
            })
            .collect();
        let feedback = self
            .viewer
            .runtime()
            .tree()
            .child_order(self.viewer.root_layers().feedback())
            .map_err(|e| e.to_string())?
            .iter()
            .filter(|id| self.viewer.visual_owner(**id).is_some())
            .count();
        let feedback_bounds = self
            .viewer
            .runtime()
            .tree()
            .child_order(self.viewer.root_layers().feedback())
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|id| self.viewer.visual_owner(*id).is_some())
            .map(|id| {
                let b = self
                    .viewer
                    .runtime()
                    .tree()
                    .figure_bounds(id)
                    .expect("owned feedback bounds");
                [b.x, b.y, b.width, b.height]
            })
            .collect();
        Ok(ProbeSnapshot {
            mode: if self.creating { "create" } else { "move" },
            active: self.domain.has_active_gesture(),
            faulted: self.viewer.is_faulted(),
            revision: self.viewer.model().revision.value(),
            history: self.domain.command_stack().undo_len(),
            builds: self.viewer.model().trace.borrow().build_count,
            feedback,
            feedback_bounds,
            scale: self.viewer.viewport_scale().map_err(|e| e.to_string())?,
            visuals,
            nodes,
            child_orders: ids
                .iter()
                .map(|id| (*id, self.viewer.model().nodes[id].children.clone()))
                .collect(),
        })
    }
}

#[cfg(feature = "web")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub struct WebProbe(InputProbe);

#[cfg(feature = "web")]
#[wasm_bindgen::prelude::wasm_bindgen]
impl WebProbe {
    #[wasm_bindgen::prelude::wasm_bindgen(constructor)]
    pub fn new(creating: bool) -> Self {
        Self(InputProbe::new(creating))
    }
    pub fn input(&mut self, kind: &str, x: f64, y: f64) -> Result<(), wasm_bindgen::JsValue> {
        let result = match kind {
            "press" => self.0.press(x, y),
            "move" => self.0.move_pointer(x, y),
            "release" => self.0.release(x, y),
            "cancel" => self.0.cancel(),
            "undo" => self.0.undo(),
            "redo" => self.0.redo(),
            "zoom" => self.0.zoom(x),
            "scroll" => self.0.scroll(x, y),
            "resize" => self.0.resize(x, y),
            "batch" => { self.0.select_batch(); Ok(()) },
            "ordered" => { self.0.ordered_target(); Ok(()) },
            "external" => self.0.external_revision(),
            "invalidate" => self.0.invalidate_target(),
            "switch" => self.0.switch_tool(),
            "drop" => self.0.drop_template(x, y),
            _ => Err("Unknown probe operation".into()),
        };
        result.map_err(|e| wasm_bindgen::JsValue::from_str(&e))
    }
    pub fn snapshot(&mut self) -> Result<String, wasm_bindgen::JsValue> {
        let snapshot = self
            .0
            .snapshot()
            .map_err(|e| wasm_bindgen::JsValue::from_str(&e))?;
        serde_json::to_string(&snapshot)
            .map_err(|e| wasm_bindgen::JsValue::from_str(&e.to_string()))
    }
}

#[cfg(test)]
mod input_bridge_tests {
    use super::*;

    #[test]
    fn template_drop_keeps_parent_offset_and_one_history_entry() {
        let mut probe = InputProbe::new(false);
        probe.drop_template(100.0, 240.0).unwrap();
        let state = probe.snapshot().unwrap();
        let created = state.nodes.iter().find(|node| node.model == 20).unwrap();
        assert_eq!(created.parent, Some(NESTED));
        assert_eq!(created.bounds, [25.0, 36.0, 30.0, 20.0]);
        assert_eq!(state.history, 1);
        assert_eq!(state.builds, 1);
        probe.undo().unwrap();
        assert!(probe.snapshot().unwrap().nodes.iter().all(|node| node.model != 20));
        probe.redo().unwrap();
        let state = probe.snapshot().unwrap();
        assert_eq!(state.nodes.iter().find(|node| node.model == 20).unwrap().bounds,
                   [25.0, 36.0, 30.0, 20.0]);
    }

    #[test]
    fn batch_bridge_preserves_order_spacing_and_atomic_commit() {
        let mut probe = InputProbe::new(false);
        probe.select_batch();
        probe.ordered_target();
        probe.press(90.0, 85.0).unwrap();
        probe.move_pointer(440.0, 160.0).unwrap();
        let preview = probe.snapshot().unwrap();
        assert_eq!(preview.feedback, 2);
        assert_eq!(preview.builds, 0);
        probe.release(440.0, 160.0).unwrap();
        let state = probe.snapshot().unwrap();
        assert_eq!(state.child_orders.iter().find(|(id, _)| *id == RIGHT).unwrap().1,
                   [FIRST, SECOND, LOCAL]);
        let first = state.nodes.iter().find(|node| node.model == FIRST).unwrap();
        let second = state.nodes.iter().find(|node| node.model == SECOND).unwrap();
        assert_eq!(second.bounds[0] - first.bounds[0], 80.0);
        assert_eq!(second.bounds[1] - first.bounds[1], 10.0);
        assert_eq!(state.history, 1);
        assert_eq!(state.builds, 1);
    }

    #[test]
    fn invalidation_bridge_cancels_both_tools_without_building_commands() {
        for creating in [false, true] {
            for reason in 0..3 {
                let mut probe = InputProbe::new(creating);
                let start = if creating { (400.0, 120.0) } else { (90.0, 85.0) };
                probe.press(start.0, start.1).unwrap();
                probe.move_pointer(440.0, 160.0).unwrap();
                match reason {
                    0 => probe.external_revision().unwrap(),
                    1 => probe.invalidate_target().unwrap(),
                    _ => probe.switch_tool().unwrap(),
                }
                let _ = probe.release(440.0, 160.0);
                let state = probe.snapshot().unwrap();
                assert!(!state.active);
                assert!(!state.faulted);
                assert_eq!(state.builds, 0);
                assert_eq!(state.history, 0);
                assert_eq!(state.feedback, 0);
                assert!(state.nodes.iter().all(|node| node.model != 20));
                assert_eq!(state.nodes.iter().find(|node| node.model == FIRST).unwrap().parent,
                           Some(LEFT));
            }
        }
    }
}
