#![windows_subsystem = "windows"]
mod append;
mod clipboard;
mod ui;

use std::{
    ffi::OsStr,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::CreateMutexW},
    UI::{Controls::DRAWITEMSTRUCT, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
};

const CLASS: &str = "QuickTodo.Native.Window.1";
const MUTEX: &str = "Local\\QuickTodo.Native.SingleInstance.1";
const TRAY: u32 = WM_APP + 1;
const ACTIVATE: u32 = WM_APP + 2;
const QUIT_MSG: u32 = WM_APP + 3;
const EDIT: i32 = 100;
const SAVE: i32 = 101;
const HIDE: i32 = 102;
const ATTACH: i32 = 105;
const OPEN: usize = 103;
const EXIT: usize = 104;
const WINDOW_WIDTH: i32 = 620;
const WINDOW_HEIGHT: i32 = 360;
struct State {
    edit: HWND,
    target: PathBuf,
    screenshot: Option<Vec<u8>>,
    icon: NOTIFYICONDATAW,
    taskbar: u32,
    ui: Option<ui::Ui>,
}
fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}
unsafe fn message(hwnd: HWND, title: &str, text: &str, flags: u32) -> i32 {
    MessageBoxW(hwnd, wide(text).as_ptr(), wide(title).as_ptr(), flags)
}
unsafe fn state(hwnd: HWND) -> *mut State {
    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State
}
unsafe fn take_pending_state(param: isize) -> *mut State {
    if param == 0 {
        return null_mut();
    }
    let pending = &mut *(param as *mut Option<Box<State>>);
    pending.take().map_or(null_mut(), Box::into_raw)
}
unsafe fn free_window_state(hwnd: HWND) -> bool {
    let ptr = state(hwnd);
    if ptr.is_null() {
        return false;
    }
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    drop(Box::from_raw(ptr));
    true
}
fn is_image_paste_shortcut(is_edit: bool, key: u32, ctrl: i16, alt: i16, shift: i16) -> bool {
    is_edit && key == VK_V as u32 && ctrl < 0 && alt >= 0 && shift >= 0
}
unsafe fn draft(hwnd: HWND) -> String {
    let n = GetWindowTextLengthW(hwnd).max(0) as usize;
    let mut buf = vec![0u16; n + 1];
    let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
    String::from_utf16_lossy(&buf[..got])
}
unsafe fn show(hwnd: HWND) {
    ShowWindow(hwnd, SW_SHOW);
    ShowWindow(hwnd, SW_RESTORE);
    SetForegroundWindow(hwnd);
    let ptr = state(hwnd);
    if !ptr.is_null() {
        SetFocus((*ptr).edit);
    }
}
unsafe fn set_screenshot(hwnd: HWND, image: Option<Vec<u8>>) {
    let ptr = state(hwnd);
    if ptr.is_null() {
        return;
    }
    let attached = image.is_some();
    (*ptr).screenshot = image;
    if let Some(ui) = &(*ptr).ui {
        ui.show_attachment(attached);
    }
}
unsafe fn paste_image(hwnd: HWND, prefer_text: bool) -> bool {
    match clipboard::read_image(hwnd, prefer_text) {
        Ok(Some(image)) => {
            set_screenshot(hwnd, Some(image));
            true
        }
        Ok(None) if prefer_text => false,
        Ok(None) => {
            message(
                hwnd,
                "No image to paste",
                "Copy an image or take a screenshot with Win+Shift+S, then paste it here.",
                MB_OK | MB_ICONINFORMATION,
            );
            true
        }
        Err(error) => {
            message(
                hwnd,
                "Could not paste image",
                &error.to_string(),
                MB_OK | MB_ICONERROR,
            );
            true
        }
    }
}
unsafe fn save(hwnd: HWND) {
    // Copy handles and owned data before calls that can reenter the window procedure.
    let ptr = state(hwnd);
    if ptr.is_null() {
        return;
    }
    let edit = (*ptr).edit;
    let target = (*ptr).target.clone();
    let text = draft(edit);
    match append::append_tasks(&target, &text, (*ptr).screenshot.as_deref()) {
        Ok(true) => {
            set_screenshot(hwnd, None);
            SetWindowTextW(edit, wide("").as_ptr());
            ShowWindow(hwnd, SW_HIDE);
        }
        Ok(false) => {}
        Err(e) => {
            message(
                hwnd,
                "Could not save",
                &format!(
                    "The tasks were not fully saved. Your draft is still here.\n\n{}\n\n{}",
                    target.display(),
                    e
                ),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
unsafe fn quit(hwnd: HWND) {
    let ptr = state(hwnd);
    if ptr.is_null() {
        return;
    }
    if ((*ptr).screenshot.is_some() || !draft((*ptr).edit).trim().is_empty())
        && message(
            hwnd,
            "Unsaved draft",
            "Discard your unsaved draft and quit?",
            MB_YESNO | MB_ICONWARNING,
        ) != IDYES
    {
        return;
    }
    if !state(hwnd).is_null() {
        DestroyWindow(hwnd);
    }
}
unsafe fn add_tray(s: &mut State) {
    Shell_NotifyIconW(NIM_ADD, &s.icon);
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = &*(lp as *const CREATESTRUCTW);
        let raw = take_pending_state(cs.lpCreateParams as isize);
        if !raw.is_null() {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, raw as isize);
        }
    }
    let ptr = state(hwnd);
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    match msg {
        WM_CREATE => {
            let ui = ui::Ui::create(hwnd);
            (*ptr).edit = ui.edit;
            (*ptr).ui = Some(ui);
            let instance = GetModuleHandleW(null());
            let icon_large = LoadImageW(instance, 1 as _, IMAGE_ICON, 32, 32, LR_SHARED) as HICON;
            let icon_small = LoadImageW(instance, 1 as _, IMAGE_ICON, 16, 16, LR_SHARED) as HICON;
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icon_large as isize);
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icon_small as isize);
            (*ptr).icon.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            (*ptr).icon.hWnd = hwnd;
            (*ptr).icon.uID = 1;
            (*ptr).icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            (*ptr).icon.uCallbackMessage = TRAY;
            (*ptr).icon.hIcon = icon_small;
            let tip = wide("Quick Todo");
            (&mut (*ptr).icon.szTip)[..tip.len()].copy_from_slice(&tip);
            add_tray(&mut *ptr);
            0
        }
        WM_ERASEBKGND => (*ptr).ui.as_ref().map_or(0, |ui| ui.erase(wp as HDC, hwnd)),
        WM_CTLCOLORSTATIC => (*ptr)
            .ui
            .as_ref()
            .map_or(0, |ui| ui.background(wp as HDC, lp as HWND)),
        WM_CTLCOLOREDIT => (*ptr)
            .ui
            .as_ref()
            .map_or(0, |ui| ui.editor_background(wp as HDC)),
        WM_DRAWITEM => (*ptr)
            .ui
            .as_ref()
            .map_or(0, |ui| ui.draw_button(&*(lp as *const DRAWITEMSTRUCT))),
        WM_COMMAND => {
            match (wp & 0xffff) as i32 {
                SAVE => save(hwnd),
                ATTACH => {
                    if (*ptr).screenshot.is_some() {
                        set_screenshot(hwnd, None);
                    } else {
                        paste_image(hwnd, false);
                    }
                    let current = state(hwnd);
                    if !current.is_null() {
                        SetFocus((*current).edit);
                    }
                }
                HIDE => {
                    ShowWindow(hwnd, SW_HIDE);
                }
                _ => {}
            }
            0
        }
        WM_SETFOCUS => {
            let edit = (*ptr).edit;
            if !edit.is_null() {
                SetFocus(edit);
            }
            0
        }
        WM_HOTKEY | ACTIVATE => {
            show(hwnd);
            0
        }
        QUIT_MSG => {
            quit(hwnd);
            0
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        TRAY => {
            match lp as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => show(hwnd),
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    let menu = CreatePopupMenu();
                    AppendMenuW(menu, MF_STRING, ACTIVATE as usize, wide("Show").as_ptr());
                    AppendMenuW(menu, MF_STRING, OPEN, wide("Open Todo.md").as_ptr());
                    AppendMenuW(menu, MF_SEPARATOR, 0, null());
                    AppendMenuW(menu, MF_STRING, EXIT, wide("Quit").as_ptr());
                    let mut p: POINT = zeroed();
                    GetCursorPos(&mut p);
                    SetForegroundWindow(hwnd);
                    let cmd = TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD | TPM_RIGHTBUTTON,
                        p.x,
                        p.y,
                        0,
                        hwnd,
                        null(),
                    );
                    DestroyMenu(menu);
                    let current = state(hwnd);
                    if current.is_null() {
                        return 0;
                    }
                    match cmd as usize {
                        x if x == ACTIVATE as usize => show(hwnd),
                        OPEN => {
                            let path = wide((*current).target.as_os_str());
                            if ShellExecuteW(
                                hwnd,
                                wide("open").as_ptr(),
                                path.as_ptr(),
                                null(),
                                null(),
                                SW_SHOWNORMAL,
                            ) as isize
                                <= 32
                            {
                                message(
                                    hwnd,
                                    "Could not open",
                                    "Windows could not open the target file.",
                                    MB_OK | MB_ICONERROR,
                                );
                            }
                        }
                        EXIT => quit(hwnd),
                        _ => {}
                    }
                }
                _ => {}
            }
            0
        }
        WM_DPICHANGED => {
            let new_dpi = ((wp >> 16) & 0xffff) as u32;
            let suggested = &*(lp as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                suggested.left,
                suggested.top,
                suggested.right - suggested.left,
                suggested.bottom - suggested.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let current = state(hwnd);
            if !current.is_null() {
                if let Some(ui) = &mut (*current).ui {
                    ui.rescale(new_dpi);
                }
            }
            0
        }
        WM_DESTROY => {
            UnregisterHotKey(hwnd, 1);
            Shell_NotifyIconW(NIM_DELETE, &(*ptr).icon);
            if let Some(ui) = &(*ptr).ui {
                ui.destroy();
            }
            free_window_state(hwnd);
            PostQuitMessage(0);
            0
        }
        WM_NCDESTROY => {
            free_window_state(hwnd);
            0
        }
        _ if msg == (*ptr).taskbar && msg != 0 => {
            add_tray(&mut *ptr);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
fn main() {
    unsafe {
        if let Err(e) = run() {
            message(null_mut(), "Quick Todo", &e, MB_OK | MB_ICONERROR);
        }
    }
}
unsafe fn run() -> Result<(), String> {
    let mut target = None;
    let mut hidden = false;
    let mut request_quit = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--file") => {
                target = Some(PathBuf::from(args.next().ok_or("--file requires a path")?))
            }
            Some("--hidden") => hidden = true,
            Some("--quit") => request_quit = true,
            _ => return Err("Usage: quick-todo.exe [--file PATH] [--hidden] [--quit]".into()),
        }
    }
    let mutex = CreateMutexW(null(), 0, wide(MUTEX).as_ptr());
    if mutex.is_null() {
        return Err("Could not create the single-instance lock.".into());
    }
    if GetLastError() == ERROR_ALREADY_EXISTS {
        let hwnd = FindWindowW(wide(CLASS).as_ptr(), null());
        if !hwnd.is_null() {
            PostMessageW(hwnd, if request_quit { QUIT_MSG } else { ACTIVATE }, 0, 0);
        }
        CloseHandle(mutex);
        return Ok(());
    }
    if request_quit {
        CloseHandle(mutex);
        return Ok(());
    }
    if target.is_none() {
        let config = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("config.txt");
        let text = std::fs::read_to_string(&config)
            .map_err(|e| format!("Could not read {}: {}", config.display(), e))?;
        for line in text.lines() {
            if let Some(value) = line.trim().strip_prefix("target=") {
                target = Some(PathBuf::from(value.trim()));
            }
        }
    }
    let target = target
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("Set target= in config.txt beside the executable.")?;
    let instance = GetModuleHandleW(null());
    let class = wide(CLASS);
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: instance,
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        hIcon: LoadIconW(instance, 1 as _),
        hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
        lpszClassName: class.as_ptr(),
        ..zeroed()
    };
    if RegisterClassW(&wc) == 0 {
        return Err("Could not register the window class.".into());
    }
    let s = Box::new(State {
        edit: null_mut(),
        target,
        screenshot: None,
        icon: zeroed(),
        taskbar: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
        ui: None,
    });
    let mut pending_state = Some(s);
    let hwnd = CreateWindowExW(
        WS_EX_CONTROLPARENT,
        class.as_ptr(),
        wide("Quick Todo").as_ptr(),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        null_mut(),
        null_mut(),
        instance,
        (&mut pending_state as *mut Option<Box<State>>).cast(),
    );
    if hwnd.is_null() {
        drop(pending_state);
        CloseHandle(mutex);
        return Err("Could not create the window.".into());
    }
    if RegisterHotKey(hwnd, 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'T' as u32) == 0 {
        DestroyWindow(hwnd);
        CloseHandle(mutex);
        return Err("Ctrl+Alt+T is unavailable. Another application may already use it.".into());
    }
    // Center on the work area of the monitor containing the cursor.
    let mut pt: POINT = zeroed();
    GetCursorPos(&mut pt);
    let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) != 0 {
        let dpi = ui::window_dpi(hwnd);
        let width = ui::scale_for_dpi(WINDOW_WIDTH, dpi);
        let height = ui::scale_for_dpi(WINDOW_HEIGHT, dpi);
        let r = info.rcWork;
        SetWindowPos(
            hwnd,
            null_mut(),
            r.left + (r.right - r.left - width) / 2,
            r.top + (r.bottom - r.top - height) / 2,
            width,
            height,
            SWP_NOZORDER,
        );
    }
    if !hidden {
        show(hwnd);
    }
    let mut msg: MSG = zeroed();
    while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
        if msg.message == WM_KEYDOWN {
            let current = state(hwnd);
            if !current.is_null()
                && is_image_paste_shortcut(
                    msg.hwnd == (*current).edit,
                    msg.wParam as u32,
                    GetKeyState(VK_CONTROL as i32),
                    GetKeyState(VK_MENU as i32),
                    GetKeyState(VK_SHIFT as i32),
                )
                && paste_image(hwnd, true)
            {
                continue;
            }
            if msg.wParam as u32 == VK_ESCAPE as u32 {
                ShowWindow(hwnd, SW_HIDE);
                continue;
            }
            if msg.wParam as u32 == VK_RETURN as u32 && GetKeyState(VK_CONTROL as i32) < 0 {
                save(hwnd);
                continue;
            }
        }
        if IsDialogMessageW(hwnd, &msg) == 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    CloseHandle(mutex);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn paste_shortcut_requires_ctrl_without_shift_or_alt() {
        let down: i16 = -32768;
        let up: i16 = 0;
        let v = VK_V as u32;

        assert!(is_image_paste_shortcut(true, v, down, up, up));
        assert!(!is_image_paste_shortcut(true, v, down, up, down));
        assert!(!is_image_paste_shortcut(true, v, down, down, up));
        assert!(!is_image_paste_shortcut(true, v, up, up, up));
        assert!(!is_image_paste_shortcut(false, v, down, up, up));
        assert!(!is_image_paste_shortcut(true, VK_C as u32, down, up, up));
    }

    #[test]
    fn pending_state_handoff_transfers_once_and_reclaims_on_failure() {
        struct DropCounter(Arc<AtomicUsize>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let s = Box::new(State {
            edit: null_mut(),
            target: PathBuf::from("Todo.md"),
            screenshot: None,
            icon: unsafe { zeroed() },
            taskbar: 0,
            ui: None,
        });
        let mut pending = Some(s);
        let param = (&mut pending as *mut Option<Box<State>>).cast::<()>() as isize;

        let raw = unsafe { take_pending_state(param) };
        assert!(!raw.is_null());
        assert!(pending.is_none());
        assert!(unsafe { take_pending_state(param) }.is_null());
        assert!(unsafe { take_pending_state(0) }.is_null());
        unsafe {
            drop(Box::from_raw(raw));
        }

        // Also verify Option<DropCounter> drops if CreateWindowExW fails before WM_NCCREATE.
        let drops = Arc::new(AtomicUsize::new(0));
        {
            let _unconsumed = Some(Box::new(DropCounter(Arc::clone(&drops))));
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn free_window_state_handles_reentrant_destroy_idempotently() {
        let s = Box::new(State {
            edit: null_mut(),
            target: PathBuf::from("Todo.md"),
            screenshot: None,
            icon: unsafe { zeroed() },
            taskbar: 0,
            ui: None,
        });
        let class = wide("STATIC");
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                wide("").as_ptr(),
                0,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            )
        };
        assert!(!hwnd.is_null());
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(s) as isize);
            assert!(free_window_state(hwnd));
            assert!(state(hwnd).is_null());
            assert!(!free_window_state(hwnd));
            // Calling reentrant helpers after WM_DESTROY zeroed GWLP_USERDATA must not UB/panic.
            quit(hwnd);
            save(hwnd);
            set_screenshot(hwnd, Some(vec![1, 2, 3]));
            DestroyWindow(hwnd);
        }
    }
}
