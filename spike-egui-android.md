# Spike: egui on Android via GameActivity (Gradle shell + cargo-ndk .so)

Adi is NOT the test subject anymore. This spike must prove keyboard + text
input OURSELVES on an emulator before any APK reaches the owner.

## Why GameActivity (root cause, verified from source)
- android-activity 0.6.x `native_activity::set_text_input_state` = NOP
  (src/native_activity/mod.rs:444) and `text_input_state()` returns empty.
- winit 0.30 android backend drops `InputEvent::TextEvent` entirely
  (platform_impl/android/mod.rs only matches MotionEvent/KeyEvent).
- game_activity's `text_input_state()` IS real (GameTextInput FFI) and
  `set_text_input_state` works too, so the app loop can pull committed text
  each frame and push back selection state.
- winit PR #4451 (Android IME) is still a draft -> cannot wait for it.
- GameActivity requires the Java class + resources from
  `androidx.games:games-activity` AAR -> needs Gradle (cargo-apk cannot dex
  Java), following rust-mobile/rust-android-examples agdk-egui.

## Build flow (CI only, never local)
1. cargo ndk -t arm64-v8a build -> .so into app/src/main/jniLibs/arm64-v8a/
2. ./gradlew :app:assembleDebug -> signed debug APK (debug keystore generated
   in CI).
3. Emulator test on macOS runner (HW accel) or ubuntu with TCG enabled:
   install APK, tap field, send IME text via `adb shell input text`,
   read logcat TYPED= counter > 0, screenshot as evidence.

## Status
- [ ] cargo game-activity layer builds in CI
- [ ] gradle APK assembles and installs
- [ ] emulator: keyboard visible + chars>0 (self-verified)
