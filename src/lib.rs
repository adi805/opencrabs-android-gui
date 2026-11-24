// Spike v5: GameActivity IME bridge proof for the OpenCrabs Android GUI.
//
// Root cause of the previous "-100" build, verified against upstream source
// (not guessed, file:line included):
//
//   1. android-activity 0.6 `native_activity` implements `text_input_state()`
//      as "always return an empty state" and `set_text_input_state()` as an
//      explicit NOP
//      (android-activity-0.6.0/src/native_activity/mod.rs:381 and :390).
//      The old spike could therefore never receive typed characters, no matter
//      what the Java layer did.
//
//   2. winit 0.30's Android backend only maps `InputEvent::MotionEvent` and
//      `InputEvent::KeyEvent`
//      (winit-0.30.0/src/platform_impl/android/mod.rs:378 and :427).
//      `InputEvent::TextEvent` is never forwarded, so egui never sees an IME
//      commit on native_activity.
//
//   3. `game_activity` has a real bridge. AGDK's Java GameActivity implements
//      `stateChanged(State, boolean)` and forwards it to
//      `onTextInputEventNative(...)`, which lands in
//      `GameTextInput::processEvent` and updates the native state returned by
//      `GameActivity_getTextInputState()`
//      (android-activity-0.6.0/game-activity-csrc/game-text-input/gametextinput.cpp:280).
//
// So this spike polls `AndroidApp::text_input_state()` once per frame and
// adopts whatever the IME committed into the egui TextEdit buffer.
//
// Push-back (`set_text_input_state`) is used ONLY for programmatic edits:
// AGDK's `InputConnection.setState()` clears the editable and re-inserts the
// whole text, so pushing it every frame would wipe characters the IME just
// committed.

use eframe::egui;

#[cfg(target_os = "android")]
use std::time::Duration;

#[cfg(target_os = "android")]
use winit::platform::android::activity::{input::TextInputState, input::TextSpan, AndroidApp};

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("OcGui"),
    );
    log::info!("APP_START glue=game-activity");

    let options = eframe::NativeOptions {
        android_app: Some(app.clone()),
        ..Default::default()
    };

    eframe::run_native(
        "OpenCrabs GUI",
        options,
        Box::new(|_cc| Ok(Box::new(Spike::new(app)))),
    )
    .expect("eframe::run_native failed");
}

#[cfg(target_os = "android")]
struct Spike {
    app: AndroidApp,
    /// Buffer owned by the egui TextEdit.
    typed: String,
    /// Last text the IME reported through `text_input_state()`.
    ime_text: String,
    /// Text we asked the IME to adopt; while pending we must not adopt the
    /// stale IME state back, otherwise a programmatic edit gets undone.
    pending_push: Option<String>,
    focus_requested: bool,
    last_focus: Option<bool>,
    ime_calls: u32,
    next_ime_at: f64,
    frame: u64,
    last_px: (i32, i32),
    last_chars: usize,
    last_ime_chars: usize,
}

#[cfg(target_os = "android")]
impl Spike {
    fn new(app: AndroidApp) -> Self {
        Self {
            app,
            typed: String::new(),
            ime_text: String::new(),
            pending_push: None,
            focus_requested: false,
            last_focus: None,
            ime_calls: 0,
            next_ime_at: 0.0,
            frame: 0,
            last_px: (-1, -1),
            last_chars: usize::MAX,
            last_ime_chars: usize::MAX,
        }
    }

    /// Push a full replacement of the IME buffer (used by the Clear button).
    fn push_text(&mut self, text: &str) {
        let len = text.len();
        self.app.set_text_input_state(TextInputState {
            text: text.to_string(),
            selection: TextSpan {
                start: len,
                end: len,
            },
            compose_region: None,
        });
        self.pending_push = Some(text.to_string());
        log::info!("IME_PUSH chars={} text={:?}", text.chars().count(), text);
    }
}

#[cfg(target_os = "android")]
impl eframe::App for Spike {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame += 1;
        let now = ui.input(|i| i.time);

        // 1. Read the IME state FIRST. AGDK's InputConnection.setState()
        //    replaces the whole editable, so we must never push before we know
        //    what the IME currently holds.
        let ts = self.app.text_input_state();
        let ime_chars = ts.text.chars().count();
        if ime_chars != self.last_ime_chars || ts.text != self.ime_text {
            log::info!(
                "IME_TEXT chars={} text={:?} sel={}..{}",
                ime_chars,
                ts.text,
                ts.selection.start,
                ts.selection.end
            );
            self.last_ime_chars = ime_chars;
        }
        self.ime_text = ts.text.clone();

        // 2. Adopt what the IME committed, unless we are waiting for our own
        //    push-back to land.
        match &self.pending_push {
            Some(expected) if *expected == self.ime_text => {
                self.pending_push = None;
            }
            Some(_) => {}
            None => {
                if self.ime_text != self.typed {
                    log::info!(
                        "IME_ADOPT chars={} text={:?}",
                        self.ime_text.chars().count(),
                        self.ime_text
                    );
                    self.typed = self.ime_text.clone();
                }
            }
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("OpenCrabs GUI spike v5 (game-activity)");
            ui.separator();

            let out = egui::TextEdit::multiline(&mut self.typed)
                .hint_text("ketik di sini")
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .show(ui);
            let focused = out.response.has_focus();

            if !self.focus_requested {
                out.response.request_focus();
                self.focus_requested = true;
            }

            if self.last_focus != Some(focused) {
                log::info!("FOCUS {}", focused);
                self.last_focus = Some(focused);
            }

            // AGDK's setSoftKeyboardActive(true) calls restartInput(), so the
            // IME must be requested on an edge (and retried a few times), never
            // every frame.
            if focused && (self.ime_calls == 0 || (self.ime_calls < 6 && now >= self.next_ime_at))
            {
                self.app.show_soft_input(true);
                self.ime_calls += 1;
                self.next_ime_at = now + 1.5;
                log::info!("IME_REQUEST n={}", self.ime_calls);
            }

            ui.add_space(8.0);
            ui.label(format!("ime buffer: {:?}", self.ime_text));
            ui.label(format!("focused: {}", focused));
            let chars = self.typed.chars().count();
            ui.label(format!("typed: {} '{}'", chars, self.typed));

            if ui.button("Clear").clicked() {
                self.typed.clear();
                self.push_text("");
            }

            // CI evidence hooks.
            let ppp = ui.ctx().pixels_per_point();
            let rect = out.response.rect;
            let px = (
                (rect.center().x * ppp).round() as i32,
                (rect.center().y * ppp).round() as i32,
            );
            if px != self.last_px {
                log::info!(
                    "TEXTFIELD pxcx={} pxcy={} w={} h={} ppp={}",
                    px.0,
                    px.1,
                    (rect.width() * ppp).round() as i32,
                    (rect.height() * ppp).round() as i32,
                    ppp
                );
                self.last_px = px;
            }
            if chars != self.last_chars {
                log::info!("TYPED chars={} text={:?}", chars, self.typed);
                self.last_chars = chars;
            }
            if self.frame == 1 {
                log::info!(
                    "APP_READY ppp={} field={}x{}",
                    ppp,
                    (rect.width() * ppp).round() as i32,
                    (rect.height() * ppp).round() as i32
                );
            }
        });

        // Keep polling the IME even when nothing else happens.
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}
