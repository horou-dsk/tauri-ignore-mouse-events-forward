use std::{
    cell::Cell,
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
};

use crossbeam::channel::Sender;
use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
    UI::{
        Controls::WM_MOUSELEAVE,
        WindowsAndMessaging::{
            CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, WH_MOUSE_LL,
            WM_MOUSEMOVE,
        },
    },
};

static mut MOUSE_HHOOK: Option<HHOOK> = None;

pub static mut MOUSE_MOVE_TX: Option<Sender<Event>> = None;

type EventName = &'static str;

#[derive(Clone)]
pub struct Event {
    #[allow(unused)]
    pub wparam: WPARAM,
    pub lparam: LPARAM,
}

impl Event {
    pub fn new(wparam: WPARAM, lparam: LPARAM) -> Self {
        Self { wparam, lparam }
    }
}

type Handler = Box<dyn Fn(Event) + Send>;

#[derive(Clone)]
pub struct MouseEvent {
    handlers: Arc<Mutex<HashMap<EventName, Handler>>>,
}

impl MouseEvent {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn unlisten<S: AsRef<str>>(&self, event_name: S) {
        self.handlers.lock().unwrap().remove(event_name.as_ref());
    }

    #[allow(unused)]
    pub fn once<F: FnOnce(Event) + Send + 'static>(&self, event_name: EventName, handler: F) {
        let self_ = self.clone();
        let handler = Cell::new(Some(handler));

        self.listen(event_name, move |event| {
            let handler = handler.take().unwrap();
            handler(event);
            self_.unlisten(event_name);
        });
    }

    pub fn listen<F: Fn(Event) + Send + 'static>(&self, event_name: EventName, callback: F) {
        self.handlers
            .lock()
            .unwrap()
            .insert(event_name, Box::new(callback));
    }

    pub fn emit(&self, event_name: &str, event: Event) {
        if let Some(handler) = self.handlers.lock().unwrap().get(event_name) {
            (handler)(event);
        }
    }
}

pub static MOUSE_EVENT: LazyLock<MouseEvent> = LazyLock::new(MouseEvent::new);

pub unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        match wparam.0 as u32 {
            WM_MOUSEMOVE => {
                // MOUSE_EVENT.emit("mousemove", Event::new(wparam, lparam));
                if let Some(tx) = &MOUSE_MOVE_TX {
                    let _ = tx.send(Event::new(wparam, lparam));
                }
            }
            WM_MOUSELEAVE => {
                println!("mouse leave...");
            }
            _ => {}
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
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

pub fn set_mouse_hook() {
    unsafe {
        if MOUSE_HHOOK.is_some() {
            return;
        }
        let hinstance: HINSTANCE = HINSTANCE::default();
        MOUSE_HHOOK = Some(SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), hinstance, 0).unwrap());
    }
}

pub fn unset_mouse_hook() {
    unsafe {
        if let Some(h) = MOUSE_HHOOK.take() {
            match UnhookWindowsHookEx(h) {
                Ok(()) => {
                    println!("UnhookWindowsHookEx success");
                }
                Err(err) => {
                    eprintln!("UnhookWindowsHookEx error: {}", err);
                }
            }
        }
    }
}
