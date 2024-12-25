#![allow(static_mut_refs)]
use std::ffi::c_void;

use windows::Win32::{
    Foundation::{BOOL, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Controls::WM_MOUSELEAVE,
        WindowsAndMessaging::{
            CallWindowProcW, DefWindowProcW, GetWindowLongPtrW, SetWindowLongPtrW, GWLP_WNDPROC,
            GWL_WNDPROC, WNDPROC,
        },
    },
};

#[no_mangle]
pub extern "system" fn dll_add(left: *const u32) -> u32 {
    unsafe {
        let right = 6;
        println!("dll_add called... left: {}, right: {}", *left, right);
        *left + right
    }
}

#[no_mangle]
pub unsafe extern "system" fn remove_subclass(hwnd_value: *const HWND) -> u32 {
    let hwnd = *hwnd_value;
    if let Some(original_proc) = ORIGINAL_WND_PROC.take() {
        let result = SetWindowLongPtrW(hwnd, GWL_WNDPROC, original_proc.ptr);
        result as u32
    } else {
        0
    }
}

#[derive(Clone, Copy)]
struct OriginalWndProc {
    proc: WNDPROC,
    ptr: isize,
}

// 存储原始窗口过程的静态变量
static mut ORIGINAL_WND_PROC: Option<OriginalWndProc> = None;

#[no_mangle]
pub unsafe extern "system" fn set_subclass(hwnd_value: *const HWND) -> u32 {
    let hwnd = *hwnd_value;
    // FIXME: 不知道为啥在同一个进程里面用这种方式设置窗口的子类化还是失败，还需要在同一个线程？
    // let set_window_sub_class = SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0);

    // 只能使用这中原始的方式
    // 获取窗口的原始过程
    let original_proc = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
    if original_proc == 0 {
        return 102;
    }
    // 保存原始窗口过程
    ORIGINAL_WND_PROC = Some(OriginalWndProc {
        proc: std::mem::transmute(original_proc),
        ptr: original_proc,
    });
    let result = SetWindowLongPtrW(hwnd, GWL_WNDPROC, subclass_proc as isize);
    result as u32
}

pub unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_MOUSELEAVE => LRESULT(0),
        _ => {
            if let Some(original_proc) = ORIGINAL_WND_PROC {
                CallWindowProcW(original_proc.proc, hwnd, msg, wparam, lparam)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
    }
}

// pub unsafe extern "system" fn subclass_proc(
//     hwnd: HWND,
//     msg: u32,
//     wparam: WPARAM,
//     lparam: LPARAM,
//     _subclass_id: usize,
//     _ref_data: usize,
// ) -> LRESULT {
//     println!("subclass proc...");
//     match msg {
//         WM_MOUSELEAVE => {
//             println!("mouse leave...");
//             LRESULT(0)
//         }
//         _ => DefSubclassProc(hwnd, msg, wparam, lparam),
//     }
// }

#[no_mangle]
pub extern "system" fn DllMain(
    _hinst_dll: HINSTANCE,
    _fdw_reason: u32,
    _lp_reserved: *mut c_void,
) -> BOOL {
    println!("DllMain called...");
    BOOL::from(true)
}
