use std::{
    collections::HashMap,
    ffi::CString,
    sync::{LazyLock, Mutex},
};

use windows::{
    core::{s, Error as WindowsError, PCSTR},
    Win32::{
        Foundation::{CloseHandle, FreeLibrary, BOOL, ERROR_NOT_FOUND, HANDLE, HMODULE, HWND},
        System::{
            Diagnostics::Debug::WriteProcessMemory,
            LibraryLoader::{
                GetModuleHandleA, GetProcAddress, LoadLibraryA, LoadLibraryExA,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            },
            Memory::{
                VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, PAGE_EXECUTE_READWRITE,
                PAGE_READWRITE,
            },
            ProcessStatus::{K32EnumProcessModules, K32GetModuleBaseNameA},
            Threading::{
                CreateRemoteThread, GetExitCodeThread, OpenProcess, WaitForSingleObject,
                LPTHREAD_START_ROUTINE, PROCESS_ALL_ACCESS,
            },
        },
        UI::WindowsAndMessaging::{GetWindow, GetWindowThreadProcessId, GW_CHILD},
    },
};

use crate::get_last_error_message;

pub struct SubClassHwnd {
    inners: Mutex<HashMap<usize, HANDLE>>,
    dll_path: CString,
}

unsafe impl Send for SubClassHwnd {}
unsafe impl Sync for SubClassHwnd {}

pub static SUB_CLASS_HWND: LazyLock<SubClassHwnd> = LazyLock::new(|| {
    // debug: current_path = src-tauri
    let current_path = std::env::current_dir().unwrap();
    let dll_path = current_path.join("sub_dll/target/release/sub_dll.dll");
    SubClassHwnd::new(CString::new(dll_path.to_str().unwrap()).unwrap())
});

impl SubClassHwnd {
    pub fn new(dll_path: CString) -> Self {
        Self {
            inners: Mutex::new(HashMap::new()),
            dll_path,
        }
    }

    pub unsafe fn reject_dll(&self, hwnd: HWND) -> Result<(), WindowsError> {
        let hwnd_value = hwnd.0 as usize;
        if self.inners.lock().unwrap().contains_key(&hwnd_value) {
            return Ok(());
        }
        let hwnd = GetWindow(hwnd, GW_CHILD)?;
        let hwnd = GetWindow(hwnd, GW_CHILD)?;
        let hwnd = GetWindow(hwnd, GW_CHILD)?;
        let hwnd = GetWindow(hwnd, GW_CHILD)?;

        let mut pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let h_process = OpenProcess(PROCESS_ALL_ACCESS, BOOL(0), pid).unwrap();
        if get_module_from_process(h_process, "sub_dll.dll").is_err() {
            hook_sub(h_process, &self.dll_path)?;
        }
        call_remote_function::<HWND, u32>(
            h_process,
            PCSTR::from_raw(self.dll_path.as_ptr() as *const u8),
            s!("set_subclass"),
            Some(hwnd),
        )?;
        self.inners.lock().unwrap().insert(hwnd_value, h_process);
        Ok(())
    }

    pub unsafe fn unhook_sub(&self, hwnd: HWND) -> Result<(), WindowsError> {
        let hwnd_value = hwnd.0 as usize;
        if !self.inners.lock().unwrap().contains_key(&hwnd_value) {
            return Ok(());
        }
        let hwnd = GetWindow(hwnd, GW_CHILD)?;
        let hwnd = GetWindow(hwnd, GW_CHILD)?;
        let hwnd = GetWindow(hwnd, GW_CHILD)?;
        let hwnd = GetWindow(hwnd, GW_CHILD)?;
        let h_process = self.inners.lock().unwrap().remove(&hwnd_value).unwrap();
        call_remote_function::<HWND, u32>(
            h_process,
            PCSTR::from_raw(self.dll_path.as_ptr() as *const u8),
            s!("remove_subclass"),
            Some(hwnd),
        )?;
        CloseHandle(h_process)?;
        Ok(())
    }
}

unsafe fn hook_sub(h_process: HANDLE, dll_path: &CString) -> Result<(), WindowsError> {
    let alloc_size = dll_path.as_bytes_with_nul().len();
    let remote_buffer = VirtualAllocEx(
        h_process,
        None,
        alloc_size,
        MEM_COMMIT,
        PAGE_EXECUTE_READWRITE,
    );

    println!("remote_buffer: {:?}", remote_buffer.is_null());

    WriteProcessMemory(
        h_process,
        remote_buffer,
        dll_path.as_ptr() as _,
        alloc_size,
        None,
    )
    .unwrap();

    // 获取 LoadLibraryA 地址
    let kernel32 = GetModuleHandleA(s!("kernel32.dll")).unwrap();
    let load_library: LPTHREAD_START_ROUTINE =
        std::mem::transmute(GetProcAddress(kernel32, s!("LoadLibraryA")));

    if load_library.is_none() {
        println!("load_library is none");
        VirtualFreeEx(h_process, remote_buffer, 0, MEM_RELEASE).unwrap();
        return Err(WindowsError::new(
            ERROR_NOT_FOUND.into(),
            get_last_error_message(),
        ));
    }

    let thread_handle = CreateRemoteThread(
        h_process,
        None,
        0,
        load_library,
        Some(remote_buffer),
        0,
        None,
    )
    .expect("CreateRemoteThread failed");

    WaitForSingleObject(thread_handle, u32::MAX);

    let mut module_handle: HMODULE = HMODULE::default();

    GetExitCodeThread(thread_handle, &mut module_handle as *mut _ as *mut u32).unwrap();
    println!("module_handle: {:?}", module_handle);
    VirtualFreeEx(h_process, remote_buffer, 0, MEM_RELEASE).unwrap();
    CloseHandle(thread_handle).unwrap();
    // println!(
    //     "verify_dll_dependencies: {:?}",
    //     verify_dll_dependencies(PCSTR::from_raw(dll_path.as_ptr() as *const u8))
    // );

    Ok(())
}

unsafe fn call_remote_function<T: Default, R: Default + std::fmt::Display>(
    h_process: HANDLE,
    module_path: PCSTR,
    function_name: PCSTR,
    arg: Option<T>,
) -> Result<R, WindowsError> {
    // let module_handle = GetModuleHandleA(module_name)?;
    // let module_handle = get_module_from_process(h_process, module_name)?;
    // let h_process = GetCurrentProcess();
    let module_handle = LoadLibraryExA(module_path, None, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR)?;
    // println!("module_handle: {:?}", module_handle);
    // let func_addr = GetProcAddress(module_handle, function_name);
    let func_addr: LPTHREAD_START_ROUTINE =
        std::mem::transmute(GetProcAddress(module_handle, function_name));

    if func_addr.is_none() {
        println!("func_addr is none, error: {}", get_last_error_message());
        return Ok(R::default());
    }

    // println!("hwnd size: {} {}", std::mem::size_of::<HWND>(), hwnd_value);
    let mut param_buffer: Option<*const std::ffi::c_void> = None;
    if let Some(arg) = arg {
        let buffer = VirtualAllocEx(
            h_process,
            None,
            std::mem::size_of::<T>(),
            MEM_COMMIT,
            PAGE_READWRITE,
        );
        WriteProcessMemory(
            h_process,
            buffer,
            &arg as *const T as _,
            std::mem::size_of::<T>(),
            None,
        )
        .unwrap();
        param_buffer = Some(buffer);
    }

    // let mut param_buffers = Vec::new();
    // // 在目标进程分配内存并写入参数
    // for arg in args {
    //     let param_buffer = VirtualAllocEx(
    //         h_process,
    //         None,
    //         std::mem::size_of::<u64>(),
    //         MEM_COMMIT,
    //         PAGE_READWRITE,
    //     );
    //     param_buffers.push(param_buffer);
    //     WriteProcessMemory(
    //         h_process,
    //         param_buffer,
    //         arg as *const u64 as _,
    //         std::mem::size_of::<u64>(),
    //         None,
    //     )
    //     .unwrap();
    // }

    // let return_buffer = VirtualAllocEx(
    //     h_process,
    //     None,
    //     std::mem::size_of::<R>(),
    //     MEM_COMMIT,
    //     PAGE_READWRITE,
    // );

    // println!("return_buffer: {:?}", return_buffer.is_null());

    // WriteProcessMemory(
    //     h_process,
    //     return_buffer,
    //     &R::default() as *const R as *const _,
    //     std::mem::size_of::<R>(),
    //     None,
    // )
    // .unwrap();

    let thread_handle =
        CreateRemoteThread(h_process, None, 0, func_addr, param_buffer, 0, None).unwrap();

    WaitForSingleObject(thread_handle, u32::MAX);

    let mut exit_code: R = R::default();
    GetExitCodeThread(thread_handle, &mut exit_code as *mut _ as *mut u32).unwrap();
    println!("线程退出码: {}", exit_code);

    // let mut result: R = std::mem::zeroed();

    // ReadProcessMemory(
    //     h_process,
    //     return_buffer,
    //     &mut result as *mut _ as *mut _,
    //     std::mem::size_of::<R>(),
    //     None,
    // )
    // .unwrap();

    // 释放分配的内存
    if let Some(buffer) = param_buffer {
        VirtualFreeEx(h_process, buffer as *mut _, 0, MEM_RELEASE).unwrap();
    }

    CloseHandle(thread_handle).unwrap();
    FreeLibrary(module_handle).unwrap();

    Ok(exit_code)
}

unsafe fn get_module_from_process(
    h_process: HANDLE,
    module_name: &str,
) -> Result<HMODULE, WindowsError> {
    let mut modules: [HMODULE; 1024] = std::mem::zeroed();
    let mut needed_bytes = 0;
    K32EnumProcessModules(
        h_process,
        modules.as_mut_ptr(),
        modules.len() as u32,
        &mut needed_bytes,
    )
    .unwrap();

    // 计算模块数量
    let module_count = needed_bytes as usize / std::mem::size_of::<HMODULE>();

    println!("module_count: {}", module_count);

    // 收集模块名称
    for module in &modules[..module_count] {
        let mut filename = [0u8; 256];
        let len = K32GetModuleBaseNameA(h_process, *module, &mut filename);
        if len > 0 {
            let filename = CString::from_vec_unchecked(filename[..len as usize].to_vec());
            let filename = filename.to_str().unwrap();
            if filename == module_name {
                return Ok(*module);
            }
        }
    }

    Err(WindowsError::new(
        ERROR_NOT_FOUND.into(),
        "module not found",
    ))
}

#[allow(unused)]
unsafe fn verify_dll_dependencies(dll_path: PCSTR) -> Result<(), WindowsError> {
    // 获取 DLL 所在目录
    // let dll_dir = Path::new(dll_path).parent().ok_or("Invalid DLL path")?;

    // // 将 DLL 目录添加到搜索路径
    // let old_path = std::env::var("PATH").unwrap_or_default();
    // let new_path = format!("{};{}", dll_dir.display(), old_path);
    // std::env::set_var("PATH", &new_path);

    // 现在尝试加载 DLL
    let handle = LoadLibraryA(dll_path)?;

    // 成功加载后再尝试获取函数
    let proc_addr = GetProcAddress(handle, s!("dll_main"));
    if proc_addr.is_none() {
        FreeLibrary(handle)?;
        return Err(WindowsError::new(
            ERROR_NOT_FOUND.into(),
            get_last_error_message(),
        ));
    }

    FreeLibrary(handle)?;
    Ok(())
}
