//! BreezeMusic For Windows — 原生 Win32 单实例互斥与跨进程零延迟唤醒
//!
//! 设计目标：
//! 1. 在 Tauri / WebView2 / 系统托盘初始化之前完成进程级单实例互斥判定，
//!    彻底杜绝小窗模式、托盘常驻或快速双击桌面快捷方式 / 任务栏固定图标时无上限拉起多进程与重复托盘图标。
//! 2. 主实例在获取命名 Mutex 后立即启动 0% CPU 空闲休眠的 `HWND_MESSAGE` 隐藏消息宿主窗口，
//!    消除冷启动瞬间的并发竞态空档。
//! 3. 次实例命中 `ERROR_ALREADY_EXISTS` / `ERROR_ACCESS_DENIED` 后，通过 `AllowSetForegroundWindow`
//!    将本次用户点击获得的前台焦点令牌移交给主实例，并通过 `SendMessageTimeoutW` 通知主实例平滑退出小窗
//!    或从托盘/最小化状态恢复并置前聚焦，随后次实例在数毫秒内静默退出。

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub const DEFAULT_MUTEX_NAME: &str =
        "Local\\BreezeMusicDesktopSingleInstanceMutex_com.concept.breezemusic.desktop_v1";
    pub const DEFAULT_IPC_CLASS: &str =
        "BreezeMusicDesktopSingleInstanceMsgClass_com.concept.breezemusic.desktop_v1";
    pub const DEFAULT_IPC_TITLE: &str =
        "BreezeMusicDesktopSingleInstanceMsgHost_com.concept.breezemusic.desktop_v1";

    const ERROR_ACCESS_DENIED: u32 = 5;
    const ERROR_ALREADY_EXISTS: u32 = 183;
    const HWND_MESSAGE: isize = -3;
    const ASFW_ANY: u32 = 0xFFFF_FFFF;
    const WM_NCDESTROY: u32 = 0x0082;
    const WM_APP: u32 = 0x8000;
    const WM_BM_SINGLE_INSTANCE_WAKE: u32 = WM_APP + 0x57;
    const WM_BM_SINGLE_INSTANCE_STOP: u32 = WM_APP + 0x58;
    const WAKE_MAGIC_WPARAM: usize = 0x424D_574B;
    const WAKE_ACK_RESULT: isize = 0x424D_4F4B;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    const GWLP_USERDATA: i32 = -21;

    #[repr(C)]
    struct WinClassExW {
        cb_size: u32,
        style: u32,
        lpfn_wnd_proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
        cb_cls_extra: i32,
        cb_wnd_extra: i32,
        h_instance: isize,
        h_icon: isize,
        h_cursor: isize,
        hbr_background: isize,
        lpsz_menu_name: *const u16,
        lpsz_class_name: *const u16,
        h_icon_sm: isize,
    }

    #[repr(C)]
    #[derive(Default)]
    struct WinPoint {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct WinMsg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: WinPoint,
        l_private: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(
            lp_mutex_attributes: *const c_void,
            b_initial_owner: i32,
            lp_name: *const u16,
        ) -> isize;
        fn CloseHandle(h_object: isize) -> i32;
        fn GetLastError() -> u32;
        fn GetModuleHandleW(lp_module_name: *const u16) -> isize;
    }

    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassExW(lpwcx: *const WinClassExW) -> u16;
        fn UnregisterClassW(lp_class_name: *const u16, h_instance: isize) -> i32;
        fn CreateWindowExW(
            dw_ex_style: u32,
            lp_class_name: *const u16,
            lp_window_name: *const u16,
            dw_style: u32,
            x: i32,
            y: i32,
            n_width: i32,
            n_height: i32,
            h_wnd_parent: isize,
            h_menu: isize,
            h_instance: isize,
            lp_param: isize,
        ) -> isize;
        fn DestroyWindow(hwnd: isize) -> i32;
        fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn FindWindowExW(
            hwnd_parent: isize,
            hwnd_child_after: isize,
            lpsz_class: *const u16,
            lpsz_window: *const u16,
        ) -> isize;
        fn GetWindowThreadProcessId(hwnd: isize, lpdw_process_id: *mut u32) -> u32;
        fn AllowSetForegroundWindow(dw_process_id: u32) -> i32;
        fn SendMessageTimeoutW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: isize,
            fu_flags: u32,
            u_timeout: u32,
            lpdw_result: *mut usize,
        ) -> isize;
        fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn PostQuitMessage(n_exit_code: i32);
        fn GetMessageW(
            lp_msg: *mut WinMsg,
            hwnd: isize,
            w_msg_filter_min: u32,
            w_msg_filter_max: u32,
        ) -> i32;
        fn TranslateMessage(lp_msg: *const WinMsg) -> i32;
        fn DispatchMessageW(lp_msg: *const WinMsg) -> isize;
        fn SetWindowLongPtrW(hwnd: isize, n_index: i32, dw_new_long: isize) -> isize;
        fn GetWindowLongPtrW(hwnd: isize, n_index: i32) -> isize;
    }

    type WakeCallback = Arc<dyn Fn() + Send + Sync + 'static>;

    struct WakeHub {
        callback: Mutex<Option<WakeCallback>>,
        pending_wake: AtomicBool,
        last_wake_ms: AtomicU64,
        debounce_ms: u64,
    }

    impl WakeHub {
        fn new(debounce_ms: u64) -> Self {
            Self {
                callback: Mutex::new(None),
                pending_wake: AtomicBool::new(false),
                last_wake_ms: AtomicU64::new(0),
                debounce_ms,
            }
        }

        fn should_dispatch_now(&self) -> bool {
            if self.debounce_ms == 0 {
                return true;
            }
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(1)
                .max(1);
            let prev = self.last_wake_ms.load(Ordering::Relaxed);
            if prev != 0 && now_ms.saturating_sub(prev) < self.debounce_ms {
                return false;
            }
            self.last_wake_ms
                .compare_exchange(prev, now_ms, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
        }

        fn trigger_wake(&self) {
            if !self.should_dispatch_now() {
                return;
            }
            let cb_opt = self.callback.lock().ok().and_then(|g| g.clone());
            if let Some(cb) = cb_opt {
                cb();
            } else {
                self.pending_wake.store(true, Ordering::Release);
            }
        }
    }

    fn encode_wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe extern "system" fn ipc_host_wnd_proc(
        hwnd: isize,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        match msg {
            WM_BM_SINGLE_INSTANCE_WAKE if wparam == WAKE_MAGIC_WPARAM => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WakeHub;
                if !ptr.is_null() {
                    let hub = &*ptr;
                    hub.trigger_wake();
                }
                WAKE_ACK_RESULT
            }
            WM_BM_SINGLE_INSTANCE_STOP => {
                let _ = DestroyWindow(hwnd);
                PostQuitMessage(0);
                0
            }
            WM_NCDESTROY => {
                let prev = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *const WakeHub;
                if !prev.is_null() {
                    drop(Arc::from_raw(prev));
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    fn notify_existing_instance(
        ipc_class: &str,
        ipc_title: &str,
        max_attempts: usize,
        retry_delay_ms: u64,
    ) -> bool {
        let wide_cls = encode_wide_null(ipc_class);
        let wide_title = encode_wide_null(ipc_title);

        for attempt in 0..max_attempts.max(1) {
            let target_hwnd = unsafe {
                FindWindowExW(HWND_MESSAGE, 0, wide_cls.as_ptr(), wide_title.as_ptr())
            };
            if target_hwnd != 0 {
                unsafe {
                    let mut target_pid: u32 = 0;
                    let _ = GetWindowThreadProcessId(target_hwnd, &mut target_pid);
                    let granted = if target_pid != 0 {
                        AllowSetForegroundWindow(target_pid) != 0
                    } else {
                        false
                    };
                    if !granted {
                        let _ = AllowSetForegroundWindow(ASFW_ANY);
                    }

                    let mut result: usize = 0;
                    let sent = SendMessageTimeoutW(
                        target_hwnd,
                        WM_BM_SINGLE_INSTANCE_WAKE,
                        WAKE_MAGIC_WPARAM,
                        0,
                        SMTO_ABORTIFHUNG,
                        1500,
                        &mut result,
                    );
                    if sent != 0 && result as isize == WAKE_ACK_RESULT {
                        return true;
                    }
                    // 若主实例消息队列瞬时繁忙，降级异步 PostMessageW 确保唤醒信号不丢失
                    if PostMessageW(
                        target_hwnd,
                        WM_BM_SINGLE_INSTANCE_WAKE,
                        WAKE_MAGIC_WPARAM,
                        0,
                    ) != 0
                    {
                        return true;
                    }
                }
            }
            if attempt + 1 < max_attempts {
                std::thread::sleep(Duration::from_millis(retry_delay_ms));
            }
        }
        false
    }

    pub struct SingleInstanceGuard {
        mutex_handle: isize,
        ipc_hwnd: Arc<AtomicIsize>,
        hub: Arc<WakeHub>,
        listener_thread: Option<JoinHandle<()>>,
    }

    pub enum AcquireOutcome {
        Primary(SingleInstanceGuard),
        SecondaryWokeExisting { notified: bool },
    }

    impl SingleInstanceGuard {
        /// 进程启动最早期调用：原子抢占单实例互斥锁；若已有实例运行，则移交前台焦点权限、唤醒主实例并指示次实例退出。
        pub fn acquire() -> AcquireOutcome {
            Self::acquire_named(
                DEFAULT_MUTEX_NAME,
                DEFAULT_IPC_CLASS,
                DEFAULT_IPC_TITLE,
                260,
                40,
                50,
            )
        }

        pub fn acquire_named(
            mutex_name: &str,
            ipc_class: &str,
            ipc_title: &str,
            debounce_ms: u64,
            notify_attempts: usize,
            notify_retry_ms: u64,
        ) -> AcquireOutcome {
            let wide_mutex = encode_wide_null(mutex_name);
            let (handle, last_err) = unsafe {
                let h = CreateMutexW(std::ptr::null(), 1, wide_mutex.as_ptr());
                let err = GetLastError();
                (h, err)
            };

            let already_running = (handle != 0 && last_err == ERROR_ALREADY_EXISTS)
                || (handle == 0 && last_err == ERROR_ACCESS_DENIED);

            if already_running {
                if handle != 0 {
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                }
                let notified =
                    notify_existing_instance(ipc_class, ipc_title, notify_attempts, notify_retry_ms);
                return AcquireOutcome::SecondaryWokeExisting { notified };
            }

            let hub = Arc::new(WakeHub::new(debounce_ms));
            let ipc_hwnd = Arc::new(AtomicIsize::new(0));
            let (ready_tx, ready_rx) = mpsc::channel::<isize>();

            let cls_owned = ipc_class.to_string();
            let title_owned = ipc_title.to_string();
            let hub_for_thread = hub.clone();
            let hwnd_for_thread = ipc_hwnd.clone();

            let listener_thread = std::thread::Builder::new()
                .name("bm-single-instance-ipc".to_string())
                .spawn(move || unsafe {
                    let hinst = GetModuleHandleW(std::ptr::null());
                    let wide_cls = encode_wide_null(&cls_owned);
                    let wide_title = encode_wide_null(&title_owned);

                    let wc = WinClassExW {
                        cb_size: std::mem::size_of::<WinClassExW>() as u32,
                        style: 0,
                        lpfn_wnd_proc: Some(ipc_host_wnd_proc),
                        cb_cls_extra: 0,
                        cb_wnd_extra: 0,
                        h_instance: hinst,
                        h_icon: 0,
                        h_cursor: 0,
                        hbr_background: 0,
                        lpsz_menu_name: std::ptr::null(),
                        lpsz_class_name: wide_cls.as_ptr(),
                        h_icon_sm: 0,
                    };
                    let _ = RegisterClassExW(&wc);

                    let hwnd = CreateWindowExW(
                        0,
                        wide_cls.as_ptr(),
                        wide_title.as_ptr(),
                        0,
                        0,
                        0,
                        0,
                        0,
                        HWND_MESSAGE,
                        0,
                        hinst,
                        0,
                    );
                    if hwnd == 0 {
                        let _ = ready_tx.send(0);
                        return;
                    }

                    let raw_hub = Arc::into_raw(hub_for_thread);
                    let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, raw_hub as isize);
                    hwnd_for_thread.store(hwnd, Ordering::Release);
                    let _ = ready_tx.send(hwnd);

                    let mut msg = WinMsg::default();
                    while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                        let _ = TranslateMessage(&msg);
                        let _ = DispatchMessageW(&msg);
                    }

                    hwnd_for_thread.store(0, Ordering::Release);
                    let _ = UnregisterClassW(wide_cls.as_ptr(), hinst);
                })
                .ok();

            let _ = ready_rx.recv_timeout(Duration::from_millis(800));

            AcquireOutcome::Primary(SingleInstanceGuard {
                mutex_handle: handle,
                ipc_hwnd,
                hub,
                listener_thread,
            })
        }

        /// 在 Tauri `.setup()` 中挂载主实例唤醒回调；若冷启动竞态期间已有次实例发来唤醒请求，会立刻补发一次。
        pub fn attach_wake_handler<F>(&self, handler: F)
        where
            F: Fn() + Send + Sync + 'static,
        {
            let cb: WakeCallback = Arc::new(handler);
            if let Ok(mut slot) = self.hub.callback.lock() {
                *slot = Some(cb.clone());
            }
            if self.hub.pending_wake.swap(false, Ordering::AcqRel) {
                cb();
            }
        }
    }

    impl Drop for SingleInstanceGuard {
        fn drop(&mut self) {
            let hwnd = self.ipc_hwnd.swap(0, Ordering::AcqRel);
            if hwnd != 0 {
                unsafe {
                    let _ = PostMessageW(hwnd, WM_BM_SINGLE_INSTANCE_STOP, 0, 0);
                }
            }
            if let Some(handle) = self.listener_thread.take() {
                let _ = handle.join();
            }
            if self.mutex_handle != 0 {
                unsafe {
                    let _ = CloseHandle(self.mutex_handle);
                }
                self.mutex_handle = 0;
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    pub struct SingleInstanceGuard;

    pub enum AcquireOutcome {
        Primary(SingleInstanceGuard),
        SecondaryWokeExisting { notified: bool },
    }

    impl SingleInstanceGuard {
        pub fn acquire() -> AcquireOutcome {
            AcquireOutcome::Primary(SingleInstanceGuard)
        }

        pub fn attach_wake_handler<F>(&self, _handler: F)
        where
            F: Fn() + Send + Sync + 'static,
        {
        }
    }
}

pub use platform::{AcquireOutcome, SingleInstanceGuard};

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::platform::{AcquireOutcome, SingleInstanceGuard};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_single_instance_blocks_duplicates_wakes_primary_and_releases_cleanly() {
        let uid = format!(
            "{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let mutex = format!("Local\\BmTestSingleInstanceMutex_{}", uid);
        let cls = format!("BmTestSingleInstanceClass_{}", uid);
        let title = format!("BmTestSingleInstanceHost_{}", uid);

        // 1. 首个实例必须成功抢占主锁并就绪 HWND_MESSAGE 宿主
        let primary = match SingleInstanceGuard::acquire_named(&mutex, &cls, &title, 0, 10, 20) {
            AcquireOutcome::Primary(guard) => guard,
            AcquireOutcome::SecondaryWokeExisting { .. } => {
                panic!("首个独立实例不应被判定为次实例")
            }
        };

        let wake_hits = Arc::new(AtomicUsize::new(0));
        let wake_hits_cb = wake_hits.clone();
        primary.attach_wake_handler(move || {
            wake_hits_cb.fetch_add(1, Ordering::AcqRel);
        });

        // 2. 连续 5 次模拟点击任务栏固定图标 / 桌面快捷方式启动新进程：
        //    全部必须被拦截为 SecondaryWokeExisting，且每次都精准唤醒主实例！
        for attempt in 1..=5 {
            match SingleInstanceGuard::acquire_named(&mutex, &cls, &title, 0, 10, 20) {
                AcquireOutcome::SecondaryWokeExisting { notified } => {
                    assert!(notified, "第 {} 次次实例启动必须成功向主实例投递唤醒消息", attempt);
                }
                AcquireOutcome::Primary(_) => {
                    panic!("主实例存活期间绝不允许第 {} 次抢占成为重复主实例", attempt);
                }
            }
        }
        assert_eq!(
            wake_hits.load(Ordering::Acquire),
            5,
            "5 次次实例拉起应累计触发 5 次主实例唤醒回调"
        );

        // 3. 主实例退出释放 Guard 后，下一次启动应能重新正常成为主实例
        drop(primary);
        let next_primary =
            match SingleInstanceGuard::acquire_named(&mutex, &cls, &title, 0, 10, 20) {
                AcquireOutcome::Primary(guard) => guard,
                AcquireOutcome::SecondaryWokeExisting { .. } => {
                    panic!("原主实例退出释放句柄后，新启动应能正常成为主实例")
                }
            };
        drop(next_primary);
    }

    #[test]
    fn test_single_instance_cold_start_pending_wake_and_debounce() {
        let uid = format!(
            "cold_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let mutex = format!("Local\\BmTestSingleInstanceMutex_{}", uid);
        let cls = format!("BmTestSingleInstanceClass_{}", uid);
        let title = format!("BmTestSingleInstanceHost_{}", uid);

        // 带 300ms 防抖窗口的主实例（模拟双击桌面快捷方式防动画打断）
        let primary = match SingleInstanceGuard::acquire_named(&mutex, &cls, &title, 300, 10, 20) {
            AcquireOutcome::Primary(guard) => guard,
            AcquireOutcome::SecondaryWokeExisting { .. } => panic!("首个实例应为主实例"),
        };

        // 在 attach_wake_handler 之前就有次实例拉起（冷启动竞态）+ 立即连点第二次（防抖合并）
        for _ in 0..2 {
            match SingleInstanceGuard::acquire_named(&mutex, &cls, &title, 300, 10, 20) {
                AcquireOutcome::SecondaryWokeExisting { notified } => assert!(notified),
                AcquireOutcome::Primary(_) => panic!("不应拉起第二个主实例"),
            }
        }

        // .setup() 延迟挂载回调后，pending_wake 应立刻补发恰好 1 次
        let wake_hits = Arc::new(AtomicUsize::new(0));
        let wake_hits_cb = wake_hits.clone();
        primary.attach_wake_handler(move || {
            wake_hits_cb.fetch_add(1, Ordering::AcqRel);
        });
        assert_eq!(
            wake_hits.load(Ordering::Acquire),
            1,
            "冷启动竞态内连点两次应在挂载回调后合并补发 1 次唤醒"
        );
        drop(primary);
    }
}
