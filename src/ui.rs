use crate::{wide, EDIT, HIDE, SAVE};
use std::{
    mem::{size_of, zeroed},
    ptr::null,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::*, WindowsAndMessaging::*},
};

#[repr(C)]
struct HighContrast {
    size: u32,
    flags: u32,
    scheme: *mut u16,
}
const BG: u32 = 0x00f6f7f5;
const INK: u32 = 0x00293632;
const MUTED: u32 = 0x0053605b;
const TEAL: u32 = 0x00555f15;

pub struct Ui {
    pub edit: HWND,
    font: HFONT,
    heading: HFONT,
    background: HBRUSH,
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
            400,
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
            font,
            heading,
            background,
            high_contrast,
        }
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
                0x00404a10
            } else {
                TEAL
            }
        } else if pressed {
            0x00e3e9e5
        } else {
            0x00ffffff
        };
        let brush = CreateSolidBrush(fill);
        let border = CreateSolidBrush(if primary { fill } else { 0x00c4d0c9 });
        FillRect(item.hDC, &item.rcItem, brush);
        FrameRect(item.hDC, &item.rcItem, border);
        SetBkMode(item.hDC, TRANSPARENT as i32);
        SetTextColor(item.hDC, if primary { 0x00ffffff } else { INK });
        let old = SelectObject(item.hDC, self.font as _);
        let mut rect = item.rcItem;
        let text = wide(if primary { "Save" } else { "Hide" });
        DrawTextW(
            item.hDC,
            text.as_ptr(),
            -1,
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
    }
}
