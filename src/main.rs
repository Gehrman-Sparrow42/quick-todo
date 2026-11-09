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
    SetFocus((*state(hwnd)).edit);
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
    let edit = (*state(hwnd)).edit;
    let target = (*state(hwnd)).target.clone();
    let text = draft(edit);
    match append::append_tasks(&target, &text, (*state(hwnd)).screenshot.as_deref()) {
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
    if ((*state(hwnd)).screenshot.is_some() || !draft((*state(hwnd)).edit).trim().is_empty())
        && message(
            hwnd,
            "Unsaved draft",
            "Discard your unsaved draft and quit?",
            MB_YESNO | MB_ICONWARNING,
        ) != IDYES
    {
        return;
    }
    DestroyWindow(hwnd);
}
unsafe fn add_tray(s: &mut State) {
    Shell_NotifyIconW(NIM_ADD, &s.icon);
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = &*(lp as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
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
                    match cmd as usize {
                        x if x == ACTIVATE as usize => show(hwnd),
                        OPEN => {
                            let path = wide((*state(hwnd)).target.as_os_str());
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
        WM_DESTROY => {
            UnregisterHotKey(hwnd, 1);
            Shell_NotifyIconW(NIM_DELETE, &(*ptr).icon);
            if let Some(ui) = &(*ptr).ui {
                ui.destroy();
            }
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr));
            PostQuitMessage(0);
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
    let raw = Box::into_raw(s);
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
        raw as _,
    );
    if hwnd.is_null() {
        return Err("Could not create the window.".into());
    }
    if RegisterHotKey(hwnd, 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'T' as u32) == 0 {
        DestroyWindow(hwnd);
        return Err("Ctrl+Alt+T is unavailable. Another application may already use it.".into());
    }
    // Center on the work area of the monitor containing the cursor.
    let mut pt: POINT = zeroed();
    GetCursorPos(&mut pt);
    let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) != 0 {
        let r = info.rcWork;
        SetWindowPos(
            hwnd,
            null_mut(),
            r.left + (r.right - r.left - WINDOW_WIDTH) / 2,
            r.top + (r.bottom - r.top - WINDOW_HEIGHT) / 2,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER,
        );
    }
    if !hidden {
        show(hwnd);
    }
    let mut msg: MSG = zeroed();
    while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
        if msg.message == WM_KEYDOWN {
            if msg.hwnd == (*state(hwnd)).edit
                && msg.wParam as u32 == VK_V as u32
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_MENU as i32) >= 0
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
