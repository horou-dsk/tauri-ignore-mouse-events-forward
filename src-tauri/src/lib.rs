#![allow(static_mut_refs)]

use std::{ffi::OsString, os::windows::ffi::OsStringExt};

use mouse_event::{set_mouse_hook, unset_mouse_hook, MOUSE_EVENT, MOUSE_MOVE_TX};
use tauri::Manager;
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{GetLastError, HWND, LPARAM, RECT, WPARAM},
        Graphics::Gdi::{PtInRect, ScreenToClient},
        System::Diagnostics::Debug::{FormatMessageW, FORMAT_MESSAGE_FROM_SYSTEM},
        UI::WindowsAndMessaging::{
            GetClientRect, GetWindow, GetWindowLongW, SendMessageW, SetWindowLongA, GWL_EXSTYLE,
            GW_CHILD, MSLLHOOKSTRUCT, WINDOW_EX_STYLE, WM_MOUSEMOVE, WS_EX_LAYERED,
            WS_EX_TRANSPARENT,
        },
    },
};
mod hook_sub;
mod mouse_event;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

macro_rules! MAKELPARAM {
    ($low:expr, $high:expr) => {
        ((($low & 0xffff) as u32) | (($high & 0xffff) as u32) << 16) as _
    };
}

fn get_last_error_message() -> String {
    unsafe {
        let error_code = GetLastError();
        if error_code.0 == 0 {
            return "No error.".to_string();
        }

        let mut buffer: [u16; 512] = [0; 512];
        let len = FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM,
            None,
            error_code.0,
            0,
            PWSTR(buffer.as_mut_ptr()),
            buffer.len() as u32,
            None,
        );

        OsString::from_wide(&buffer[..len as usize])
            .to_string_lossy()
            .to_string()
    }
}

fn init_mouse_event_channel() {
    std::thread::spawn(|| {
        let (tx, rx) = crossbeam::channel::bounded(8);
        unsafe {
            MOUSE_MOVE_TX = Some(tx);
        }
        while let Ok(event) = rx.recv() {
            MOUSE_EVENT.emit("mousemove", event);
        }
    });
}

fn ignore_cursor_events(hwnd: HWND, ignore: bool) {
    unsafe {
        if ignore {
            let nindex = GWL_EXSTYLE;
            let ex_style = WINDOW_EX_STYLE(GetWindowLongW(hwnd, nindex) as u32);
            let style = ex_style | WS_EX_LAYERED | WS_EX_TRANSPARENT;
            SetWindowLongA(hwnd, nindex, style.0 as i32);
        } else {
            let nindex = GWL_EXSTYLE;
            let ex_style = WINDOW_EX_STYLE(GetWindowLongW(hwnd, nindex) as u32);
            let style = ex_style & !(WS_EX_LAYERED | WS_EX_TRANSPARENT);
            SetWindowLongA(hwnd, nindex, style.0 as i32);
        }
    }
}

#[tauri::command]
fn ignore_mouse_events(window: tauri::Window, ignore: bool, forward: Option<bool>) {
    let hwnd = window.hwnd().unwrap();
    let forward = forward.unwrap_or(false);
    unsafe {
        ignore_cursor_events(hwnd, ignore);

        if forward {
            hook_sub::SUB_CLASS_HWND.reject_dll(hwnd).unwrap();
            // FIXME: 因为这个窗口是在不同的进程中创建的，所以设置子类化会失败
            {
                // let set_window_sub_class = SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0);
                // if !set_window_sub_class.as_bool() {
                //     println!("error: {:?}", get_last_error_message());
                // }
                // println!("set_window_sub_class {:?}", set_window_sub_class.as_bool());
            }

            // 获取到需要的子窗口
            let hwnd = GetWindow(hwnd, GW_CHILD).unwrap();
            let hwnd = GetWindow(hwnd, GW_CHILD).unwrap();
            let hwnd = GetWindow(hwnd, GW_CHILD).unwrap();
            let hwnd = GetWindow(hwnd, GW_CHILD).unwrap();
            let hwnd = hwnd.0 as usize;

            // 16进制显示
            println!("hwnd {:02X}", hwnd);

            MOUSE_EVENT.unlisten("mousemove");
            MOUSE_EVENT.listen("mousemove", move |event| {
                let hwnd = HWND(hwnd as *mut std::ffi::c_void);
                let l_param = event.lparam;
                let p = l_param.0 as *const MSLLHOOKSTRUCT;
                let p = (*p).pt;
                let mut client_rect = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                GetClientRect(hwnd, &mut client_rect).unwrap();
                let mut p = p;
                ScreenToClient(hwnd, &mut p).unwrap();

                if PtInRect(&client_rect, p).as_bool() {
                    let l = LPARAM(MAKELPARAM!(p.x, p.y));
                    let w = WPARAM(0);
                    // println!("post mouse move x: {}, y: {}", p.x, p.y);
                    SendMessageW(hwnd, WM_MOUSEMOVE, w, l);
                }
            });
        } else {
            hook_sub::SUB_CLASS_HWND.unhook_sub(hwnd).unwrap();
            MOUSE_EVENT.unlisten("mousemove");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_mouse_event_channel();
    set_mouse_hook();
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet, ignore_mouse_events,])
        .setup(|app| {
            let main_window = app.get_webview_window("main").unwrap();
            main_window.open_devtools();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    println!("tauri application exit");
    unset_mouse_hook();
}
