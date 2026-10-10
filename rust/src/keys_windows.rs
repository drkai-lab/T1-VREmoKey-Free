//! Low-level keyboard hook. `CallNextHookEx` always runs, so keys are not swallowed.

use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK,
    KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WM_KEYUP, WM_QUIT, WM_SYSKEYUP,
};

use crate::focus::{process_matches, Target};
use crate::keys::KeyEvent;
use crate::paths::{log_line, AppPaths};

const LLKHF_UP: u32 = 0x80;

static HOOK: AtomicIsize = AtomicIsize::new(0);
static THREAD_ID: AtomicU32 = AtomicU32::new(0);
static TX: Mutex<Option<Sender<(u16, bool)>>> = Mutex::new(None);

pub struct WinKeyHook {
    rx: Receiver<(u16, bool)>,
    thread: Option<JoinHandle<()>>,
}

impl WinKeyHook {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        *TX.lock().unwrap_or_else(|err| err.into_inner()) = Some(tx);
        let thread = std::thread::Builder::new()
            .name("expr-hook".into())
            .spawn(hook_thread)
            .ok();
        Self { rx, thread }
    }
}

impl crate::keys::KeySource for WinKeyHook {
    fn poll(&mut self, timeout: Duration) -> Vec<KeyEvent> {
        let mut items = Vec::new();
        let first = if timeout.is_zero() {
            self.rx.try_recv().ok()
        } else {
            self.rx.recv_timeout(timeout).ok()
        };
        let Some(first) = first else {
            return items;
        };
        items.push(to_event(first));
        while let Ok(next) = self.rx.try_recv() {
            items.push(to_event(next));
        }
        items
    }

    fn close(&mut self) {
        let _ = TX.lock().map(|mut guard| guard.take());
        for _ in 0..25 {
            let id = THREAD_ID.load(Ordering::SeqCst);
            if id != 0 {
                unsafe {
                    PostThreadMessageW(id, WM_QUIT, 0, 0);
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn to_event((vk, is_up): (u16, bool)) -> KeyEvent {
    KeyEvent::Vk { vk, is_up }
}

fn hook_thread() {
    unsafe {
        THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);
        let module = GetModuleHandleW(std::ptr::null());
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level), module, 0);
        if hook.is_null() {
            log_line(&AppPaths::system(), "SetWindowsHookExW failed");
            return;
        }
        HOOK.store(hook as isize, Ordering::SeqCst);
        let mut msg = std::mem::zeroed();
        loop {
            let result = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if result == 0 || result == -1 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        UnhookWindowsHookEx(hook);
        HOOK.store(0, Ordering::SeqCst);
    }
}

unsafe extern "system" fn low_level(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && lparam != 0 {
        let info = &*(lparam as *const KBDLLHOOKSTRUCT);
        let vk = info.vkCode;
        if is_shift(vk) || is_tracked_fkey(vk) {
            let is_up = info.flags & LLKHF_UP != 0
                || wparam == WM_KEYUP as usize
                || wparam == WM_SYSKEYUP as usize;
            if let Ok(guard) = TX.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send((vk as u16, is_up));
                }
            }
        }
    }
    let hook = HOOK.load(Ordering::SeqCst) as HHOOK;
    CallNextHookEx(hook, code, wparam, lparam)
}

fn is_shift(vk: u32) -> bool {
    matches!(vk, 0x10 | 0xA0 | 0xA1)
}

fn is_tracked_fkey(vk: u32) -> bool {
    (0x70..=0x73).contains(&vk)
}

pub fn foreground_matches(target: &Target) -> Option<bool> {
    let image = query_foreground_image()?;
    Some(process_matches(&image, &target.windows_process))
}

fn query_foreground_image() -> Option<String> {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buffer = [0u16; 32768];
        let mut size = buffer.len() as u32;
        let ok =
            QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut size);
        CloseHandle(handle);
        if ok == 0 || size == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer[..size as usize]))
    }
}
