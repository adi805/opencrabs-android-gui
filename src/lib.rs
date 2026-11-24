// Spike: keyboard + rendering check for a full-Rust egui app on Android.
// GO/NO-GO gate for the OpenCrabs Android GUI (see opencrabs-android/files/DECISIONS.md).
//
// Fix round 2 (keyboard): winit's Android backend only shows/hides the soft
// keyboard (set_ime_allowed -> show_soft_input) but never converts
// android-activity's TextInputState into winit Ime events, so typed text can
// never reach egui through the stock chain. This spike bridges it directly:
// poll text_input_state() every frame, mirror it into the egui buffer, and
// push our buffer back with set_text_input_state() so the IME stays in sync.

use eframe::egui;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let options = eframe::NativeOptions {
        android_app: Some(app.clone()),
        ..Default::default()
    };
    eframe::run_native(
        "OpenCrabs GUI",
        options,
        Box::new(move |_cc| Ok(Box::new(Spike::new(app)))),
    )
    .unwrap();
}

struct Spike {
    typed: String,
    #[cfg(target_os = "android")]
    app: winit::platform::android::activity::AndroidApp,
    #[cfg(target_os = "android")]
    kb_shown: bool,
}

#[cfg(target_os = "android")]
impl Spike {
    fn new(app: winit::platform::android::activity::AndroidApp) -> Self {
        Self {
            typed: String::new(),
            app,
            kb_shown: false,
        }
    }
}

#[cfg(not(target_os = "android"))]
impl Default for Spike {
    fn default() -> Self {
        Self {
            typed: String::new(),
        }
    }
}

impl eframe::App for Spike {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Keep content clear of the Android status bar.
        egui::Panel::top("status_bar_space").show(ui, |ui| {
            ui.set_height(32.0);
        });

        #[cfg(target_os = "android")]
        {
            // Adopt whatever the IME committed since the last frame.
            let ime_text = self.app.text_input_state().text;
            if ime_text != self.typed {
                self.typed = ime_text;
            }
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("OpenCrabs GUI spike");
            ui.separator();
            ui.label("GO test: tap the field, type, text must appear.");
            let response = egui::TextEdit::singleline(&mut self.typed)
                .hint_text("ketik di sini")
                .show(ui);
            ui.add_space(8.0);
            ui.monospace(format!("echo: {}", self.typed));
            ui.separator();
            ui.label(format!(
                "frames ok, chars typed: {}",
                self.typed.chars().count()
            ));

            #[cfg(target_os = "android")]
            {
                // Manual controls: if the auto path ever fails on a device,
                // these still let the tester see which half is broken.
                ui.horizontal(|ui| {
                    if ui.button("Show keyboard").clicked() {
                        self.app.show_soft_input(true);
                        self.kb_shown = true;
                    }
                    if ui.button("Hide keyboard").clicked() {
                        self.app.hide_soft_input(false);
                        self.kb_shown = false;
                    }
                });

                let focused = response.response.has_focus();
                if focused && !self.kb_shown {
                    self.app.show_soft_input(true);
                    self.kb_shown = true;
                } else if !focused && self.kb_shown {
                    self.app.hide_soft_input(false);
                    self.kb_shown = false;
                }

                // Keep the IME buffer identical to ours so composition and
                // the cursor land in the right place.
                let len = self.typed.len();
                self.app.set_text_input_state(
                    winit::platform::android::activity::input::TextInputState {
                        text: self.typed.clone(),
                        selection: winit::platform::android::activity::input::TextSpan {
                            start: len,
                            end: len,
                        },
                        compose_region: None,
                    },
                );
            }
        });
    }
}
