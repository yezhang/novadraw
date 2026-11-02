//! Matched native UI shell for release artifact size comparison.

struct Shell;

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Shapes editor");
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "novadraw-egui-shell-baseline",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|_| Ok(Box::new(Shell))),
    )
}
