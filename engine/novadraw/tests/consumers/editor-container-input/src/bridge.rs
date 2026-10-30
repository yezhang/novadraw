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
