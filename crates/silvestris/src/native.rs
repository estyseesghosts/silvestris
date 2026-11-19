//! Platform window adapters. All unsafe platform code lives here
//! (handbook 03: "Keep all platform unsafe code behind ui-native").

use gpui::Window;

/// What the custom frame got from the platform, for the platform matrix and
/// tests (handbook 03: "capability flags … instead of compile-time lying").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCapabilities {
    /// The OS rounds the window silhouette (Windows 11 DWM).
    pub os_rounded_corners: bool,
    /// The OS hairline border was turned off (Windows 11).
    pub os_border_suppressed: bool,
}

/// Apply native frame settings once the window exists.
pub fn configure_frame(window: &Window) -> FrameCapabilities {
    imp::configure_frame(window)
}

/// Make the bottom-right `size` × `size` logical pixels of the client area
/// resize the window from the corner (HIG PDF p. 221 size control). On
/// Windows the hit test answers HTBOTTOMRIGHT there, so the OS runs the size
/// loop and sets the cursor; 0 removes the area. Elsewhere the drawn grip
/// calls `Window::start_window_resize` instead, and this is a no-op.
pub fn set_resize_grip(window: &Window, size: f32) {
    imp::set_resize_grip(window, size)
}

/// Caret blink half-period, or `None` when the user turned blinking off.
/// Windows: GetCaretBlinkTime, which returns INFINITE when blinking is off
/// (Microsoft Learn, GetCaretBlinkTime). designed: 500 ms elsewhere.
pub fn caret_blink() -> Option<std::time::Duration> {
    imp::caret_blink()
}

#[cfg(windows)]
mod imp {
    use super::FrameCapabilities;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{GetCaretBlinkTime, GetClientRect, IsZoomed, HTBOTTOMRIGHT, HTCLIENT, WM_NCDESTROY, WM_NCHITTEST};
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUNDSMALL, DWM_WINDOW_CORNER_PREFERENCE,
    };

    /// Ask DWM for the small corner radius. Windows 11 rounds top-level
    /// windows at 8 px by default and at 4 px for DWMWCP_ROUNDSMALL
    /// (Microsoft Learn, "Apply rounded corners in desktop apps for Windows
    /// 11"); 10.6 windows measure ~4 px (M: win.corner.top_left). DWM does
    /// not round maximized or snapped windows, and the attribute fails on
    /// Windows 10, which keeps square corners.
    ///
    /// Also removes the 1 px border Windows 11 draws around every window: it
    /// covered the chrome's highlight row (`#37393c` measured over the
    /// expected `#e2e2e2`, M: chrome.unified.active). DWMWA_COLOR_NONE
    /// "suppresses the drawing of the window border" (Microsoft Learn,
    /// DWMWINDOWATTRIBUTE).
    pub fn configure_frame(window: &gpui::Window) -> FrameCapabilities {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return FrameCapabilities::default();
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return FrameCapabilities::default();
        };
        let hwnd = HWND(win32.hwnd.get() as *mut core::ffi::c_void);
        let preference: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUNDSMALL;
        // SAFETY: `hwnd` belongs to GPUI's live window; the attribute value is
        // a 4-byte enum passed by pointer with its exact size.
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &preference as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            )
        };
        let none: u32 = DWMWA_COLOR_NONE;
        // SAFETY: as above; COLORREF-sized value.
        let border = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &none as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            )
        };
        FrameCapabilities {
            os_rounded_corners: result.is_ok(),
            os_border_suppressed: border.is_ok(),
        }
    }

    fn hwnd(window: &gpui::Window) -> Option<HWND> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::Win32(win32) = handle.as_raw() else { return None };
        Some(HWND(win32.hwnd.get() as *mut core::ffi::c_void))
    }

    /// Subclass id for the grip hook (any value unique to this callback).
    const GRIP_SUBCLASS: usize = 0x5356_4752; // "SVGR"

    pub fn caret_blink() -> Option<std::time::Duration> {
        // SAFETY: no arguments; reads a user setting.
        let ms = unsafe { GetCaretBlinkTime() };
        match ms {
            u32::MAX => None,
            0 => Some(std::time::Duration::from_millis(530)), // call failed: system default
            ms => Some(std::time::Duration::from_millis(ms as u64)),
        }
    }

    pub fn set_resize_grip(window: &gpui::Window, size: f32) {
        let Some(hwnd) = hwnd(window) else { return };
        // Reference data carries the size in logical pixels; installing the
        // same (callback, id) again only updates it (Microsoft Learn,
        // SetWindowSubclass).
        let size = size.round().max(0.0) as usize;
        // SAFETY: `hwnd` is GPUI's live window on this (its owning) thread;
        // `grip_proc` matches SUBCLASSPROC and removes itself on WM_NCDESTROY.
        unsafe {
            let _ = SetWindowSubclass(hwnd, Some(grip_proc), GRIP_SUBCLASS, size);
        }
    }

    unsafe extern "system" fn grip_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, id: usize, size: usize) -> LRESULT {
        if msg == WM_NCDESTROY {
            // SAFETY: removing our own subclass from the window being destroyed.
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(grip_proc), id);
            }
        }
        // SAFETY: forwards to the next window procedure (GPUI's) unchanged.
        let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        // Only take over plain client area: GPUI's own answers (caption,
        // window buttons, top resize band) are left alone.
        if msg != WM_NCHITTEST || result.0 != HTCLIENT as isize || size == 0 {
            return result;
        }
        // SAFETY: plain Win32 queries on a live window.
        unsafe {
            if IsZoomed(hwnd).as_bool() {
                return result;
            }
            // lparam carries signed screen coordinates (GET_X_LPARAM/GET_Y_LPARAM).
            let mut p = POINT { x: (lparam.0 & 0xffff) as i16 as i32, y: ((lparam.0 >> 16) & 0xffff) as i16 as i32 };
            let mut rc = RECT::default();
            if !ScreenToClient(hwnd, &mut p).as_bool() || GetClientRect(hwnd, &mut rc).is_err() {
                return result;
            }
            let side = (size as f32 * GetDpiForWindow(hwnd) as f32 / 96.0).round() as i32;
            if p.x >= rc.right - side && p.y >= rc.bottom - side {
                return LRESULT(HTBOTTOMRIGHT as isize);
            }
        }
        result
    }
}

#[cfg(not(windows))]
mod imp {
    use super::FrameCapabilities;

    /// Linux draws its own rounded silhouette with client decorations
    /// (chrome.rs); macOS is not a target yet (handbook 08).
    pub fn configure_frame(_window: &gpui::Window) -> FrameCapabilities {
        FrameCapabilities::default()
    }

    pub fn set_resize_grip(_window: &gpui::Window, _size: f32) {}

    pub fn caret_blink() -> Option<std::time::Duration> {
        Some(std::time::Duration::from_millis(500))
    }
}
