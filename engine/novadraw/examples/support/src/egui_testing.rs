//! UI-only replay helpers. These deliberately discard texture uploads: no GPU renderer
//! consumes the returned frame. Production hosts must upload their full texture delta.

pub fn run_headless_ui(
    context: &egui::Context,
    input: egui::RawInput,
    draw: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let mut output = context.run_ui(input, draw);
    output.textures_delta.clear();
    output
}
