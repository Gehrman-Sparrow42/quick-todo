use crate::{wide, ATTACH, EDIT, HIDE, SAVE};
use std::{
    mem::{size_of, zeroed},
    ptr::null,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{
        Dwm::{
            DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
        },
        Gdi::*,
    },
    System::{LibraryLoader::GetModuleHandleW, SystemServices::SS_RIGHT},
    UI::{Controls::*, WindowsAndMessaging::*},
};

#[repr(C)]
struct HighContrast {
    size: u32,
    flags: u32,
    scheme: *mut u16,
}
const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | (green as u32) << 8 | (blue as u32) << 16
}

const BG: u32 = rgb(24, 28, 32);
const EDITOR_BG: u32 = rgb(32, 38, 43);
const INK: u32 = rgb(231, 237, 240);
const MUTED: u32 = rgb(171, 183, 190);
const TEAL: u32 = rgb(23, 105, 92);
const TEAL_PRESSED: u32 = rgb(17, 79, 69);
const BUTTON_BG: u32 = rgb(38, 46, 52);
const BUTTON_PRESSED: u32 = rgb(49, 59, 66);
const BUTTON_BORDER: u32 = rgb(83, 97, 106);

pub struct Ui {
    pub edit: HWND,
    attachment_status: HWND,
    attachment_button: HWND,
    font: HFONT,
    heading: HFONT,
    background: HBRUSH,
    editor_background: HBRUSH,
    high_contrast: bool,
}
unsafe fn make_font(height: i32, weight: i32) -> HFONT {
    CreateFontW(
        height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        DEFAULT_PITCH as u32,
        wide("Segoe UI").as_ptr(),
    )
}

unsafe fn dark_caption(hwnd: HWND) {
    // Keep the system frame on Windows versions that do not support these attributes.
    for (attribute, value) in [
        (DWMWA_USE_IMMERSIVE_DARK_MODE, 1),
        (DWMWA_CAPTION_COLOR, BG),
        (DWMWA_TEXT_COLOR, INK),
    ] {
        DwmSetWindowAttribute(
            hwnd,
            attribute as u32,
            (&value as *const u32).cast(),
            size_of::<u32>() as u32,
        );
    }
}
#[allow(clippy::too_many_arguments)]
unsafe fn child(
    parent: HWND,
    class: &str,
    title: &str,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: i32,
    font: HFONT,
) -> HWND {
    let hwnd = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(title).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        w,
        h,
        parent,
        id as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    SendMessageW(hwnd, WM_SETFONT, font as usize, 1);
    hwnd
}
impl Ui {
    pub unsafe fn create(hwnd: HWND) -> Self {
        let mut hc: HighContrast = zeroed();
        hc.size = size_of::<HighContrast>() as u32;
        let high_contrast =
            SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.size, &mut hc as *mut _ as _, 0) != 0
                && hc.flags & 1 != 0;
        let background = CreateSolidBrush(if high_contrast {
            GetSysColor(COLOR_WINDOW)
        } else {
            BG
        });
        let editor_background = CreateSolidBrush(if high_contrast {
            GetSysColor(COLOR_WINDOW)
        } else {
            EDITOR_BG
        });
        if !high_contrast {
            dark_caption(hwnd);
        }
        let font = make_font(-16, FW_NORMAL as i32);
        let heading = make_font(-24, FW_SEMIBOLD as i32);
        child(
            hwnd,
            "STATIC",
            "Quick Todo",
            0,
            28,
            22,
            440,
            34,
            200,
            heading,
        );
        child(
            hwnd,
            "STATIC",
            "One idea per line",
            0,
            28,
            62,
            180,
            24,
            0,
            font,
        );
        let edit = child(
            hwnd,
            "EDIT",
            "",
            WS_TABSTOP
                | WS_BORDER
                | WS_VSCROLL
                | ES_MULTILINE as u32
                | ES_AUTOVSCROLL as u32
                | ES_WANTRETURN as u32,
            28,
            92,
            548,
            152,
            EDIT,
            font,
        );
        SendMessageW(
            edit,
            EM_SETMARGINS,
            (EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize,
            (10 | (10 << 16)) as isize,
        );
        if !high_contrast {
            // Use Windows' dark scrollbar theme when the installed theme provides it.
            SetWindowTheme(edit, wide("DarkMode_Explorer").as_ptr(), null());
        }
        child(
            hwnd,
            "STATIC",
            "Ctrl+Enter to save  ·  Esc to hide",
            0,
            28,
            269,
            270,
            23,
            0,
            font,
        );
        let button_style = if high_contrast {
            BS_PUSHBUTTON as u32
        } else {
            BS_OWNERDRAW as u32
        };
        let attachment_status = child(
            hwnd, "STATIC", "Ctrl+V", SS_RIGHT, 218, 62, 196, 24, 0, font,
        );
        let attachment_button = child(
            hwnd,
            "BUTTON",
            "Paste image",
            WS_TABSTOP | button_style,
            426,
            55,
            150,
            30,
            ATTACH,
            font,
        );
        child(
            hwnd,
            "BUTTON",
            "Hide",
            WS_TABSTOP | button_style,
            378,
            261,
            78,
            38,
            HIDE,
            font,
        );
        child(
            hwnd,
            "BUTTON",
            "Save",
            WS_TABSTOP | button_style,
            468,
            261,
            108,
            38,
            SAVE,
            font,
        );
        Self {
            edit,
            attachment_status,
            attachment_button,
            font,
            heading,
            background,
            editor_background,
            high_contrast,
        }
    }
    pub unsafe fn show_attachment(&self, attached: bool) {
        let (status, action) = if attached {
            ("Screenshot attached", "Remove image")
        } else {
            ("Ctrl+V", "Paste image")
        };
        SetWindowTextW(self.attachment_status, wide(status).as_ptr());
        SetWindowTextW(self.attachment_button, wide(action).as_ptr());
        InvalidateRect(self.attachment_button, null(), 0);
    }
    pub unsafe fn background(&self, dc: HDC, label: HWND) -> LRESULT {
        if self.high_contrast {
            SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
            SetBkColor(dc, GetSysColor(COLOR_WINDOW));
        } else {
            SetTextColor(dc, if GetDlgCtrlID(label) == 0 { MUTED } else { INK });
            SetBkColor(dc, BG);
        }
        self.background as LRESULT
    }
    pub unsafe fn editor_background(&self, dc: HDC) -> LRESULT {
        let (ink, background) = if self.high_contrast {
            (GetSysColor(COLOR_WINDOWTEXT), GetSysColor(COLOR_WINDOW))
        } else {
            (INK, EDITOR_BG)
        };
        SetTextColor(dc, ink);
        SetBkColor(dc, background);
        self.editor_background as LRESULT
    }
    pub unsafe fn erase(&self, dc: HDC, hwnd: HWND) -> LRESULT {
        let mut r: RECT = zeroed();
        GetClientRect(hwnd, &mut r);
        FillRect(dc, &r, self.background);
        1
    }
    pub unsafe fn draw_button(&self, item: &DRAWITEMSTRUCT) -> LRESULT {
        let primary = item.CtlID == SAVE as u32;
        let pressed = item.itemState & ODS_SELECTED != 0;
        let fill = if primary {
            if pressed {
                TEAL_PRESSED
            } else {
                TEAL
            }
        } else if pressed {
            BUTTON_PRESSED
        } else {
            BUTTON_BG
        };
        let brush = CreateSolidBrush(fill);
        let border = CreateSolidBrush(if primary { fill } else { BUTTON_BORDER });
        FillRect(item.hDC, &item.rcItem, brush);
        FrameRect(item.hDC, &item.rcItem, border);
        SetBkMode(item.hDC, TRANSPARENT as i32);
        SetTextColor(item.hDC, INK);
        let old = SelectObject(item.hDC, self.font as _);
        let mut rect = item.rcItem;
        let mut text = [0; 32];
        let length = GetWindowTextW(item.hwndItem, text.as_mut_ptr(), text.len() as i32);
        DrawTextW(
            item.hDC,
            text.as_ptr(),
            length,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        if item.itemState & ODS_FOCUS != 0 {
            InflateRect(&mut rect, -4, -4);
            DrawFocusRect(item.hDC, &rect);
        }
        SelectObject(item.hDC, old);
        DeleteObject(brush as _);
        DeleteObject(border as _);
        1
    }
    pub unsafe fn destroy(&self) {
        DeleteObject(self.font as _);
        DeleteObject(self.heading as _);
        DeleteObject(self.background as _);
        DeleteObject(self.editor_background as _);
    }
}
