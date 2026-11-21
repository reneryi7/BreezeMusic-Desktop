pub mod kugou;

use kugou::api::KugouApiService;
use kugou::fonts::{CustomFontBinaryPayload, CustomFontRecord, InstalledFontsPayload};
use kugou::models::{
    AccountProfile, AlbumItem, CacheDiskStats, CloudDeleteItem, CloudDrivePageResult,
    CommentPageResult, DownloadProgressPayload, DownloadTaskResult, FmChannel, FollowedArtist,
    GatewayDiagResult, KugouSession, ListeningGradeSyncResult, LocalMusicScanOptions,
    LyricCandidateItem, LyricData, PagedSearchResult, PhoneLoginOutcome, PlaybackResult, Playlist,
    PurchasedAlbumItem, PurchasedSongItem, QrCodeSession, QrPollResult, RankItem,
    ResolvedSongAlbumMeta, SavedLyricResult, SearchDiscoveryData, SharedMusicResolved, Song,
    SongComment, SongDetailInfo, VipStatusInfo, ViperEffectItem,
};
use kugou::store::AppStore;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GlobalHotkeyBinding {
    pub action: String,
    pub accelerator: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GlobalHotkeySyncReport {
    pub enabled: bool,
    pub registered_count: usize,
    pub failed_actions: Vec<String>,
}

pub fn default_global_hotkey_bindings() -> Vec<GlobalHotkeyBinding> {
    vec![
        GlobalHotkeyBinding {
            action: "toggle_play".to_string(),
            accelerator: "Ctrl+Alt+Space".to_string(),
        },
        GlobalHotkeyBinding {
            action: "prev_song".to_string(),
            accelerator: "Ctrl+Alt+Left".to_string(),
        },
        GlobalHotkeyBinding {
            action: "next_song".to_string(),
            accelerator: "Ctrl+Alt+Right".to_string(),
        },
        GlobalHotkeyBinding {
            action: "toggle_desktop_lyric".to_string(),
            accelerator: "Ctrl+Alt+D".to_string(),
        },
        GlobalHotkeyBinding {
            action: "toggle_lyric_lock".to_string(),
            accelerator: "Ctrl+Shift+L".to_string(),
        },
        GlobalHotkeyBinding {
            action: "toggle_mini_mode".to_string(),
            accelerator: "Ctrl+Alt+M".to_string(),
        },
        GlobalHotkeyBinding {
            action: "volume_up".to_string(),
            accelerator: "Ctrl+Alt+Up".to_string(),
        },
        GlobalHotkeyBinding {
            action: "volume_down".to_string(),
            accelerator: "Ctrl+Alt+Down".to_string(),
        },
        GlobalHotkeyBinding {
            action: "toggle_like".to_string(),
            accelerator: "".to_string(),
        },
    ]
}

/// 将类似 `"Ctrl+Shift+L"` / `"Alt+F8"` 的跨平台快捷键字符串解析为 Win32 `(fsModifiers, vk)`。
/// 除 `F1`..`F12` 外要求至少包含一个修饰键，避免全局抢占普通字母输入。
pub fn parse_win32_accelerator(acc: &str) -> Option<(u32, u32)> {
    const MOD_ALT: u32 = 0x0001;
    const MOD_CONTROL: u32 = 0x0002;
    const MOD_SHIFT: u32 = 0x0004;
    const MOD_WIN: u32 = 0x0008;
    const MOD_NOREPEAT: u32 = 0x4000;

    let trimmed = acc.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut mods: u32 = 0;
    let mut main_key: Option<&str> = None;

    for raw_part in trimmed.split('+') {
        let token = raw_part.trim();
        if token.is_empty() {
            continue;
        }
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "alt" | "option" => mods |= MOD_ALT,
            "shift" => mods |= MOD_SHIFT,
            "win" | "meta" | "super" | "cmd" | "command" => mods |= MOD_WIN,
            _ => {
                if main_key.is_some() {
                    return None;
                }
                main_key = Some(token);
            }
        }
    }

    let key_str = main_key?;
    let key_upper = key_str.to_ascii_uppercase();

    let (vk, is_fn_key) = if key_upper.len() == 1 {
        let ch = key_upper.as_bytes()[0];
        match ch {
            b'A'..=b'Z' => (ch as u32, false),
            b'0'..=b'9' => (ch as u32, false),
            b'-' => (0xBD, false),
            b'=' => (0xBB, false),
            b'[' => (0xDB, false),
            b']' => (0xDD, false),
            b'\\' => (0xDC, false),
            b';' => (0xBA, false),
            b'\'' => (0xDE, false),
            b',' => (0xBC, false),
            b'.' => (0xBE, false),
            b'/' => (0xBF, false),
            b'`' => (0xC0, false),
            _ => return None,
        }
    } else if let Some(num_str) = key_upper.strip_prefix('F') {
        if let Ok(n) = num_str.parse::<u32>() {
            if (1..=12).contains(&n) {
                (0x6F + n, true)
            } else {
                return None;
            }
        } else {
            return None;
        }
    } else {
        match key_upper.as_str() {
            "SPACE" => (0x20, false),
            "LEFT" | "ARROWLEFT" => (0x25, false),
            "UP" | "ARROWUP" => (0x26, false),
            "RIGHT" | "ARROWRIGHT" => (0x27, false),
            "DOWN" | "ARROWDOWN" => (0x28, false),
            "HOME" => (0x24, false),
            "END" => (0x23, false),
            "PAGEUP" => (0x21, false),
            "PAGEDOWN" => (0x22, false),
            "INSERT" => (0x2D, false),
            "DELETE" | "DEL" => (0x2E, false),
            "BACKSPACE" => (0x08, false),
            "ENTER" | "RETURN" => (0x0D, false),
            "TAB" => (0x09, false),
            "MINUS" => (0xBD, false),
            "EQUAL" | "PLUS" => (0xBB, false),
            "BRACKETLEFT" => (0xDB, false),
            "BRACKETRIGHT" => (0xDD, false),
            "BACKSLASH" => (0xDC, false),
            "SEMICOLON" => (0xBA, false),
            "QUOTE" => (0xDE, false),
            "COMMA" => (0xBC, false),
            "PERIOD" => (0xBE, false),
            "SLASH" => (0xBF, false),
            "BACKQUOTE" => (0xC0, false),
            "NUMPAD0" => (0x60, false),
            "NUMPAD1" => (0x61, false),
            "NUMPAD2" => (0x62, false),
            "NUMPAD3" => (0x63, false),
            "NUMPAD4" => (0x64, false),
            "NUMPAD5" => (0x65, false),
            "NUMPAD6" => (0x66, false),
            "NUMPAD7" => (0x67, false),
            "NUMPAD8" => (0x68, false),
            "NUMPAD9" => (0x69, false),
            "NUMPADMULTIPLY" => (0x6A, false),
            "NUMPADADD" => (0x6B, false),
            "NUMPADSUBTRACT" => (0x6D, false),
            "NUMPADDECIMAL" => (0x6E, false),
            "NUMPADDIVIDE" => (0x6F, false),
            _ => return None,
        }
    };

    if mods == 0 && !is_fn_key {
        return None;
    }

    Some((mods | MOD_NOREPEAT, vk))
}

pub struct AppState {
    pub api: Arc<KugouApiService>,
    pub store: Arc<AppStore>,
    pub close_to_tray: Arc<AtomicBool>,
    pub global_hotkeys_enabled: Arc<AtomicBool>,
    pub global_hotkey_bindings: Arc<std::sync::Mutex<Vec<GlobalHotkeyBinding>>>,
    pub global_hotkeys_revision: Arc<AtomicU64>,
    pub global_hotkey_failed: Arc<std::sync::Mutex<Vec<String>>>,
    pub desktop_lyric_locked: Arc<AtomicBool>,
    pub desktop_lyric_visible: Arc<AtomicBool>,
    pub desktop_lyric_positioned: Arc<AtomicBool>,
    pub desktop_lyric_last_pos: Arc<std::sync::Mutex<Option<(f64, f64)>>>,
    pub desktop_lyric_snapshot: Arc<std::sync::Mutex<Option<serde_json::Value>>>,
    pub mini_player_visible: Arc<AtomicBool>,
    pub mini_player_positioned: Arc<AtomicBool>,
    pub mini_player_expanded: Arc<AtomicBool>,
    pub mini_player_dock_edge: Arc<std::sync::Mutex<String>>,
    pub mini_player_snapshot: Arc<std::sync::Mutex<Option<serde_json::Value>>>,
    /// 记录主窗口隐藏（进入 Mini 模式或最小化至托盘）前是否处于最大化状态，防止恢复时 Win32/WebView2 错位出现底部白边
    pub main_was_maximized: Arc<AtomicBool>,
    /// 记录主窗口当前是否已将 WebView2 内存目标切换为 LOW，避免重复 COM 调用
    pub main_low_memory_active: Arc<AtomicBool>,
    /// Win32 全局热键与穿透检测线程 ID，用于在状态改变时零延迟唤醒 MsgWaitForMultipleObjectsEx
    pub hotkey_thread_id: Arc<AtomicU32>,
    /// 活跃下载任务取消令牌表（前端可随时通过 cancel_download_task_cmd 实时中断并清理临时分片）
    pub active_download_cancels: Arc<std::sync::Mutex<HashMap<String, Arc<AtomicBool>>>>,
    /// 原生字体文件选择器白名单（SEC-FS-FONT：仅允许导入经由 pick_font_files_dialog_cmd 选中的真实路径）
    pub authorized_font_import_paths: Arc<std::sync::Mutex<HashSet<String>>>,
}

#[cfg(target_os = "windows")]
const WM_APP_HOTKEY_WAKE: u32 = 0x8001;

#[cfg(target_os = "windows")]
fn wake_hotkey_thread(thread_id: &AtomicU32) {
    #[link(name = "user32")]
    extern "system" {
        fn PostThreadMessageW(id_thread: u32, msg: u32, w_param: usize, l_param: isize) -> i32;
    }
    let tid = thread_id.load(Ordering::Acquire);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, WM_APP_HOTKEY_WAKE, 0, 0);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn wake_hotkey_thread(_thread_id: &AtomicU32) {}

#[cfg(target_os = "windows")]
fn set_webview_memory_target_low(win: &tauri::WebviewWindow, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;

    let _ = win.with_webview(move |webview| unsafe {
        if let Ok(core) = webview.controller().CoreWebView2() {
            if let Ok(wv19) = core.cast::<ICoreWebView2_19>() {
                let level = if low {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                } else {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                };
                let _ = wv19.SetMemoryUsageTargetLevel(level);
            }
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn set_webview_memory_target_low(_win: &tauri::WebviewWindow, _low: bool) {}

fn sync_main_low_memory_state(win: &tauri::WebviewWindow, flag: &AtomicBool, low: bool) {
    if flag.swap(low, Ordering::Relaxed) != low {
        set_webview_memory_target_low(win, low);
    }
}

#[cfg(target_os = "windows")]
fn set_subwindow_suspended(win: &tauri::WebviewWindow, suspend: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2TrySuspendCompletedHandler, ICoreWebView2_3,
    };
    use windows_core::Interface;

    let _ = win.with_webview(move |webview| unsafe {
        let controller = webview.controller();
        let _ = controller.SetIsVisible(!suspend);
        if let Ok(core) = controller.CoreWebView2() {
            if let Ok(wv3) = core.cast::<ICoreWebView2_3>() {
                if suspend {
                    let _ = wv3.TrySuspend(None::<&ICoreWebView2TrySuspendCompletedHandler>);
                } else {
                    let _ = wv3.Resume();
                }
            }
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn set_subwindow_suspended(_win: &tauri::WebviewWindow, _suspend: bool) {}

/// 子窗口隐藏后延迟 160ms 再切低内存并调用 WebView2 TrySuspend，确保前端收到 visibility=false 完成清理且避免快速开关竞态
fn schedule_subwindow_suspend(app: &AppHandle, label: &'static str, visible_flag: Arc<AtomicBool>) {
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(160)).await;
        if visible_flag.load(Ordering::Relaxed) {
            return;
        }
        let app_for_main = app_clone.clone();
        let _ = app_clone.run_on_main_thread(move || {
            if visible_flag.load(Ordering::Relaxed) {
                return;
            }
            if let Some(win) = app_for_main.get_webview_window(label) {
                set_webview_memory_target_low(&win, true);
                set_subwindow_suspended(&win, true);
            }
        });
    });
}

fn ensure_desktop_lyric_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(existing) = app.get_webview_window("desktop_lyric") {
        return Some(existing);
    }
    let builder = WebviewWindowBuilder::new(
        app,
        "desktop_lyric",
        WebviewUrl::App("index.html?window=desktop_lyric".into()),
    )
    .title("微风桌面歌词")
    .inner_size(820.0, 156.0)
    .min_inner_size(420.0, 88.0)
    .resizable(false)
    .fullscreen(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .visible(false);

    match builder.build() {
        Ok(win) => {
            harden_desktop_webview2(&win);
            enforce_transparent_overlay_window_style(&win);
            Some(win)
        }
        Err(_) => app.get_webview_window("desktop_lyric"),
    }
}

fn ensure_mini_player_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(existing) = app.get_webview_window("mini_player") {
        return Some(existing);
    }
    let builder = WebviewWindowBuilder::new(
        app,
        "mini_player",
        WebviewUrl::App("index.html?window=mini_player".into()),
    )
    .title("微风 Mini")
    .inner_size(480.0, 228.0)
    .min_inner_size(46.0, 40.0)
    .resizable(false)
    .fullscreen(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .visible(false);

    match builder.build() {
        Ok(win) => {
            harden_desktop_webview2(&win);
            Some(win)
        }
        Err(_) => app.get_webview_window("mini_player"),
    }
}

/// 深度隐藏 Windows WebView2 浏览器默认右键菜单、快捷键、缩放、状态栏与发行版 DevTools，
/// 彻底消除桌面音乐客户端的“浏览器/Web”观感。
#[cfg(target_os = "windows")]
fn harden_desktop_webview2(win: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Settings3, ICoreWebView2Settings5,
    };
    use windows_core::Interface;

    let _ = win.with_webview(|webview| unsafe {
        let controller = webview.controller();
        if let Ok(core) = controller.CoreWebView2() {
            if let Ok(settings) = core.Settings() {
                let _ = settings.SetAreDefaultContextMenusEnabled(false);
                let _ = settings.SetIsZoomControlEnabled(false);
                let _ = settings.SetIsStatusBarEnabled(false);
                #[cfg(not(debug_assertions))]
                let _ = settings.SetAreDevToolsEnabled(false);

                if let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>() {
                    let _ = settings3.SetAreBrowserAcceleratorKeysEnabled(false);
                }
                if let Ok(settings5) = settings.cast::<ICoreWebView2Settings5>() {
                    let _ = settings5.SetIsPinchZoomEnabled(false);
                }
            }
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn harden_desktop_webview2(_win: &tauri::WebviewWindow) {}

/// 同步主窗口原生背景色与 WebView2 DefaultBackgroundColor，彻底杜绝深色主题下窗口边缘或重绘间隙漏出白色底板
fn apply_main_window_bg_color(win: &tauri::WebviewWindow, r: u8, g: u8, b: u8) {
    let _ = win.set_background_color(Some(tauri::window::Color(r, g, b, 255)));
    #[cfg(target_os = "windows")]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2Controller2, COREWEBVIEW2_COLOR,
        };
        use windows_core::Interface;

        let _ = win.with_webview(move |webview| unsafe {
            let controller = webview.controller();
            if let Ok(ctrl2) = controller.cast::<ICoreWebView2Controller2>() {
                let _ = ctrl2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                    A: 255,
                    R: r,
                    G: g,
                    B: b,
                });
            }
        });
    }
}

/// 强制同步 Windows 无边框主窗口 DWM 客户区与 WebView2 控制器矩形，根治「最大化 -> Mini -> 退出」后窗口底部露出 7~8px 白边 BUG
#[cfg(target_os = "windows")]
fn sync_main_webview_bounds_and_frame(win: &tauri::WebviewWindow, ensure_maximized: bool) {
    #[repr(C)]
    #[derive(Default)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn ShowWindow(hwnd: isize, n_cmd_show: i32) -> i32;
        fn SetWindowPos(
            hwnd: isize,
            hwnd_insert_after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            u_flags: u32,
        ) -> i32;
        fn GetClientRect(hwnd: isize, lp_rect: *mut WinRect) -> i32;
        fn RedrawWindow(hwnd: isize, lprc_update: *const WinRect, hrgn_update: isize, flags: u32) -> i32;
    }

    const SW_SHOWMAXIMIZED: i32 = 3;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const RDW_INVALIDATE: u32 = 0x0001;
    const RDW_FRAME: u32 = 0x0400;
    const RDW_ALLCHILDREN: u32 = 0x0080;
    const RDW_UPDATENOW: u32 = 0x0100;

    let Ok(raw_hwnd) = win.hwnd() else {
        return;
    };
    let hwnd = raw_hwnd.0 as isize;
    if hwnd == 0 {
        return;
    }

    unsafe {
        if ensure_maximized {
            let _ = ShowWindow(hwnd, SW_SHOWMAXIMIZED);
        }
        let _ = SetWindowPos(
            hwnd,
            0,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }

    let _ = win.with_webview(move |webview| unsafe {
        let controller = webview.controller();
        let mut rc = WinRect::default();
        if GetClientRect(hwnd, &mut rc) != 0 && rc.right > rc.left && rc.bottom > rc.top {
            let mut bounds = std::mem::zeroed();
            if controller.Bounds(&mut bounds).is_ok() {
                bounds.left = rc.left;
                bounds.top = rc.top;
                bounds.right = rc.right;
                bounds.bottom = rc.bottom;
                let _ = controller.SetBounds(bounds);
            }
            let _ = controller.NotifyParentWindowPositionChanged();
        }
        let _ = RedrawWindow(
            hwnd,
            std::ptr::null(),
            0,
            RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
        );
    });
}

#[cfg(not(target_os = "windows"))]
fn sync_main_webview_bounds_and_frame(_win: &tauri::WebviewWindow, _ensure_maximized: bool) {}

fn show_and_restore_main_window(app: &AppHandle, state: &AppState) {
    if let Some(main_win) = app.get_webview_window("main") {
        sync_main_low_memory_state(&main_win, &state.main_low_memory_active, false);
        let was_minimized = main_win.is_minimized().unwrap_or(false);
        let was_maximized = state.main_was_maximized.load(Ordering::Relaxed)
            || main_win.is_maximized().unwrap_or(false);

        // 仅当确实处于最小化状态时才调用 unminimize()，禁止对隐藏的最大化无边框窗口调用 SW_RESTORE 导致帧错位
        if was_minimized {
            let _ = main_win.unminimize();
        }

        if was_maximized {
            sync_main_webview_bounds_and_frame(&main_win, true);
            let _ = main_win.show();
            sync_main_webview_bounds_and_frame(&main_win, true);
        } else {
            let _ = main_win.show();
            sync_main_webview_bounds_and_frame(&main_win, false);
        }
        let _ = main_win.set_focus();

        // Windows DWM 显示动画结束后 65ms 再做一次主线程兜底 Bounds 同步，确保 WebView2 与客户区像素级贴合
        let app_for_delay = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(65));
            let app_inner = app_for_delay.clone();
            let _ = app_for_delay.run_on_main_thread(move || {
                if let Some(w) = app_inner.get_webview_window("main") {
                    if w.is_visible().unwrap_or(false) {
                        let is_max = was_maximized || w.is_maximized().unwrap_or(false);
                        sync_main_webview_bounds_and_frame(&w, is_max);
                    }
                }
                let _ = app_inner.emit("bm-main-window-restored", serde_json::json!({ "maximized": was_maximized }));
            });
        });
    }
}

#[tauri::command]
async fn sync_main_window_bg_color_cmd(
    window: tauri::WebviewWindow,
    r: u8,
    g: u8,
    b: u8,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    apply_main_window_bg_color(&window, r, g, b);
    Ok(true)
}

/// SEC-03: 跨窗口最小权限边界守卫，拦截子窗口越权调用主窗口专属高权限命令
fn ensure_main_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("安全策略拒绝：非主窗口无权调用该高权限系统命令".to_string());
    }
    Ok(())
}

/// SEC-03B: 角色化窗口白名单守卫，严格限定子窗口仅能访问与其职责匹配的 IPC 命令
fn ensure_allowed_windows(window: &tauri::WebviewWindow, allowed: &[&str]) -> Result<(), String> {
    let label = window.label();
    if allowed.iter().any(|&a| a == label) {
        return Ok(());
    }
    Err(format!(
        "安全策略拒绝：窗口 '{}' 无权调用该受限命令",
        label
    ))
}

fn normalize_font_whitelist_key(raw_path: &str) -> String {
    let trimmed = raw_path.trim();
    let canon = std::fs::canonicalize(trimmed)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| trimmed.to_string());
    canon
        .strip_prefix(r"\\?\")
        .unwrap_or(&canon)
        .replace('/', "\\")
        .to_ascii_lowercase()
}

#[tauri::command]
async fn init_app_state(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Option<KugouSession>, String> {
    ensure_main_window(&window)?;
    let _ = state.api.register_device(false).await;
    Ok(state.store.session().map(|s| s.redacted_for_ipc()))
}

#[tauri::command]
async fn get_saved_accounts_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<KugouSession>, String> {
    ensure_main_window(&window)?;
    Ok(state
        .store
        .saved_accounts()
        .into_iter()
        .map(|s| s.redacted_for_ipc())
        .collect())
}

#[tauri::command]
async fn switch_saved_account_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    user_id: String,
) -> Result<Option<KugouSession>, String> {
    ensure_main_window(&window)?;
    state.api.clear_account_caches();
    Ok(state
        .store
        .switch_account(&user_id)
        .map(|s| s.redacted_for_ipc()))
}

#[tauri::command]
async fn remove_saved_account_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    user_id: String,
) -> Result<Vec<KugouSession>, String> {
    ensure_main_window(&window)?;
    Ok(state
        .store
        .remove_saved_account(&user_id)
        .into_iter()
        .map(|s| s.redacted_for_ipc())
        .collect())
}

#[tauri::command]
async fn send_sms_code(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    mobile: String,
) -> Result<String, String> {
    ensure_main_window(&window)?;
    state.api.send_mobile_code(&mobile).await
}

#[tauri::command]
async fn login_with_sms(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    mobile: String,
    code: String,
    user_id: Option<String>,
) -> Result<PhoneLoginOutcome, String> {
    ensure_main_window(&window)?;
    let mut outcome = state.api.login_by_phone(&mobile, &code, user_id).await?;
    outcome.session = outcome.session.map(|s| s.redacted_for_ipc());
    Ok(outcome)
}

#[tauri::command]
async fn create_qr_login(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<QrCodeSession, String> {
    ensure_main_window(&window)?;
    state.api.create_qr_code().await
}

#[tauri::command]
async fn check_qr_login(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    qrcode_key: String,
) -> Result<QrPollResult, String> {
    ensure_main_window(&window)?;
    let mut res = state.api.check_qr_code(&qrcode_key).await?;
    res.session = res.session.map(|s| s.redacted_for_ipc());
    Ok(res)
}

#[tauri::command]
async fn logout_account(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<(), String> {
    ensure_main_window(&window)?;
    state.api.clear_account_caches();
    state.store.set_session(None);
    Ok(())
}

#[tauri::command]
async fn get_account_profile(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    reported_d_sec: Option<i64>,
    reported_diff_sec: Option<i64>,
) -> Result<AccountProfile, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_full_profile_with_report(reported_d_sec, reported_diff_sec)
        .await
}

#[tauri::command]
async fn sync_listening_grade_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    force_report: Option<bool>,
) -> Result<ListeningGradeSyncResult, String> {
    ensure_main_window(&window)?;
    state
        .api
        .sync_listening_grade(force_report.unwrap_or(false))
        .await
}

#[tauri::command]
async fn record_listening_seconds_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    seconds: i64,
) -> Result<i64, String> {
    ensure_main_window(&window)?;
    state.api.record_listening_seconds(seconds)
}

#[tauri::command]
async fn get_vip_status(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<VipStatusInfo, String> {
    ensure_main_window(&window)?;
    state.api.fetch_vip_status().await
}

#[tauri::command]
async fn claim_daily_vip(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<VipStatusInfo, String> {
    ensure_main_window(&window)?;
    state.api.claim_daily_vip().await
}

#[tauri::command]
async fn get_daily_recommend(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_daily_recommend().await
}

#[tauri::command]
async fn get_personal_fm(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    exclude_hashes: Option<Vec<String>>,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_personal_fm_with_excludes(exclude_hashes)
        .await
}

#[tauri::command]
async fn get_new_song_express(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    period_key: Option<String>,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    let key = period_key.unwrap_or_else(|| "CHINESE".to_string());
    state.api.fetch_new_song_express(&key).await
}

#[tauri::command]
async fn get_rank_list(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<RankItem>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_rank_list().await
}

#[tauri::command]
async fn get_rank_songs(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    rank_id: i32,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_rank_songs(rank_id, page.unwrap_or(1), page_size.unwrap_or(100))
        .await
}

#[tauri::command]
async fn get_fm_channels(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<FmChannel>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_fm_channels().await
}

#[tauri::command]
async fn get_fm_songs(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_fm_songs(&channel_id).await
}

#[tauri::command]
async fn get_recent_history(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_recent_history().await
}

#[tauri::command]
async fn search_songs(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .search_songs(&keyword, page.unwrap_or(1), page_size.unwrap_or(40))
        .await
}

#[tauri::command]
async fn get_search_hot(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_search_hot().await
}

#[tauri::command]
async fn get_search_suggest_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
) -> Result<Vec<String>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_search_suggest(&keyword).await
}

#[tauri::command]
async fn search_playlists_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<Vec<Playlist>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .search_playlists(&keyword, page.unwrap_or(1), page_size.unwrap_or(30))
        .await
}

#[tauri::command]
async fn search_albums_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<Vec<AlbumItem>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .search_albums(&keyword, page.unwrap_or(1), page_size.unwrap_or(30))
        .await
}

#[tauri::command]
async fn search_artists_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<Vec<FollowedArtist>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .search_artists(&keyword, page.unwrap_or(1), page_size.unwrap_or(30))
        .await
}

#[tauri::command]
async fn search_songs_paged_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<PagedSearchResult<Song>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .search_songs_paged(&keyword, page.unwrap_or(1), page_size.unwrap_or(30))
        .await
}

#[tauri::command]
async fn search_playlists_paged_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<PagedSearchResult<Playlist>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .search_playlists_paged(&keyword, page.unwrap_or(1), page_size.unwrap_or(30))
        .await
}

#[tauri::command]
async fn search_albums_paged_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<PagedSearchResult<AlbumItem>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .search_albums_paged(&keyword, page.unwrap_or(1), page_size.unwrap_or(30))
        .await
}

#[tauri::command]
async fn search_artists_paged_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    keyword: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<PagedSearchResult<FollowedArtist>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .search_artists_paged(&keyword, page.unwrap_or(1), page_size.unwrap_or(30))
        .await
}

#[tauri::command]
async fn get_hot_artists_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    category: Option<u32>,
    count: Option<usize>,
) -> Result<Vec<FollowedArtist>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_hot_artists(category.unwrap_or(0), count.unwrap_or(24))
        .await
}

#[tauri::command]
async fn get_search_discovery_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    artist_category: Option<u32>,
) -> Result<SearchDiscoveryData, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_search_discovery(artist_category.unwrap_or(0))
        .await
}

#[tauri::command]
async fn get_user_playlists(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<Playlist>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_user_playlists().await
}

#[tauri::command]
async fn get_playlist_info_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    playlist_id: String,
) -> Result<Playlist, String> {
    ensure_main_window(&window)?;
    state.api.fetch_playlist_info(&playlist_id).await
}

#[tauri::command]
async fn get_playlist_songs(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    collection_id: String,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_playlist_songs(&collection_id).await
}

#[tauri::command]
async fn create_user_playlist_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    name: String,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state.api.create_user_playlist(&name).await
}

#[tauri::command]
async fn update_user_playlist_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    list_id: i64,
    global_id: Option<String>,
    title: String,
    description: Option<String>,
    cover_url: Option<String>,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state
        .api
        .update_user_playlist(
            list_id,
            &global_id.unwrap_or_default(),
            &title,
            &description.unwrap_or_default(),
            &cover_url.unwrap_or_default(),
        )
        .await
}

#[tauri::command]
async fn delete_user_playlist_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    list_id: i64,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state.api.delete_user_playlist(list_id).await
}

#[tauri::command]
async fn collect_playlist_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    playlist: Playlist,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state.api.collect_playlist_cloud(&playlist).await
}

#[tauri::command]
async fn uncollect_playlist_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    playlist: Playlist,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state.api.uncollect_playlist_cloud(&playlist).await
}

#[tauri::command]
async fn add_song_to_playlist_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    list_id: i64,
    song: Song,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state.api.add_song_to_user_playlist(list_id, &song).await
}

#[tauri::command]
async fn add_songs_to_playlist_batch_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    list_id: i64,
    songs: Vec<Song>,
) -> Result<usize, String> {
    ensure_main_window(&window)?;
    state.api.add_songs_to_playlist_batch(list_id, &songs).await
}

#[tauri::command]
async fn remove_song_from_playlist_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    list_id: i64,
    song: Song,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state.api.remove_song_from_user_playlist(list_id, &song).await
}

#[tauri::command]
async fn resolve_song_album_meta_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
) -> Result<ResolvedSongAlbumMeta, String> {
    ensure_main_window(&window)?;
    state.api.resolve_song_album_meta(&song).await
}

#[tauri::command]
async fn get_followed_artists(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<FollowedArtist>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_followed_artists().await
}

#[tauri::command]
async fn toggle_follow_artist_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    artist_id: String,
    follow: bool,
    target_user_id: Option<String>,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state
        .api
        .toggle_follow_artist(&artist_id, follow, target_user_id.as_deref())
        .await
}

#[tauri::command]
async fn get_purchased_albums(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<PurchasedAlbumItem>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_purchased_albums().await
}

#[tauri::command]
async fn get_purchased_songs(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<PurchasedSongItem>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_purchased_songs().await
}

#[tauri::command]
async fn get_artist_detail_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    artist_id: String,
) -> Result<FollowedArtist, String> {
    ensure_main_window(&window)?;
    state.api.fetch_artist_detail(&artist_id).await
}

#[tauri::command]
async fn get_artist_songs_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    artist_id: String,
    sort: Option<String>,
    page: Option<u32>,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_artist_songs(
            &artist_id,
            &sort.unwrap_or_else(|| "hot".to_string()),
            page.unwrap_or(1),
        )
        .await
}

#[tauri::command]
async fn get_artist_albums_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    artist_id: String,
    page: Option<u32>,
    sort: Option<String>,
) -> Result<Vec<AlbumItem>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_artist_albums(
            &artist_id,
            page.unwrap_or(1),
            &sort.unwrap_or_else(|| "new".to_string()),
        )
        .await
}

#[tauri::command]
async fn get_album_detail_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    album_id: String,
) -> Result<AlbumItem, String> {
    ensure_main_window(&window)?;
    state.api.fetch_album_detail(&album_id).await
}

#[tauri::command]
async fn get_album_songs_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    album_id: String,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_album_songs(&album_id).await
}

#[tauri::command]
async fn get_song_comment_count_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
) -> Result<i64, String> {
    ensure_main_window(&window)?;
    state.api.fetch_song_comment_count(&song).await
}

#[tauri::command]
async fn get_song_comments_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    sort_type: Option<String>,
    page: Option<u32>,
) -> Result<Vec<SongComment>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_song_comments(
            &song,
            &sort_type.unwrap_or_else(|| "hot".to_string()),
            page.unwrap_or(1),
        )
        .await
}

#[tauri::command]
async fn get_song_comments_paged_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    sort_type: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<CommentPageResult, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_song_comments_paged(
            &song,
            &sort_type.unwrap_or_else(|| "hot".to_string()),
            page.unwrap_or(1),
            page_size.unwrap_or(30),
        )
        .await
}

#[tauri::command]
async fn get_comment_floor_replies_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    comment_id: String,
    special_child_id: Option<String>,
    page: Option<u32>,
) -> Result<Vec<SongComment>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_comment_floor_replies(
            &song,
            &comment_id,
            &special_child_id.unwrap_or_default(),
            page.unwrap_or(1),
        )
        .await
}

#[tauri::command]
async fn get_comment_floor_replies_paged_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    comment_id: String,
    special_child_id: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<CommentPageResult, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_comment_floor_replies_paged(
            &song,
            &comment_id,
            &special_child_id.unwrap_or_default(),
            page.unwrap_or(1),
            page_size.unwrap_or(40),
        )
        .await
}

#[tauri::command]
async fn like_song_comment_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    comment_id: String,
    like: bool,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state.api.like_song_comment(&song, &comment_id, like).await
}

#[tauri::command]
async fn post_song_comment_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    content: String,
    reply_to_id: Option<String>,
) -> Result<SongComment, String> {
    ensure_main_window(&window)?;
    state
        .api
        .post_song_comment(&song, &content, reply_to_id.as_deref())
        .await
}

#[tauri::command]
async fn resign_device_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<GatewayDiagResult, String> {
    ensure_main_window(&window)?;
    state.api.resign_device_identity().await
}

#[tauri::command]
async fn diagnose_gateway_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<GatewayDiagResult, String> {
    ensure_main_window(&window)?;
    state.api.diagnose_gateway_latency().await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn download_song_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    preferred_quality: Option<String>,
    music_dir: Option<String>,
    lrc_dir: Option<String>,
    export_lrc: Option<bool>,
    embed_cover_and_tags: Option<bool>,
    embed_lyrics_in_audio: Option<bool>,
    bilingual_lrc: Option<bool>,
    skip_existing: Option<bool>,
    task_id: Option<String>,
    file_naming_mode: Option<String>,
    folder_organize_mode: Option<String>,
) -> Result<DownloadTaskResult, String> {
    ensure_main_window(&window)?;
    let tid = task_id.unwrap_or_default();
    let progress_cb: Option<Arc<dyn Fn(&str, u64, u64, &str) + Send + Sync>> = if tid.is_empty() {
        None
    } else {
        let win_clone = window.clone();
        let tid_clone = tid.clone();
        Some(Arc::new(move |stage: &str, done: u64, total: u64, msg: &str| {
            let pct = if stage == "done" {
                100.0
            } else if stage == "tagging" {
                95.0
            } else if total > 0 {
                (((done as f64) / (total as f64)) * 92.0).clamp(0.0, 92.0) as f32
            } else if stage == "resolving" {
                4.0
            } else {
                10.0
            };
            let _ = win_clone.emit(
                "bm-download-progress",
                DownloadProgressPayload {
                    task_id: tid_clone.clone(),
                    stage: stage.to_string(),
                    downloaded_bytes: done,
                    total_bytes: total,
                    percent: pct,
                    message: msg.to_string(),
                },
            );
        }))
    };

    let cancel_flag = Arc::new(AtomicBool::new(false));
    if !tid.is_empty() {
        if let Ok(mut guard) = state.active_download_cancels.lock() {
            guard.insert(tid.clone(), Arc::clone(&cancel_flag));
        }
    }

    let res = state
        .api
        .download_song_to_local(
            &song,
            &preferred_quality.unwrap_or_else(|| "high".to_string()),
            &music_dir.unwrap_or_default(),
            &lrc_dir.unwrap_or_default(),
            export_lrc.unwrap_or(true),
            embed_cover_and_tags.unwrap_or(true),
            embed_lyrics_in_audio.unwrap_or(true),
            bilingual_lrc.unwrap_or(true),
            skip_existing.unwrap_or(true),
            &file_naming_mode.unwrap_or_else(|| "artist_title".to_string()),
            &folder_organize_mode.unwrap_or_else(|| "flat".to_string()),
            progress_cb,
            Some(cancel_flag),
        )
        .await;

    if !tid.is_empty() {
        if let Ok(mut guard) = state.active_download_cancels.lock() {
            guard.remove(&tid);
        }
    }

    if let Err(ref err_msg) = res {
        if !tid.is_empty() {
            let _ = window.emit(
                "bm-download-progress",
                DownloadProgressPayload {
                    task_id: tid,
                    stage: "error".to_string(),
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    percent: 0.0,
                    message: err_msg.clone(),
                },
            );
        }
    }
    res
}

#[tauri::command]
async fn cancel_download_task_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    task_id: String,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    let tid = task_id.trim();
    if tid.is_empty() {
        return Ok(false);
    }
    if let Ok(guard) = state.active_download_cancels.lock() {
        if let Some(flag) = guard.get(tid) {
            flag.store(true, Ordering::Relaxed);
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
async fn save_song_lyric_only_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    lrc_dir: Option<String>,
    music_dir: Option<String>,
    bilingual_lrc: Option<bool>,
    file_naming_mode: Option<String>,
    folder_organize_mode: Option<String>,
) -> Result<SavedLyricResult, String> {
    ensure_main_window(&window)?;
    state
        .api
        .save_song_lyric_only(
            &song,
            &lrc_dir.unwrap_or_default(),
            &music_dir.unwrap_or_default(),
            bilingual_lrc.unwrap_or(true),
            &file_naming_mode.unwrap_or_else(|| "artist_title".to_string()),
            &folder_organize_mode.unwrap_or_else(|| "flat".to_string()),
        )
        .await
}

#[tauri::command]
async fn parse_shared_music_cmd(
    state: State<'_, AppState>,
    raw_text: String,
) -> Result<SharedMusicResolved, String> {
    state.api.parse_shared_music_text(&raw_text).await
}

#[tauri::command]
async fn resolve_song_url(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    preferred_quality: Option<String>,
) -> Result<PlaybackResult, String> {
    ensure_main_window(&window)?;
    let pref = preferred_quality.unwrap_or_else(|| "flac".to_string());
    state.api.resolve_song_url(&song, &pref).await
}

#[tauri::command]
async fn resolve_song_accompaniment_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
) -> Result<PlaybackResult, String> {
    ensure_main_window(&window)?;
    state.api.resolve_song_accompaniment(&song).await
}

#[tauri::command]
async fn get_song_detail_info_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    preferred_quality: Option<String>,
    existing_playback: Option<PlaybackResult>,
) -> Result<SongDetailInfo, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_song_detail_info_with_quality(&song, preferred_quality.as_deref(), existing_playback)
        .await
}

#[tauri::command]
async fn get_cache_stats_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<CacheDiskStats, String> {
    ensure_main_window(&window)?;
    state.api.get_cache_stats()
}

#[tauri::command]
async fn clear_cache_category_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    category: String,
) -> Result<CacheDiskStats, String> {
    ensure_main_window(&window)?;
    state.api.clear_cache_category(&category)
}

fn resolve_desktop_lyric_target_monitor(
    app: &AppHandle,
    win: &tauri::WebviewWindow,
) -> Option<tauri::Monitor> {
    app.get_webview_window("main")
        .and_then(|m| m.current_monitor().ok().flatten())
        .or_else(|| win.current_monitor().ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten())
}

fn is_rect_visible_on_any_monitor(
    win: &tauri::WebviewWindow,
    x_phys: i32,
    y_phys: i32,
    w_phys: i32,
    h_phys: i32,
) -> bool {
    let Ok(monitors) = win.available_monitors() else {
        return false;
    };
    let right = x_phys + w_phys;
    let bottom = y_phys + h_phys;
    for mon in monitors {
        let wa = mon.work_area();
        let mx1 = wa.position.x;
        let my1 = wa.position.y;
        let mx2 = mx1 + wa.size.width as i32;
        let my2 = my1 + wa.size.height as i32;
        let overlap_x = (right.min(mx2) - x_phys.max(mx1)).max(0);
        let overlap_y = (bottom.min(my2) - y_phys.max(my1)).max(0);
        if overlap_x >= 120.min(w_phys / 2) && overlap_y >= 48.min(h_phys / 2) {
            return true;
        }
    }
    false
}

fn record_desktop_lyric_pos(
    app: &AppHandle,
    state: &AppState,
    lx: f64,
    ly: f64,
) {
    if !lx.is_finite() || !ly.is_finite() || lx < -8000.0 || ly < -8000.0 {
        return;
    }
    let rx = lx.round();
    let ry = ly.round();
    if let Ok(mut guard) = state.desktop_lyric_last_pos.lock() {
        if *guard == Some((rx, ry)) {
            return;
        }
        *guard = Some((rx, ry));
    }
    state.desktop_lyric_positioned.store(true, Ordering::Relaxed);
    let _ = app.emit_to(
        "main",
        "bm-desktop-lyric-moved",
        serde_json::json!({ "x": rx, "y": ry }),
    );
}

fn capture_desktop_lyric_current_pos(
    app: &AppHandle,
    win: &tauri::WebviewWindow,
    state: &AppState,
) -> Option<(f64, f64)> {
    let sf = win.scale_factor().unwrap_or(1.0).max(1.0);
    if let Ok(pos) = win.outer_position() {
        if pos.x > -10000 && pos.y > -10000 {
            let lx = (pos.x as f64 / sf).round();
            let ly = (pos.y as f64 / sf).round();
            record_desktop_lyric_pos(app, state, lx, ly);
            return Some((lx, ly));
        }
    }
    None
}

fn place_desktop_lyric_at_anchor(
    app: &AppHandle,
    win: &tauri::WebviewWindow,
    state: &AppState,
    target_w_logical: f64,
    target_h_logical: f64,
    anchor: &str,
) -> Option<(f64, f64)> {
    if let Some(monitor) = resolve_desktop_lyric_target_monitor(app, win) {
        let sf = monitor.scale_factor().max(1.0);
        let wa = monitor.work_area();
        let phys_w = (target_w_logical * sf).round() as i32;
        let phys_h = (target_h_logical * sf).round() as i32;
        let default_x = wa.position.x + ((wa.size.width as i32 - phys_w) / 2).max(12);
        let margin = (22.0 * sf).round() as i32;
        let default_y = match anchor {
            "top" => wa.position.y + margin,
            "center" => wa.position.y + ((wa.size.height as i32 - phys_h) / 2).max(12),
            _ => (wa.position.y + wa.size.height as i32 - phys_h - margin).max(wa.position.y + 12),
        };
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(
            default_x, default_y,
        )));
        let lx = (default_x as f64 / sf).round();
        let ly = (default_y as f64 / sf).round();
        record_desktop_lyric_pos(app, state, lx, ly);
        return Some((lx, ly));
    }
    None
}

fn place_desktop_lyric_default(
    app: &AppHandle,
    win: &tauri::WebviewWindow,
    state: &AppState,
    target_w_logical: f64,
    target_h_logical: f64,
) {
    place_desktop_lyric_at_anchor(app, win, state, target_w_logical, target_h_logical, "bottom");
}

fn clamp_desktop_lyric_within_work_area(
    app: &AppHandle,
    win: &tauri::WebviewWindow,
    target_w_logical: f64,
    target_h_logical: f64,
) {
    let monitor = win
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| resolve_desktop_lyric_target_monitor(app, win));
    if let (Ok(pos), Some(mon)) = (win.outer_position(), monitor) {
        let sf = mon.scale_factor().max(1.0);
        let wa = mon.work_area();
        let phys_w = (target_w_logical * sf).round() as i32;
        let phys_h = (target_h_logical * sf).round() as i32;
        let min_x = wa.position.x;
        let max_x = (wa.position.x + wa.size.width as i32 - phys_w).max(min_x);
        let min_y = wa.position.y;
        let max_y = (wa.position.y + wa.size.height as i32 - phys_h - (6.0 * sf).round() as i32)
            .max(min_y);
        let clamped_x = pos.x.clamp(min_x, max_x);
        let clamped_y = pos.y.clamp(min_y, max_y);
        if clamped_x != pos.x || clamped_y != pos.y {
            let _ = win.set_position(Position::Physical(PhysicalPosition::new(
                clamped_x, clamped_y,
            )));
        }
    }
}

/// 彻底剥离 Windows 11 透明顶层悬浮窗（桌面歌词）的系统方形阴影、1px 边框与圆角裁切伪影
fn enforce_transparent_overlay_window_style(win: &tauri::WebviewWindow) {
    let _ = win.set_shadow(false);
    #[cfg(target_os = "windows")]
    {
        if let Ok(raw_hwnd) = win.hwnd() {
            let hwnd = raw_hwnd.0 as isize;
            if hwnd != 0 {
                #[link(name = "dwmapi")]
                extern "system" {
                    fn DwmSetWindowAttribute(
                        hwnd: isize,
                        dw_attribute: u32,
                        pv_attribute: *const std::ffi::c_void,
                        cb_attribute: u32,
                    ) -> i32;
                }
                #[link(name = "user32")]
                extern "system" {
                    fn GetWindowLongW(hwnd: isize, n_index: i32) -> i32;
                    fn SetWindowLongW(hwnd: isize, n_index: i32, dw_new_long: i32) -> i32;
                    fn GetClassLongPtrW(hwnd: isize, n_index: i32) -> usize;
                    fn SetClassLongPtrW(hwnd: isize, n_index: i32, dw_new_long: isize) -> usize;
                    fn SetWindowPos(
                        hwnd: isize,
                        hwnd_insert_after: isize,
                        x: i32,
                        y: i32,
                        cx: i32,
                        cy: i32,
                        u_flags: u32,
                    ) -> i32;
                }

                const DWMWA_TRANSITIONS_FORCEDISABLED: u32 = 3;
                const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
                const DWMWA_BORDER_COLOR: u32 = 34;
                const DWMWCP_DONOTROUND: u32 = 1;
                const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;
                const GWL_EXSTYLE: i32 = -20;
                const GCL_STYLE: i32 = -26;
                const WS_EX_TOOLWINDOW: i32 = 0x00000080;
                const WS_EX_APPWINDOW: i32 = 0x00040000;
                const CS_DROPSHADOW: usize = 0x00020000;
                const SWP_NOSIZE: u32 = 0x0001;
                const SWP_NOMOVE: u32 = 0x0002;
                const SWP_NOZORDER: u32 = 0x0004;
                const SWP_NOACTIVATE: u32 = 0x0010;
                const SWP_FRAMECHANGED: u32 = 0x0020;

                unsafe {
                    let disable_trans: u32 = 1;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_TRANSITIONS_FORCEDISABLED,
                        &disable_trans as *const u32 as *const std::ffi::c_void,
                        std::mem::size_of::<u32>() as u32,
                    );
                    let corner_pref: u32 = DWMWCP_DONOTROUND;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_WINDOW_CORNER_PREFERENCE,
                        &corner_pref as *const u32 as *const std::ffi::c_void,
                        std::mem::size_of::<u32>() as u32,
                    );
                    let border_none: u32 = DWMWA_COLOR_NONE;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_BORDER_COLOR,
                        &border_none as *const u32 as *const std::ffi::c_void,
                        std::mem::size_of::<u32>() as u32,
                    );

                    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
                    let next_ex = (ex_style | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW;
                    if next_ex != ex_style {
                        SetWindowLongW(hwnd, GWL_EXSTYLE, next_ex);
                    }

                    let cls_style = GetClassLongPtrW(hwnd, GCL_STYLE);
                    if (cls_style & CS_DROPSHADOW) != 0 {
                        SetClassLongPtrW(hwnd, GCL_STYLE, (cls_style & !CS_DROPSHADOW) as isize);
                    }

                    let _ = SetWindowPos(
                        hwnd,
                        0,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    );
                }
            }
        }
    }
}

#[tauri::command]
async fn set_desktop_lyric_window_state_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    visible: bool,
    locked: bool,
    width: Option<f64>,
    height: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
    reset_position: Option<bool>,
    apply_visibility: Option<bool>,
) -> Result<bool, String> {
    ensure_allowed_windows(&window, &["main", "desktop_lyric"])?;
    let should_apply_visibility = apply_visibility.unwrap_or(true);
    if should_apply_visibility {
        state.desktop_lyric_locked.store(locked, Ordering::Relaxed);
        state.desktop_lyric_visible.store(visible, Ordering::Relaxed);
        wake_hotkey_thread(&state.hotkey_thread_id);
    }

    let target_win = if should_apply_visibility && visible {
        ensure_desktop_lyric_window(&app)
    } else {
        app.get_webview_window("desktop_lyric")
    };

    if let Some(win) = target_win {
        let current_sf = win.scale_factor().unwrap_or(1.0).max(1.0);
        let current_size = win
            .inner_size()
            .ok()
            .map(|sz| (sz.width as f64 / current_sf, sz.height as f64 / current_sf))
            .filter(|(cw, ch)| *cw >= 300.0 && *ch >= 88.0);
        let target_w = width
            .unwrap_or_else(|| current_size.map(|(cw, _)| cw).unwrap_or(820.0))
            .clamp(420.0, 1280.0);
        let target_h = height
            .filter(|&h| h >= 88.0)
            .unwrap_or_else(|| current_size.map(|(_, ch)| ch).unwrap_or(156.0))
            .clamp(88.0, 660.0);

        if should_apply_visibility && !visible {
            // 关闭/隐藏前务必抓取最新物理位置并同步给主窗口持久化，下次打开原位回归
            capture_desktop_lyric_current_pos(&app, &win, &state);
        }

        if width.is_some() || height.is_some() {
            let _ = win.set_size(Size::Logical(LogicalSize::new(target_w, target_h)));
        }

        let phys_w = (target_w * current_sf).round() as i32;
        let phys_h = (target_h * current_sf).round() as i32;

        let saved_pos = state
            .desktop_lyric_last_pos
            .lock()
            .ok()
            .and_then(|g| *g);
        let effective_xy = match (x, y) {
            (Some(lx), Some(ly)) if lx.is_finite() && ly.is_finite() => Some((lx, ly)),
            _ => saved_pos,
        };

        if reset_position.unwrap_or(false) {
            place_desktop_lyric_default(&app, &win, &state, target_w, target_h);
        } else if let Some((lx, ly)) = effective_xy {
            if should_apply_visibility && visible || (x.is_some() && y.is_some()) {
                let px = (lx * current_sf).round() as i32;
                let py = (ly * current_sf).round() as i32;
                if is_rect_visible_on_any_monitor(&win, px, py, phys_w, phys_h) {
                    let _ = win.set_position(Position::Physical(PhysicalPosition::new(px, py)));
                    record_desktop_lyric_pos(&app, &state, lx, ly);
                    clamp_desktop_lyric_within_work_area(&app, &win, target_w, target_h);
                } else {
                    place_desktop_lyric_default(&app, &win, &state, target_w, target_h);
                }
            } else if width.is_some() || height.is_some() {
                clamp_desktop_lyric_within_work_area(&app, &win, target_w, target_h);
            }
        } else if should_apply_visibility
            && visible
            && !state.desktop_lyric_positioned.load(Ordering::Relaxed)
        {
            place_desktop_lyric_default(&app, &win, &state, target_w, target_h);
        } else if width.is_some() || height.is_some() {
            clamp_desktop_lyric_within_work_area(&app, &win, target_w, target_h);
        }

        if should_apply_visibility {
            enforce_transparent_overlay_window_style(&win);
            let _ = win.set_always_on_top(true);
            let _ = win.set_ignore_cursor_events(locked);
            if visible {
                set_subwindow_suspended(&win, false);
                set_webview_memory_target_low(&win, false);
                let _ = win.show();
                enforce_transparent_overlay_window_style(&win);
                let _ = win.set_always_on_top(true);
            } else {
                let _ = win.hide();
                schedule_subwindow_suspend(
                    &app,
                    "desktop_lyric",
                    state.desktop_lyric_visible.clone(),
                );
            }
            let _ = app.emit(
                "bm-desktop-lyric-visibility",
                serde_json::json!({
                    "visible": visible,
                    "locked": locked,
                }),
            );
            if visible {
                if let Some(snap) = state
                    .desktop_lyric_snapshot
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
                {
                    let _ = app.emit_to("desktop_lyric", "bm-desktop-lyric-sync", snap);
                }
                let _ = app.emit_to(
                    "main",
                    "bm-desktop-lyric-cmd",
                    serde_json::json!({ "type": "CMD_REQUEST_SYNC" }),
                );
            }
        }
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
async fn resize_desktop_lyric_window_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    width: f64,
    height: f64,
    anchor_center: Option<bool>,
) -> Result<bool, String> {
    ensure_allowed_windows(&window, &["main", "desktop_lyric"])?;
    if let Some(win) = app.get_webview_window("desktop_lyric") {
        let target_w = width.clamp(440.0, 1280.0);
        let target_h = height.clamp(88.0, 680.0);
        let sf = win.scale_factor().unwrap_or(1.0).max(1.0);
        let phys_w = (target_w * sf).round() as i32;
        let phys_h = (target_h * sf).round() as i32;

        #[cfg(target_os = "windows")]
        {
            if let (Ok(raw_hwnd), Ok(cur_pos), Ok(cur_size)) =
                (win.hwnd(), win.outer_position(), win.outer_size())
            {
                let hwnd = raw_hwnd.0 as isize;
                if hwnd != 0 && cur_pos.x > -10000 && cur_pos.y > -10000 {
                    let mut next_x = if anchor_center.unwrap_or(false) {
                        let center_x = cur_pos.x + (cur_size.width as i32) / 2;
                        center_x - phys_w / 2
                    } else {
                        cur_pos.x
                    };
                    let mut next_y = cur_pos.y;

                    if let Some(monitor) = resolve_desktop_lyric_target_monitor(&app, &win) {
                        let wa = monitor.work_area();
                        let min_x = wa.position.x + 8;
                        let max_x = (wa.position.x + wa.size.width as i32 - phys_w - 8).max(min_x);
                        let min_y = wa.position.y + 8;
                        let max_y = (wa.position.y + wa.size.height as i32 - phys_h - 8).max(min_y);
                        next_x = next_x.clamp(min_x, max_x);
                        next_y = next_y.clamp(min_y, max_y);
                    }

                    #[link(name = "user32")]
                    extern "system" {
                        fn SetWindowPos(
                            hwnd: isize,
                            hwnd_insert_after: isize,
                            x: i32,
                            y: i32,
                            cx: i32,
                            cy: i32,
                            u_flags: u32,
                        ) -> i32;
                    }
                    const SWP_NOZORDER: u32 = 0x0004;
                    const SWP_NOACTIVATE: u32 = 0x0010;
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            0,
                            next_x,
                            next_y,
                            phys_w,
                            phys_h,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                    if anchor_center.unwrap_or(false) {
                        record_desktop_lyric_pos(
                            &app,
                            &state,
                            (next_x as f64 / sf).round(),
                            (next_y as f64 / sf).round(),
                        );
                    }
                    return Ok(true);
                }
            }
        }

        let _ = win.set_size(Size::Logical(LogicalSize::new(target_w, target_h)));
        clamp_desktop_lyric_within_work_area(&app, &win, target_w, target_h);
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
async fn save_desktop_lyric_position_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<(f64, f64)>, String> {
    ensure_allowed_windows(&window, &["main", "desktop_lyric"])?;
    if let Some(win) = app.get_webview_window("desktop_lyric") {
        return Ok(capture_desktop_lyric_current_pos(&app, &win, &state));
    }
    Ok(None)
}

#[tauri::command]
async fn reset_desktop_lyric_position_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    ensure_allowed_windows(&window, &["main", "desktop_lyric"])?;
    Ok(snap_desktop_lyric_position_cmd(window, app, state, "bottom".to_string(), None).await?.is_some())
}

#[tauri::command]
async fn snap_desktop_lyric_position_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    anchor: String,
    base_height: Option<f64>,
) -> Result<Option<(f64, f64)>, String> {
    ensure_allowed_windows(&window, &["main", "desktop_lyric"])?;
    if let Some(win) = app.get_webview_window("desktop_lyric") {
        let sf = win.scale_factor().unwrap_or(1.0).max(1.0);
        let (win_w, win_h) = win
            .inner_size()
            .ok()
            .map(|sz| (sz.width as f64 / sf, sz.height as f64 / sf))
            .filter(|(cw, ch)| *cw >= 300.0 && *ch >= 80.0)
            .unwrap_or((820.0, 176.0));
        let anchor_h = base_height
            .filter(|h| h.is_finite() && *h >= 88.0)
            .map(|h| h.clamp(88.0, 420.0))
            .unwrap_or_else(|| win_h.clamp(88.0, 660.0));
        let normalized = match anchor.as_str() {
            "top" | "center" | "bottom" => anchor.as_str(),
            _ => "bottom",
        };
        let placed = place_desktop_lyric_at_anchor(&app, &win, &state, win_w, anchor_h, normalized);
        return Ok(placed);
    }
    Ok(None)
}

#[tauri::command]
async fn focus_main_window_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    ensure_allowed_windows(&window, &["main", "desktop_lyric", "mini_player"])?;
    if app.get_webview_window("main").is_some() {
        show_and_restore_main_window(&app, &state);
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
async fn push_desktop_lyric_sync_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    payload: serde_json::Value,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    if payload.get("type").and_then(|v| v.as_str()) == Some("STATE_SYNC") {
        if let Some(locked_val) = payload.get("locked").and_then(|v| v.as_bool()) {
            state
                .desktop_lyric_locked
                .store(locked_val, Ordering::Relaxed);
        }
        if let Some(enabled_val) = payload.get("enabled").and_then(|v| v.as_bool()) {
            state
                .desktop_lyric_visible
                .store(enabled_val, Ordering::Relaxed);
        }
        if let Ok(mut guard) = state.desktop_lyric_snapshot.lock() {
            *guard = Some(payload.clone());
        }
    } else if payload.get("type").and_then(|v| v.as_str()) == Some("POSITION_SYNC") {
        if let Ok(mut guard) = state.desktop_lyric_snapshot.lock() {
            if let Some(serde_json::Value::Object(ref mut map)) = *guard {
                let snap_hash = map
                    .get("song")
                    .and_then(|s| s.get("hash"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let pos_hash = payload
                    .get("songHash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if snap_hash.is_empty() || pos_hash.is_empty() || snap_hash == pos_hash {
                    for key in [
                        "positionMs",
                        "durationMs",
                        "isPlaying",
                        "playbackRate",
                        "lyricOffsetMs",
                        "bluetoothDelayMs",
                        "sentAtMs",
                    ] {
                        if let Some(val) = payload.get(key) {
                            map.insert(key.to_string(), val.clone());
                        }
                    }
                }
            }
        }
    }
    let _ = app.emit_to("desktop_lyric", "bm-desktop-lyric-sync", payload);
    Ok(true)
}

#[tauri::command]
async fn pull_desktop_lyric_sync_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<serde_json::Value>, String> {
    ensure_allowed_windows(&window, &["main", "desktop_lyric"])?;
    let _ = app.emit_to(
        "main",
        "bm-desktop-lyric-cmd",
        serde_json::json!({ "type": "CMD_REQUEST_SYNC" }),
    );
    let snapshot = state
        .desktop_lyric_snapshot
        .lock()
        .ok()
        .and_then(|g| g.clone());
    Ok(snapshot)
}

#[tauri::command]
async fn send_desktop_lyric_command_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    command: serde_json::Value,
) -> Result<bool, String> {
    ensure_allowed_windows(&window, &["main", "desktop_lyric"])?;
    if let Some(cmd_type) = command.get("type").and_then(|v| v.as_str()) {
        match cmd_type {
            "CMD_LOCK" => {
                state.desktop_lyric_locked.store(true, Ordering::Relaxed);
                wake_hotkey_thread(&state.hotkey_thread_id);
                if let Some(win) = app.get_webview_window("desktop_lyric") {
                    let _ = win.set_ignore_cursor_events(true);
                }
                let _ = app.emit(
                    "bm-desktop-lyric-lock-sync",
                    serde_json::json!({ "locked": true, "notify": false }),
                );
            }
            "CMD_UNLOCK" => {
                state.desktop_lyric_locked.store(false, Ordering::Relaxed);
                wake_hotkey_thread(&state.hotkey_thread_id);
                if let Some(win) = app.get_webview_window("desktop_lyric") {
                    let _ = win.set_ignore_cursor_events(false);
                }
                let _ = app.emit_to(
                    "desktop_lyric",
                    "bm-desktop-lyric-locked-hover",
                    serde_json::json!({
                        "hoveringWindow": false,
                        "hoveringUnlock": false,
                    }),
                );
                let _ = app.emit(
                    "bm-desktop-lyric-lock-sync",
                    serde_json::json!({ "locked": false, "notify": false }),
                );
            }
            "CMD_CLOSE" => {
                state.desktop_lyric_visible.store(false, Ordering::Relaxed);
                wake_hotkey_thread(&state.hotkey_thread_id);
                if let Some(win) = app.get_webview_window("desktop_lyric") {
                    let _ = win.hide();
                    schedule_subwindow_suspend(
                        &app,
                        "desktop_lyric",
                        state.desktop_lyric_visible.clone(),
                    );
                }
                let locked = state.desktop_lyric_locked.load(Ordering::Relaxed);
                let _ = app.emit(
                    "bm-desktop-lyric-visibility",
                    serde_json::json!({
                        "visible": false,
                        "locked": locked,
                    }),
                );
            }
            _ => {}
        }
    }
    let _ = app.emit_to("main", "bm-desktop-lyric-cmd", command);
    Ok(true)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MiniDragAnchor {
    cursor_x: i32,
    cursor_y: i32,
    win_x: i32,
    win_y: i32,
    win_w: i32,
    win_h: i32,
    scale_factor: f64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MiniDragStepResult {
    dock_edge: String,
    near_edge: String,
    expanded: bool,
    scale_factor: f64,
    win_x: i32,
    win_y: i32,
    win_w: i32,
    win_h: i32,
    grab_offset_x: i32,
    grab_offset_y: i32,
}

fn get_os_cursor_physical_pos() -> Option<(i32, i32)> {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        #[derive(Default)]
        struct WinPoint {
            x: i32,
            y: i32,
        }
        #[link(name = "user32")]
        extern "system" {
            fn GetCursorPos(lp_point: *mut WinPoint) -> i32;
        }
        let mut pt = WinPoint::default();
        if unsafe { GetCursorPos(&mut pt) } != 0 {
            return Some((pt.x, pt.y));
        }
    }
    None
}

fn resolve_mini_player_monitor_at_point(
    app: &AppHandle,
    win: &tauri::WebviewWindow,
    px: i32,
    py: i32,
) -> Option<tauri::Monitor> {
    if let Ok(monitors) = win.available_monitors() {
        for mon in &monitors {
            let wa = mon.work_area();
            let mx1 = wa.position.x;
            let my1 = wa.position.y;
            let mx2 = mx1 + wa.size.width as i32;
            let my2 = my1 + wa.size.height as i32;
            if px >= mx1 && px < mx2 && py >= my1 && py < my2 {
                return Some(mon.clone());
            }
        }
        // Fallback to monitor full rect if point sits in taskbar/margin
        for mon in monitors {
            let pos = mon.position();
            let sz = mon.size();
            let mx1 = pos.x;
            let my1 = pos.y;
            let mx2 = mx1 + sz.width as i32;
            let my2 = my1 + sz.height as i32;
            if px >= mx1 && px < mx2 && py >= my1 && py < my2 {
                return Some(mon);
            }
        }
    }
    win.current_monitor()
        .ok()
        .flatten()
        .or_else(|| {
            app.get_webview_window("main")
                .and_then(|m| m.current_monitor().ok().flatten())
        })
        .or_else(|| win.primary_monitor().ok().flatten())
}

fn mini_logical_dims(_dock_edge: &str, _expanded: bool) -> (f64, f64) {
    // 恒定 480x228 逻辑尺寸舞台（436x184 卡片 + 四周 22px 透明阴影安全留白），
    // 展开/收起全程 0 次 set_size 窗口缩放，配合 16ms 局部热区穿透彻底根除 WebView2 表面错位闪跳
    (480.0, 228.0)
}

fn place_mini_player_default(app: &AppHandle, win: &tauri::WebviewWindow, state: &AppState) {
    let monitor = app
        .get_webview_window("main")
        .and_then(|m| m.current_monitor().ok().flatten())
        .or_else(|| win.current_monitor().ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten());
    if let Some(mon) = monitor {
        let sf = mon.scale_factor().max(1.0);
        let wa = mon.work_area();
        let (lw, lh) = mini_logical_dims("none", true);
        let pw = (lw * sf).round() as i32;
        let ph = (lh * sf).round() as i32;
        let margin = (24.0 * sf).round() as i32;
        let x = (wa.position.x + wa.size.width as i32 - pw - margin).max(wa.position.x + margin);
        let y = (wa.position.y + margin).min(wa.position.y + wa.size.height as i32 - ph - margin);
        let _ = win.set_size(Size::Logical(LogicalSize::new(lw, lh)));
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        state.mini_player_positioned.store(true, Ordering::Relaxed);
    }
}

fn restore_main_from_mini(app: &AppHandle, state: &AppState) {
    state.mini_player_visible.store(false, Ordering::Relaxed);
    wake_hotkey_thread(&state.hotkey_thread_id);
    if let Some(mini_win) = app.get_webview_window("mini_player") {
        let _ = mini_win.hide();
        schedule_subwindow_suspend(app, "mini_player", state.mini_player_visible.clone());
    }
    show_and_restore_main_window(app, state);
    let _ = app.emit(
        "bm-mini-player-visibility",
        serde_json::json!({ "visible": false }),
    );
}

#[tauri::command]
async fn set_mini_player_mode_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    mini: bool,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    if mini {
        state.mini_player_visible.store(true, Ordering::Relaxed);
        wake_hotkey_thread(&state.hotkey_thread_id);
        if let Some(main_win) = app.get_webview_window("main") {
            let is_max = main_win.is_maximized().unwrap_or(false);
            state.main_was_maximized.store(is_max, Ordering::Relaxed);
            sync_main_low_memory_state(&main_win, &state.main_low_memory_active, true);
            let _ = main_win.hide();
        }
        if let Some(mini_win) = ensure_mini_player_window(&app) {
            set_subwindow_suspended(&mini_win, false);
            set_webview_memory_target_low(&mini_win, false);
            if !state.mini_player_positioned.load(Ordering::Relaxed) {
                place_mini_player_default(&app, &mini_win, &state);
            }
            let _ = mini_win.set_always_on_top(true);
            let _ = mini_win.show();
            let _ = mini_win.set_focus();
            if let Some(snap) = state
                .mini_player_snapshot
                .lock()
                .ok()
                .and_then(|g| g.clone())
            {
                let _ = app.emit_to("mini_player", "bm-mini-player-sync", snap);
            }
            let _ = app.emit_to(
                "main",
                "bm-mini-player-cmd",
                serde_json::json!({ "type": "CMD_REQUEST_SYNC" }),
            );
            let _ = app.emit(
                "bm-mini-player-visibility",
                serde_json::json!({ "visible": true }),
            );
            return Ok(true);
        }
        return Ok(false);
    } else {
        restore_main_from_mini(&app, &state);
        return Ok(false);
    }
}

#[tauri::command]
async fn push_mini_player_sync_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    payload: serde_json::Value,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    if payload.get("type").and_then(|v| v.as_str()) == Some("STATE_SYNC") {
        if let Ok(mut guard) = state.mini_player_snapshot.lock() {
            *guard = Some(payload.clone());
        }
    } else if payload.get("type").and_then(|v| v.as_str()) == Some("POSITION_SYNC") {
        if let Ok(mut guard) = state.mini_player_snapshot.lock() {
            if let Some(serde_json::Value::Object(ref mut map)) = *guard {
                for key in [
                    "positionMs",
                    "durationMs",
                    "isPlaying",
                    "playbackRate",
                    "lyricOffsetMs",
                    "bluetoothDelayMs",
                    "volume",
                    "sentAtMs",
                ] {
                    if let Some(val) = payload.get(key) {
                        map.insert(key.to_string(), val.clone());
                    }
                }
            }
        }
    }
    let _ = app.emit_to("mini_player", "bm-mini-player-sync", payload);
    Ok(true)
}

#[tauri::command]
async fn pull_mini_player_sync_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<serde_json::Value>, String> {
    ensure_allowed_windows(&window, &["main", "mini_player"])?;
    let _ = app.emit_to(
        "main",
        "bm-mini-player-cmd",
        serde_json::json!({ "type": "CMD_REQUEST_SYNC" }),
    );
    let snapshot = state
        .mini_player_snapshot
        .lock()
        .ok()
        .and_then(|g| g.clone());
    Ok(snapshot)
}

#[tauri::command]
async fn send_mini_player_command_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    command: serde_json::Value,
) -> Result<bool, String> {
    ensure_allowed_windows(&window, &["main", "mini_player"])?;
    if let Some(cmd_type) = command.get("type").and_then(|v| v.as_str()) {
        match cmd_type {
            "CMD_RESTORE_MAIN" => {
                restore_main_from_mini(&app, &state);
            }
            "CMD_OPEN_NOW_PLAYING" => {
                restore_main_from_mini(&app, &state);
            }
            "CMD_HIDE_TO_TRAY" => {
                state.mini_player_visible.store(false, Ordering::Relaxed);
                wake_hotkey_thread(&state.hotkey_thread_id);
                if let Some(mini_win) = app.get_webview_window("mini_player") {
                    let _ = mini_win.hide();
                    schedule_subwindow_suspend(
                        &app,
                        "mini_player",
                        state.mini_player_visible.clone(),
                    );
                }
                let _ = app.emit(
                    "bm-mini-player-visibility",
                    serde_json::json!({ "visible": false }),
                );
            }
            _ => {}
        }
    }
    let _ = app.emit_to("main", "bm-mini-player-cmd", command);
    Ok(true)
}

#[tauri::command]
async fn get_mini_player_drag_anchor_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Result<Option<MiniDragAnchor>, String> {
    ensure_allowed_windows(&window, &["main", "mini_player"])?;
    let Some(win) = app.get_webview_window("mini_player") else {
        return Ok(None);
    };
    let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else {
        return Ok(None);
    };
    let sf = win.scale_factor().unwrap_or(1.0).max(1.0);
    let (cx, cy) = get_os_cursor_physical_pos().unwrap_or((pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2));
    Ok(Some(MiniDragAnchor {
        cursor_x: cx,
        cursor_y: cy,
        win_x: pos.x,
        win_y: pos.y,
        win_w: size.width as i32,
        win_h: size.height as i32,
        scale_factor: sf,
    }))
}

#[tauri::command]
async fn drag_mini_player_step_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    grab_offset_x: i32,
    grab_offset_y: i32,
    dock_edge: String,
    expanded: bool,
) -> Result<Option<MiniDragStepResult>, String> {
    ensure_allowed_windows(&window, &["main", "mini_player"])?;
    let Some(win) = app.get_webview_window("mini_player") else {
        return Ok(None);
    };
    let Some((cx, cy)) = get_os_cursor_physical_pos() else {
        return Ok(None);
    };
    let Some(mon) = resolve_mini_player_monitor_at_point(&app, &win, cx, cy) else {
        return Ok(None);
    };
    let sf = mon.scale_factor().max(1.0);
    let wa = mon.work_area();
    let wa_l = wa.position.x;
    let wa_t = wa.position.y;
    let wa_r = wa_l + wa.size.width as i32;
    let wa_b = wa_t + wa.size.height as i32;

    let mut next_edge = dock_edge.clone();
    let mut next_expanded = expanded;
    let mut next_grab_x = grab_offset_x;
    let mut next_grab_y = grab_offset_y;

    let pull_off_thresh = (30.0 * sf).round() as i32;
    let snap_thresh = (20.0 * sf).round() as i32;
    let card_pad = (22.0 * sf).round() as i32;

    // 若当前处于停靠收起态胶囊，检测是否拖离所属屏幕边缘 -> 自动在手下变成自由悬浮卡片
    if next_edge != "none" && !next_expanded {
        let pulled_away = match next_edge.as_str() {
            "top" => cy - wa_t > (36.0 * sf).round() as i32 + pull_off_thresh,
            "bottom" => wa_b - cy > (36.0 * sf).round() as i32 + pull_off_thresh,
            "left" => cx - wa_l > (50.0 * sf).round() as i32 + pull_off_thresh,
            "right" => wa_r - cx > (50.0 * sf).round() as i32 + pull_off_thresh,
            _ => false,
        };
        if pulled_away {
            let prev_edge = next_edge.clone();
            next_edge = "none".to_string();
            next_expanded = true;
            state.mini_player_expanded.store(true, Ordering::Relaxed);
            wake_hotkey_thread(&state.hotkey_thread_id);
            if let Ok(mut g) = state.mini_player_dock_edge.lock() {
                *g = "none".to_string();
            }
            let _ = win.set_ignore_cursor_events(false);
            let (fw, fh) = mini_logical_dims("none", true);
            let pw = (fw * sf).round() as i32;
            let ph = (fh * sf).round() as i32;
            match prev_edge.as_str() {
                "top" => {
                    next_grab_y = grab_offset_y.max((32.0 * sf).round() as i32);
                }
                "bottom" => {
                    next_grab_y = grab_offset_y.min(ph - (32.0 * sf).round() as i32);
                }
                "left" => {
                    next_grab_x = grab_offset_x.max((56.0 * sf).round() as i32);
                }
                "right" => {
                    next_grab_x = grab_offset_x.min(pw - (56.0 * sf).round() as i32);
                }
                _ => {}
            }
            let tx = (cx - next_grab_x).clamp(wa_l - card_pad, (wa_r - pw + card_pad).max(wa_l - card_pad));
            let ty = (cy - next_grab_y).clamp(wa_t - card_pad, (wa_b - ph + card_pad).max(wa_t - card_pad));
            let _ = win.set_position(Position::Physical(PhysicalPosition::new(tx, ty)));
            state.mini_player_positioned.store(true, Ordering::Relaxed);
            return Ok(Some(MiniDragStepResult {
                dock_edge: next_edge,
                near_edge: "none".to_string(),
                expanded: next_expanded,
                scale_factor: sf,
                win_x: tx,
                win_y: ty,
                win_w: pw,
                win_h: ph,
                grab_offset_x: next_grab_x,
                grab_offset_y: next_grab_y,
            }));
        }

        // 仍在边缘阈值内：沿当前停靠边缘顺滑滑动（恒定 480x228 舞台）
        let (cw, ch) = mini_logical_dims(&next_edge, false);
        let pw = (cw * sf).round() as i32;
        let ph = (ch * sf).round() as i32;
        let (tx, ty) = match next_edge.as_str() {
            "top" => (
                (cx - next_grab_x).clamp(wa_l - card_pad, (wa_r - pw + card_pad).max(wa_l - card_pad)),
                wa_t,
            ),
            "bottom" => (
                (cx - next_grab_x).clamp(wa_l - card_pad, (wa_r - pw + card_pad).max(wa_l - card_pad)),
                (wa_b - ph).max(wa_t),
            ),
            "left" => (
                wa_l,
                (cy - next_grab_y).clamp(wa_t - card_pad, (wa_b - ph + card_pad).max(wa_t - card_pad)),
            ),
            "right" => (
                (wa_r - pw).max(wa_l),
                (cy - next_grab_y).clamp(wa_t - card_pad, (wa_b - ph + card_pad).max(wa_t - card_pad)),
            ),
            _ => (cx - next_grab_x, cy - next_grab_y),
        };
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(tx, ty)));
        state.mini_player_positioned.store(true, Ordering::Relaxed);
        return Ok(Some(MiniDragStepResult {
            dock_edge: next_edge,
            near_edge: "none".to_string(),
            expanded: false,
            scale_factor: sf,
            win_x: tx,
            win_y: ty,
            win_w: pw,
            win_h: ph,
            grab_offset_x: next_grab_x,
            grab_offset_y: next_grab_y,
        }));
    }

    // 展开态或自由悬浮态拖动：若原本处于停靠展开态，一旦拖动超过边缘直接解除停靠
    let (fw, fh) = mini_logical_dims("none", true);
    let pw = (fw * sf).round() as i32;
    let ph = (fh * sf).round() as i32;
    let raw_x = cx - next_grab_x;
    let raw_y = cy - next_grab_y;

    let min_x = wa_l - card_pad;
    let max_x = (wa_r - pw + card_pad).max(min_x);
    let min_y = wa_t - card_pad;
    let max_y = (wa_b - ph + card_pad).max(min_y);
    let tx = raw_x.clamp(min_x, max_x);
    let ty = raw_y.clamp(min_y, max_y);

    // 卡片实体边缘（扣除透明阴影留白）
    let card_left = tx + card_pad;
    let card_top = ty + card_pad;
    let card_right = tx + pw - card_pad;
    let card_bottom = ty + ph - card_pad;

    let dist_top = card_top - wa_t;
    let dist_bottom = wa_b - card_bottom;
    let dist_left = card_left - wa_l;
    let dist_right = wa_r - card_right;

    let mut near_edge = "none".to_string();
    let mut best_dist = snap_thresh + 1;
    for (edge_name, dist) in [
        ("top", dist_top),
        ("bottom", dist_bottom),
        ("left", dist_left),
        ("right", dist_right),
    ] {
        if dist <= snap_thresh && dist < best_dist {
            best_dist = dist;
            near_edge = edge_name.to_string();
        }
    }

    if next_edge != "none" && near_edge != next_edge {
        next_edge = "none".to_string();
    }
    state.mini_player_expanded.store(true, Ordering::Relaxed);
    wake_hotkey_thread(&state.hotkey_thread_id);
    if let Ok(mut g) = state.mini_player_dock_edge.lock() {
        *g = next_edge.clone();
    }

    let _ = win.set_position(Position::Physical(PhysicalPosition::new(tx, ty)));
    state.mini_player_positioned.store(true, Ordering::Relaxed);
    Ok(Some(MiniDragStepResult {
        dock_edge: next_edge,
        near_edge,
        expanded: true,
        scale_factor: sf,
        win_x: tx,
        win_y: ty,
        win_w: pw,
        win_h: ph,
        grab_offset_x: next_grab_x,
        grab_offset_y: next_grab_y,
    }))
}

#[tauri::command]
async fn snap_mini_player_to_edge_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    edge: String,
) -> Result<bool, String> {
    ensure_allowed_windows(&window, &["main", "mini_player"])?;
    if let Ok(mut g) = state.mini_player_dock_edge.lock() {
        *g = edge.clone();
    }
    state.mini_player_expanded.store(false, Ordering::Relaxed);
    wake_hotkey_thread(&state.hotkey_thread_id);

    let Some(win) = app.get_webview_window("mini_player") else {
        return Ok(false);
    };
    let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else {
        return Ok(false);
    };
    let center_x = pos.x + size.width as i32 / 2;
    let center_y = pos.y + size.height as i32 / 2;
    let Some(mon) = resolve_mini_player_monitor_at_point(&app, &win, center_x, center_y) else {
        return Ok(false);
    };
    let sf = mon.scale_factor().max(1.0);
    let wa = mon.work_area();
    let wa_l = wa.position.x;
    let wa_t = wa.position.y;
    let wa_r = wa_l + wa.size.width as i32;
    let wa_b = wa_t + wa.size.height as i32;

    let (fw, fh) = mini_logical_dims(&edge, false);
    let pw = (fw * sf).round() as i32;
    let ph = (fh * sf).round() as i32;
    let card_pad = (22.0 * sf).round() as i32;
    let (tx, ty) = match edge.as_str() {
        "top" => (
            (center_x - pw / 2).clamp(wa_l - card_pad, (wa_r - pw + card_pad).max(wa_l - card_pad)),
            wa_t,
        ),
        "bottom" => (
            (center_x - pw / 2).clamp(wa_l - card_pad, (wa_r - pw + card_pad).max(wa_l - card_pad)),
            (wa_b - ph).max(wa_t),
        ),
        "left" => (
            wa_l,
            (center_y - ph / 2).clamp(wa_t - card_pad, (wa_b - ph + card_pad).max(wa_t - card_pad)),
        ),
        "right" => (
            (wa_r - pw).max(wa_l),
            (center_y - ph / 2).clamp(wa_t - card_pad, (wa_b - ph + card_pad).max(wa_t - card_pad)),
        ),
        _ => return Ok(false),
    };
    if (size.width as i32 - pw).abs() > 2 || (size.height as i32 - ph).abs() > 2 {
        let _ = win.set_size(Size::Logical(LogicalSize::new(fw, fh)));
    }
    let dx = tx - pos.x;
    let dy = ty - pos.y;
    let dist = ((dx * dx + dy * dy) as f64).sqrt();
    if dist > 1.0 && dist <= 64.0 * sf {
        let win_clone = win.clone();
        let sx = pos.x;
        let sy = pos.y;
        tauri::async_runtime::spawn(async move {
            let steps = 9;
            for i in 1..=steps {
                let t = i as f64 / steps as f64;
                let eased = 1.0 - (1.0 - t).powi(3);
                let cx_pos = sx + (dx as f64 * eased).round() as i32;
                let cy_pos = sy + (dy as f64 * eased).round() as i32;
                let _ = win_clone.set_position(Position::Physical(PhysicalPosition::new(cx_pos, cy_pos)));
                tokio::time::sleep(std::time::Duration::from_millis(12)).await;
            }
        });
    } else if dist > 1.0 {
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(tx, ty)));
    }
    let _ = win.set_always_on_top(true);
    state.mini_player_positioned.store(true, Ordering::Relaxed);
    Ok(true)
}

#[tauri::command]
async fn expand_mini_player_from_dock_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    edge: String,
) -> Result<bool, String> {
    ensure_allowed_windows(&window, &["main", "mini_player"])?;
    if let Ok(mut g) = state.mini_player_dock_edge.lock() {
        *g = edge.clone();
    }
    state.mini_player_expanded.store(true, Ordering::Relaxed);
    wake_hotkey_thread(&state.hotkey_thread_id);

    let Some(win) = app.get_webview_window("mini_player") else {
        return Ok(false);
    };
    let _ = win.set_ignore_cursor_events(false);
    let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else {
        return Ok(false);
    };
    let center_x = pos.x + size.width as i32 / 2;
    let center_y = pos.y + size.height as i32 / 2;
    let Some(mon) = resolve_mini_player_monitor_at_point(&app, &win, center_x, center_y) else {
        return Ok(false);
    };
    let sf = mon.scale_factor().max(1.0);
    let wa = mon.work_area();
    let wa_l = wa.position.x;
    let wa_t = wa.position.y;
    let wa_r = wa_l + wa.size.width as i32;
    let wa_b = wa_t + wa.size.height as i32;

    let (fw, fh) = mini_logical_dims("none", true);
    let pw = (fw * sf).round() as i32;
    let ph = (fh * sf).round() as i32;
    let card_pad = (22.0 * sf).round() as i32;

    let (tx, ty) = match edge.as_str() {
        "top" => (
            (center_x - pw / 2).clamp(wa_l - card_pad, (wa_r - pw + card_pad).max(wa_l - card_pad)),
            wa_t,
        ),
        "bottom" => (
            (center_x - pw / 2).clamp(wa_l - card_pad, (wa_r - pw + card_pad).max(wa_l - card_pad)),
            (wa_b - ph).max(wa_t),
        ),
        "left" => (
            wa_l,
            (center_y - ph / 2).clamp(wa_t - card_pad, (wa_b - ph + card_pad).max(wa_t - card_pad)),
        ),
        "right" => (
            (wa_r - pw).max(wa_l),
            (center_y - ph / 2).clamp(wa_t - card_pad, (wa_b - ph + card_pad).max(wa_t - card_pad)),
        ),
        _ => (pos.x, pos.y),
    };
    if (size.width as i32 - pw).abs() > 2 || (size.height as i32 - ph).abs() > 2 {
        let _ = win.set_size(Size::Logical(LogicalSize::new(fw, fh)));
    }
    if (pos.x - tx).abs() > 2 || (pos.y - ty).abs() > 2 {
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(tx, ty)));
    }
    let _ = win.set_always_on_top(true);
    let _ = win.set_focus();
    Ok(true)
}

#[tauri::command]
async fn collapse_mini_player_to_dock_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    edge: String,
) -> Result<bool, String> {
    ensure_allowed_windows(&window, &["main", "mini_player"])?;
    if let Ok(mut g) = state.mini_player_dock_edge.lock() {
        *g = edge;
    }
    state.mini_player_expanded.store(false, Ordering::Relaxed);
    wake_hotkey_thread(&state.hotkey_thread_id);
    if let Some(win) = app.get_webview_window("mini_player") {
        let _ = win.set_always_on_top(true);
    }
    Ok(true)
}

#[tauri::command]
async fn set_close_to_tray_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state.close_to_tray.store(enabled, Ordering::Relaxed);
    Ok(enabled)
}

#[tauri::command]
async fn set_global_hotkeys_enabled_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state.global_hotkeys_enabled.store(enabled, Ordering::Relaxed);
    state
        .global_hotkeys_revision
        .fetch_add(1, Ordering::Relaxed);
    wake_hotkey_thread(&state.hotkey_thread_id);
    Ok(enabled)
}

#[tauri::command]
async fn update_global_hotkeys_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    bindings: Vec<GlobalHotkeyBinding>,
) -> Result<GlobalHotkeySyncReport, String> {
    ensure_main_window(&window)?;
    const ALLOWED_ACTIONS: [&str; 9] = [
        "toggle_play",
        "prev_song",
        "next_song",
        "toggle_desktop_lyric",
        "toggle_lyric_lock",
        "toggle_mini_mode",
        "volume_up",
        "volume_down",
        "toggle_like",
    ];

    let sanitized: Vec<GlobalHotkeyBinding> = bindings
        .into_iter()
        .filter(|b| ALLOWED_ACTIONS.contains(&b.action.as_str()))
        .map(|b| GlobalHotkeyBinding {
            action: b.action,
            accelerator: b.accelerator.trim().chars().take(48).collect(),
        })
        .collect();

    if let Ok(mut guard) = state.global_hotkey_bindings.lock() {
        *guard = sanitized;
    }
    state
        .global_hotkeys_revision
        .fetch_add(1, Ordering::Relaxed);
    wake_hotkey_thread(&state.hotkey_thread_id);

    // 稍候热键线程唤醒并完成 RegisterHotKey 收集真实冲突结果
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let enabled = state.global_hotkeys_enabled.load(Ordering::Relaxed);
    let failed_actions = state
        .global_hotkey_failed
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let total_non_empty = state
        .global_hotkey_bindings
        .lock()
        .map(|g| {
            g.iter()
                .filter(|b| parse_win32_accelerator(&b.accelerator).is_some())
                .count()
        })
        .unwrap_or(0);
    let registered_count = if enabled {
        total_non_empty.saturating_sub(failed_actions.len())
    } else {
        0
    };

    Ok(GlobalHotkeySyncReport {
        enabled,
        registered_count,
        failed_actions,
    })
}

#[tauri::command]
async fn set_audio_cache_quota_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    max_quota_gb: f64,
) -> Result<CacheDiskStats, String> {
    ensure_main_window(&window)?;
    let clamped_gb = if !max_quota_gb.is_finite() || max_quota_gb <= 0.0 {
        0.0
    } else {
        max_quota_gb.clamp(0.5, 64.0)
    };
    let max_bytes = if clamped_gb <= 0.0 {
        0u64
    } else {
        (clamped_gb * 1024.0 * 1024.0 * 1024.0).round() as u64
    };
    state.api.audio_proxy.set_max_cache_bytes(max_bytes);
    state.api.get_cache_stats()
}

fn resolve_safe_explorer_target_dir(
    store: &AppStore,
    raw_path: &str,
) -> Result<std::path::PathBuf, String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() || trimmed.chars().any(|c| c.is_control()) {
        return Err("无效的目录路径".to_string());
    }

    let install_root = kugou::store::resolve_install_root_dir();
    let default_cache = kugou::store::resolve_cache_dir();
    let default_downloads = kugou::store::resolve_default_download_dir();
    let default_lyrics = default_downloads.join("Lyrics");
    let default_fonts = kugou::store::resolve_custom_fonts_dir();

    let candidate = match trimmed.to_ascii_lowercase().as_str() {
        "cache" => default_cache.clone(),
        "downloads" => default_downloads.clone(),
        "lyrics" => {
            let _ = std::fs::create_dir_all(&default_lyrics);
            default_lyrics.clone()
        }
        "fonts" => default_fonts.clone(),
        "data_root" | "root" => install_root.clone(),
        _ => {
            let p = std::path::PathBuf::from(trimmed);
            if p.components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err("安全策略拒绝包含上级目录穿越的路径".to_string());
            }
            if p.is_relative() {
                install_root.join(p)
            } else {
                p
            }
        }
    };

    let clean_norm = |p: &std::path::Path| -> String {
        let s = p.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
        s.strip_prefix(r"\\?\")
            .unwrap_or(&s)
            .trim_end_matches('\\')
            .to_string()
    };

    let cand_norm = clean_norm(&candidate);
    let root_norm = clean_norm(&install_root);
    let is_app_owned = cand_norm == root_norm
        || cand_norm.starts_with(&format!(r"{}\", root_norm));

    if !is_app_owned
        && !store.is_user_authorized_dir(&candidate)
        && !kugou::store::is_safe_user_write_dir(&candidate)
    {
        return Err("安全策略拒绝打开受保护系统目录或未授权路径".to_string());
    }

    // 若目标目录尚不存在但属于默认下载/歌词目录或安全用户目录，尝试创建或回退至已存在父目录
    let final_dir = if candidate.exists() {
        if candidate.is_file() {
            candidate
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(candidate)
        } else {
            candidate
        }
    } else if cand_norm == clean_norm(&default_downloads)
        || cand_norm == clean_norm(&default_lyrics)
        || cand_norm == clean_norm(&default_cache)
    {
        let _ = std::fs::create_dir_all(&candidate);
        candidate
    } else {
        let mut cur = candidate.as_path();
        let mut found = None;
        while let Some(parent) = cur.parent() {
            if parent.exists()
                && (clean_norm(parent) == root_norm
                    || kugou::store::is_safe_user_write_dir(parent))
            {
                found = Some(parent.to_path_buf());
                break;
            }
            cur = parent;
        }
        found.ok_or_else(|| "目标目录尚未创建，请先选择或下载文件后再打开".to_string())?
    };

    Ok(final_dir)
}

#[tauri::command]
async fn open_path_in_explorer_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    path: String,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    let target_dir = resolve_safe_explorer_target_dir(&state.store, &path)?;
    let canonical = std::fs::canonicalize(&target_dir).unwrap_or(target_dir);
    let raw_str = canonical.to_string_lossy().to_string();
    let clean_str = raw_str.strip_prefix(r"\\?\").unwrap_or(&raw_str).to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(&clean_str)
            .spawn()
            .map_err(|e| format!("无法打开资源管理器: {}", e))?;
        return Ok(true);
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&clean_str)
            .spawn()
            .map_err(|e| format!("无法打开访达: {}", e))?;
        return Ok(true);
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&clean_str)
            .spawn()
            .map_err(|e| format!("无法打开文件管理器: {}", e))?;
        return Ok(true);
    }
}

/// 在系统资源管理器中打开目录并精准高亮选中指定文件（explorer.exe /select,<path>）
#[tauri::command]
async fn reveal_path_in_explorer_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    path: String,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.chars().any(|c| c.is_control()) {
        return Err("无效的文件路径".to_string());
    }
    let raw_p = std::path::PathBuf::from(trimmed);
    if raw_p
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("安全策略拒绝包含上级目录穿越的路径".to_string());
    }
    let parent_dir = resolve_safe_explorer_target_dir(&state.store, trimmed)?;
    let target = if raw_p.is_file() {
        std::fs::canonicalize(&raw_p).unwrap_or(raw_p)
    } else {
        std::fs::canonicalize(&parent_dir).unwrap_or(parent_dir)
    };
    let raw_str = target.to_string_lossy().to_string();
    let clean_str = raw_str.strip_prefix(r"\\?\").unwrap_or(&raw_str).to_string();

    #[cfg(target_os = "windows")]
    {
        if target.is_file() {
            std::process::Command::new("explorer.exe")
                .arg(format!("/select,{}", clean_str))
                .spawn()
                .map_err(|e| format!("无法在资源管理器中定位文件: {}", e))?;
        } else {
            std::process::Command::new("explorer.exe")
                .arg(&clean_str)
                .spawn()
                .map_err(|e| format!("无法打开资源管理器: {}", e))?;
        }
        return Ok(true);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = clean_str;
        Ok(false)
    }
}

/// 枚举电脑已安装系统字体与用户导入自定义字体（纯 Rust 解析 OpenType/TTF/TTC 表，含中英文名、真实字重与 CJK 检测）
#[tauri::command]
async fn list_installed_fonts_cmd(
    window: tauri::WebviewWindow,
) -> Result<InstalledFontsPayload, String> {
    ensure_main_window(&window)?;
    tokio::task::spawn_blocking(kugou::fonts::list_installed_fonts)
        .await
        .map_err(|e| format!("扫描系统字体失败: {}", e))
}

/// 打开系统原生多选文件对话框选择本地字体文件（.ttf / .otf / .woff2 / .woff / .ttc / .otc）
#[tauri::command]
async fn pick_font_files_dialog_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    ensure_main_window(&window)?;
    let paths = tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            const STATIC_PS_SCRIPT: &str = "\
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
                Add-Type -AssemblyName System.Windows.Forms; \
                $dlg = New-Object System.Windows.Forms.OpenFileDialog; \
                $dlg.Title = '选择要导入的字体文件 (支持多选)'; \
                $dlg.Filter = '字体文件 (*.ttf;*.otf;*.woff2;*.woff;*.ttc;*.otc)|*.ttf;*.otf;*.woff2;*.woff;*.ttc;*.otc|所有文件 (*.*)|*.*'; \
                $dlg.Multiselect = $true; \
                if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { \
                    foreach ($f in $dlg.FileNames) { Write-Output $f } \
                }";
            let output = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", STATIC_PS_SCRIPT])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("无法拉起系统字体文件选择器: {}", e))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let paths: Vec<String> = stdout
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            return Ok(paths);
        }

        #[cfg(not(target_os = "windows"))]
        {
            Ok(Vec::new())
        }
    })
    .await
    .map_err(|e| e.to_string())??;

    // SEC-FS-FONT: 记录本次原生系统对话框选中的合法字体路径白名单，防止任意文件读取
    if !paths.is_empty() {
        if let Ok(mut guard) = state.authorized_font_import_paths.lock() {
            if guard.len() > 256 {
                guard.clear();
            }
            for p in &paths {
                guard.insert(normalize_font_whitelist_key(p));
            }
        }
    }
    Ok(paths)
}

/// 导入用户手动选择的外部字体文件并持久化至 `<数据根目录>/fonts/`（自动拆包 .ttc，严格校验原生选择器白名单）
#[tauri::command]
async fn import_custom_font_files_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<CustomFontRecord>, String> {
    ensure_main_window(&window)?;
    let authorized_paths: Vec<String> = {
        let mut guard = state
            .authorized_font_import_paths
            .lock()
            .map_err(|_| "字体导入授权状态锁定失败".to_string())?;
        let mut allowed = Vec::new();
        for p in paths {
            let key = normalize_font_whitelist_key(&p);
            if guard.remove(&key) {
                allowed.push(p);
            } else {
                return Err("安全策略拒绝：仅允许导入经由系统文件选择器授权的字体文件".to_string());
            }
        }
        allowed
    };
    tokio::task::spawn_blocking(move || kugou::fonts::import_custom_font_files(&authorized_paths))
        .await
        .map_err(|e| format!("导入字体失败: {}", e))?
}

/// 读取已导入自定义字体的 Base64 二进制数据（允许主窗口、桌面歌词与迷你播放器窗口调用，仅限清单内已注册 font_id）
#[tauri::command]
async fn read_custom_font_binary_cmd(
    window: tauri::WebviewWindow,
    font_id: String,
) -> Result<CustomFontBinaryPayload, String> {
    let label = window.label();
    if label != "main" && label != "desktop_lyric" && label != "mini_player" {
        return Err("拒绝来自未授权窗口的字体读取请求".to_string());
    }
    tokio::task::spawn_blocking(move || kugou::fonts::read_custom_font_binary(&font_id))
        .await
        .map_err(|e| format!("读取字体数据失败: {}", e))?
}

/// 卸载并删除指定的已导入自定义字体
#[tauri::command]
async fn delete_custom_font_cmd(
    window: tauri::WebviewWindow,
    font_id: String,
) -> Result<Vec<CustomFontRecord>, String> {
    ensure_main_window(&window)?;
    tokio::task::spawn_blocking(move || kugou::fonts::delete_custom_font(&font_id))
        .await
        .map_err(|e| format!("删除自定义字体失败: {}", e))?
}

#[cfg(target_os = "windows")]
mod win_autostart {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    type Hkey = isize;
    // Windows SDK <winreg.h>: #define HKEY_CURRENT_USER ((HKEY)(ULONG_PTR)((LONG)0x80000001))
    const HKEY_CURRENT_USER: Hkey = 0x8000_0001_u32 as i32 as isize;
    const KEY_READ: u32 = 0x20019;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_SZ: u32 = 1;
    const ERROR_SUCCESS: i32 = 0;
    const ERROR_FILE_NOT_FOUND: i32 = 2;

    const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APP_VALUE_NAME: &str = "BreezeMusic";

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            h_key: Hkey,
            lp_sub_key: *const u16,
            ul_options: u32,
            sam_desired: u32,
            phk_result: *mut Hkey,
        ) -> i32;
        fn RegCreateKeyExW(
            h_key: Hkey,
            lp_sub_key: *const u16,
            reserved: u32,
            lp_class: *const u16,
            dw_options: u32,
            sam_desired: u32,
            lp_security_attributes: *const std::ffi::c_void,
            phk_result: *mut Hkey,
            lpdw_disposition: *mut u32,
        ) -> i32;
        fn RegQueryValueExW(
            h_key: Hkey,
            lp_value_name: *const u16,
            lp_reserved: *mut u32,
            lp_type: *mut u32,
            lp_data: *mut u8,
            lpcb_data: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            h_key: Hkey,
            lp_value_name: *const u16,
            reserved: u32,
            dw_type: u32,
            lp_data: *const u8,
            cb_data: u32,
        ) -> i32;
        fn RegDeleteValueW(h_key: Hkey, lp_value_name: *const u16) -> i32;
        fn RegCloseKey(h_key: Hkey) -> i32;
    }

    fn to_wide_nul(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    pub fn is_enabled() -> Result<bool, String> {
        let subkey = to_wide_nul(RUN_SUBKEY);
        let value_name = to_wide_nul(APP_VALUE_NAME);
        let mut hkey: Hkey = 0;
        let open_rc = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_READ,
                &mut hkey,
            )
        };
        if open_rc == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        if open_rc != ERROR_SUCCESS {
            return Err(format!("读取开机自启动配置失败 (code={})", open_rc));
        }

        let mut reg_type: u32 = 0;
        let mut data_len: u32 = 0;
        let query_rc = unsafe {
            RegQueryValueExW(
                hkey,
                value_name.as_ptr(),
                ptr::null_mut(),
                &mut reg_type,
                ptr::null_mut(),
                &mut data_len,
            )
        };
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        Ok(query_rc == ERROR_SUCCESS && data_len > 2)
    }

    pub fn set_enabled(enabled: bool) -> Result<bool, String> {
        let subkey = to_wide_nul(RUN_SUBKEY);
        let value_name = to_wide_nul(APP_VALUE_NAME);
        let mut hkey: Hkey = 0;
        let mut disp: u32 = 0;
        let open_rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                ptr::null(),
                0,
                KEY_SET_VALUE | KEY_READ,
                ptr::null(),
                &mut hkey,
                &mut disp,
            )
        };
        if open_rc != ERROR_SUCCESS {
            return Err(format!("无法访问开机自启动注册表项 (code={})", open_rc));
        }

        let op_res = if enabled {
            let exe_path = std::env::current_exe()
                .map_err(|e| format!("无法获取当前程序路径: {}", e))?;
            let raw = exe_path.to_string_lossy().to_string();
            let clean = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
            let cmd = format!("\"{}\"", clean);
            let wide = to_wide_nul(&cmd);
            let byte_len = (wide.len() * std::mem::size_of::<u16>()) as u32;
            let rc = unsafe {
                RegSetValueExW(
                    hkey,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    wide.as_ptr() as *const u8,
                    byte_len,
                )
            };
            if rc == ERROR_SUCCESS {
                Ok(true)
            } else {
                Err(format!("写入开机自启动项失败 (code={})", rc))
            }
        } else {
            let rc = unsafe { RegDeleteValueW(hkey, value_name.as_ptr()) };
            if rc == ERROR_SUCCESS || rc == ERROR_FILE_NOT_FOUND {
                Ok(false)
            } else {
                Err(format!("移除开机自启动项失败 (code={})", rc))
            }
        };

        unsafe {
            let _ = RegCloseKey(hkey);
        }
        op_res
    }
}

#[tauri::command]
async fn get_autostart_enabled_cmd(window: tauri::WebviewWindow) -> Result<bool, String> {
    ensure_main_window(&window)?;
    #[cfg(target_os = "windows")]
    {
        return win_autostart::is_enabled();
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

#[tauri::command]
async fn set_autostart_enabled_cmd(
    window: tauri::WebviewWindow,
    enabled: bool,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    #[cfg(target_os = "windows")]
    {
        return win_autostart::set_enabled(enabled);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Ok(false)
    }
}

#[tauri::command]
async fn pick_folder_dialog_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    initial_dir: Option<String>,
    title: Option<String>,
) -> Result<Option<String>, String> {
    ensure_main_window(&window)?;
    let prompt_title: String = title
        .unwrap_or_else(|| "选择保存目录".to_string())
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    let init_path: String = initial_dir
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(260)
        .collect();

    let picked_opt = tokio::task::spawn_blocking(move || -> Result<Option<String>, String> {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            // SEC-01: 使用 100% 静态常量 PowerShell 脚本并通过进程环境变量传参，彻底根除脚本字符串拼接与 Unicode 引号逃逸注入
            const STATIC_PS_SCRIPT: &str = "\
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
                Add-Type -AssemblyName System.Windows.Forms; \
                $dlg = New-Object System.Windows.Forms.FolderBrowserDialog; \
                $dlg.Description = [string]$env:BM_DLG_TITLE; \
                $dlg.ShowNewFolderButton = $true; \
                $initDir = [string]$env:BM_DLG_INIT; \
                if ($initDir -ne '' -and (Test-Path -LiteralPath $initDir)) { $dlg.SelectedPath = $initDir }; \
                if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $dlg.SelectedPath }";
            let output = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-STA",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    STATIC_PS_SCRIPT,
                ])
                .env("BM_DLG_TITLE", &prompt_title)
                .env("BM_DLG_INIT", &init_path)
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("无法唤起系统目录选择框: {}", e))?;
            let chosen = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if chosen.is_empty() {
                return Ok(None);
            }
            return Ok(Some(chosen));
        }

        #[cfg(target_os = "macos")]
        {
            // SEC-01: 使用静态 AppleScript 从环境变量读取标题，消除反斜杠与双引号拼接逃逸
            const STATIC_OSA_SCRIPT: &str =
                "POSIX path of (choose folder with prompt (system attribute \"BM_DLG_TITLE\"))";
            let output = std::process::Command::new("osascript")
                .args(["-e", STATIC_OSA_SCRIPT])
                .env("BM_DLG_TITLE", &prompt_title)
                .output()
                .map_err(|e| format!("无法唤起 macOS 目录选择框: {}", e))?;
            let chosen = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if chosen.is_empty() {
                return Ok(None);
            }
            return Ok(Some(chosen));
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            let output = std::process::Command::new("zenity")
                .args([
                    "--file-selection",
                    "--directory",
                    "--title",
                    &prompt_title,
                    "--filename",
                    &init_path,
                ])
                .output();
            if let Ok(out) = output {
                let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !chosen.is_empty() {
                    return Ok(Some(chosen));
                }
            }
            Ok(None)
        }
    })
    .await
    .map_err(|e| format!("目录选择任务异常: {}", e))??;

    if let Some(ref chosen_dir) = picked_opt {
        let _ = state
            .store
            .authorize_local_music_dir(std::path::Path::new(chosen_dir));
    }
    Ok(picked_opt)
}

#[tauri::command]
async fn report_playback_telemetry_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    is_start: bool,
    played_duration_ms: Option<i64>,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    state
        .api
        .report_playback_telemetry(&song, is_start, played_duration_ms.unwrap_or(0))
        .await
}

#[tauri::command]
async fn get_viper_effects_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<ViperEffectItem>, String> {
    ensure_main_window(&window)?;
    state.api.fetch_viper_effects().await
}

#[tauri::command]
async fn get_song_lyric(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
) -> Result<LyricData, String> {
    ensure_main_window(&window)?;
    state.api.fetch_song_lyric(&song).await
}

/// 封面字节原生安全拉取命令：绕过 WebView2 跨域 CORS / 重定向画布污染限制，保障封面像素 100% 可分析
#[tauri::command]
async fn fetch_cover_image_base64_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    url: String,
) -> Result<String, String> {
    ensure_main_window(&window)?;
    state.api.transport.fetch_cover_image_base64(&url).await
}

fn decode_image_data_url_bytes(raw_base64: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let payload = raw_base64
        .trim()
        .split_once("base64,")
        .map(|(_, b64)| b64)
        .unwrap_or_else(|| raw_base64.trim());
    if payload.is_empty() || payload.len() > 36 * 1024 * 1024 {
        return Err("图片体积无效或超出限制".to_string());
    }
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| format!("图片编码解析失败: {}", e))
}

/// 弹出原生系统文件保存框并将 PNG/JPG 图像导出到本地（支持按扩展名无损写 PNG 或高清编码 JPG）
#[tauri::command]
async fn save_image_base64_with_dialog_cmd(
    window: tauri::WebviewWindow,
    suggested_name: String,
    data_base64: String,
) -> Result<Option<String>, String> {
    ensure_main_window(&window)?;
    let bytes = decode_image_data_url_bytes(&data_base64)?;
    let clean_name: String = suggested_name
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .take(120)
        .collect();
    let default_file = if clean_name.is_empty() {
        "cover.png".to_string()
    } else {
        clean_name
    };

    tokio::task::spawn_blocking(move || -> Result<Option<String>, String> {
        let tmp_dir = kugou::store::resolve_cache_dir();
        let tmp_src = tmp_dir.join(format!("bm_export_{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&tmp_src, &bytes).map_err(|e| format!("写入临时图片失败: {}", e))?;

        #[cfg(target_os = "windows")]
        let res = {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            const STATIC_SAVE_PS: &str = "\
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
                Add-Type -AssemblyName System.Windows.Forms; \
                Add-Type -AssemblyName System.Drawing; \
                $dlg = New-Object System.Windows.Forms.SaveFileDialog; \
                $dlg.Filter = 'PNG 无损图片 (*.png)|*.png|JPEG 高清图片 (*.jpg;*.jpeg)|*.jpg'; \
                $dlg.FileName = [string]$env:BM_SAVE_NAME; \
                if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { \
                    $outPath = $dlg.FileName; \
                    if ($outPath -match '\\.(jpg|jpeg)$') { \
                        $img = [System.Drawing.Image]::FromFile([string]$env:BM_TMP_PNG); \
                        $img.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Jpeg); \
                        $img.Dispose(); \
                    } else { \
                        [System.IO.File]::Copy([string]$env:BM_TMP_PNG, $outPath, $true); \
                    } \
                    Write-Output $outPath; \
                }";
            let out = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-STA",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    STATIC_SAVE_PS,
                ])
                .env("BM_SAVE_NAME", &default_file)
                .env("BM_TMP_PNG", tmp_src.to_string_lossy().as_ref())
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("无法唤起系统保存对话框: {}", e))?;
            let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if chosen.is_empty() {
                Ok(None)
            } else {
                Ok(Some(chosen))
            }
        };

        #[cfg(not(target_os = "windows"))]
        let res = {
            let dest = kugou::store::resolve_default_download_dir().join(&default_file);
            std::fs::copy(&tmp_src, &dest).map_err(|e| format!("保存图片失败: {}", e))?;
            Ok(Some(dest.to_string_lossy().to_string()))
        };

        let _ = std::fs::remove_file(&tmp_src);
        res
    })
    .await
    .map_err(|e| format!("图片保存任务失败: {}", e))?
}

/// 将生成的海报 PNG 写入系统剪贴板（用于 Web ClipboardItem 权限受限时的零依赖 Windows 原生兜底）
#[tauri::command]
async fn copy_image_base64_to_clipboard_cmd(
    window: tauri::WebviewWindow,
    png_base64: String,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    let bytes = decode_image_data_url_bytes(&png_base64)?;

    tokio::task::spawn_blocking(move || -> Result<bool, String> {
        let tmp_dir = kugou::store::resolve_cache_dir();
        let tmp_src = tmp_dir.join(format!("bm_clip_{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&tmp_src, &bytes).map_err(|e| format!("准备剪贴板图片失败: {}", e))?;

        #[cfg(target_os = "windows")]
        let res = {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            const STATIC_CLIP_PS: &str = "\
                Add-Type -AssemblyName System.Windows.Forms; \
                Add-Type -AssemblyName System.Drawing; \
                $fs = [System.IO.File]::OpenRead([string]$env:BM_CLIP_PNG); \
                $img = [System.Drawing.Image]::FromStream($fs); \
                [System.Windows.Forms.Clipboard]::SetImage($img); \
                $img.Dispose(); \
                $fs.Dispose();";
            let out = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-STA",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    STATIC_CLIP_PS,
                ])
                .env("BM_CLIP_PNG", tmp_src.to_string_lossy().as_ref())
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("写入剪贴板失败: {}", e))?;
            if out.status.success() {
                Ok(true)
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
        };

        #[cfg(not(target_os = "windows"))]
        let res: Result<bool, String> = Ok(false);

        let _ = std::fs::remove_file(&tmp_src);
        res
    })
    .await
    .map_err(|e| format!("剪贴板任务失败: {}", e))?
}

#[tauri::command]
async fn search_lyric_candidates_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
    custom_keyword: Option<String>,
) -> Result<Vec<LyricCandidateItem>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .search_lyric_candidates(&song, custom_keyword.as_deref())
        .await
}

#[tauri::command]
async fn apply_lyric_candidate_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song_hash: String,
    candidate_id: String,
    accesskey: String,
    fmt: Option<String>,
) -> Result<LyricData, String> {
    ensure_main_window(&window)?;
    state
        .api
        .apply_lyric_candidate(
            &song_hash,
            &candidate_id,
            &accesskey,
            &fmt.unwrap_or_else(|| "krc".to_string()),
        )
        .await
}

#[tauri::command]
async fn get_user_cloud_songs_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<CloudDrivePageResult, String> {
    ensure_main_window(&window)?;
    state
        .api
        .fetch_user_cloud_songs(page.unwrap_or(1), page_size.unwrap_or(50))
        .await
}

#[tauri::command]
async fn delete_cloud_songs_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    items: Vec<CloudDeleteItem>,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    let pairs: Vec<(i64, i64)> = items
        .into_iter()
        .map(|it| (it.kv_id, it.album_audio_id))
        .collect();
    state.api.delete_user_cloud_songs(pairs).await
}

#[tauri::command]
async fn scan_local_music_dir_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    dir_path: Option<String>,
    options: Option<LocalMusicScanOptions>,
) -> Result<Vec<Song>, String> {
    ensure_main_window(&window)?;
    state
        .api
        .scan_local_music_dir(&dir_path.unwrap_or_default(), options)
        .await
}

/// 在 Windows 平台注册操作系统级全局热键 (Win32 RegisterHotKey，支持动态自定义组合键 + 多媒体键) 及锁定态桌面歌词光标命中检测：
/// - 支持从前端动态热更新各动作组合键，并实时回报被其他程序占用的热键列表
/// - 始终保留硬件多媒体键（播放/暂停、上一曲、下一曲）
/// - 桌面歌词锁定时：开启硬件穿透，仅当光标悬停于顶部“解锁按钮区域”时精准临时解除穿透，移出即恢复穿透
#[cfg(target_os = "windows")]
fn spawn_windows_global_hotkey_listener(
    app_handle: AppHandle,
    hotkeys_enabled: Arc<AtomicBool>,
    hotkey_bindings: Arc<std::sync::Mutex<Vec<GlobalHotkeyBinding>>>,
    hotkey_revision: Arc<AtomicU64>,
    hotkey_failed: Arc<std::sync::Mutex<Vec<String>>>,
    hotkey_thread_id: Arc<AtomicU32>,
    lyric_locked: Arc<AtomicBool>,
    lyric_visible: Arc<AtomicBool>,
    mini_visible: Arc<AtomicBool>,
    mini_expanded: Arc<AtomicBool>,
    mini_dock_edge: Arc<std::sync::Mutex<String>>,
) {
    std::thread::spawn(move || {
        #[repr(C)]
        #[derive(Default, Clone, Copy)]
        struct Point {
            x: i32,
            y: i32,
        }

        #[repr(C)]
        #[derive(Default)]
        struct Msg {
            hwnd: isize,
            message: u32,
            w_param: usize,
            l_param: isize,
            time: u32,
            pt: Point,
        }

        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentThreadId() -> u32;
        }

        #[link(name = "user32")]
        extern "system" {
            fn RegisterHotKey(hwnd: isize, id: i32, fs_modifiers: u32, vk: u32) -> i32;
            fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
            fn PeekMessageW(
                lp_msg: *mut Msg,
                hwnd: isize,
                w_msg_filter_min: u32,
                w_msg_filter_max: u32,
                w_remove_msg: u32,
            ) -> i32;
            fn GetCursorPos(lp_point: *mut Point) -> i32;
            fn MsgWaitForMultipleObjectsEx(
                n_count: u32,
                p_handles: *const isize,
                dw_milliseconds: u32,
                dw_wake_mask: u32,
                dw_flags: u32,
            ) -> u32;
        }

        const MOD_NOREPEAT: u32 = 0x4000;
        const WM_HOTKEY: u32 = 0x0312;
        const PM_REMOVE: u32 = 0x0001;
        const INFINITE: u32 = 0xFFFF_FFFF;
        const QS_ALLINPUT: u32 = 0x04FF;

        const HK_MEDIA_PLAY_PAUSE: i32 = 2001; // VK_MEDIA_PLAY_PAUSE (0xB3)
        const HK_MEDIA_PREV: i32 = 2002;       // VK_MEDIA_PREV_TRACK (0xB1)
        const HK_MEDIA_NEXT: i32 = 2003;       // VK_MEDIA_NEXT_TRACK (0xB0)
        const MEDIA_SPECS: [(i32, u32, u32, &str); 3] = [
            (HK_MEDIA_PLAY_PAUSE, MOD_NOREPEAT, 0xB3, "toggle_play"),
            (HK_MEDIA_PREV, MOD_NOREPEAT, 0xB1, "prev_song"),
            (HK_MEDIA_NEXT, MOD_NOREPEAT, 0xB0, "next_song"),
        ];

        let mut registered_ids: Vec<(i32, String)> = Vec::new();
        let mut applied_enabled = false;
        let mut applied_revision: u64 = u64::MAX;
        let mut msg = Msg::default();
        // 先调用一次 PeekMessageW 强制 Windows 为工作线程创建消息队列，再发布 ThreadId
        unsafe {
            let _ = PeekMessageW(&mut msg, 0, 0, 0, 0);
            hotkey_thread_id.store(GetCurrentThreadId(), Ordering::Release);
        }

        let mut was_in_window = false;
        let mut was_in_unlock_zone = false;
        let mut last_ignore_cursor: Option<bool> = None;
        let mut was_in_mini_island = false;
        let mut last_mini_ignore_cursor: Option<bool> = None;
        let mut last_lyric_cursor: Option<(i32, i32)> = None;
        let mut last_mini_cursor: Option<(i32, i32)> = None;

        loop {
            let should_enable = hotkeys_enabled.load(Ordering::Relaxed);
            let current_revision = hotkey_revision.load(Ordering::Relaxed);
            if should_enable != applied_enabled || current_revision != applied_revision {
                // 先安全卸载旧热键
                unsafe {
                    for (id, _) in &registered_ids {
                        let _ = UnregisterHotKey(0, *id);
                    }
                }
                registered_ids.clear();

                let mut failed_list: Vec<String> = Vec::new();
                if should_enable {
                    let snapshot = hotkey_bindings
                        .lock()
                        .map(|g| g.clone())
                        .unwrap_or_else(|_| default_global_hotkey_bindings());
                    let mut next_id: i32 = 1001;
                    unsafe {
                        for item in snapshot {
                            let trimmed = item.accelerator.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            if let Some((mods, vk)) = parse_win32_accelerator(trimmed) {
                                let id = next_id;
                                next_id += 1;
                                if RegisterHotKey(0, id, mods, vk) != 0 {
                                    registered_ids.push((id, item.action));
                                } else {
                                    failed_list.push(item.action);
                                }
                            } else {
                                failed_list.push(item.action);
                            }
                        }
                        for &(id, mods, vk, action) in &MEDIA_SPECS {
                            if RegisterHotKey(0, id, mods, vk) != 0 {
                                registered_ids.push((id, action.to_string()));
                            }
                        }
                    }
                }

                if let Ok(mut guard) = hotkey_failed.lock() {
                    *guard = failed_list.clone();
                }
                applied_enabled = should_enable;
                applied_revision = current_revision;
                let _ = app_handle.emit(
                    "bm-global-hotkeys-report",
                    serde_json::json!({
                        "enabled": should_enable,
                        "registeredCount": registered_ids.len(),
                        "failedActions": failed_list,
                    }),
                );
            }

            // 排空消息队列中的 WM_HOTKEY 与 WM_APP_HOTKEY_WAKE 唤醒消息
            unsafe {
                while PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) != 0 {
                    if msg.message == WM_HOTKEY && applied_enabled {
                        let hit_id = msg.w_param as i32;
                        if let Some((_, action)) =
                            registered_ids.iter().find(|(id, _)| *id == hit_id)
                        {
                            match action.as_str() {
                                "toggle_lyric_lock" => {
                                    let next_locked = !lyric_locked.load(Ordering::Relaxed);
                                    lyric_locked.store(next_locked, Ordering::Relaxed);
                                    was_in_window = false;
                                    was_in_unlock_zone = false;
                                    last_ignore_cursor = Some(next_locked);
                                    last_lyric_cursor = None;
                                    if let Some(win) =
                                        app_handle.get_webview_window("desktop_lyric")
                                    {
                                        let _ = win.set_ignore_cursor_events(next_locked);
                                    }
                                    let _ = app_handle.emit(
                                        "bm-desktop-lyric-lock-sync",
                                        serde_json::json!({
                                            "locked": next_locked,
                                            "notify": true,
                                        }),
                                    );
                                    let _ = app_handle.emit_to(
                                        "desktop_lyric",
                                        "bm-desktop-lyric-locked-hover",
                                        serde_json::json!({
                                            "hoveringWindow": false,
                                            "hoveringUnlock": false,
                                        }),
                                    );
                                }
                                "toggle_play"
                                | "prev_song"
                                | "next_song"
                                | "toggle_desktop_lyric"
                                | "toggle_mini_mode"
                                | "volume_up"
                                | "volume_down"
                                | "toggle_like" => {
                                    let _ = app_handle.emit("tray-action", action.as_str());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            // 锁定态桌面歌词光标命中检测（支持距离自适应防空转）：
            let is_vis = lyric_visible.load(Ordering::Relaxed);
            let is_lck = lyric_locked.load(Ordering::Relaxed);
            let mut near_cursor_zone = false;
            let mut cursor_moved_this_tick = false;

            if !is_vis {
                was_in_window = false;
                was_in_unlock_zone = false;
                last_ignore_cursor = None;
                last_lyric_cursor = None;
            } else if !is_lck {
                last_lyric_cursor = None;
                if last_ignore_cursor != Some(false) {
                    if let Some(win) = app_handle.get_webview_window("desktop_lyric") {
                        let _ = win.set_ignore_cursor_events(false);
                    }
                    last_ignore_cursor = Some(false);
                }
                if was_in_window || was_in_unlock_zone {
                    was_in_window = false;
                    was_in_unlock_zone = false;
                    let _ = app_handle.emit_to(
                        "desktop_lyric",
                        "bm-desktop-lyric-locked-hover",
                        serde_json::json!({
                            "hoveringWindow": false,
                            "hoveringUnlock": false,
                        }),
                    );
                }
            } else {
                let mut cursor = Point::default();
                let cursor_ok = unsafe { GetCursorPos(&mut cursor) != 0 };
                if cursor_ok {
                    let pt = (cursor.x, cursor.y);
                    let moved = last_lyric_cursor != Some(pt);
                    if moved {
                        cursor_moved_this_tick = true;
                        last_lyric_cursor = Some(pt);
                    }
                    if moved || was_in_window || was_in_unlock_zone {
                        if let Some(win) = app_handle.get_webview_window("desktop_lyric") {
                            if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
                                let sf = win.scale_factor().unwrap_or(1.0).max(1.0);
                                let wx = pos.x as f64;
                                let wy = pos.y as f64;
                                let ww = size.width as f64;
                                let wh = size.height as f64;
                                let cx = cursor.x as f64;
                                let cy = cursor.y as f64;

                                let near_pad = 160.0 * sf;
                                if cx >= wx - near_pad
                                    && cx <= wx + ww + near_pad
                                    && cy >= wy - near_pad
                                    && cy <= wy + wh + near_pad
                                {
                                    near_cursor_zone = true;
                                }

                                let win_pad = if was_in_window { 8.0 * sf } else { 0.0 };
                                let in_window = cx >= wx - win_pad
                                    && cx <= wx + ww + win_pad
                                    && cy >= wy - win_pad
                                    && cy <= wy + wh + win_pad;

                                let center_x = wx + ww * 0.5;
                                let unlock_half_w = if was_in_unlock_zone { 126.0 * sf } else { 114.0 * sf };
                                let unlock_min_y = wy - (if was_in_unlock_zone { 4.0 * sf } else { 0.0 });
                                let unlock_max_y = wy + (if was_in_unlock_zone { 60.0 * sf } else { 50.0 * sf });

                                let in_unlock_zone = in_window
                                    && (cx - center_x).abs() <= unlock_half_w
                                    && cy >= unlock_min_y
                                    && cy <= unlock_max_y;

                                let desired_ignore = !in_unlock_zone;
                                if last_ignore_cursor != Some(desired_ignore) {
                                    let _ = win.set_ignore_cursor_events(desired_ignore);
                                    last_ignore_cursor = Some(desired_ignore);
                                }

                                if in_window != was_in_window || in_unlock_zone != was_in_unlock_zone {
                                    was_in_window = in_window;
                                    was_in_unlock_zone = in_unlock_zone;
                                    let _ = app_handle.emit_to(
                                        "desktop_lyric",
                                        "bm-desktop-lyric-locked-hover",
                                        serde_json::json!({
                                            "hoveringWindow": in_window,
                                            "hoveringUnlock": in_unlock_zone,
                                        }),
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // Mini 灵动岛收起态局部热区穿透检测：
            let is_mini_vis = mini_visible.load(Ordering::Relaxed);
            let is_mini_exp = mini_expanded.load(Ordering::Relaxed);
            if !is_mini_vis {
                was_in_mini_island = false;
                last_mini_ignore_cursor = None;
                last_mini_cursor = None;
            } else {
                let edge = mini_dock_edge
                    .lock()
                    .ok()
                    .map(|g| g.clone())
                    .unwrap_or_else(|| "none".to_string());
                if is_mini_exp || edge == "none" {
                    last_mini_cursor = None;
                    if last_mini_ignore_cursor != Some(false) {
                        if let Some(win) = app_handle.get_webview_window("mini_player") {
                            let _ = win.set_ignore_cursor_events(false);
                        }
                        last_mini_ignore_cursor = Some(false);
                    }
                    was_in_mini_island = true;
                } else {
                    let mut cursor = Point::default();
                    let cursor_ok = unsafe { GetCursorPos(&mut cursor) != 0 };
                    if cursor_ok {
                        let pt = (cursor.x, cursor.y);
                        let moved = last_mini_cursor != Some(pt);
                        if moved {
                            cursor_moved_this_tick = true;
                            last_mini_cursor = Some(pt);
                        }
                        if moved || was_in_mini_island {
                            if let Some(win) = app_handle.get_webview_window("mini_player") {
                                if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
                                    let sf = win.scale_factor().unwrap_or(1.0).max(1.0);
                                    let wx = pos.x as f64;
                                    let wy = pos.y as f64;
                                    let ww = size.width as f64;
                                    let wh = size.height as f64;
                                    let cx = cursor.x as f64;
                                    let cy = cursor.y as f64;

                                    let near_pad = 160.0 * sf;
                                    if cx >= wx - near_pad
                                        && cx <= wx + ww + near_pad
                                        && cy >= wy - near_pad
                                        && cy <= wy + wh + near_pad
                                    {
                                        near_cursor_zone = true;
                                    }

                                    let pad = if was_in_mini_island { 10.0 * sf } else { 4.0 * sf };

                                    let in_island = match edge.as_str() {
                                        "top" => {
                                            let pill_w = 336.0 * sf;
                                            let pill_h = 38.0 * sf;
                                            let left = wx + (ww - pill_w) * 0.5 - pad;
                                            let right = wx + (ww + pill_w) * 0.5 + pad;
                                            cx >= left && cx <= right && cy >= wy - pad && cy <= wy + pill_h + pad
                                        }
                                        "bottom" => {
                                            let pill_w = 336.0 * sf;
                                            let pill_h = 38.0 * sf;
                                            let left = wx + (ww - pill_w) * 0.5 - pad;
                                            let right = wx + (ww + pill_w) * 0.5 + pad;
                                            cx >= left
                                                && cx <= right
                                                && cy >= wy + wh - pill_h - pad
                                                && cy <= wy + wh + pad
                                        }
                                        "left" => {
                                            let pick_w = 54.0 * sf;
                                            let pick_h = 100.0 * sf;
                                            let top = wy + (wh - pick_h) * 0.5 - pad;
                                            let bottom = wy + (wh + pick_h) * 0.5 + pad;
                                            cx >= wx - pad && cx <= wx + pick_w + pad && cy >= top && cy <= bottom
                                        }
                                        "right" => {
                                            let pick_w = 54.0 * sf;
                                            let pick_h = 100.0 * sf;
                                            let top = wy + (wh - pick_h) * 0.5 - pad;
                                            let bottom = wy + (wh + pick_h) * 0.5 + pad;
                                            cx >= wx + ww - pick_w - pad
                                                && cx <= wx + ww + pad
                                                && cy >= top
                                                && cy <= bottom
                                        }
                                        _ => true,
                                    };

                                    let desired_ignore = !in_island;
                                    if last_mini_ignore_cursor != Some(desired_ignore) {
                                        let _ = win.set_ignore_cursor_events(desired_ignore);
                                        last_mini_ignore_cursor = Some(desired_ignore);
                                    }
                                    if was_in_mini_island && !in_island {
                                        let _ = app_handle.emit_to(
                                            "mini_player",
                                            "bm-mini-island-cursor-leave",
                                            serde_json::json!({}),
                                        );
                                    }
                                    was_in_mini_island = in_island;
                                }
                            }
                        }
                    }
                }
            }

            let need_cursor_poll = (is_vis && is_lck) || (is_mini_vis && !is_mini_exp);
            let wait_timeout_ms = if !need_cursor_poll {
                // 无穿透检测需求时彻底阻塞休眠：0 次/秒 CPU 唤醒，热键与状态消息到达即刻瞬时唤醒
                INFINITE
            } else if near_cursor_zone && cursor_moved_this_tick {
                16
            } else if near_cursor_zone {
                24
            } else if cursor_moved_this_tick {
                48
            } else {
                64
            };
            unsafe {
                let _ = MsgWaitForMultipleObjectsEx(0, std::ptr::null(), wait_timeout_ms, QS_ALLINPUT, 0);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 强制将 WebView2 用户数据与浏览器缓存目录重定向至软件安装目录下的 webview_data，避免占用 C 盘 AppData\Local
    let webview_data_dir = kugou::store::resolve_webview_data_dir();
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &webview_data_dir);
    }
    let _ = kugou::store::resolve_cache_dir();
    let _ = kugou::store::resolve_default_download_dir();

    let store = Arc::new(AppStore::new());
    let api = Arc::new(KugouApiService::new(store.clone()));
    let close_to_tray = Arc::new(AtomicBool::new(true));
    let close_to_tray_for_event = close_to_tray.clone();
    let global_hotkeys_enabled = Arc::new(AtomicBool::new(true));
    let global_hotkeys_for_setup = global_hotkeys_enabled.clone();
    let global_hotkey_bindings = Arc::new(std::sync::Mutex::new(default_global_hotkey_bindings()));
    let global_hotkey_bindings_for_setup = global_hotkey_bindings.clone();
    let global_hotkeys_revision = Arc::new(AtomicU64::new(1));
    let global_hotkeys_revision_for_setup = global_hotkeys_revision.clone();
    let global_hotkey_failed = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let global_hotkey_failed_for_setup = global_hotkey_failed.clone();
    let desktop_lyric_locked = Arc::new(AtomicBool::new(false));
    let desktop_lyric_locked_for_setup = desktop_lyric_locked.clone();
    let desktop_lyric_visible = Arc::new(AtomicBool::new(false));
    let desktop_lyric_visible_for_setup = desktop_lyric_visible.clone();
    let desktop_lyric_positioned = Arc::new(AtomicBool::new(false));
    let desktop_lyric_last_pos = Arc::new(std::sync::Mutex::new(None));
    let desktop_lyric_snapshot = Arc::new(std::sync::Mutex::new(None));
    let mini_player_visible = Arc::new(AtomicBool::new(false));
    let mini_player_visible_for_setup = mini_player_visible.clone();
    let mini_player_positioned = Arc::new(AtomicBool::new(false));
    let mini_player_expanded = Arc::new(AtomicBool::new(true));
    let mini_player_expanded_for_setup = mini_player_expanded.clone();
    let mini_player_dock_edge = Arc::new(std::sync::Mutex::new("none".to_string()));
    let mini_player_dock_edge_for_setup = mini_player_dock_edge.clone();
    let mini_player_snapshot = Arc::new(std::sync::Mutex::new(None));
    let main_was_maximized = Arc::new(AtomicBool::new(false));
    let main_was_maximized_for_event = main_was_maximized.clone();
    let main_low_memory_active = Arc::new(AtomicBool::new(false));
    let main_low_memory_for_event = main_low_memory_active.clone();
    let hotkey_thread_id = Arc::new(AtomicU32::new(0));
    let hotkey_thread_id_for_setup = hotkey_thread_id.clone();
    let active_download_cancels = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let authorized_font_import_paths = Arc::new(std::sync::Mutex::new(HashSet::new()));

    tauri::Builder::default()
        .manage(AppState {
            api,
            store,
            close_to_tray,
            global_hotkeys_enabled,
            global_hotkey_bindings,
            global_hotkeys_revision,
            global_hotkey_failed,
            desktop_lyric_locked,
            desktop_lyric_visible,
            desktop_lyric_positioned,
            desktop_lyric_last_pos,
            desktop_lyric_snapshot,
            mini_player_visible,
            mini_player_positioned,
            mini_player_expanded,
            mini_player_dock_edge,
            mini_player_snapshot,
            main_was_maximized,
            main_low_memory_active,
            hotkey_thread_id,
            active_download_cancels,
            authorized_font_import_paths,
        })
        .setup(move |app| {
            if let Some(w) = app.get_webview_window("main") {
                harden_desktop_webview2(&w);
            }
            #[cfg(target_os = "windows")]
            spawn_windows_global_hotkey_listener(
                app.handle().clone(),
                global_hotkeys_for_setup.clone(),
                global_hotkey_bindings_for_setup.clone(),
                global_hotkeys_revision_for_setup.clone(),
                global_hotkey_failed_for_setup.clone(),
                hotkey_thread_id_for_setup.clone(),
                desktop_lyric_locked_for_setup.clone(),
                desktop_lyric_visible_for_setup.clone(),
                mini_player_visible_for_setup.clone(),
                mini_player_expanded_for_setup.clone(),
                mini_player_dock_edge_for_setup.clone(),
            );

            let show_i = MenuItemBuilder::with_id("show", "显示主窗口").build(app)?;
            let toggle_play_i =
                MenuItemBuilder::with_id("toggle_play", "播放 / 暂停").build(app)?;
            let prev_i = MenuItemBuilder::with_id("prev_song", "上一首").build(app)?;
            let next_i = MenuItemBuilder::with_id("next_song", "下一首").build(app)?;
            let desktop_lyric_i =
                MenuItemBuilder::with_id("toggle_desktop_lyric", "开关桌面歌词").build(app)?;
            let lyric_lock_i =
                MenuItemBuilder::with_id("toggle_lyric_lock", "锁定 / 解锁桌面歌词").build(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "退出微风音乐").build(app)?;

            let menu = MenuBuilder::new(app)
                .items(&[
                    &show_i,
                    &toggle_play_i,
                    &prev_i,
                    &next_i,
                    &desktop_lyric_i,
                    &lyric_lock_i,
                    &quit_i,
                ])
                .build()?;

            let mut tray_builder = TrayIconBuilder::with_id("main-tray")
                .menu(&menu)
                .tooltip("BreezeMusic 微风音乐 · 酷狗概念版")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app_handle = tray.app_handle();
                        let st = app_handle.state::<AppState>();
                        restore_main_from_mini(app_handle, &st);
                    }
                })
                .on_menu_event(|app_handle, event| match event.id().as_ref() {
                    "show" => {
                        let st = app_handle.state::<AppState>();
                        restore_main_from_mini(app_handle, &st);
                    }
                    "toggle_play" => {
                        let _ = app_handle.emit("tray-action", "toggle_play");
                    }
                    "prev_song" => {
                        let _ = app_handle.emit("tray-action", "prev_song");
                    }
                    "next_song" => {
                        let _ = app_handle.emit("tray-action", "next_song");
                    }
                    "toggle_desktop_lyric" => {
                        let _ = app_handle.emit("tray-action", "toggle_desktop_lyric");
                    }
                    "toggle_lyric_lock" => {
                        let st = app_handle.state::<AppState>();
                        let next_locked = !st.desktop_lyric_locked.load(Ordering::Relaxed);
                        st.desktop_lyric_locked.store(next_locked, Ordering::Relaxed);
                        wake_hotkey_thread(&st.hotkey_thread_id);
                        if let Some(win) = app_handle.get_webview_window("desktop_lyric") {
                            let _ = win.set_ignore_cursor_events(next_locked);
                        }
                        let _ = app_handle.emit(
                            "bm-desktop-lyric-lock-sync",
                            serde_json::json!({
                                "locked": next_locked,
                                "notify": true,
                            }),
                        );
                        let _ = app_handle.emit_to(
                            "desktop_lyric",
                            "bm-desktop-lyric-locked-hover",
                            serde_json::json!({
                                "hoveringWindow": false,
                                "hoveringUnlock": false,
                            }),
                        );
                    }
                    "quit" => {
                        app_handle.exit(0);
                    }
                    _ => {}
                });

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }
            let _ = tray_builder.build(app);
            Ok(())
        })
        .on_window_event(move |window, event| {
            if window.label() == "main" {
                match event {
                    WindowEvent::Resized(_) => {
                        let is_min = window.is_minimized().unwrap_or(false);
                        let is_vis = window.is_visible().unwrap_or(false);
                        if is_vis && !is_min {
                            main_was_maximized_for_event.store(
                                window.is_maximized().unwrap_or(false),
                                Ordering::Relaxed,
                            );
                            if let Some(mw) = window.app_handle().get_webview_window("main") {
                                sync_main_low_memory_state(&mw, &main_low_memory_for_event, false);
                            }
                        } else if is_min {
                            if let Some(mw) = window.app_handle().get_webview_window("main") {
                                sync_main_low_memory_state(&mw, &main_low_memory_for_event, true);
                            }
                        }
                    }
                    WindowEvent::CloseRequested { api, .. } => {
                        if close_to_tray_for_event.load(Ordering::Relaxed) {
                            api.prevent_close();
                            main_was_maximized_for_event.store(
                                window.is_maximized().unwrap_or(false),
                                Ordering::Relaxed,
                            );
                            if let Some(mw) = window.app_handle().get_webview_window("main") {
                                sync_main_low_memory_state(&mw, &main_low_memory_for_event, true);
                            }
                            let _ = window.hide();
                        } else {
                            window.app_handle().exit(0);
                        }
                    }
                    _ => {}
                }
            } else if window.label() == "mini_player" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let app = window.app_handle();
                    let state = app.state::<AppState>();
                    restore_main_from_mini(&app, &state);
                }
            } else if window.label() == "desktop_lyric" {
                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let app = window.app_handle();
                        let state = app.state::<AppState>();
                        state.desktop_lyric_visible.store(false, Ordering::Relaxed);
                        wake_hotkey_thread(&state.hotkey_thread_id);
                        let _ = window.hide();
                        schedule_subwindow_suspend(
                            &app,
                            "desktop_lyric",
                            state.desktop_lyric_visible.clone(),
                        );
                        let locked = state.desktop_lyric_locked.load(Ordering::Relaxed);
                        let _ = app.emit(
                            "bm-desktop-lyric-visibility",
                            serde_json::json!({
                                "visible": false,
                                "locked": locked,
                            }),
                        );
                    }
                    WindowEvent::Moved(pos) => {
                        let app = window.app_handle();
                        let state = app.state::<AppState>();
                        if state.desktop_lyric_visible.load(Ordering::Relaxed)
                            && pos.x > -10000
                            && pos.y > -10000
                        {
                            let sf = window.scale_factor().unwrap_or(1.0).max(1.0);
                            let lx = (pos.x as f64 / sf).round();
                            let ly = (pos.y as f64 / sf).round();
                            record_desktop_lyric_pos(&app, &state, lx, ly);
                        }
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            init_app_state,
            get_saved_accounts_cmd,
            switch_saved_account_cmd,
            remove_saved_account_cmd,
            send_sms_code,
            login_with_sms,
            create_qr_login,
            check_qr_login,
            logout_account,
            get_account_profile,
            sync_listening_grade_cmd,
            record_listening_seconds_cmd,
            get_vip_status,
            claim_daily_vip,
            get_daily_recommend,
            get_personal_fm,
            get_new_song_express,
            get_rank_list,
            get_rank_songs,
            get_fm_channels,
            get_fm_songs,
            get_recent_history,
            search_songs,
            search_songs_paged_cmd,
            get_search_hot,
            get_search_suggest_cmd,
            get_hot_artists_cmd,
            get_search_discovery_cmd,
            search_playlists_cmd,
            search_playlists_paged_cmd,
            search_albums_cmd,
            search_albums_paged_cmd,
            search_artists_cmd,
            search_artists_paged_cmd,
            get_user_playlists,
            get_playlist_info_cmd,
            get_playlist_songs,
            create_user_playlist_cmd,
            update_user_playlist_cmd,
            delete_user_playlist_cmd,
            collect_playlist_cmd,
            uncollect_playlist_cmd,
            add_song_to_playlist_cmd,
            add_songs_to_playlist_batch_cmd,
            remove_song_from_playlist_cmd,
            resolve_song_album_meta_cmd,
            get_followed_artists,
            toggle_follow_artist_cmd,
            get_purchased_albums,
            get_purchased_songs,
            get_artist_detail_cmd,
            get_artist_songs_cmd,
            get_artist_albums_cmd,
            get_album_detail_cmd,
            get_album_songs_cmd,
            get_song_comment_count_cmd,
            get_song_comments_cmd,
            get_song_comments_paged_cmd,
            get_comment_floor_replies_cmd,
            get_comment_floor_replies_paged_cmd,
            like_song_comment_cmd,
            post_song_comment_cmd,
            resign_device_cmd,
            diagnose_gateway_cmd,
            download_song_cmd,
            cancel_download_task_cmd,
            save_song_lyric_only_cmd,
            parse_shared_music_cmd,
            resolve_song_url,
            resolve_song_accompaniment_cmd,
            get_song_detail_info_cmd,
            get_cache_stats_cmd,
            clear_cache_category_cmd,
            set_desktop_lyric_window_state_cmd,
            resize_desktop_lyric_window_cmd,
            save_desktop_lyric_position_cmd,
            reset_desktop_lyric_position_cmd,
            snap_desktop_lyric_position_cmd,
            focus_main_window_cmd,
            sync_main_window_bg_color_cmd,
            push_desktop_lyric_sync_cmd,
            pull_desktop_lyric_sync_cmd,
            send_desktop_lyric_command_cmd,
            set_mini_player_mode_cmd,
            push_mini_player_sync_cmd,
            pull_mini_player_sync_cmd,
            send_mini_player_command_cmd,
            get_mini_player_drag_anchor_cmd,
            drag_mini_player_step_cmd,
            snap_mini_player_to_edge_cmd,
            expand_mini_player_from_dock_cmd,
            collapse_mini_player_to_dock_cmd,
            set_close_to_tray_cmd,
            set_global_hotkeys_enabled_cmd,
            update_global_hotkeys_cmd,
            set_audio_cache_quota_cmd,
            open_path_in_explorer_cmd,
            reveal_path_in_explorer_cmd,
            list_installed_fonts_cmd,
            pick_font_files_dialog_cmd,
            import_custom_font_files_cmd,
            read_custom_font_binary_cmd,
            delete_custom_font_cmd,
            get_autostart_enabled_cmd,
            set_autostart_enabled_cmd,
            pick_folder_dialog_cmd,
            report_playback_telemetry_cmd,
            get_viper_effects_cmd,
            get_song_lyric,
            fetch_cover_image_base64_cmd,
            save_image_base64_with_dialog_cmd,
            copy_image_base64_to_clipboard_cmd,
            search_lyric_candidates_cmd,
            apply_lyric_candidate_cmd,
            get_user_cloud_songs_cmd,
            delete_cloud_songs_cmd,
            scan_local_music_dir_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running BreezeMusic Windows application");
}

#[cfg(test)]
mod settings_backend_tests {
    use super::*;

    #[test]
    fn test_parse_win32_accelerator_valid_and_safe() {
        // 标准组合键
        assert_eq!(
            parse_win32_accelerator("Ctrl+Shift+L"),
            Some((0x0002 | 0x0004 | 0x4000, 0x4C))
        );
        assert_eq!(
            parse_win32_accelerator("Ctrl+Alt+Space"),
            Some((0x0002 | 0x0001 | 0x4000, 0x20))
        );
        assert_eq!(
            parse_win32_accelerator("Ctrl+Alt+Right"),
            Some((0x0002 | 0x0001 | 0x4000, 0x27))
        );
        assert_eq!(
            parse_win32_accelerator("Alt+ArrowUp"),
            Some((0x0001 | 0x4000, 0x26))
        );
        // 功能键允许裸键
        assert_eq!(parse_win32_accelerator("F9"), Some((0x4000, 0x78)));
        // 裸字母/空格禁止作为系统全局热键（防止劫持打字）
        assert_eq!(parse_win32_accelerator("A"), None);
        assert_eq!(parse_win32_accelerator("Space"), None);
        assert_eq!(parse_win32_accelerator(""), None);
        assert_eq!(parse_win32_accelerator("Ctrl+Shift"), None);
        assert_eq!(parse_win32_accelerator("Ctrl+A+B"), None);
    }

    #[test]
    fn test_resolve_safe_explorer_target_dir_guards() {
        let store = AppStore::new();
        assert!(resolve_safe_explorer_target_dir(&store, "cache").is_ok());
        assert!(resolve_safe_explorer_target_dir(&store, "downloads").is_ok());
        assert!(resolve_safe_explorer_target_dir(&store, "data_root").is_ok());
        assert!(resolve_safe_explorer_target_dir(&store, r"C:\Windows\System32").is_err());
        assert!(resolve_safe_explorer_target_dir(&store, r"..\..\Windows").is_err());
        assert!(resolve_safe_explorer_target_dir(&store, "").is_err());
    }
}
