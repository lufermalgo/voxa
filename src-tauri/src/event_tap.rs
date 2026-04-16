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

        // Bundle ID — needed to detect browsers
        let bundle_id_str: Option<String> = {
            let bid_ns: cocoa::base::id = msg_send![running_app, bundleIdentifier];
            if bid_ns.is_null() { None } else {
                let bytes: *const std::os::raw::c_char = msg_send![bid_ns, UTF8String];
                if bytes.is_null() { None } else {
                    Some(std::ffi::CStr::from_ptr(bytes).to_string_lossy().into_owned())
                }
            }
        };

        let mut info = crate::pipeline::AppInfo { pid, name, icon_base64 };

        // Browser override: replace app name/icon with active web app identity
        if let Some(ref bid) = bundle_id_str {
            if is_browser_bundle_id(bid) {
                if let Some(url) = get_browser_tab_url(pid, bid) {
                    if let Some(domain) = domain_from_url(&url) {
                        if let Some(web_name) = web_app_name_from_domain(&domain) {
                            info.name = web_name.to_string();
                        }
                        if let Some(favicon) = get_favicon_from_browser_cache(&domain, bid) {
                            info.icon_base64 = Some(favicon);
                        }
                    }
                }
            }
        }

        Some(info)
    }
}

// ---------------------------------------------------------------------------
// Browser tab URL detection
// ---------------------------------------------------------------------------

/// Returns true if the given bundle ID belongs to a supported browser.
#[cfg(target_os = "macos")]
pub fn is_browser_bundle_id(bundle_id: &str) -> bool {
    matches!(bundle_id,
        "com.apple.Safari" | "com.google.Chrome" | "com.brave.Browser" |
        "company.thebrowser.Browser" | "com.microsoft.edgemac" |
        "org.mozilla.firefox" | "org.chromium.Chromium"
    )
}

/// Reads the active tab URL from a known browser via the macOS Accessibility API.
/// Returns None gracefully on any failure (missing permissions, unusual browser state, etc).
#[cfg(target_os = "macos")]
pub fn get_browser_tab_url(pid: i32, bundle_id: &str) -> Option<String> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    type Ref = *mut std::os::raw::c_void;
    type AXError = i32;
    const AX_OK: AXError = 0;

    #[allow(clashing_extern_declarations)]
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> Ref;
        fn AXUIElementCopyAttributeValue(elem: Ref, attr: Ref, val: *mut Ref) -> AXError;
        fn CFRelease(cf: Ref);
        fn CFRetain(cf: Ref) -> Ref;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(arr: Ref) -> isize;
        fn CFArrayGetValueAtIndex(arr: Ref, idx: isize) -> Ref;
        fn CFGetTypeID(cf: Ref) -> usize;
        fn CFStringGetTypeID() -> usize;
    }

    if pid <= 0 { return None; }

    // Read a string AX attribute. Returns None on type mismatch or missing value.
    let ax_str = |elem: Ref, attr: &str| -> Option<String> {
        unsafe {
            let a = CFString::new(attr);
            let mut val: Ref = std::ptr::null_mut();
            if AXUIElementCopyAttributeValue(elem, a.as_concrete_TypeRef() as Ref, &mut val) != AX_OK
                || val.is_null() { return None; }
            if CFGetTypeID(val) != CFStringGetTypeID() { CFRelease(val); return None; }
            let s = CFString::wrap_under_create_rule(val as core_foundation::string::CFStringRef).to_string();
            if s.is_empty() { None } else { Some(s) }
        }
    };

    // Get a child AX element (retained — caller must CFRelease).
    let ax_elem = |parent: Ref, attr: &str| -> Option<Ref> {
        unsafe {
            let a = CFString::new(attr);
            let mut val: Ref = std::ptr::null_mut();
            if AXUIElementCopyAttributeValue(parent, a.as_concrete_TypeRef() as Ref, &mut val) != AX_OK
                || val.is_null() { return None; }
            Some(val)
        }
    };

    // Get children as individually-retained Refs (caller must CFRelease each).
    let ax_children = |parent: Ref| -> Vec<Ref> {
        unsafe {
            let a = CFString::new("AXChildren");
            let mut arr: Ref = std::ptr::null_mut();
            if AXUIElementCopyAttributeValue(parent, a.as_concrete_TypeRef() as Ref, &mut arr) != AX_OK
                || arr.is_null() { return vec![]; }
            let count = CFArrayGetCount(arr);
            let mut out = Vec::with_capacity(count as usize);
            for i in 0..count {
                let child = CFArrayGetValueAtIndex(arr, i);
                if !child.is_null() { out.push(CFRetain(child)); }
            }
            CFRelease(arr);
            out
        }
    };

    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() { return None; }

        let b = bundle_id.to_lowercase();
        let window = ax_elem(app, "AXFocusedWindow");
        CFRelease(app);
        let window = window?;

        // Safari exposes AXDocument on the focused window — fast and reliable.
        if b == "com.apple.safari" {
            let url = ax_str(window, "AXDocument");
            CFRelease(window);
            return url;
        }

        // Chromium + Firefox: true BFS (VecDeque, FIFO) so shallow toolbar elements
        // are visited before deep web content.
        // Queue items are retained references we own and must CFRelease.
        let mut queue: std::collections::VecDeque<(Ref, u32)> = std::collections::VecDeque::new();
        queue.push_back((window, 10));
        let mut found: Option<String> = None;

        while let Some((elem, depth)) = queue.pop_front() {
            if found.is_none() {
                let role = ax_str(elem, "AXRole").unwrap_or_default();

                // Skip web content subtrees — avoids traversing the page DOM.
                if role != "AXWebArea" {
                    if role == "AXTextField" || role == "AXComboBox" {
                        let id   = ax_str(elem, "AXIdentifier").unwrap_or_default();
                        let desc = ax_str(elem, "AXDescription").unwrap_or_default();
                        let id_lc   = id.to_lowercase();
                        let desc_lc = desc.to_lowercase();

                        // Match by identifier OR description (Chrome ≥100 often has empty identifier).
                        let looks_like_url_bar =
                            id_lc.contains("address") || id_lc.contains("url") ||
                            id == "urlbar-input" ||
                            desc_lc.contains("address") || desc_lc.contains("search");

                        if looks_like_url_bar {
                            if let Some(val) = ax_str(elem, "AXValue") {
                                if !val.is_empty() && !val.contains('\n') {
                                    // Normalize: Chrome omits the scheme when bar is unfocused.
                                    let url = if val.starts_with("http://") || val.starts_with("https://") {
                                        val
                                    } else {
                                        format!("https://{}", val)
                                    };
                                    found = Some(url);
                                }
                            }
                        }
                    }
                    if found.is_none() && depth > 0 {
                        for child in ax_children(elem) {
                            queue.push_back((child, depth - 1));
                        }
                    }
                }
            }
            CFRelease(elem);
        }

        found
    }
}

/// Extracts the hostname from a URL, stripping scheme, www. prefix, and port.
/// "https://mail.google.com/mail/u/0/" → "mail.google.com"
pub fn domain_from_url(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url); // handle scheme-less URLs already normalised by caller
    let host = without_scheme.split('/').next()?;
    let host_no_port = host.split(':').next()?;
    let domain = host_no_port.strip_prefix("www.").unwrap_or(host_no_port);
    if domain.is_empty() || !domain.contains('.') { None } else { Some(domain.to_lowercase()) }
}

/// Maps a known web app domain to a human-readable display name.
pub fn web_app_name_from_domain(domain: &str) -> Option<&'static str> {
    let d = domain;
    if d == "mail.google.com"                            { return Some("Gmail"); }
    if d == "docs.google.com"                            { return Some("Google Docs"); }
    if d == "sheets.google.com"                          { return Some("Google Sheets"); }
    if d == "slides.google.com"                          { return Some("Google Slides"); }
    if d == "calendar.google.com"                        { return Some("Google Calendar"); }
    if d == "drive.google.com"                           { return Some("Google Drive"); }
    if d == "notion.so" || d.ends_with(".notion.so")     { return Some("Notion"); }
    if d == "github.com" || d.ends_with(".github.com")   { return Some("GitHub"); }
    if d == "linear.app" || d.ends_with(".linear.app")   { return Some("Linear"); }
    if d.ends_with(".slack.com") || d == "app.slack.com" { return Some("Slack"); }
    if d == "discord.com" || d.ends_with(".discord.com") { return Some("Discord"); }
    if d == "figma.com"  || d.ends_with(".figma.com")    { return Some("Figma"); }
    if d == "twitter.com" || d == "x.com"                { return Some("X"); }
    if d == "linkedin.com" || d.ends_with(".linkedin.com") { return Some("LinkedIn"); }
    if d == "reddit.com"  || d.ends_with(".reddit.com")  { return Some("Reddit"); }
    if d == "youtube.com" || d.ends_with(".youtube.com") { return Some("YouTube"); }
    if d == "claude.ai"                                  { return Some("Claude"); }
    if d == "chat.openai.com" || d == "chatgpt.com"      { return Some("ChatGPT"); }
    if d.contains("outlook.")  || d == "outlook.com"     { return Some("Outlook"); }
    if d.ends_with(".atlassian.net") && d.starts_with("jira") { return Some("Jira"); }
    if d.ends_with(".atlassian.net")                     { return Some("Confluence"); }
    if d.contains("confluence")                          { return Some("Confluence"); }
    if d == "coda.io" || d.ends_with(".coda.io")         { return Some("Coda"); }
    if d == "airtable.com"                               { return Some("Airtable"); }
    if d == "trello.com"                                 { return Some("Trello"); }
    if d == "miro.com" || d.ends_with(".miro.com")       { return Some("Miro"); }
    if d == "loom.com"  || d.ends_with(".loom.com")      { return Some("Loom"); }
    None
}

/// Reads the favicon for a domain from the browser's local SQLite favicon cache.
/// Only supports Chromium-family browsers (Chrome, Brave, Edge, Arc).
/// Returns None silently on any failure — DB locked, not found, wrong schema, etc.
#[cfg(target_os = "macos")]
pub fn get_favicon_from_browser_cache(domain: &str, bundle_id: &str) -> Option<String> {
    use base64::Engine as _;

    let home = std::env::var("HOME").ok()?;
    let b = bundle_id.to_lowercase();

    let db_str = if b == "com.google.chrome" {
        format!("{}/Library/Application Support/Google/Chrome/Default/Favicons", home)
    } else if b == "com.brave.browser" {
        format!("{}/Library/Application Support/BraveSoftware/Brave-Browser/Default/Favicons", home)
    } else if b == "com.microsoft.edgemac" {
        format!("{}/Library/Application Support/Microsoft Edge/Default/Favicons", home)
    } else if b == "company.thebrowser.browser" {
        format!("{}/Library/Application Support/Arc/User Data/Default/Favicons", home)
    } else {
        return None;
    };

    if !std::path::Path::new(&db_str).exists() { return None; }

    let conn = rusqlite::Connection::open_with_flags(
        &db_str,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ).ok()?;
    let _ = conn.busy_timeout(std::time::Duration::from_millis(100));

    let pattern = format!("%{}%", domain);
    let data: Option<Vec<u8>> = conn.query_row(
        "SELECT fb.image_data \
         FROM icon_mapping im \
         JOIN favicon_bitmaps fb ON im.icon_id = fb.icon_id \
         WHERE im.page_url LIKE ?1 \
         ORDER BY fb.width DESC LIMIT 1",
        rusqlite::params![pattern],
        |row| row.get(0),
    ).ok();

    data.map(|d| base64::engine::general_purpose::STANDARD.encode(&d))
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
