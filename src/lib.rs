// Spike: keyboard + rendering check for a full-Rust egui app on Android.
// GO/NO-GO gate for the OpenCrabs Android GUI (see opencrabs-android/files/DECISIONS.md).
//
// Fix round 3 (self-healing + CI-drivable test):
// - Auto-focus the text field on the first frame so the keyboard opens
//   without any tap at all.
// - show_soft_input is re-requested EVERY frame while focused. The old
//   kb_shown cache could desync from the real Android IME state and then
//   never retry, which looked like "keyboard never appears".
// - The app logs its own TextField geometry (PXCX/PXCY = physical px center)
//   and typed-char count to logcat, so CI can drive the emulator test with
//   exact coordinates instead of guessing from a screenshot.

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
    focused_once: bool,
    #[cfg(target_os = "android")]
    last_px: (f32, f32),
    #[cfg(target_os = "android")]
    last_chars: usize,
    #[cfg(target_os = "android")]
    app: winit::platform::android::activity::AndroidApp,
}

// Desktop entrypoint never runs in the Android build; the AndroidApp field is
// cfg'd out there, so Default only needs the plain fields.
#[cfg(not(target_os = "android"))]
impl Default for Spike {
    fn default() -> Self {
        Self {
            typed: String::new(),
            focused_once: false,
        }
    }
}

impl Spike {
    #[cfg(target_os = "android")]
    fn new(app: winit::platform::android::activity::AndroidApp) -> Self {
        Self {
            typed: String::new(),
            focused_once: false,
            last_px: (0.0, 0.0),
            last_chars: 0,
            app,
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
            ui.label("GO test: field auto-focuses, keyboard opens, type, text appears.");
            let response = egui::TextEdit::singleline(&mut self.typed)
                .hint_text("ketik di sini")
                .show(ui);

            if !self.focused_once {
                response.response.request_focus();
                self.focused_once = true;
            }

            ui.add_space(8.0);
            ui.monospace(format!("echo: {}", self.typed));
            ui.separator();
            let chars = self.typed.chars().count();
            ui.label(format!("frames ok, chars typed: {}", chars));

            // CI hook: log the field's physical-pixel center so the emulator
            // job can tap it precisely. Only when geometry changes, so logcat
            // stays quiet after the first layout pass.
            let ppp = ui.ctx().pixels_per_point();
            let r = response.response.rect;
            let px = ((r.center().x * ppp).round(), (r.center().y * ppp).round());
            if (px.0 - self.last_px.0).abs() > 0.5 || (px.1 - self.last_px.1).abs() > 0.5 {
                log::info!(
                    "TEXTFIELD ppp={:.3} rect=({:.1},{:.1})-({:.1},{:.1}) PXCX={} PXCY={}",
                    ppp,
                    r.left(),
                    r.top(),
                    r.right(),
                    r.bottom(),
                    px.0,
                    px.1
                );
                self.last_px = px;
            }
            if chars != self.last_chars {
                log::info!("TYPED chars={}", chars);
                self.last_chars = chars;
            }

            #[cfg(target_os = "android")]
            {
                // Manual controls for on-device isolation testing.
                ui.horizontal(|ui| {
                    if ui.button("Show keyboard").clicked() {
                        self.app.show_soft_input(true);
                    }
                    if ui.button("Hide keyboard").clicked() {
                        self.app.hide_soft_input(false);
                    }
                });

                let focused = response.response.has_focus();
                // Self-healing: re-request the IME every frame while focused.
                // Idempotent on Android and immune to state desync.
                if focused {
                    self.app.show_soft_input(true);
                } else {
                    self.app.hide_soft_input(false);
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
