use eframe::{App, Frame, NativeOptions};

struct ArchanaApp {
    #[allow(dead_code)]
    archive_path: Option<String>,
    entries: Vec<archana_core::Entry>,
    #[allow(dead_code)]
    selected: Option<String>,
    status: String,
}

impl Default for ArchanaApp {
    fn default() -> Self {
        Self {
            archive_path: None,
            entries: Vec::new(),
            selected: None,
            status: "Ready".to_string(),
        }
    }
}

impl App for ArchanaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("archana");
            ui.horizontal(|ui| {
                if ui.button("Open").clicked() {
                }
                if ui.button("Extract").clicked() {
                }
            });
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                for entry in &self.entries {
                    ui.label(&entry.name);
                }
            });
            
            ui.separator();
            ui.label(&self.status);
        });
    }
}

fn main() {
    let options = NativeOptions::default();
    let _ = eframe::run_native("archana", options, Box::new(|_| Ok(Box::new(ArchanaApp::default()))));
}