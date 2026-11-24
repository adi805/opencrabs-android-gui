# Spike: keyboard + rendering check for a full-Rust egui app on Android.
# GO/NO-GO gate for the OpenCrabs Android GUI (see opencrabs-android/files/DECISIONS.md).

use eframe::egui;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    eframe::run_native("OpenCrabs GUI", options, Box::new(|_cc| Ok(Box::new(Spike::default()))))
        .unwrap();
}

#[derive(Default)]
struct Spike {
    typed: String,
}

impl eframe::App for Spike {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Keep content clear of the Android status bar.
        egui::Panel::top("status_bar_space").show(ui, |ui| {
            ui.set_height(32.0);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("OpenCrabs GUI spike");
            ui.separator();
            ui.label("GO test: tap the field, type, text must appear.");
            egui::TextEdit::singleline(&mut self.typed)
                .hint_text("ketik di sini")
                .show(ui);
            ui.add_space(8.0);
            ui.monospace(format!("echo: {}", self.typed));
            ui.separator();
            ui.label(format!("frames ok, chars typed: {}", self.typed.chars().count()));
        });
    }
}
