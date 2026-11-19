use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Sets or clears native Always-on-Top window level immediately at runtime.
/// - macOS: `NSWindow.setLevel` (`NSFloatingWindowLevel = 3` vs `NSNormalWindowLevel = 0`)
/// - Windows: `SetWindowPos(HWND_TOPMOST / HWND_NOTOPMOST)`
pub fn set_always_on_top(window: &Window, enabled: bool) -> bool {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return false;
    };

    match handle.as_raw() {
        #[cfg(target_os = "macos")]
        RawWindowHandle::AppKit(appkit) => {
            use objc2_app_kit::NSView;
            // SAFETY: GPUI guarantees `ns_view` is a valid live `NSView` pointer for the lifetime of `Window`.
            unsafe {
                let view_ptr = appkit.ns_view.as_ptr() as *const NSView;
                if let Some(view) = view_ptr.as_ref() {
                    if let Some(ns_window) = view.window() {
                        // NSFloatingWindowLevel = 3, NSNormalWindowLevel = 0
                        let level: isize = if enabled { 3 } else { 0 };
                        ns_window.setLevel(level);
                        return true;
                    }
                }
            }
            false
        }
        #[cfg(target_os = "windows")]
        RawWindowHandle::Win32(win32) => {
            use windows_sys::Win32::Foundation::HWND;
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
            };
            let hwnd = win32.hwnd.get() as HWND;
            let insert_after = if enabled {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            };
            // SAFETY: Valid Win32 HWND obtained from GPUI window handle.
            let ok = unsafe {
                SetWindowPos(
                    hwnd,
                    insert_after,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )
            };
            ok != 0
        }
        _ => {
            let _ = enabled;
            false
        }
    }
}
