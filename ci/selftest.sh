#!/usr/bin/env bash
# Emulator self-test for the OpenCrabs Android GUI spike.
#
# WHAT THIS PROVES
#   The AGDK GameActivity IME bridge delivers committed text into the egui
#   TextEdit, end to end, on a booted Android emulator:
#     IME (ADBKeyboard) commitText -> InputConnection.stateUpdated()
#       -> listener.stateChanged() [GameActivity]
#       -> onTextInputEventNative (GameActivity.cpp:979)
#       -> GameTextInput_processEvent -> GameActivity_getTextInputState
#       -> AndroidApp::text_input_state() [android-activity 0.6 game_activity]
#       -> Spike adopts it -> egui shows "typed: 7 'halo123'".
#   The app ONLY ever reads text_input_state(), so "TYPED chars>0" in the
#   OcGui log can only have come through that bridge. Nothing else in the
#   app writes the buffer except the Clear button (which we never touch).
#
# WHAT THIS DOES NOT PROVE
#   Hardware-keyboard input, device-specific IMEs (Gboard autocorrect,
#   voice input), or anything beyond this TextEdit.
#
# WHY A SEPARATE FILE
#   reactivecircus/android-emulator-runner@v2 splits its `script:` input into
#   single lines and executes each one as its own `sh -c`
#   (lib/script-parser.js + lib/main.js:212-215, verified from source).
#   Shell variables do not survive across lines and multi-line
#   `until ... do ... done` blocks get shredded. The workflow passes exactly
#   one line: `bash ci/selftest.sh`.
#
# WHY AN EXTRA IME
#   `adb shell input text` injects raw KeyEvents. AGDK routes those to
#   onKeyDownNative/onKeyUpNative (GameActivity.cpp:975-977) which feed the
#   native key callback and never touch the text state, so they cannot
#   exercise the IME bridge. ADBKeyboard (senzhk/ADBKeyBoard, MIT) is a real
#   InputMethodService: its receiver commits text through
#   InputConnection.commitText(), which is exactly the path a phone keyboard
#   takes. It is downloaded into ci/ by the workflow, which verifies the
#   pinned sha256 BEFORE this script runs.

set -u
set -o pipefail

ADB="${ADB:-adb}"
ROOT="${GITHUB_WORKSPACE:-$(pwd)}"
APK="${APK:-$ROOT/app/build/outputs/apk/debug/app-debug.apk}"
ADBKEYBOARD_APK="${ADBKEYBOARD_APK:-$ROOT/ci/ADBKeyboard.apk}"
PKG="io.opencrabs.mobile.gui"
ACTIVITY="$PKG/.MainActivity"
IME_ID="com.android.adbkeyboard/.AdbIME"
TEXT_TO_TYPE="halo123"

say() { echo "[selftest] $*"; }

shot() { # $1 = output filename
  local name="$1"
  $ADB shell screencap -p "/sdcard/$name" >/dev/null 2>&1 || return 0
  $ADB pull "/sdcard/$name" "$ROOT/$name" >/dev/null 2>&1 || true
  say "screenshot: $ROOT/$name"
}

diag() {
  say "===== DIAGNOSTICS ====="
  echo "--- adb devices ---"; $ADB devices -l 2>&1 || true
  echo "--- wm size / density ---"; $ADB shell wm size 2>&1; $ADB shell wm density 2>&1
  echo "--- ime list -s ---"; $ADB shell ime list -s 2>&1 || true
  echo "--- dumpsys input_method (key fields) ---"
  $ADB shell dumpsys input_method 2>&1 | grep -E 'mCurMethodId|mCurShown|mShowRequested|mInputShown|mInputViewShown|mServedView|mCurFocusedWindow|mHaveConnection' || true
  echo "--- dumpsys window (focus + ime window) ---"
  $ADB shell dumpsys window 2>&1 | grep -E 'mCurrentFocus|mFocusedApp|mImeWindowVis|InputMethod' | head -12 || true
  echo "--- app log (OcGui) ---"
  $ADB logcat -d -s OcGui 2>&1 | tail -60 || true
  echo "--- logcat: fatal / IME related ---"
  $ADB logcat -d 2>&1 | grep -iE 'FATAL|AndroidRuntime|AdbIME|adbkeyboard|game-activity|InputMethodService' | tail -40 || true
  echo "===== END DIAGNOSTICS ====="
}

fail() {
  say "SELFTEST-FAIL: $*"
  shot "fail-selftest.png"
  diag
  exit 1
}

say "cwd: $(pwd)"
say "APK: $APK"
[ -f "$APK" ] || fail "APK not found: $APK"
[ -f "$ADBKEYBOARD_APK" ] || fail "ADBKeyboard APK not found: $ADBKEYBOARD_APK"

$ADB wait-for-device || fail "adb: no device"

BOOTED=0
for _ in $(seq 1 60); do
  if [ "$($ADB shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; then
    BOOTED=1; break
  fi
  sleep 5
done
[ "$BOOTED" = "1" ] || fail "emulator never finished booting"
say "device booted"

say "installing app APK"
$ADB install -r "$APK" 2>&1 | tail -3 || fail "adb install app failed"

say "installing ADBKeyboard IME"
$ADB install -r "$ADBKEYBOARD_APK" 2>&1 | tail -3 || fail "adb install ADBKeyboard failed"

# Run 45487962664 lesson: `ime enable` fired 2s after `install Success` and
# failed with "Unknown input method" because the system had not finished
# scanning the package yet. At that time the emulator ran under forced TCG
# (software emulation) because the runner user could not open /dev/kvm, so the
# whole system was crawling and even `system` showed an ANR. KVM is now
# enabled upstream (workflow step "Enable KVM access"), so the system is not
# overloaded any more, but the IME must still be polled in `ime list -s`
# until it is REGISTERED, however long that takes.
say "waiting for ADBKeyboard to register: $IME_ID"
IME_SEEN=0
for _ in $(seq 1 60); do
  if $ADB shell ime list -s 2>/dev/null | grep -q "^${IME_ID}$"; then IME_SEEN=1; break; fi
  sleep 10
done
[ "$IME_SEEN" = "1" ] || fail "ADBKeyboard never appeared in ime list -s"
say "selecting IME: $IME_ID"
$ADB shell ime enable "$IME_ID" 2>&1 | tail -1 || fail "ime enable failed"
$ADB shell ime set "$IME_ID" 2>&1 | tail -1 || fail "ime set failed"
$ADB shell dumpsys input_method 2>/dev/null | grep -o 'mCurMethodId=.*' | head -1
$ADB shell dumpsys input_method 2>/dev/null | grep -q "mCurMethodId=${IME_ID}" || fail "ime set did not take effect"
# The emulator ships a hardware keyboard; without this the soft IME window is
# suppressed and mInputShown stays false for the wrong reason.
$ADB shell settings put secure show_ime_with_hard_keyboard 1 2>&1 || true

$ADB logcat -c
say "starting $ACTIVITY"
$ADB shell am start -W -n "$ACTIVITY" 2>&1 | tail -5 || fail "am start failed"

# 1. App must be up and must report the TextEdit screen position.
say "waiting for TEXTFIELD log (app up + field coords)"
FOUND=0
for _ in $(seq 1 36); do
  if $ADB logcat -d -s OcGui 2>/dev/null | grep -q "TEXTFIELD"; then FOUND=1; break; fi
  sleep 5
done
[ "$FOUND" = "1" ] || fail "app never logged TEXTFIELD (did it even start?)"
$ADB logcat -d -s OcGui | grep -E 'APP_START|APP_READY|TEXTFIELD|FOCUS|IME_REQUEST' | tail -20

PXCX=$($ADB logcat -d -s OcGui | grep "TEXTFIELD" | tail -1 | sed 's/.*pxcx=\([0-9]*\).*/\1/')
PXCY=$($ADB logcat -d -s OcGui | grep "TEXTFIELD" | tail -1 | sed 's/.*pxcy=\([0-9]*\).*/\1/')
case "$PXCX" in ''|*[!0-9]*) fail "bad pxcx from log: '$PXCX'";; esac
case "$PXCY" in ''|*[!0-9]*) fail "bad pxcy from log: '$PXCY'";; esac
[ "$PXCX" -gt 0 ] && [ "$PXCY" -gt 0 ] || fail "invalid tap coords: $PXCX x $PXCY"
say "tap target: $PXCX x $PXCY"

say "tapping the TextEdit"
$ADB shell input tap "$PXCX" "$PXCY"

# 2. The IME window must actually be visible.
ime_shown() {
  $ADB shell dumpsys input_method 2>/dev/null | grep -qE 'mInputShown=true|mInputViewShown=true' && return 0
  $ADB shell dumpsys window 2>/dev/null | grep -qE 'mImeWindowVis=0x2' && return 0
  return 1
}
say "waiting for IME window to show"
KB=0
for _ in $(seq 1 24); do
  if ime_shown; then KB=1; break; fi
  sleep 5
done
[ "$KB" = "1" ] || fail "IME window never showed (mInputShown stayed false)"
say "KEYBOARD SHOWN"
shot "keyboard-shown.png"

# 3. Type through the IME bridge and require egui to receive it.
#    commitText replaces the whole editable each time, so retrying the same
#    broadcast is safe (text stays "halo123").
say "sending text via IME commit: $TEXT_TO_TYPE"
TYPED=0
for attempt in 1 2 3 4 5 6; do
  $ADB shell am broadcast -a ADB_INPUT_TEXT --es msg "$TEXT_TO_TYPE" 2>&1 | tail -2 || true
  for _ in $(seq 1 8); do
    if $ADB logcat -d -s OcGui 2>/dev/null | grep -qE 'TYPED chars=[1-9]'; then TYPED=1; break; fi
    sleep 5
  done
  [ "$TYPED" = "1" ] && break
  say "attempt $attempt: no TYPED yet, retrying"
done
[ "$TYPED" = "1" ] || fail "typed text never reached egui (IME bridge broken)"
say "TYPED REACHED EGUI"

$ADB logcat -d -s OcGui | grep -E 'IME_TEXT|IME_ADOPT|TYPED|IME_REQUEST' | tail -30
shot "pass.png"
say "SELFTEST PASS: keyboard shown + typed chars > 0"
