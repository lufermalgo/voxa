// Native macOS CGEventTap — FFI, callback, and helper functions.
// This module owns the global event tap and all related OS-level utilities.

use std::sync::atomic::{AtomicPtr, AtomicU64, AtomicBool, Ordering};
use tauri::Manager;

use crate::pipeline::{DictationEvent, DictationSender, RecordingState};
use crate::shortcuts::NATIVE_SHORTCUTS;

pub static LAST_EVENT_TIME: AtomicU64 = AtomicU64::new(0);
pub static IS_PTT_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Static storage for the AppHandle pointer used by the event tap callback.
/// We store it here (rather than leaking it) so we can guard against double-init.
static EVENT_TAP_HANDLE: AtomicPtr<tauri::AppHandle> = AtomicPtr::new(std::ptr::null_mut());

// ---------------------------------------------------------------------------
// Native FFI
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub mod native_ffi {
    pub type CGEventRef = *mut std::os::raw::c_void;
    pub type CFMachPortRef = *mut std::os::raw::c_void;
    pub type CFRunLoopRef = *mut std::os::raw::c_void;
    pub type CFRunLoopSourceRef = *mut std::os::raw::c_void;
    pub type CFStringRef = *mut std::os::raw::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGEventTapCreate(
            tap: core_graphics::event::CGEventTapLocation,
            place: core_graphics::event::CGEventTapPlacement,
            options: core_graphics::event::CGEventTapOptions,
            eventsOfInterest: u64,
            callback: unsafe extern "C" fn(
                proxy: *mut std::os::raw::c_void,
                type_: u32,
                event: CGEventRef,
                refcon: *mut std::os::raw::c_void,
            ) -> CGEventRef,
            refcon: *mut std::os::raw::c_void,
        ) -> CFMachPortRef;

        pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
        pub fn CGEventGetFlags(event: CGEventRef) -> u64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFMachPortCreateRunLoopSource(
            allocator: *mut std::os::raw::c_void,
            port: CFMachPortRef,
            order: isize,
        ) -> CFRunLoopSourceRef;
        pub fn CFRunLoopGetMain() -> CFRunLoopRef;
        pub fn CFRunLoopAddSource(
            rl: CFRunLoopRef,
            source: CFRunLoopSourceRef,
            mode: CFStringRef,
        );
        pub static kCFRunLoopCommonModes: CFStringRef;
    }
}

// ---------------------------------------------------------------------------
// Key-name helpers
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub fn macos_keycode_to_name(keycode: u16) -> String {
    match keycode {
        36 => "Enter".to_string(),
        48 => "Tab".to_string(),
        49 => "Space".to_string(),
        51 => "Backspace".to_string(),
        53 => "Escape".to_string(),
        115 => "Home".to_string(),
        116 => "PageUp".to_string(),
        117 => "Delete".to_string(),
        119 => "End".to_string(),
        121 => "PageDown".to_string(),
        122 => "F1".to_string(),
        120 => "F2".to_string(),
        99  => "F3".to_string(),
        118 => "F4".to_string(),
        96  => "F5".to_string(),
        80  => "F5".to_string(),
        176 => "F5".to_string(), // Hardware Dictation/Microphone key
        97  => "F6".to_string(),
        98  => "F7".to_string(),
        100 => "F8".to_string(),
        101 => "F9".to_string(),
        109 => "F10".to_string(),
        103 => "F11".to_string(),
        111 => "F12".to_string(),
        123 => "Left".to_string(),
        124 => "Right".to_string(),
        125 => "Down".to_string(),
        126 => "Up".to_string(),
        179 => "F5".to_string(),
        160 => "MissionControl".to_string(),
        0   => "A".to_string(),
        1   => "S".to_string(),
        2   => "D".to_string(),
        3   => "F".to_string(),
        4   => "H".to_string(),
        5   => "G".to_string(),
        6   => "Z".to_string(),
        7   => "X".to_string(),
        8   => "C".to_string(),
        9   => "V".to_string(),
        11  => "B".to_string(),
        12  => "Q".to_string(),
        13  => "W".to_string(),
        14  => "E".to_string(),
        15  => "R".to_string(),
        16  => "Y".to_string(),
        17  => "T".to_string(),
        31  => "O".to_string(),
        32  => "U".to_string(),
        34  => "I".to_string(),
        35  => "P".to_string(),
        37  => "L".to_string(),
        38  => "J".to_string(),
        40  => "K".to_string(),
        45  => "N".to_string(),
        46  => "M".to_string(),
        _   => format!("Key_{}", keycode),
    }
}

/// Convert CGEventFlags to our internal modifier-prefix string.
#[cfg(target_os = "macos")]
pub fn flags_to_string(flags: core_graphics::event::CGEventFlags) -> String {
    use core_graphics::event::CGEventFlags;
    let mut s = String::new();
    if flags.contains(CGEventFlags::CGEventFlagCommand)  { s.push_str("CommandOrControl+"); }
    if flags.contains(CGEventFlags::CGEventFlagAlternate){ s.push_str("Alt+"); }
    if flags.contains(CGEventFlags::CGEventFlagControl)  { s.push_str("Control+"); }
    if flags.contains(CGEventFlags::CGEventFlagShift)    { s.push_str("Shift+"); }
    s
}

// ---------------------------------------------------------------------------
// OS utilities
// ---------------------------------------------------------------------------

pub fn play_sound(name: &str) {
    let path = format!("/System/Library/Sounds/{}.aiff", name);
    let _ = std::process::Command::new("afplay").arg(path).spawn();
}

/// Returns the PID of the current frontmost application, excluding Voxa itself.
#[cfg(target_os = "macos")]
pub fn get_frontmost_app_pid() -> Option<i32> {
    unsafe {
        let workspace: cocoa::base::id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let frontmost: cocoa::base::id = msg_send![workspace, frontmostApplication];
        if frontmost.is_null() { return None; }
        let pid: i32 = msg_send![frontmost, processIdentifier];
        let own_pid = std::process::id() as i32;
        if pid == own_pid { return None; }
        Some(pid)
    }
}

/// Returns the app name, and icon (base64 PNG, 32×32) for the given PID.
#[cfg(target_os = "macos")]
pub fn get_app_info_for_pid(pid: i32) -> Option<crate::pipeline::AppInfo> {
    use base64::Engine as _;
    if pid <= 0 { return None; }
    unsafe {
        let running_app: cocoa::base::id = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        if running_app.is_null() { return None; }

        // Localized name
        let name_ns: cocoa::base::id = msg_send![running_app, localizedName];
        let name = if name_ns.is_null() {
            String::new()
        } else {
            let bytes: *const std::os::raw::c_char = msg_send![name_ns, UTF8String];
            if bytes.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(bytes).to_string_lossy().into_owned()
            }
        };

        // Icon: NSImage → TIFF → NSBitmapImageRep → PNG → base64
        let icon_base64 = (|| -> Option<String> {
            let icon: cocoa::base::id = msg_send![running_app, icon];
            if icon.is_null() { return None; }

            let size = cocoa::foundation::NSSize { width: 32.0, height: 32.0 };
            let _: () = msg_send![icon, setSize: size];

            let tiff: cocoa::base::id = msg_send![icon, TIFFRepresentation];
            if tiff.is_null() { return None; }

            let bitmap: cocoa::base::id = msg_send![
                class!(NSBitmapImageRep),
                imageRepWithData: tiff
            ];
            if bitmap.is_null() { return None; }

            let props: cocoa::base::id = msg_send![class!(NSDictionary), dictionary];
            // NSPNGFileType = 4
            let png_data: cocoa::base::id = msg_send![
                bitmap,
                representationUsingType: 4u64
                properties: props
            ];
            if png_data.is_null() { return None; }

            let length: usize = msg_send![png_data, length];
            let bytes: *const u8 = msg_send![png_data, bytes];
            if bytes.is_null() || length == 0 { return None; }

            let slice = std::slice::from_raw_parts(bytes, length);
            Some(base64::engine::general_purpose::STANDARD.encode(slice))
        })();

        Some(crate::pipeline::AppInfo { pid, name, icon_base64 })
    }
}

/// Re-activates an app by PID using NSRunningApplication.
#[cfg(target_os = "macos")]
pub fn activate_app_by_pid(pid: i32) {
    if pid <= 0 { return; }
    unsafe {
        let running_app: cocoa::base::id = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        if !running_app.is_null() {
            let _: objc::runtime::BOOL = msg_send![running_app, activateWithOptions: 3u64];
        }
    }
}

/// Reads the text surrounding the cursor in the frontmost application via the
/// macOS Accessibility API. Returns `(pre_text, post_text)` — up to 200 chars
/// before and after the cursor. Returns empty strings on any failure.
#[cfg(target_os = "macos")]
pub fn get_cursor_context() -> (String, String) {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    // AXUIElement / AXValue types — declared locally to avoid pulling in a full AX crate.
    type AXUIElementRef = *mut std::os::raw::c_void;
    type AXValueRef    = *mut std::os::raw::c_void;
    type CFTypeRef     = *mut std::os::raw::c_void;
    type AXError       = i32;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: core_foundation::string::CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn AXValueGetValue(value: AXValueRef, ax_type: i32, value_ptr: *mut std::os::raw::c_void) -> bool;
        fn CFRelease(cf: CFTypeRef);
    }

    // CFRange mirrors NSRange (location + length), both are usize on 64-bit.
    #[repr(C)]
    #[derive(Default, Debug, Clone, Copy)]
    struct CFRange {
        location: isize,
        length:   isize,
    }

    const AX_ERROR_SUCCESS: AXError = 0;
    // AXValueType for CFRange is 3
    const AX_VALUE_TYPE_CFRANGE: i32 = 3;

    unsafe {
        // 1. Get focused UI element
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() { return (String::new(), String::new()); }

        let attr_focused = CFString::new("AXFocusedUIElement");
        let mut focused_ref: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            system_wide,
            attr_focused.as_concrete_TypeRef(),
            &mut focused_ref,
        );
        CFRelease(system_wide as _);
        if err != AX_ERROR_SUCCESS || focused_ref.is_null() {
            return (String::new(), String::new());
        }
        let focused_element = focused_ref as AXUIElementRef;

        // 2. Get the full text value of the focused element
        let attr_value = CFString::new("AXValue");
        let mut value_ref: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            focused_element,
            attr_value.as_concrete_TypeRef(),
            &mut value_ref,
        );
        if err != AX_ERROR_SUCCESS || value_ref.is_null() {
            CFRelease(focused_ref);
            return (String::new(), String::new());
        }
        // value_ref is a CFStringRef — wrap it for safe handling
        let full_text = {
            let cf_str = CFString::wrap_under_create_rule(value_ref as core_foundation::string::CFStringRef);
            cf_str.to_string()
        };

        // 3. Get selected text range (cursor position)
        let attr_range = CFString::new("AXSelectedTextRange");
        let mut range_ref: CFTypeRef = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            focused_element,
            attr_range.as_concrete_TypeRef(),
            &mut range_ref,
        );
        CFRelease(focused_ref);
        if err != AX_ERROR_SUCCESS || range_ref.is_null() {
            return (String::new(), String::new());
        }

        let mut range = CFRange::default();
        let got = AXValueGetValue(range_ref as AXValueRef, AX_VALUE_TYPE_CFRANGE, &mut range as *mut _ as *mut _);
        CFRelease(range_ref);
        if !got {
            return (String::new(), String::new());
        }

        // 4. Slice up to 200 chars before/after cursor position (char-boundary safe)
        let cursor_pos = range.location.max(0) as usize;
        let chars: Vec<char> = full_text.chars().collect();
        let total = chars.len();

        let pre_start = cursor_pos.saturating_sub(200);
        let pre_text: String = chars[pre_start..cursor_pos.min(total)].iter().collect();

        let post_end = (cursor_pos + 200).min(total);
        let post_text: String = chars[cursor_pos.min(total)..post_end].iter().collect();

        (pre_text, post_text)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_cursor_context() -> (String, String) {
    (String::new(), String::new())
}

/// Sends Cmd+V to the currently active application via CGEvent.
#[cfg(target_os = "macos")]
pub fn simulate_paste() {
    use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        let key_v: CGKeyCode = 9; // kVK_ANSI_V
        if let Ok(key_down) = CGEvent::new_keyboard_event(source.clone(), key_v, true) {
            key_down.set_flags(CGEventFlags::CGEventFlagCommand);
            key_down.post(core_graphics::event::CGEventTapLocation::HID);
        }
        if let Ok(key_up) = CGEvent::new_keyboard_event(source, key_v, false) {
            key_up.set_flags(CGEventFlags::CGEventFlagCommand);
            key_up.post(core_graphics::event::CGEventTapLocation::HID);
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn simulate_paste() {
    // Placeholder for non-macOS platforms
}

// ---------------------------------------------------------------------------
// Event tap callback
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub unsafe extern "C" fn native_tap_callback(
    _proxy: *mut std::os::raw::c_void,
    _type: u32,
    event_ref: native_ffi::CGEventRef,
    _refcon: *mut std::os::raw::c_void,
) -> native_ffi::CGEventRef {
    use core_graphics::event::CGEventFlags;

    let handle_ptr = EVENT_TAP_HANDLE.load(Ordering::SeqCst);
    if handle_ptr.is_null() { return event_ref; }
    let app_handle = &*handle_ptr;

    let is_system_event  = _type == 14;
    let is_key_down      = _type == 10;
    let is_key_up        = _type == 11;
    let is_flags_changed = _type == 12;

    let key_code = unsafe { native_ffi::CGEventGetIntegerValueField(event_ref, 9) } as u16;
    let raw_flags = unsafe { native_ffi::CGEventGetFlags(event_ref) };
    let flags = CGEventFlags::from_bits_truncate(raw_flags);
    let key_name = macos_keycode_to_name(key_code);

    if is_key_down || is_system_event {
        let mut current_accel = flags_to_string(flags);
        current_accel.push_str(&key_name);

        if let Some(shortcuts_mutex) = NATIVE_SHORTCUTS.get() {
            if let Ok(shortcuts) = shortcuts_mutex.lock() {
                let mut matched = false;
                let mut event_to_send = None;

                let has_modifiers = current_accel.contains("CommandOrControl+")
                    || current_accel.contains("Alt+")
                    || current_accel.contains("Control+")
                    || current_accel.contains("Shift+");
                let is_hardware_key = key_code == 176 || key_code == 179;

                if current_accel == shortcuts.ptt {
                    let is_autorepeat =
                        unsafe { native_ffi::CGEventGetIntegerValueField(event_ref, 7) } != 0;
                    if is_autorepeat {
                        return std::ptr::null_mut();
                    }
                    let is_recording = app_handle
                        .state::<RecordingState>()
                        .0
                        .load(Ordering::SeqCst);
                    if !is_recording {
                        matched = true;
                        let (pre, post) = get_cursor_context();
                        event_to_send = Some(DictationEvent::StartRecording { pre_text: pre, post_text: post });
                    } else {
                        return std::ptr::null_mut();
                    }
                } else if current_accel == shortcuts.hands_free
                    || (key_code == 176
                        && (shortcuts.hands_free == "F5" || shortcuts.hands_free == "Dictation"))
                {
                    matched = true;
                    let is_recording = app_handle
                        .state::<RecordingState>()
                        .0
                        .load(Ordering::SeqCst);
                    event_to_send = if is_recording {
                        Some(DictationEvent::StopRecording)
                    } else {
                        let (pre, post) = get_cursor_context();
                        Some(DictationEvent::StartRecording { pre_text: pre, post_text: post })
                    };
                } else if current_accel == shortcuts.paste {
                    matched = true;
                } else if current_accel == shortcuts.cancel {
                    let is_recording = app_handle
                        .state::<RecordingState>()
                        .0
                        .load(Ordering::SeqCst);
                    if is_recording {
                        matched = true;
                        event_to_send = Some(DictationEvent::CancelRecording);
                    }
                }

                if matched && !is_hardware_key && !has_modifiers {
                    if key_name != "Escape" || current_accel != shortcuts.cancel {
                        matched = false;
                        event_to_send = None;
                    }
                }

                if matched {
                    if current_accel == shortcuts.paste {
                        simulate_paste();
                    }
                    if key_code == 176 {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let last = LAST_EVENT_TIME.load(Ordering::SeqCst);
                        if now - last < 300 {
                            return std::ptr::null_mut();
                        }
                        LAST_EVENT_TIME.store(now, Ordering::SeqCst);
                    }

                    if let Some(ev) = event_to_send {
                        if let Ok(tx) = app_handle.state::<DictationSender>().0.lock() {
                            match ev {
                                DictationEvent::StartRecording { .. } => {
                                    app_handle
                                        .state::<RecordingState>()
                                        .0
                                        .store(true, Ordering::SeqCst);
                                    if current_accel == shortcuts.ptt {
                                        IS_PTT_ACTIVE.store(true, Ordering::SeqCst);
                                    }
                                    play_sound("Tink");
                                }
                                DictationEvent::StopRecording
                                | DictationEvent::CancelRecording => {
                                    app_handle
                                        .state::<RecordingState>()
                                        .0
                                        .store(false, Ordering::SeqCst);
                                    IS_PTT_ACTIVE.store(false, Ordering::SeqCst);
                                    play_sound("Pop");
                                }
                            }
                            let _ = tx.send(ev);
                        }
                    }
                    return std::ptr::null_mut();
                }
            }
        }
    } else if is_key_up {
        let mut current_accel = flags_to_string(flags);
        current_accel.push_str(&key_name);

        if let Some(shortcuts_mutex) = NATIVE_SHORTCUTS.get() {
            if let Ok(shortcuts) = shortcuts_mutex.lock() {
                if current_accel == shortcuts.ptt
                    || IS_PTT_ACTIVE.load(Ordering::SeqCst)
                {
                    if shortcuts.ptt.ends_with(&key_name) {
                        if let Ok(tx) = app_handle.state::<DictationSender>().0.lock() {
                            app_handle
                                .state::<RecordingState>()
                                .0
                                .store(false, Ordering::SeqCst);
                            IS_PTT_ACTIVE.store(false, Ordering::SeqCst);
                            let _res = tx.send(DictationEvent::StopRecording);
                            play_sound("Pop");
                        }
                        return std::ptr::null_mut();
                    }
                }
                if current_accel == shortcuts.hands_free
                    || (key_code == 176
                        && (shortcuts.hands_free == "F5"
                            || shortcuts.hands_free == "Dictation"))
                {
                    return std::ptr::null_mut();
                }
            }
        }
    } else if is_flags_changed {
        if IS_PTT_ACTIVE.load(Ordering::SeqCst) {
            if let Some(shortcuts_mutex) = NATIVE_SHORTCUTS.get() {
                if let Ok(shortcuts) = shortcuts_mutex.lock() {
                    let current_modifiers = flags_to_string(flags);
                    if !shortcuts.ptt.starts_with(&current_modifiers) {
                        if let Ok(tx) = app_handle.state::<DictationSender>().0.lock() {
                            app_handle
                                .state::<RecordingState>()
                                .0
                                .store(false, Ordering::SeqCst);
                            IS_PTT_ACTIVE.store(false, Ordering::SeqCst);
                            let _res = tx.send(DictationEvent::StopRecording);
                            play_sound("Pop");
                        }
                    }
                }
            }
        }
    }

    event_ref
}

// ---------------------------------------------------------------------------
// Event tap setup
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub fn setup_native_event_tap(app_handle: tauri::AppHandle) {
    use core_graphics::event::{CGEventTapLocation, CGEventTapPlacement, CGEventTapOptions};

    let handle_ptr = Box::into_raw(Box::new(app_handle));

    // Only install once — if already set, free the duplicate and return.
    if EVENT_TAP_HANDLE
        .compare_exchange(
            std::ptr::null_mut(),
            handle_ptr,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_err()
    {
        unsafe { drop(Box::from_raw(handle_ptr)); }
        return;
    }

    // KeyDown (10), KeyUp (11), FlagsChanged (12), NSSystemDefined (14)
    let mask = (1 << 10) | (1 << 11) | (1 << 12) | (1 << 14);

    unsafe {
        // Use Session (not HID) level to avoid requiring Input Monitoring permission.
        // HID-level taps require kTCCServiceListenEvent which macOS denies for ad-hoc
        // signed apps launched from Finder/Launchpad. Session level only needs Accessibility.
        let tap_port = native_ffi::CGEventTapCreate(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            mask,
            native_tap_callback,
            EVENT_TAP_HANDLE.load(Ordering::SeqCst) as *mut _,
        );

        if !tap_port.is_null() {
            let loop_source_ref =
                native_ffi::CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap_port, 0);
            if !loop_source_ref.is_null() {
                let main_loop = native_ffi::CFRunLoopGetMain();
                native_ffi::CFRunLoopAddSource(
                    main_loop,
                    loop_source_ref,
                    native_ffi::kCFRunLoopCommonModes,
                );
                log::info!("Native event tap initialized.");
            }
        } else {
            log::error!(
                "Native event tap failed — check Accessibility permissions."
            );
        }
    }
}
