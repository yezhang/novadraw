//! Drive the actual host controls through egui pointer and keyboard events.
use novadraw::runtime::Runtime;
use novadraw_example_support::panel::{Panel, egui};

pub fn record(ui: &egui::Ui, label: &str, response: &egui::Response) {
    ui.ctx().data_mut(|data| {
        data.insert_temp(egui::Id::new(label), (response.rect, response.enabled()));
    });
}

pub struct Driver {
    context: egui::Context,
    panel: Panel,
    pub runtime: Runtime,
}

impl Driver {
    pub fn new(name: &str) -> Self {
        let (runtime, panel) = super::panel_scene(name).unwrap();
        let mut driver = Self {
            context: egui::Context::default(),
            panel: panel.unwrap(),
            runtime,
        };
        driver.draw(Vec::new());
        driver
    }

    pub fn draw(&mut self, events: Vec<egui::Event>) {
        self.context.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 1000.0),
            )),
            events,
            ..Default::default()
        });
        egui::Window::new("Example controls")
            .default_width(350.0)
            .show(&self.context, |ui| {
                self.panel.show(ui, &mut self.runtime);
            });
        let _ = self.context.end_pass();
        self.runtime.frames().stabilize_for_query().unwrap();
    }

    pub fn enabled(&self, label: &str) -> bool {
        self.context.data(|data| {
            data.get_temp::<(egui::Rect, bool)>(egui::Id::new(label))
                .unwrap_or_else(|| panic!("missing control: {label}"))
                .1
        })
    }

    pub fn click(&mut self, label: &str) {
        let position = self.context.data(|data| {
            data.get_temp::<(egui::Rect, bool)>(egui::Id::new(label))
                .unwrap_or_else(|| panic!("missing control: {label}"))
                .0
                .center()
        });
        self.draw(vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
        ]);
        self.draw(vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        }]);
    }

    pub fn edit(&mut self, label: &str, text: &str) {
        self.click(label);
        self.draw(vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    command: true,
                    ctrl: true,
                    ..Default::default()
                },
            },
            egui::Event::Text(text.to_owned()),
            egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            },
        ]);
    }
}
