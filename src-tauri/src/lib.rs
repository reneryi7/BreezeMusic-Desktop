pub mod kugou;
pub mod single_instance;
pub mod window_transition;

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
    /// 磁吸贴边缓动动画递增代号，任意新拖动/展开/切屏时递增即可立即终止旧动画避免坐标竞态
    pub mini_snap_anim_gen: Arc<AtomicU64>,
    /// 标记 Mini 灵动岛当前是否正为同屏全屏独占应用（全屏视频/游戏）静默礼让
    pub mini_yielded_for_fullscreen: Arc<AtomicBool>,
    /// 标记 Mini 独立子窗口前端已完成首次挂载就绪（预热后进入小窗 0ms 冷启动延迟）
    pub mini_player_ready: Arc<AtomicBool>,
    /// 标记 Mini 独立子窗口当前是否处于短暂（~128ms）退场淡出窗口期（期间不触发幽灵窗强杀，结束后必校验彻底隐藏）
    pub mini_exiting_anim: Arc<AtomicBool>,
    /// 记录主窗口隐藏（进入 Mini 模式或最小化至托盘）前是否处于最大化状态
    pub main_was_maximized: Arc<AtomicBool>,
    /// 主窗口 <-> Mini / 托盘空间过渡动画递增世代令牌（防快速连按竞态与状态撕裂）
    pub main_transition_gen: Arc<AtomicU64>,
    /// 同步自前端的动效档位（"best" | "standard" | "none"）
    pub motion_level: Arc<std::sync::Mutex<String>>,
    /// 同步自前端的封面双主色 RGB（用于跨主题兜底卡面无缝流光过渡）
    pub cover_accent_rgb: Arc<std::sync::Mutex<((u8, u8, u8), (u8, u8, u8))>>,
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
        let _ = win.emit(
            "bm-main-window-visibility",
            serde_json::json!({ "visible": !low }),
        );
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
            enforce_transparent_overlay_window_style(&win);
            Some(win)
        }
        Err(_) => app.get_webview_window("mini_player"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhysRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[cfg(test)]
impl PhysRect {
    pub(crate) fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub(crate) fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }
}

/// 纯几何辅助：校验某窗口外框是否在任一活跃显示器工作区内拥有足够可见面积（防止切屏/拔显示器后飘出屏幕外）
pub(crate) fn is_rect_visible_in_any_work_area(
    rect: PhysRect,
    areas: &[PhysRect],
    min_visible_px: i32,
) -> bool {
    let min_px = min_visible_px.max(1);
    for wa in areas {
        let inter_l = rect.left.max(wa.left);
        let inter_t = rect.top.max(wa.top);
        let inter_r = rect.right.min(wa.right);
        let inter_b = rect.bottom.min(wa.bottom);
        if inter_r - inter_l >= min_px && inter_b - inter_t >= min_px {
            return true;
        }
    }
    false
}

/// 纯几何辅助：根据当前停靠边缘与显示器工作区计算安全贴边或钳制坐标
pub(crate) fn compute_mini_dock_or_clamped_pos(
    cur_x: i32,
    cur_y: i32,
    win_w: i32,
    win_h: i32,
    edge: &str,
    card_pad: i32,
    wa: PhysRect,
) -> (i32, i32) {
    let center_x = cur_x + win_w / 2;
    let center_y = cur_y + win_h / 2;
    let min_x = wa.left - card_pad;
    let max_x = (wa.right - win_w + card_pad).max(min_x);
    let min_y = wa.top - card_pad;
    let max_y = (wa.bottom - win_h + card_pad).max(min_y);

    match edge {
        "top" => ((center_x - win_w / 2).clamp(min_x, max_x), wa.top),
        "bottom" => (
            (center_x - win_w / 2).clamp(min_x, max_x),
            (wa.bottom - win_h).max(wa.top),
        ),
        "left" => (wa.left, (center_y - win_h / 2).clamp(min_y, max_y)),
        "right" => (
            (wa.right - win_w).max(wa.left),
            (center_y - win_h / 2).clamp(min_y, max_y),
        ),
        _ => (cur_x.clamp(min_x, max_x), cur_y.clamp(min_y, max_y)),
    }
}

/// 纯几何辅助：检测最大化无边框主窗口的客户区是否偏离显示器工作区（允许自动隐藏任务栏 1px 容差）或 WebView2 Bounds 未铺满客户区
#[cfg(test)]
pub(crate) fn maximized_client_needs_reconcile(
    client_screen: PhysRect,
    work_area: PhysRect,
    webview_bounds: Option<PhysRect>,
) -> bool {
    let left_gap = (client_screen.left - work_area.left).abs();
    let top_gap = (client_screen.top - work_area.top).abs();
    let right_gap = (work_area.right - client_screen.right).abs();
    let bottom_gap = (work_area.bottom - client_screen.bottom).abs();
    if left_gap > 1 || top_gap > 1 || right_gap > 1 || bottom_gap > 1 {
        return true;
    }
    if let Some(wb) = webview_bounds {
        let cw = client_screen.width();
        let ch = client_screen.height();
        if wb.left != 0 || wb.top != 0 || wb.width() != cw || wb.height() != ch {
            return true;
        }
    }
    false
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

/// 主窗口当前主题背景色 COLORREF (0x00BBGGRR)，供非客户区 WM_NCPAINT / WM_NCACTIVATE 兜底填充使用，默认 D1 #0C0E14
#[cfg(target_os = "windows")]
static MAIN_BG_COLORREF: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(12u32 | (14u32 << 8) | (20u32 << 16));

#[cfg(target_os = "windows")]
const MAIN_NC_SUBCLASS_ID: usize = 0x424D_4E43;

/// 记录主窗口上一次非客户区/尺寸消息是否处于最大化态，用于「最大化 -> 还原默认窗口」瞬间主动触发一次原生 DWM 边框重绘
#[cfg(target_os = "windows")]
static MAIN_LAST_ZOOMED_NC: AtomicBool = AtomicBool::new(false);

/// 仅在主窗口处于最大化态（IsZoomed != 0）时使用当前主题背景色覆盖非客户区（先 ExcludeClipRect 保护客户区 WebView2），
/// 彻底消灭最大化恢复或自动隐藏任务栏 1px 留边时的纯白底边；
/// 严禁对非最大化（默认窗口）执行 GDI 非客户区涂色，完整保留 Windows 11 原生 DWM 圆角与透明缩放阴影边，杜绝左右/底边出现异常色带！
#[cfg(target_os = "windows")]
fn paint_main_window_non_client_area(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct WinPoint {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn IsZoomed(hwnd: isize) -> i32;
        fn GetWindowDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
        fn GetWindowRect(hwnd: isize, lp_rect: *mut WinRect) -> i32;
        fn GetClientRect(hwnd: isize, lp_rect: *mut WinRect) -> i32;
        fn ClientToScreen(hwnd: isize, lp_point: *mut WinPoint) -> i32;
        fn FillRect(hdc: isize, lprc: *const WinRect, hbr: isize) -> i32;
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateSolidBrush(color: u32) -> isize;
        fn DeleteObject(ho: isize) -> i32;
        fn ExcludeClipRect(hdc: isize, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    }

    unsafe {
        // 核心铁律：非最大化默认窗口的 left/right/bottom 存在 8~12px DWM 透明阴影与缩放热区，
        // 绝不能用 GDI FillRect 涂色覆盖！
        if IsZoomed(hwnd) == 0 {
            return;
        }
        let hdc = GetWindowDC(hwnd);
        if hdc == 0 {
            return;
        }
        let mut wr = WinRect::default();
        let mut cr = WinRect::default();
        if GetWindowRect(hwnd, &mut wr) != 0 && GetClientRect(hwnd, &mut cr) != 0 {
            let win_w = wr.right - wr.left;
            let win_h = wr.bottom - wr.top;
            let cw = cr.right - cr.left;
            let ch = cr.bottom - cr.top;
            if win_w > 0 && win_h > 0 {
                let mut tl = WinPoint { x: 0, y: 0 };
                if cw > 0 && ch > 0 && ClientToScreen(hwnd, &mut tl) != 0 {
                    let cx = tl.x - wr.left;
                    let cy = tl.y - wr.top;
                    let _ = ExcludeClipRect(hdc, cx, cy, cx + cw, cy + ch);
                }
                let brush = CreateSolidBrush(MAIN_BG_COLORREF.load(Ordering::Relaxed));
                if brush != 0 {
                    let rc_full = WinRect {
                        left: 0,
                        top: 0,
                        right: win_w,
                        bottom: win_h,
                    };
                    let _ = FillRect(hdc, &rc_full, brush);
                    let _ = DeleteObject(brush);
                }
            }
        }
        let _ = ReleaseDC(hwnd, hdc);
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn main_window_nc_subclass_proc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _uid_subclass: usize,
    _ref_data: usize,
) -> isize {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "comctl32")]
    extern "system" {
        fn DefSubclassProc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn RemoveWindowSubclass(
            hwnd: isize,
            pfn_subclass: Option<
                unsafe extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize,
            >,
            uid_subclass: usize,
        ) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn IsZoomed(hwnd: isize) -> i32;
        fn GetClientRect(hwnd: isize, lp_rect: *mut WinRect) -> i32;
        fn FillRect(hdc: isize, lprc: *const WinRect, hbr: isize) -> i32;
        fn RedrawWindow(
            hwnd: isize,
            lprc_update: *const WinRect,
            hrgn_update: isize,
            flags: u32,
        ) -> i32;
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateSolidBrush(color: u32) -> isize;
        fn DeleteObject(ho: isize) -> i32;
    }

    const WM_SIZE: u32 = 0x0005;
    const WM_ERASEBKGND: u32 = 0x0014;
    const WM_SHOWWINDOW: u32 = 0x0018;
    const WM_WINDOWPOSCHANGED: u32 = 0x0047;
    const WM_NCDESTROY: u32 = 0x0082;
    const WM_NCPAINT: u32 = 0x0085;
    const WM_NCACTIVATE: u32 = 0x0086;
    const RDW_INVALIDATE: u32 = 0x0001;
    const RDW_FRAME: u32 = 0x0400;

    match msg {
        WM_NCPAINT => {
            // 核心铁律（Windows 11 DWM 实测验证）：
            // 无论窗口当前是否最大化，WM_NCPAINT 都必须先原样透传给 DefSubclassProc / DefWindowProcW！
            // 一旦在任何状态（哪怕仅最大化态）直接 return 0 吞掉 WM_NCPAINT，Win32/DWM 会永久撤销该 HWND 的
            // 透明阴影扩展帧（DWMWA_EXTENDED_FRAME_BOUNDS 从 ~7px 归零），导致随后还原为默认窗口大小时
            // 左右/底边永久退化为 Windows 7 Aero Basic 蓝灰粗边框！
            // 先透传让 DWM 维持完整帧状态，再仅于最大化态补刷主题底色覆盖 1px 自动隐藏任务栏底边，二者兼得。
            let res = DefSubclassProc(hwnd, msg, wparam, lparam);
            if IsZoomed(hwnd) != 0 {
                paint_main_window_non_client_area(hwnd);
            }
            return res;
        }
        WM_NCACTIVATE => {
            // 始终原样透传 (wparam, lparam) 给 DefSubclassProc 维护原生 DWM 焦点与帧状态（绝不篡改 lparam = -1），
            // 最大化态返回后再补刷主题背景色杜绝底边漏白。
            let res = DefSubclassProc(hwnd, msg, wparam, lparam);
            if IsZoomed(hwnd) != 0 {
                paint_main_window_non_client_area(hwnd);
            }
            return res;
        }
        WM_ERASEBKGND => {
            let hdc = wparam as isize;
            if hdc != 0 {
                let mut rc = WinRect::default();
                if GetClientRect(hwnd, &mut rc) != 0 && rc.right > rc.left && rc.bottom > rc.top {
                    let brush = CreateSolidBrush(MAIN_BG_COLORREF.load(Ordering::Relaxed));
                    if brush != 0 {
                        let _ = FillRect(hdc, &rc, brush);
                        let _ = DeleteObject(brush);
                        return 1;
                    }
                }
            }
        }
        WM_SHOWWINDOW | WM_WINDOWPOSCHANGED | WM_SIZE => {
            let res = DefSubclassProc(hwnd, msg, wparam, lparam);
            let zoomed_now = IsZoomed(hwnd) != 0;
            let was_zoomed = MAIN_LAST_ZOOMED_NC.swap(zoomed_now, Ordering::Relaxed);
            if zoomed_now {
                paint_main_window_non_client_area(hwnd);
            } else if was_zoomed {
                // 从最大化切回默认窗口尺寸的瞬间，主动刷新一次非客户区框架，确保 DWM 清除旧缓存、呈现无瑕圆角与柔影
                let _ = RedrawWindow(
                    hwnd,
                    std::ptr::null(),
                    0,
                    RDW_FRAME | RDW_INVALIDATE,
                );
            }
            return res;
        }
        WM_NCDESTROY => {
            MAIN_LAST_ZOOMED_NC.store(false, Ordering::Relaxed);
            let _ = RemoveWindowSubclass(
                hwnd,
                Some(main_window_nc_subclass_proc),
                MAIN_NC_SUBCLASS_ID,
            );
            return DefSubclassProc(hwnd, msg, wparam, lparam);
        }
        _ => {}
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[cfg(target_os = "windows")]
pub(crate) fn install_nc_paint_subclass_on_hwnd(hwnd: isize) {
    #[link(name = "comctl32")]
    extern "system" {
        fn SetWindowSubclass(
            hwnd: isize,
            pfn_subclass: Option<
                unsafe extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize,
            >,
            uid_subclass: usize,
            dw_ref_data: usize,
        ) -> i32;
    }
    if hwnd != 0 {
        unsafe {
            let _ = SetWindowSubclass(
                hwnd,
                Some(main_window_nc_subclass_proc),
                MAIN_NC_SUBCLASS_ID,
                0,
            );
        }
        paint_main_window_non_client_area(hwnd);
    }
}

#[cfg(target_os = "windows")]
fn install_main_window_nc_paint_subclass(win: &tauri::WebviewWindow) {
    if let Ok(raw_hwnd) = win.hwnd() {
        install_nc_paint_subclass_on_hwnd(raw_hwnd.0 as isize);
    }
}

#[cfg(not(target_os = "windows"))]
fn install_main_window_nc_paint_subclass(_win: &tauri::WebviewWindow) {}

/// 在 Windows 11 上禁用主窗口的 1px 系统浅灰描边（保留柔和窗口阴影），彻底杜绝无边框最大化恢复后底边亮线
#[cfg(target_os = "windows")]
fn apply_main_window_zero_border_dwm(win: &tauri::WebviewWindow) {
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: isize,
            dw_attribute: u32,
            pv_attribute: *const std::ffi::c_void,
            cb_attribute: u32,
        ) -> i32;
    }

    const DWMWA_BORDER_COLOR: u32 = 34;
    const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;

    if let Ok(raw_hwnd) = win.hwnd() {
        let hwnd = raw_hwnd.0 as isize;
        if hwnd != 0 {
            unsafe {
                let border_none: u32 = DWMWA_COLOR_NONE;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_BORDER_COLOR,
                    &border_none as *const u32 as *const std::ffi::c_void,
                    std::mem::size_of::<u32>() as u32,
                );
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_main_window_zero_border_dwm(_win: &tauri::WebviewWindow) {}

#[cfg(target_os = "windows")]
fn is_hwnd_zoomed(win: &tauri::WebviewWindow) -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn IsZoomed(hwnd: isize) -> i32;
    }
    if let Ok(raw_hwnd) = win.hwnd() {
        let hwnd = raw_hwnd.0 as isize;
        if hwnd != 0 {
            return unsafe { IsZoomed(hwnd) != 0 };
        }
    }
    win.is_maximized().unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_hwnd_zoomed(win: &tauri::WebviewWindow) -> bool {
    win.is_maximized().unwrap_or(false)
}

/// 同步主窗口原生每窗背景色（由 tao 在 WM_ERASEBKGND 按窗口绘制，严禁篡改共享 WNDCLASS 画刷污染透明子窗口）、
/// 非客户区防白边着色画刷、DWM 无描边属性与 WebView2 DefaultBackgroundColor，彻底杜绝深色主题下窗口边缘或重绘间隙漏出白色底板
fn apply_main_window_bg_color(win: &tauri::WebviewWindow, r: u8, g: u8, b: u8) {
    let _ = win.set_background_color(Some(tauri::window::Color(r, g, b, 255)));
    #[cfg(target_os = "windows")]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2Controller2, COREWEBVIEW2_COLOR,
        };
        use windows_core::Interface;

        let colorref: u32 = (r as u32) | ((g as u32) << 8) | ((b as u32) << 16);
        MAIN_BG_COLORREF.store(colorref, Ordering::Relaxed);

        apply_main_window_zero_border_dwm(win);
        if let Ok(raw_hwnd) = win.hwnd() {
            paint_main_window_non_client_area(raw_hwnd.0 as isize);
        }

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

fn parse_hex_color_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
        let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
        let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
        return Some((r, g, b));
    }
    if clean.len() == 3 {
        let r = u8::from_str_radix(&clean[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&clean[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&clean[2..3], 16).ok()? * 17;
        return Some((r, g, b));
    }
    None
}

fn current_motion_style(state: &AppState, override_level: Option<&str>) -> window_transition::TransitionMotionStyle {
    if let Some(raw) = override_level {
        return window_transition::TransitionMotionStyle::from_level_str(raw);
    }
    let raw = state
        .motion_level
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_else(|| "best".to_string());
    window_transition::TransitionMotionStyle::from_level_str(&raw)
}

fn current_motion_level_str(state: &AppState, override_level: Option<&str>) -> String {
    match current_motion_style(state, override_level) {
        window_transition::TransitionMotionStyle::None => "none".to_string(),
        window_transition::TransitionMotionStyle::Standard => "standard".to_string(),
        window_transition::TransitionMotionStyle::Rich => "best".to_string(),
    }
}

/// 计算 Mini 小窗当前肉眼可见实体（自由悬浮 436x184 卡片 / 上下 332x34 灵动岛 / 左右 46x96 黑胶耳）的精确物理屏幕矩形
fn compute_mini_visual_entity_rect(
    mini_win: &tauri::WebviewWindow,
    edge: &str,
    expanded: bool,
) -> Option<window_transition::RectPx> {
    let (Ok(pos), Ok(size)) = (mini_win.outer_position(), mini_win.outer_size()) else {
        return None;
    };
    let sf = mini_win.scale_factor().unwrap_or(1.0).max(1.0);
    let wx = pos.x as f64;
    let wy = pos.y as f64;
    let ww = size.width as f64;
    let wh = size.height as f64;

    if expanded || edge == "none" {
        let cw = 436.0 * sf;
        let ch = 184.0 * sf;
        let left = (wx + (ww - cw) * 0.5).round() as i32;
        let top = (wy + (wh - ch) * 0.5).round() as i32;
        return Some(window_transition::RectPx {
            left,
            top,
            right: left + cw.round() as i32,
            bottom: top + ch.round() as i32,
        });
    }

    match edge {
        "top" => {
            let pw = 332.0 * sf;
            let ph = 34.0 * sf;
            let left = (wx + (ww - pw) * 0.5).round() as i32;
            let top = wy.round() as i32;
            Some(window_transition::RectPx {
                left,
                top,
                right: left + pw.round() as i32,
                bottom: top + ph.round() as i32,
            })
        }
        "bottom" => {
            let pw = 332.0 * sf;
            let ph = 34.0 * sf;
            let left = (wx + (ww - pw) * 0.5).round() as i32;
            let bottom = (wy + wh).round() as i32;
            Some(window_transition::RectPx {
                left,
                top: bottom - ph.round() as i32,
                right: left + pw.round() as i32,
                bottom,
            })
        }
        "left" => {
            let pw = 46.0 * sf;
            let ph = 96.0 * sf;
            let left = (wx + 4.0 * sf).round() as i32;
            let top = (wy + (wh - ph) * 0.5).round() as i32;
            Some(window_transition::RectPx {
                left,
                top,
                right: left + pw.round() as i32,
                bottom: top + ph.round() as i32,
            })
        }
        "right" => {
            let pw = 46.0 * sf;
            let ph = 96.0 * sf;
            let right = (wx + ww - 4.0 * sf).round() as i32;
            let top = (wy + (wh - ph) * 0.5).round() as i32;
            Some(window_transition::RectPx {
                left: right - pw.round() as i32,
                top,
                right,
                bottom: top + ph.round() as i32,
            })
        }
        _ => None,
    }
}

/// 原子切换 Mini 透明小窗的点击穿透态：
/// Windows 下直接操作 GWL_EXSTYLE（保 WS_EX_TOOLWINDOW、清 WS_EX_APPWINDOW），绝不走 tao::set_ignore_cursor_events
/// 以彻底避免 tao apply_diff 篡改可见态（误调用 ShowWindow(SW_HIDE)）或抢夺前台焦点。
fn set_mini_click_through(win: &tauri::WebviewWindow, ignore: bool) {
    #[cfg(target_os = "windows")]
    {
        if let Ok(raw) = win.hwnd() {
            window_transition::win_impl::set_overlay_click_through_raw(raw.0 as isize, ignore);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = win.set_ignore_cursor_events(ignore);
    }
}

/// 必须在主线程调用：安全显示 Mini 独立悬浮小窗，同步 tao WindowFlags::VISIBLE = true，
/// 并立即加固 WS_EX_TOOLWINDOW + 零边框零阴影零圆角 + 目标点击穿透态 + HWND_TOPMOST。
fn show_mini_player_window_safely(win: &tauri::WebviewWindow, click_through: bool) {
    set_subwindow_suspended(win, false);
    set_webview_memory_target_low(win, false);
    set_mini_click_through(win, click_through);
    let _ = win.show();
    enforce_transparent_overlay_window_style(win);
    set_mini_click_through(win, click_through);
    #[cfg(target_os = "windows")]
    {
        if let Ok(raw) = win.hwnd() {
            window_transition::win_impl::show_overlay_window_native(raw.0 as isize, click_through);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = win.set_always_on_top(true);
    }
}

/// 必须在主线程调用：仅执行底层 HWND 隐藏（开启点击穿透 + tao win.hide() + Win32 强制 SW_HIDE + 去置顶），不触发挂起定时器
fn hide_mini_player_hwnd_only(win: &tauri::WebviewWindow) {
    set_mini_click_through(win, true);
    let _ = win.hide();
    #[cfg(target_os = "windows")]
    {
        if let Ok(raw) = win.hwnd() {
            window_transition::win_impl::hide_overlay_window_native(raw.0 as isize);
        }
    }
}

/// 必须在主线程调用：四重防线彻底隐藏 Mini 独立悬浮小窗，杜绝任何情况下残留透明遮挡幽灵窗：
/// 1. 立即开启点击穿透（WS_EX_TRANSPARENT | WS_EX_LAYERED）
/// 2. 同步 tao win.hide() 更新 WindowFlags::VISIBLE = false
/// 3. 原生 Win32 强制 ShowWindow(SW_HIDE) + SetWindowPos(HWND_NOTOPMOST, SWP_HIDEWINDOW)（绝不受 tao 缓存漂移影响）
/// 4. 可选重置前端阶段为 hidden 并延迟挂起 WebView2 省内存
fn hide_mini_player_window_safely(
    app: &AppHandle,
    win: &tauri::WebviewWindow,
    vis_flag: Arc<AtomicBool>,
    emit_hidden: bool,
) {
    hide_mini_player_hwnd_only(win);
    if emit_hidden {
        let _ = app.emit_to(
            "mini_player",
            "bm-mini-player-transition",
            serde_json::json!({
                "phase": "hidden",
                "instant": true,
            }),
        );
    }
    schedule_subwindow_suspend(app, "mini_player", vis_flag);
}

/// 纯原生安全隐藏主窗口（进入小窗或收起至托盘），彻底避开 tao::set_visible 的最大化白边陷阱，并同步清理任何残留 GPU 过渡舞台
fn hide_main_window_cleanly(win: &tauri::WebviewWindow, state: &AppState) {
    state.main_transition_gen.fetch_add(1, Ordering::AcqRel);
    #[cfg(target_os = "windows")]
    {
        let app = win.app_handle().clone();
        let max_flag = state.main_was_maximized.clone();
        let low_mem = state.main_low_memory_active.clone();
        let app_ui = app.clone();
        let _ = app.run_on_main_thread(move || {
            window_transition::win_impl::cancel_and_hide_gpu_stage_on_main_thread();
            if let Some(main_win) = app_ui.get_webview_window("main") {
                if let Ok(raw) = main_win.hwnd() {
                    let hwnd = raw.0 as isize;
                    if window_transition::win_impl::is_hwnd_visible_and_not_minimized(hwnd)
                        && !window_transition::win_impl::is_main_cloaked()
                    {
                        let is_max = window_transition::win_impl::is_hwnd_maximized_native(hwnd);
                        max_flag.store(is_max, Ordering::Relaxed);
                    }
                }
                sync_main_low_memory_state(&main_win, &low_mem, true);
                window_transition::win_impl::hide_main_window_native(&main_win);
            }
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        let is_max = is_hwnd_zoomed(win);
        state.main_was_maximized.store(is_max, Ordering::Relaxed);
        sync_main_low_memory_state(win, &state.main_low_memory_active, true);
        let _ = win.hide();
    }
}

/// 统一的主窗口恢复与 GPU 空间展开编排：
/// - 无论是最大化还是默认窗口、无论来自 Mini 小窗还是系统托盘，均走同一套零白线、零跳变管线。
/// - 最大化主窗口直接在 DWMWA_CLOAK 隐身斗篷下一步 SW_SHOWMAXIMIZED 恢复并校准客户区与 WebView2 Bounds，
///   再由 DirectComposition GPU 舞台从 Mini 方向平滑圆角消融舒展铺满全屏并 65ms 丝滑无痕揭幕，
///   彻底杜绝「先变回默认窗口大小再快速最大化」与底部白线！
/// - 退出 Mini 第 0ms 立即对 Mini 施加点击穿透，退场完成后通过四重防线强制原生 SW_HIDE + 去置顶，彻底杜绝透明遮挡幽灵窗。
fn orchestrate_restore_main_window(app: &AppHandle, state: &AppState, from_mini: bool) {
    let gen = state.main_transition_gen.fetch_add(1, Ordering::AcqRel) + 1;
    let was_mini_visible = state.mini_player_visible.swap(false, Ordering::AcqRel);
    let effective_from_mini = from_mini || was_mini_visible;
    state
        .mini_yielded_for_fullscreen
        .store(false, Ordering::Relaxed);
    state.mini_snap_anim_gen.fetch_add(1, Ordering::Relaxed);
    wake_hotkey_thread(&state.hotkey_thread_id);

    let motion_style = current_motion_style(state, None);
    let motion_str = current_motion_level_str(state, None);
    let app_for_main = app.clone();
    let gen_ref = state.main_transition_gen.clone();
    let mini_vis_ref = state.mini_player_visible.clone();
    let mini_exiting_ref = state.mini_exiting_anim.clone();
    let hotkey_tid_ref = state.hotkey_thread_id.clone();
    let low_mem_ref = state.main_low_memory_active.clone();
    let max_flag_ref = state.main_was_maximized.clone();
    let dock_edge = state
        .mini_player_dock_edge
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_else(|| "none".to_string());
    let mini_expanded = state.mini_player_expanded.load(Ordering::Relaxed);
    let (accent1_rgb, accent2_rgb) = state
        .cover_accent_rgb
        .lock()
        .ok()
        .map(|g| *g)
        .unwrap_or(((94, 106, 210), (45, 212, 191)));

    let _ = app.run_on_main_thread(move || {
        if gen_ref.load(Ordering::Acquire) != gen {
            return;
        }

        #[cfg(target_os = "windows")]
        {
            use window_transition::win_impl as wt;

            // 先清理可能残留在屏上的旧过渡舞台
            wt::cancel_and_hide_gpu_stage_on_main_thread();

            let mini_win_opt = app_for_main.get_webview_window("mini_player");

            let Some(main_win) = app_for_main.get_webview_window("main") else {
                if let Some(ref mw) = mini_win_opt {
                    if !mini_vis_ref.load(Ordering::Acquire) {
                        mini_exiting_ref.store(false, Ordering::Release);
                        hide_mini_player_window_safely(&app_for_main, mw, mini_vis_ref.clone(), true);
                    }
                }
                return;
            };
            let Ok(raw_main) = main_win.hwnd() else {
                return;
            };
            let main_hwnd = raw_main.0 as isize;

            // 若主窗口当前已经完全可见且未最小化，并且并非从小窗退回，则确保无残留 Mini 幽灵窗后直接置前聚焦
            if !effective_from_mini && wt::is_hwnd_visible_and_not_minimized(main_hwnd) {
                if !mini_vis_ref.load(Ordering::Acquire) {
                    mini_exiting_ref.store(false, Ordering::Release);
                    if let Some(ref mw) = mini_win_opt {
                        hide_mini_player_window_safely(&app_for_main, mw, mini_vis_ref.clone(), true);
                    }
                }
                wt::reveal_main_window_native(&main_win, true, paint_main_window_non_client_area);
                let _ = app_for_main.emit(
                    "bm-main-window-restored",
                    serde_json::json!({ "maximized": wt::is_hwnd_maximized_native(main_hwnd) }),
                );
                return;
            }

            // 读取退出前 Mini 实体位置（在隐藏 Mini 前采样，作为主窗口舒展归位的空间锚点）
            let mini_hwnd = mini_win_opt
                .as_ref()
                .and_then(|w| w.hwnd().ok())
                .map(|h| h.0 as isize)
                .unwrap_or(0);
            let mini_entity_rect = mini_win_opt
                .as_ref()
                .and_then(|w| compute_mini_visual_entity_rect(w, &dock_edge, mini_expanded));

            // 第 0ms 立即令 Mini 进入点击穿透态（即便在短暂 128ms 退场淡出期间也绝不拦截桌面或主窗口任何点击！）
            if let Some(ref mw) = mini_win_opt {
                if !mini_vis_ref.load(Ordering::Acquire) {
                    set_mini_click_through(mw, true);
                }
            }

            let expect_max = max_flag_ref.load(Ordering::Relaxed)
                || wt::is_hwnd_maximized_native(main_hwnd);

            // 1. 在 DWMWA_CLOAK = 1 隐身斗篷下，一步到位原子恢复主窗口并校准客户区/WebView2 Bounds（0 弹跳、0 白线）
            sync_main_low_memory_state(&main_win, &low_mem_ref, false);
            let restored_client_rect = wt::restore_main_window_cloaked_native(
                &main_win,
                expect_max,
                paint_main_window_non_client_area,
            );
            let final_is_max = wt::is_hwnd_maximized_native(main_hwnd);
            max_flag_ref.store(final_is_max, Ordering::Relaxed);

            let main_rect = restored_client_rect
                .or_else(|| wt::get_window_client_screen_rect(main_hwnd))
                .or_else(|| wt::get_window_visual_rect(main_hwnd));

            let sf = main_win.scale_factor().unwrap_or(1.0).max(1.0);
            let same_mon = effective_from_mini && wt::is_same_monitor(main_hwnd, mini_hwnd);
            let kind = if effective_from_mini {
                window_transition::TransitionKind::ExitMini
            } else {
                window_transition::TransitionKind::RestoreFromTray
            };

            let plan = main_rect.map(|mr| {
                window_transition::compute_spatial_transition_plan(
                    kind,
                    mr,
                    mini_entity_rect,
                    final_is_max,
                    same_mon,
                    sf,
                    motion_style,
                )
            });

            // 2. 第 0ms 通知 Mini 前端播放优雅的释能退场动效（或在无动效档位/非小窗退回下立即原生强杀隐藏）
            if effective_from_mini {
                let _ = app_for_main.emit(
                    "bm-mini-player-visibility",
                    serde_json::json!({ "visible": false }),
                );
                let should_animate_mini_exit = plan.map(|p| p.duration_ms > 0).unwrap_or(false)
                    && mini_hwnd != 0
                    && wt::is_hwnd_visible_and_not_minimized(mini_hwnd);
                if should_animate_mini_exit {
                    if let Some(p) = plan {
                        mini_exiting_ref.store(true, Ordering::Release);
                        let _ = app_for_main.emit_to(
                            "mini_player",
                            "bm-mini-player-transition",
                            serde_json::json!({
                                "phase": "exiting",
                                "offsetX": p.mini_hint_offset_x,
                                "offsetY": p.mini_hint_offset_y,
                                "dockEdge": dock_edge,
                                "motionLevel": motion_str,
                                "transitionGen": gen,
                            }),
                        );
                    }
                } else if let Some(ref mw) = mini_win_opt {
                    mini_exiting_ref.store(false, Ordering::Release);
                    hide_mini_player_window_safely(&app_for_main, mw, mini_vis_ref.clone(), true);
                }
            } else if let Some(ref mw) = mini_win_opt {
                if !mini_vis_ref.load(Ordering::Acquire) {
                    mini_exiting_ref.store(false, Ordering::Release);
                    hide_mini_player_window_safely(&app_for_main, mw, mini_vis_ref.clone(), true);
                }
            }

            let _ = app_for_main.emit(
                "bm-main-window-restored",
                serde_json::json!({ "maximized": final_is_max }),
            );

            // 3. 若无动效档位或未能获取有效几何矩形，直接原子揭开真实主窗口（0 闪烁、0 弹跳、0 白线）
            let (Some(mr), Some(p)) = (main_rect, plan) else {
                wt::reveal_main_window_native(&main_win, true, paint_main_window_non_client_area);
                return;
            };
            if p.duration_ms == 0 {
                wt::reveal_main_window_native(&main_win, true, paint_main_window_non_client_area);
                return;
            }

            // 4. 三级智能优选 GPU 舒展纹理（优先实帧 -> 同尺寸同主题高清快照 -> 四主题双主色流光卡面，100% 绝不闪黑屏）
            let stage_rect = wt::get_monitor_rects_for_hwnd(main_hwnd)
                .map(|(mon, _work)| mon)
                .unwrap_or(mr);
            let bg_colorref = MAIN_BG_COLORREF.load(Ordering::Relaxed);
            let bg_rgb = wt::colorref_to_rgb(bg_colorref);
            let target_w = mr.width().max(16) as u32;
            let target_h = mr.height().max(16) as u32;

            let (tex_w, tex_h, bgra) =
                if let Some((cw, ch, live_px)) = wt::capture_window_client_bgra(main_hwnd, false) {
                    (cw, ch, live_px)
                } else if let Some(cached) =
                    window_transition::load_cached_main_snapshot(target_w, target_h, bg_colorref)
                {
                    (cached.width, cached.height, cached.bgra_premultiplied)
                } else {
                    // 1/2 降采样合成流光底板，GPU 自动线性放大 + 硬件圆角，速度 < 1ms 且极度顺滑
                    let sw = (target_w / 2).clamp(64, 960);
                    let sh = (target_h / 2).clamp(64, 540);
                    let px = window_transition::synthesize_theme_aura_card_bgra(
                        sw,
                        sh,
                        bg_rgb,
                        accent1_rgb,
                        accent2_rgb,
                    );
                    (sw, sh, px)
                };

            let started = wt::play_gpu_transition_on_main_thread(
                stage_rect, mr, tex_w, tex_h, &bgra, p,
            );

            // 若是从 Mini 退回且 Mini 正在播放 115ms 释能淡出动效，将其保持在 GPU 舞台正上方（已开启点击穿透，绝不挡点击）
            if started && effective_from_mini && mini_hwnd != 0 && mini_exiting_ref.load(Ordering::Acquire) {
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
                const HWND_TOPMOST: isize = -1;
                const SWP_NOSIZE: u32 = 0x0001;
                const SWP_NOMOVE: u32 = 0x0002;
                const SWP_NOACTIVATE: u32 = 0x0010;
                unsafe {
                    let _ = SetWindowPos(
                        mini_hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }

            if !started {
                wt::reveal_main_window_native(&main_win, true, paint_main_window_non_client_area);
                if let Some(ref mw) = mini_win_opt {
                    if !mini_vis_ref.load(Ordering::Acquire) {
                        mini_exiting_ref.store(false, Ordering::Release);
                        hide_mini_player_window_safely(
                            &app_for_main,
                            mw,
                            mini_vis_ref.clone(),
                            true,
                        );
                    }
                }
                return;
            }

            // 5. 异步收尾时序：
            //    - 128ms：只要 mini_player_visible 仍为 false（不因重复点击恢复而跳过！），必执行四重防线强制原生隐藏 Mini
            //    - p.duration_ms：GPU 舞台 100% 像素级重合归位 -> 揭开真实主窗口 + 触发 65ms 丝滑交叉溶解揭幕
            //    - p.duration_ms + p.reveal_dissolve_ms + 20ms：隐藏并释放 GPU 过渡舞台
            if effective_from_mini && mini_exiting_ref.load(Ordering::Acquire) {
                let app_hide_mini = app_for_main.clone();
                let mini_vis_check = mini_vis_ref.clone();
                let mini_exit_check = mini_exiting_ref.clone();
                let hotkey_tid_done = hotkey_tid_ref.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(128)).await;
                    if mini_vis_check.load(Ordering::Acquire) {
                        mini_exit_check.store(false, Ordering::Release);
                        return;
                    }
                    let app_ui = app_hide_mini.clone();
                    let vis_flag = mini_vis_check.clone();
                    let exit_flag = mini_exit_check.clone();
                    let tid_flag = hotkey_tid_done.clone();
                    let _ = app_hide_mini.run_on_main_thread(move || {
                        exit_flag.store(false, Ordering::Release);
                        if !vis_flag.load(Ordering::Acquire) {
                            if let Some(mw) = app_ui.get_webview_window("mini_player") {
                                hide_mini_player_window_safely(&app_ui, &mw, vis_flag, true);
                            }
                        }
                        wake_hotkey_thread(&tid_flag);
                    });
                });
            }

            let app_reveal = app_for_main.clone();
            let gen_reveal = gen_ref.clone();
            let mini_vis_reveal = mini_vis_ref.clone();
            let mini_exit_reveal = mini_exiting_ref.clone();
            let dur_ms = p.duration_ms;
            let dissolve_ms = p.reveal_dissolve_ms;
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(dur_ms)).await;
                if gen_reveal.load(Ordering::Acquire) != gen {
                    return;
                }
                let app_ui = app_reveal.clone();
                let gen_ui = gen_reveal.clone();
                let vis_ui = mini_vis_reveal.clone();
                let exit_ui = mini_exit_reveal.clone();
                let _ = app_reveal.run_on_main_thread(move || {
                    if gen_ui.load(Ordering::Acquire) != gen {
                        return;
                    }
                    // 终态双保险：揭开主窗口时若 Mini 仍未隐藏且 mini_player_visible=false，立即强制隐藏
                    if !vis_ui.load(Ordering::Acquire) {
                        exit_ui.store(false, Ordering::Release);
                        if let Some(mini_w) = app_ui.get_webview_window("mini_player") {
                            hide_mini_player_window_safely(&app_ui, &mini_w, vis_ui, false);
                        }
                    }
                    if let Some(mw) = app_ui.get_webview_window("main") {
                        wt::reveal_main_window_native(&mw, true, paint_main_window_non_client_area);
                    }
                    if dissolve_ms > 0 {
                        wt::start_gpu_cross_dissolve_on_main_thread(dissolve_ms);
                    } else {
                        wt::cancel_and_hide_gpu_stage_on_main_thread();
                    }
                });

                if dissolve_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(dissolve_ms + 22)).await;
                    if gen_reveal.load(Ordering::Acquire) != gen {
                        return;
                    }
                    let gen_cleanup = gen_reveal.clone();
                    let _ = app_reveal.run_on_main_thread(move || {
                        if gen_cleanup.load(Ordering::Acquire) == gen {
                            wt::cancel_and_hide_gpu_stage_on_main_thread();
                        }
                    });
                }
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (&motion_str, &dock_edge, mini_expanded, accent1_rgb, accent2_rgb, &mini_exiting_ref, &hotkey_tid_ref);
            if effective_from_mini {
                if let Some(mw) = app_for_main.get_webview_window("mini_player") {
                    hide_mini_player_window_safely(&app_for_main, &mw, mini_vis_ref.clone(), true);
                }
                let _ = app_for_main.emit(
                    "bm-mini-player-visibility",
                    serde_json::json!({ "visible": false }),
                );
            }
            if let Some(main_win) = app_for_main.get_webview_window("main") {
                sync_main_low_memory_state(&main_win, &low_mem_ref, false);
                if main_win.is_minimized().unwrap_or(false) {
                    let _ = main_win.unminimize();
                }
                let _ = main_win.show();
                let _ = main_win.set_focus();
                let _ = app_for_main.emit(
                    "bm-main-window-restored",
                    serde_json::json!({ "maximized": main_win.is_maximized().unwrap_or(false) }),
                );
            }
        }
    });
}

fn show_and_restore_main_window(app: &AppHandle, state: &AppState) {
    orchestrate_restore_main_window(app, state, false);
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

#[tauri::command]
async fn sync_motion_level_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    level: String,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    let normalized = match level.trim().to_ascii_lowercase().as_str() {
        "none" => "none",
        "standard" => "standard",
        _ => "best",
    };
    if let Ok(mut g) = state.motion_level.lock() {
        *g = normalized.to_string();
    }
    Ok(true)
}

#[tauri::command]
async fn hide_main_to_tray_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    hide_main_window_cleanly(&window, &state);
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
async fn enrich_song_for_share_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    song: Song,
) -> Result<Song, String> {
    ensure_main_window(&window)?;
    Ok(state.api.enrich_song_for_share(&song).await)
}

#[tauri::command]
async fn enrich_playlist_for_share_cmd(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    playlist: Playlist,
) -> Result<Playlist, String> {
    ensure_main_window(&window)?;
    Ok(state.api.enrich_playlist_for_share(&playlist).await)
}

/// 校验外部打开 URL 安全性（阻断 file://、javascript:、cmd 注入、内网 IP 与 URL 凭据；仅放行公网 HTTP(S) 及酷狗官方唤端 Scheme）
fn is_safe_external_open_url(raw_url: &str) -> bool {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() || trimmed.len() > 4096 || trimmed.bytes().any(|b| b < 0x20 || b == b'"')
    {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("kugou://start.weixin?") || lower.starts_with("kugouyouth://start.weixin?")
    {
        return true;
    }
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return false;
    }
    if !kugou::transport::KugouTransport::is_safe_external_url(trimmed, false) {
        return false;
    }
    if let Ok(parsed) = reqwest::Url::parse(trimmed) {
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return false;
        }
        true
    } else {
        false
    }
}

#[tauri::command]
async fn open_external_url_cmd(
    window: tauri::WebviewWindow,
    url: String,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    let trimmed = url.trim().to_string();
    if !is_safe_external_open_url(&trimmed) {
        return Err("已拦截不安全的外部链接地址".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(
                hwnd: isize,
                lp_operation: *const u16,
                lp_file: *const u16,
                lp_parameters: *const u16,
                lp_directory: *const u16,
                n_show_cmd: i32,
            ) -> isize;
        }
        let op: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();
        let target: Vec<u16> = OsStr::new(&trimmed).encode_wide().chain(Some(0)).collect();
        let rc = unsafe {
            ShellExecuteW(
                0,
                op.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if rc <= 32 {
            return Err("无法调用系统默认浏览器打开该链接".to_string());
        }
        Ok(true)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = trimmed;
        Err("当前平台暂不支持外部链接打开".to_string())
    }
}

/// Windows 原生剪贴板文本安全读取（零第三方依赖、无 WebView2 弹窗授权拦截，受前端设置开关严格管控，上限 16384 字符）
#[tauri::command]
async fn read_clipboard_text_cmd(window: tauri::WebviewWindow) -> Result<String, String> {
    ensure_main_window(&window)?;
    #[cfg(target_os = "windows")]
    {
        #[link(name = "user32")]
        extern "system" {
            fn OpenClipboard(hwnd_new_owner: isize) -> i32;
            fn CloseClipboard() -> i32;
            fn IsClipboardFormatAvailable(format: u32) -> i32;
            fn GetClipboardData(u_format: u32) -> isize;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalLock(h_mem: isize) -> *const u16;
            fn GlobalUnlock(h_mem: isize) -> i32;
        }
        const CF_UNICODETEXT: u32 = 13;
        const MAX_CLIPBOARD_U16: usize = 16384;

        let res = tokio::task::spawn_blocking(move || -> Result<String, String> {
            let mut opened = false;
            for _ in 0..4 {
                if unsafe { OpenClipboard(0) } != 0 {
                    opened = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(18));
            }
            if !opened {
                return Ok(String::new());
            }
            let out = unsafe {
                if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
                    String::new()
                } else {
                    let handle = GetClipboardData(CF_UNICODETEXT);
                    if handle == 0 {
                        String::new()
                    } else {
                        let ptr = GlobalLock(handle);
                        if ptr.is_null() {
                            String::new()
                        } else {
                            let mut len = 0usize;
                            while len < MAX_CLIPBOARD_U16 && *ptr.add(len) != 0 {
                                len += 1;
                            }
                            let slice = std::slice::from_raw_parts(ptr, len);
                            let text = String::from_utf16_lossy(slice);
                            let _ = GlobalUnlock(handle);
                            text
                        }
                    }
                }
            };
            unsafe {
                let _ = CloseClipboard();
            }
            Ok(out)
        })
        .await
        .map_err(|e| e.to_string())??;
        Ok(res)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(String::new())
    }
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

/// 精准识别与 Mini 灵动岛处于同一显示器的全屏独占应用（如全屏视频、全屏游戏、全屏 PPT），
/// 显式排除 Windows 桌面/任务栏/壳层与本应用自身窗体，避免 Win+D 显示桌面被误判为全屏。
#[cfg(target_os = "windows")]
fn is_foreground_fullscreen_on_same_monitor(
    mini_hwnd: isize,
    main_hwnd: isize,
    lyric_hwnd: isize,
) -> bool {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct WinMonitorInfo {
        cb_size: u32,
        rc_monitor: WinRect,
        rc_work: WinRect,
        dw_flags: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn IsWindowVisible(hwnd: isize) -> i32;
        fn IsIconic(hwnd: isize) -> i32;
        fn GetClassNameW(hwnd: isize, lp_class_name: *mut u16, n_max_count: i32) -> i32;
        fn GetWindowRect(hwnd: isize, lp_rect: *mut WinRect) -> i32;
        fn MonitorFromWindow(hwnd: isize, dw_flags: u32) -> isize;
        fn GetMonitorInfoW(h_monitor: isize, lpmi: *mut WinMonitorInfo) -> i32;
    }

    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    if mini_hwnd == 0 {
        return false;
    }

    unsafe {
        let fg = GetForegroundWindow();
        if fg == 0 || fg == mini_hwnd || fg == main_hwnd || fg == lyric_hwnd {
            return false;
        }
        if IsWindowVisible(fg) == 0 || IsIconic(fg) != 0 {
            return false;
        }

        let mut class_buf = [0u16; 128];
        let len = GetClassNameW(fg, class_buf.as_mut_ptr(), class_buf.len() as i32);
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
            if matches!(
                class_name.as_str(),
                "Progman"
                    | "WorkerW"
                    | "Shell_TrayWnd"
                    | "Shell_SecondaryTrayWnd"
                    | "NotifyIconOverflowWindow"
                    | "Windows.UI.Core.CoreWindow"
                    | "TaskListThumbnailWnd"
                    | "MultitaskingViewFrame"
                    | "XamlExplorerHostIslandWindow"
            ) {
                return false;
            }
        }

        let mini_mon = MonitorFromWindow(mini_hwnd, MONITOR_DEFAULTTONEAREST);
        let fg_mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        if mini_mon == 0 || fg_mon == 0 || mini_mon != fg_mon {
            return false;
        }

        let mut fg_rect = WinRect::default();
        if GetWindowRect(fg, &mut fg_rect) == 0 {
            return false;
        }
        let mut mi = WinMonitorInfo {
            cb_size: std::mem::size_of::<WinMonitorInfo>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(fg_mon, &mut mi) == 0 {
            return false;
        }

        let m = mi.rc_monitor;
        let covers_full_monitor = fg_rect.left <= m.left + 2
            && fg_rect.top <= m.top + 2
            && fg_rect.right >= m.right - 2
            && fg_rect.bottom >= m.bottom - 2;
        covers_full_monitor
    }
}

#[cfg(not(target_os = "windows"))]
fn is_foreground_fullscreen_on_same_monitor(
    _mini_hwnd: isize,
    _main_hwnd: isize,
    _lyric_hwnd: isize,
) -> bool {
    false
}

/// 校验 Mini 灵动岛是否因切屏/拔出显示器/DPI 变更而跑出有效工作区；如越界或强制重吸附则自动拉回安全边缘。
fn reclamp_mini_player_if_needed(
    app: &AppHandle,
    win: &tauri::WebviewWindow,
    state: &AppState,
    force_resnap: bool,
) {
    let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else {
        return;
    };
    let rect = PhysRect {
        left: pos.x,
        top: pos.y,
        right: pos.x + size.width as i32,
        bottom: pos.y + size.height as i32,
    };

    let areas: Vec<PhysRect> = win
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let wa = m.work_area();
            PhysRect {
                left: wa.position.x,
                top: wa.position.y,
                right: wa.position.x + wa.size.width as i32,
                bottom: wa.position.y + wa.size.height as i32,
            }
        })
        .collect();

    let is_visible_in_monitor =
        !areas.is_empty() && is_rect_visible_in_any_work_area(rect, &areas, 36);

    let edge = state
        .mini_player_dock_edge
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_else(|| "none".to_string());

    if !force_resnap && is_visible_in_monitor {
        // 即使还在可见区域，若是贴边状态也检查一下是否已严重脱离该边缘（如分辨率变更或任务栏高度变化）
        let center_x = pos.x + size.width as i32 / 2;
        let center_y = pos.y + size.height as i32 / 2;
        if edge != "none" {
            if let Some(mon) = resolve_mini_player_monitor_at_point(app, win, center_x, center_y) {
                let sf = mon.scale_factor().max(1.0);
                let wa = mon.work_area();
                let wa_rect = PhysRect {
                    left: wa.position.x,
                    top: wa.position.y,
                    right: wa.position.x + wa.size.width as i32,
                    bottom: wa.position.y + wa.size.height as i32,
                };
                let (fw, fh) =
                    mini_logical_dims(&edge, state.mini_player_expanded.load(Ordering::Relaxed));
                let pw = (fw * sf).round() as i32;
                let ph = (fh * sf).round() as i32;
                let card_pad = (22.0 * sf).round() as i32;
                let (tx, ty) = compute_mini_dock_or_clamped_pos(
                    pos.x, pos.y, pw, ph, &edge, card_pad, wa_rect,
                );
                let drifted = (pos.x - tx).abs() > (12.0 * sf).round() as i32
                    || (pos.y - ty).abs() > (12.0 * sf).round() as i32
                    || (size.width as i32 - pw).abs() > 4
                    || (size.height as i32 - ph).abs() > 4;
                if drifted {
                    state.mini_snap_anim_gen.fetch_add(1, Ordering::Relaxed);
                    if (size.width as i32 - pw).abs() > 2 || (size.height as i32 - ph).abs() > 2 {
                        let _ = win.set_size(Size::Logical(LogicalSize::new(fw, fh)));
                    }
                    let _ = win.set_position(Position::Physical(PhysicalPosition::new(tx, ty)));
                }
            }
        }
        return;
    }

    let center_x = pos.x + size.width as i32 / 2;
    let center_y = pos.y + size.height as i32 / 2;
    let mon = resolve_mini_player_monitor_at_point(app, win, center_x, center_y)
        .or_else(|| win.primary_monitor().ok().flatten());
    let Some(mon) = mon else {
        return;
    };

    let sf = mon.scale_factor().max(1.0);
    let wa = mon.work_area();
    let wa_rect = PhysRect {
        left: wa.position.x,
        top: wa.position.y,
        right: wa.position.x + wa.size.width as i32,
        bottom: wa.position.y + wa.size.height as i32,
    };
    let (fw, fh) = mini_logical_dims(&edge, state.mini_player_expanded.load(Ordering::Relaxed));
    let pw = (fw * sf).round() as i32;
    let ph = (fh * sf).round() as i32;
    let card_pad = (22.0 * sf).round() as i32;
    let (tx, ty) =
        compute_mini_dock_or_clamped_pos(pos.x, pos.y, pw, ph, &edge, card_pad, wa_rect);

    state.mini_snap_anim_gen.fetch_add(1, Ordering::Relaxed);
    if (size.width as i32 - pw).abs() > 2 || (size.height as i32 - ph).abs() > 2 {
        let _ = win.set_size(Size::Logical(LogicalSize::new(fw, fh)));
    }
    let _ = win.set_position(Position::Physical(PhysicalPosition::new(tx, ty)));
    state.mini_player_positioned.store(true, Ordering::Relaxed);
}

/// Windows 级 Mini 灵动岛自愈与方案 1 智能全屏礼让守护：
/// - 若 mini_player_visible == false 且不在退场淡出窗口内，若检测到窗口仍物理可见则立即强制四重隐藏（彻底杜绝透明遮挡幽灵窗）
/// - 切窗 / Alt+Tab / Win+D / 切屏后自动无抢焦复活并维持 HWND_TOPMOST 与 WS_EX_TOOLWINDOW
/// - 同一显示器运行全屏视频/游戏时暂不遮挡画面，退出全屏后立刻自动复活置顶
#[cfg(target_os = "windows")]
fn heal_mini_player_window(app: &AppHandle, state: &AppState, force_resnap: bool) {
    if !state.mini_player_visible.load(Ordering::Acquire) {
        state
            .mini_yielded_for_fullscreen
            .store(false, Ordering::Relaxed);
        if !state.mini_exiting_anim.load(Ordering::Acquire) {
            if let Some(mini_win) = app.get_webview_window("mini_player") {
                if let Ok(raw) = mini_win.hwnd() {
                    if window_transition::win_impl::is_hwnd_visible_and_not_minimized(raw.0 as isize) {
                        hide_mini_player_window_safely(
                            app,
                            &mini_win,
                            state.mini_player_visible.clone(),
                            true,
                        );
                    }
                }
            }
        }
        return;
    }
    let Some(mini_win) = app.get_webview_window("mini_player") else {
        return;
    };
    let Ok(raw_mini) = mini_win.hwnd() else {
        return;
    };
    let mini_hwnd = raw_mini.0 as isize;
    if mini_hwnd == 0 {
        return;
    }
    let main_hwnd = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize)
        .unwrap_or(0);
    let lyric_hwnd = app
        .get_webview_window("desktop_lyric")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize)
        .unwrap_or(0);

    #[link(name = "user32")]
    extern "system" {
        fn IsIconic(hwnd: isize) -> i32;
        fn IsWindowVisible(hwnd: isize) -> i32;
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

    const HWND_TOPMOST: isize = -1;
    const HWND_NOTOPMOST: isize = -2;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;

    // 方案 1：检测同屏全屏独占应用（如全屏视频/游戏）
    if is_foreground_fullscreen_on_same_monitor(mini_hwnd, main_hwnd, lyric_hwnd) {
        if !state
            .mini_yielded_for_fullscreen
            .swap(true, Ordering::Relaxed)
        {
            unsafe {
                let _ = SetWindowPos(
                    mini_hwnd,
                    HWND_NOTOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
        return;
    }

    let was_yielded = state
        .mini_yielded_for_fullscreen
        .swap(false, Ordering::Relaxed);

    // 边界与切屏自检（放在显示前，防止窗口留在已断开的屏幕坐标）
    reclamp_mini_player_if_needed(app, &mini_win, state, force_resnap);

    let dock_edge = state
        .mini_player_dock_edge
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_else(|| "none".to_string());
    let is_expanded = state.mini_player_expanded.load(Ordering::Relaxed);
    let default_click_through = dock_edge != "none" && !is_expanded;

    unsafe {
        let iconic = IsIconic(mini_hwnd) != 0;
        let visible = IsWindowVisible(mini_hwnd) != 0;
        if iconic || !visible || was_yielded {
            set_subwindow_suspended(&mini_win, false);
            set_webview_memory_target_low(&mini_win, false);
            window_transition::win_impl::show_overlay_window_native(mini_hwnd, default_click_through);
        } else {
            let _ = SetWindowPos(
                mini_hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn heal_mini_player_window(app: &AppHandle, state: &AppState, force_resnap: bool) {
    if !state.mini_player_visible.load(Ordering::Relaxed) {
        return;
    }
    if let Some(mini_win) = app.get_webview_window("mini_player") {
        reclamp_mini_player_if_needed(app, &mini_win, state, force_resnap);
        let _ = mini_win.set_always_on_top(true);
    }
}

fn restore_main_from_mini(app: &AppHandle, state: &AppState) {
    orchestrate_restore_main_window(app, state, false);
}

fn orchestrate_enter_mini_mode(
    app: &AppHandle,
    state: &AppState,
    motion_override: Option<String>,
) {
    let gen = state.main_transition_gen.fetch_add(1, Ordering::AcqRel) + 1;
    if let Some(ref raw) = motion_override {
        let norm = match raw.trim().to_ascii_lowercase().as_str() {
            "none" => "none",
            "standard" => "standard",
            _ => "best",
        };
        if let Ok(mut g) = state.motion_level.lock() {
            *g = norm.to_string();
        }
    }
    let motion_style = current_motion_style(state, motion_override.as_deref());
    let motion_str = current_motion_level_str(state, motion_override.as_deref());

    state.mini_exiting_anim.store(false, Ordering::Release);
    state.mini_player_visible.store(true, Ordering::Release);
    state
        .mini_yielded_for_fullscreen
        .store(false, Ordering::Relaxed);
    state.mini_snap_anim_gen.fetch_add(1, Ordering::Relaxed);

    let app_for_main = app.clone();
    let gen_ref = state.main_transition_gen.clone();
    let mini_vis_ref = state.mini_player_visible.clone();
    let mini_pos_ref = state.mini_player_positioned.clone();
    let mini_exp_ref = state.mini_player_expanded.clone();
    let mini_edge_ref = state.mini_player_dock_edge.clone();
    let mini_snap_ref = state.mini_player_snapshot.clone();
    let hotkey_tid_ref = state.hotkey_thread_id.clone();
    let max_flag_ref = state.main_was_maximized.clone();
    let low_mem_ref = state.main_low_memory_active.clone();
    let (accent1_rgb, accent2_rgb) = state
        .cover_accent_rgb
        .lock()
        .ok()
        .map(|g| *g)
        .unwrap_or(((94, 106, 210), (45, 212, 191)));

    let _ = app.run_on_main_thread(move || {
        if gen_ref.load(Ordering::Acquire) != gen {
            return;
        }
        let st_ref = app_for_main.state::<AppState>();

        // 1. 预热并精准定位 Mini 独立子窗（先发 entering 透明起势态，确保 0 硬切闪现）
        let Some(mini_win) = ensure_mini_player_window(&app_for_main) else {
            return;
        };
        set_subwindow_suspended(&mini_win, false);
        set_webview_memory_target_low(&mini_win, false);
        enforce_transparent_overlay_window_style(&mini_win);
        if !mini_pos_ref.load(Ordering::Relaxed) {
            place_mini_player_default(&app_for_main, &mini_win, &st_ref);
        } else {
            reclamp_mini_player_if_needed(&app_for_main, &mini_win, &st_ref, false);
        }

        let dock_edge = mini_edge_ref
            .lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or_else(|| "none".to_string());
        let expanded = mini_exp_ref.load(Ordering::Relaxed);
        let mini_entity_rect = compute_mini_visual_entity_rect(&mini_win, &dock_edge, expanded);

        if let Some(snap) = mini_snap_ref.lock().ok().and_then(|g| g.clone()) {
            let _ = app_for_main.emit_to("mini_player", "bm-mini-player-sync", snap);
        }

        #[cfg(target_os = "windows")]
        {
            use window_transition::win_impl as wt;

            wt::cancel_and_hide_gpu_stage_on_main_thread();

            let main_win_opt = app_for_main.get_webview_window("main");
            let main_hwnd = main_win_opt
                .as_ref()
                .and_then(|w| w.hwnd().ok())
                .map(|h| h.0 as isize)
                .unwrap_or(0);
            let mini_hwnd = mini_win.hwnd().ok().map(|h| h.0 as isize).unwrap_or(0);

            let main_was_visible =
                wt::is_hwnd_visible_and_not_minimized(main_hwnd) && !wt::is_main_cloaked();
            let is_max = if main_was_visible {
                let z = wt::is_hwnd_maximized_native(main_hwnd);
                max_flag_ref.store(z, Ordering::Relaxed);
                z
            } else {
                max_flag_ref.load(Ordering::Relaxed)
            };

            let main_rect = if main_was_visible {
                wt::get_window_client_screen_rect(main_hwnd)
                    .or_else(|| wt::get_window_visual_rect(main_hwnd))
            } else {
                None
            };
            let sf = main_win_opt
                .as_ref()
                .and_then(|w| w.scale_factor().ok())
                .unwrap_or(1.0)
                .max(1.0);
            let same_mon = main_was_visible && wt::is_same_monitor(main_hwnd, mini_hwnd);

            let plan = main_rect.map(|mr| {
                window_transition::compute_spatial_transition_plan(
                    window_transition::TransitionKind::EnterMini,
                    mr,
                    mini_entity_rect,
                    is_max,
                    same_mon,
                    sf,
                    motion_style,
                )
            });

            let (hint_ox, hint_oy, handoff_delay_ms, gpu_dur_ms) = match plan {
                Some(p) => (
                    p.mini_hint_offset_x,
                    p.mini_hint_offset_y,
                    p.mini_handoff_delay_ms,
                    p.duration_ms,
                ),
                None => (0.0, -14.0, 0, 0),
            };

            let use_anim = motion_style != window_transition::TransitionMotionStyle::None;

            // 先令 Mini 前端就位到 entering 起势态（opacity: 0）并无抢焦显示窗口，让 WebView2 提前完成首帧合成
            if use_anim {
                let _ = app_for_main.emit_to(
                    "mini_player",
                    "bm-mini-player-transition",
                    serde_json::json!({
                        "phase": "entering",
                        "offsetX": hint_ox,
                        "offsetY": hint_oy,
                        "dockEdge": dock_edge,
                        "motionLevel": motion_str,
                        "transitionGen": gen,
                    }),
                );
            }

            let mut gpu_started = false;
            if let (Some(main_win), Some(mr), Some(p)) = (&main_win_opt, main_rect, plan) {
                if main_was_visible && p.duration_ms > 0 {
                    let stage_rect = wt::get_monitor_rects_for_hwnd(main_hwnd)
                        .map(|(mon, _work)| mon)
                        .unwrap_or(mr);
                    let bg_colorref = MAIN_BG_COLORREF.load(Ordering::Relaxed);
                    let bg_rgb = wt::colorref_to_rgb(bg_colorref);
                    let target_w = mr.width().max(16) as u32;
                    let target_h = mr.height().max(16) as u32;

                    let (tex_w, tex_h, bgra) =
                        if let Some((cw, ch, live_px)) = wt::capture_window_client_bgra(main_hwnd, true)
                        {
                            // 缓存高清真画面快照供退出小窗时实现极速无黑屏交叉溶解展开
                            window_transition::store_cached_main_snapshot(
                                window_transition::CachedMainSnapshot {
                                    width: cw,
                                    height: ch,
                                    bg_colorref,
                                    bgra_premultiplied: live_px.clone(),
                                },
                            );
                            (cw, ch, live_px)
                        } else {
                            let sw = (target_w / 2).clamp(64, 960);
                            let sh = (target_h / 2).clamp(64, 540);
                            let px = window_transition::synthesize_theme_aura_card_bgra(
                                sw,
                                sh,
                                bg_rgb,
                                accent1_rgb,
                                accent2_rgb,
                            );
                            (sw, sh, px)
                        };

                    gpu_started = wt::play_gpu_transition_on_main_thread(
                        stage_rect, mr, tex_w, tex_h, &bgra, p,
                    );
                }

                // GPU 舞台已提交并严丝合缝接管屏幕画面后，原生隐身隐藏真实主窗口（0 闪烁、保留最大化态）
                sync_main_low_memory_state(main_win, &low_mem_ref, true);
                wt::hide_main_window_native(main_win);
            } else if let Some(main_win) = &main_win_opt {
                sync_main_low_memory_state(main_win, &low_mem_ref, true);
                wt::hide_main_window_native(main_win);
            }

            let initial_click_through = dock_edge != "none" && !expanded;

            let _ = app_for_main.emit_to(
                "main",
                "bm-mini-player-cmd",
                serde_json::json!({ "type": "CMD_REQUEST_SYNC" }),
            );

            // 2. 在主窗口收束的势能交接甜点时刻（~58ms），再同步显示 Mini（更新 tao VISIBLE + TOOLWINDOW + TOPMOST）并弹性浮现落位，
            //    既彻底避免 0ms 抢跑露出旧帧，又确保 tao WindowFlags::VISIBLE 100% 同步！
            if gpu_started && handoff_delay_ms > 0 {
                let app_handoff = app_for_main.clone();
                let gen_handoff = gen_ref.clone();
                let mini_vis_check = mini_vis_ref.clone();
                let hotkey_tid_h = hotkey_tid_ref.clone();
                let dock_edge_h = dock_edge.clone();
                let motion_h = motion_str.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(handoff_delay_ms)).await;
                    if gen_handoff.load(Ordering::Acquire) != gen
                        || !mini_vis_check.load(Ordering::Acquire)
                    {
                        return;
                    }
                    let app_ui = app_handoff.clone();
                    let gen_ui = gen_handoff.clone();
                    let vis_ui = mini_vis_check.clone();
                    let tid_ui = hotkey_tid_h.clone();
                    let _ = app_handoff.run_on_main_thread(move || {
                        if gen_ui.load(Ordering::Acquire) != gen || !vis_ui.load(Ordering::Acquire) {
                            return;
                        }
                        if let Some(mw) = app_ui.get_webview_window("mini_player") {
                            show_mini_player_window_safely(&mw, initial_click_through);
                        }
                        let _ = app_ui.emit(
                            "bm-mini-player-visibility",
                            serde_json::json!({ "visible": true }),
                        );
                        let _ = app_ui.emit_to(
                            "mini_player",
                            "bm-mini-player-transition",
                            serde_json::json!({
                                "phase": "visible",
                                "offsetX": hint_ox,
                                "offsetY": hint_oy,
                                "dockEdge": dock_edge_h,
                                "motionLevel": motion_h,
                                "instant": false,
                                "transitionGen": gen,
                            }),
                        );
                        wake_hotkey_thread(&tid_ui);
                    });
                });
            } else {
                show_mini_player_window_safely(&mini_win, initial_click_through);
                let _ = app_for_main.emit(
                    "bm-mini-player-visibility",
                    serde_json::json!({ "visible": true }),
                );
                let _ = app_for_main.emit_to(
                    "mini_player",
                    "bm-mini-player-transition",
                    serde_json::json!({
                        "phase": "visible",
                        "offsetX": hint_ox,
                        "offsetY": hint_oy,
                        "dockEdge": dock_edge,
                        "motionLevel": motion_str,
                        "instant": !use_anim,
                        "transitionGen": gen,
                    }),
                );
                wake_hotkey_thread(&hotkey_tid_ref);
            }

            // 3. GPU 收缩动画完成后隐藏并释放过渡舞台资源
            if gpu_started && gpu_dur_ms > 0 {
                let app_done = app_for_main.clone();
                let gen_done = gen_ref.clone();
                let mini_vis_done = mini_vis_ref.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(gpu_dur_ms + 24)).await;
                    if gen_done.load(Ordering::Acquire) != gen {
                        return;
                    }
                    let app_ui = app_done.clone();
                    let gen_ui = gen_done.clone();
                    let vis_ui = mini_vis_done.clone();
                    let _ = app_done.run_on_main_thread(move || {
                        if gen_ui.load(Ordering::Acquire) == gen {
                            wt::cancel_and_hide_gpu_stage_on_main_thread();
                            if vis_ui.load(Ordering::Acquire) {
                                let st_inner = app_ui.state::<AppState>();
                                heal_mini_player_window(&app_ui, &st_inner, false);
                            }
                        }
                    });
                });
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (mini_entity_rect, accent1_rgb, accent2_rgb, mini_vis_ref);
            if let Some(main_win) = app_for_main.get_webview_window("main") {
                hide_main_window_cleanly(&main_win, &st_ref);
            }
            let _ = mini_win.set_always_on_top(true);
            let _ = mini_win.show();
            let _ = mini_win.set_focus();
            let _ = app_for_main.emit_to(
                "mini_player",
                "bm-mini-player-transition",
                serde_json::json!({
                    "phase": "visible",
                    "offsetX": 0.0,
                    "offsetY": -14.0,
                    "dockEdge": dock_edge,
                    "motionLevel": motion_str,
                    "instant": motion_style == window_transition::TransitionMotionStyle::None,
                    "transitionGen": gen,
                }),
            );
            let _ = app_for_main.emit_to(
                "main",
                "bm-mini-player-cmd",
                serde_json::json!({ "type": "CMD_REQUEST_SYNC" }),
            );
            let _ = app_for_main.emit(
                "bm-mini-player-visibility",
                serde_json::json!({ "visible": true }),
            );
        }
    });
}

#[tauri::command]
async fn set_mini_player_mode_cmd(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    mini: bool,
    motion_level: Option<String>,
) -> Result<bool, String> {
    ensure_main_window(&window)?;
    if mini {
        orchestrate_enter_mini_mode(&app, &state, motion_level);
        return Ok(true);
    } else {
        if let Some(ref raw) = motion_level {
            let norm = match raw.trim().to_ascii_lowercase().as_str() {
                "none" => "none",
                "standard" => "standard",
                _ => "best",
            };
            if let Ok(mut g) = state.motion_level.lock() {
                *g = norm.to_string();
            }
        }
        orchestrate_restore_main_window(&app, &state, true);
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
        if let Some(level) = payload.get("motionLevel").and_then(|v| v.as_str()) {
            let norm = match level.trim().to_ascii_lowercase().as_str() {
                "none" => "none",
                "standard" => "standard",
                _ => "best",
            };
            if let Ok(mut g) = state.motion_level.lock() {
                *g = norm.to_string();
            }
        }
        if let (Some(c1), Some(c2)) = (
            payload
                .get("coverAccentPrimary")
                .and_then(|v| v.as_str())
                .and_then(parse_hex_color_rgb),
            payload
                .get("coverAccentSecondary")
                .and_then(|v| v.as_str())
                .and_then(parse_hex_color_rgb),
        ) {
            if let Ok(mut g) = state.cover_accent_rgb.lock() {
                *g = (c1, c2);
            }
        }
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
            "CMD_MINI_PLAYER_READY" => {
                state.mini_player_ready.store(true, Ordering::Relaxed);
                if state.mini_player_visible.load(Ordering::Relaxed) {
                    // 冷启动新建窗口场景：前端挂载完成后若当前已在小窗态，立即补发一次优雅弹性入场信号，防止停留在透明起势态
                    let motion_str = current_motion_level_str(&state, None);
                    let dock_edge = state
                        .mini_player_dock_edge
                        .lock()
                        .ok()
                        .map(|g| g.clone())
                        .unwrap_or_else(|| "none".to_string());
                    let gen = state.main_transition_gen.load(Ordering::Acquire);
                    let _ = app.emit_to(
                        "mini_player",
                        "bm-mini-player-transition",
                        serde_json::json!({
                            "phase": "visible",
                            "offsetX": 0.0,
                            "offsetY": -14.0,
                            "dockEdge": dock_edge,
                            "motionLevel": motion_str,
                            "instant": motion_str == "none",
                            "transitionGen": gen,
                        }),
                    );
                } else {
                    // 启动后后台空闲预热完成的隐藏窗口：挂起省内存，用户首次点击即可 0ms 瞬时平滑进入
                    schedule_subwindow_suspend(
                        &app,
                        "mini_player",
                        state.mini_player_visible.clone(),
                    );
                }
            }
            "CMD_RESTORE_MAIN" => {
                orchestrate_restore_main_window(&app, &state, true);
            }
            "CMD_OPEN_NOW_PLAYING" => {
                orchestrate_restore_main_window(&app, &state, true);
            }
            "CMD_HIDE_TO_TRAY" => {
                state.mini_exiting_anim.store(false, Ordering::Release);
                state.mini_player_visible.store(false, Ordering::Release);
                state.main_transition_gen.fetch_add(1, Ordering::AcqRel);
                wake_hotkey_thread(&state.hotkey_thread_id);
                let app_hide = app.clone();
                let vis_flag = state.mini_player_visible.clone();
                let app_ui = app_hide.clone();
                let _ = app_hide.run_on_main_thread(move || {
                    #[cfg(target_os = "windows")]
                    window_transition::win_impl::cancel_and_hide_gpu_stage_on_main_thread();
                    if let Some(mini_win) = app_ui.get_webview_window("mini_player") {
                        hide_mini_player_window_safely(&app_ui, &mini_win, vis_flag, true);
                    }
                });
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
    state.mini_snap_anim_gen.fetch_add(1, Ordering::Relaxed);
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
            set_mini_click_through(&win, false);
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
    let anim_gen = state.mini_snap_anim_gen.fetch_add(1, Ordering::Relaxed) + 1;
    let dx = tx - pos.x;
    let dy = ty - pos.y;
    let dist = ((dx * dx + dy * dy) as f64).sqrt();
    if dist > 1.0 && dist <= 64.0 * sf {
        let win_clone = win.clone();
        let gen_ref = state.mini_snap_anim_gen.clone();
        let sx = pos.x;
        let sy = pos.y;
        tauri::async_runtime::spawn(async move {
            let steps = 9;
            for i in 1..=steps {
                if gen_ref.load(Ordering::Relaxed) != anim_gen {
                    break;
                }
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
    state.mini_snap_anim_gen.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut g) = state.mini_player_dock_edge.lock() {
        *g = edge.clone();
    }
    state.mini_player_expanded.store(true, Ordering::Relaxed);
    wake_hotkey_thread(&state.hotkey_thread_id);

    let Some(win) = app.get_webview_window("mini_player") else {
        return Ok(false);
    };
    set_mini_click_through(&win, false);
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
    if state.mini_player_visible.load(Ordering::Relaxed) {
        let _ = win.set_focus();
    }
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
    mini_exiting_anim: Arc<AtomicBool>,
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
            fn GetForegroundWindow() -> isize;
            fn GetSystemMetrics(n_index: i32) -> i32;
            fn IsIconic(hwnd: isize) -> i32;
            fn IsWindowVisible(hwnd: isize) -> i32;
            fn GetWindowLongW(hwnd: isize, n_index: i32) -> i32;
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
        const GWL_EXSTYLE: i32 = -20;
        const WS_EX_TOPMOST: i32 = 0x00000008;
        const SM_XVIRTUALSCREEN: i32 = 76;
        const SM_YVIRTUALSCREEN: i32 = 77;
        const SM_CXVIRTUALSCREEN: i32 = 78;
        const SM_CYVIRTUALSCREEN: i32 = 79;
        const SM_CMONITORS: i32 = 80;

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
        let mut last_fg_hwnd: isize = 0;
        let mut last_display_sig: (i32, i32, i32, i32, i32) = (0, 0, 0, 0, 0);
        let mut last_mini_heal_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .unwrap_or_else(std::time::Instant::now);

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

            // Mini 灵动岛收起态局部热区穿透检测（采用原生 GWL_EXSTYLE 原子切换，零抢焦且绝不触发 tao 误隐藏）：
            let is_mini_vis = mini_visible.load(Ordering::Acquire);
            let is_mini_exp = mini_expanded.load(Ordering::Relaxed);
            if !is_mini_vis {
                was_in_mini_island = false;
                last_mini_ignore_cursor = None;
                last_mini_cursor = None;
                // 幽灵窗终极看门狗：只要已退出小窗且短暂退场动画已结束，若检测到 mini_player HWND 仍可见，立刻派发主线程四重强杀！
                if !mini_exiting_anim.load(Ordering::Acquire) {
                    if let Some(win) = app_handle.get_webview_window("mini_player") {
                        if let Ok(raw_h) = win.hwnd() {
                            let mh = raw_h.0 as isize;
                            if mh != 0 && unsafe { IsWindowVisible(mh) != 0 } {
                                let app_kill = app_handle.clone();
                                let vis_kill = mini_visible.clone();
                                let exit_kill = mini_exiting_anim.clone();
                                let _ = app_handle.run_on_main_thread(move || {
                                    if !vis_kill.load(Ordering::Acquire)
                                        && !exit_kill.load(Ordering::Acquire)
                                    {
                                        if let Some(mw) = app_kill.get_webview_window("mini_player") {
                                            hide_mini_player_window_safely(
                                                &app_kill, &mw, vis_kill, true,
                                            );
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            } else {
                let edge = mini_dock_edge
                    .lock()
                    .ok()
                    .map(|g| g.clone())
                    .unwrap_or_else(|| "none".to_string());
                if is_mini_exp || edge == "none" {
                    last_mini_cursor = None;
                    if last_mini_ignore_cursor != Some(false) {
                        let app_ct = app_handle.clone();
                        let vis_ct = mini_visible.clone();
                        let _ = app_handle.run_on_main_thread(move || {
                            if let Some(win) = app_ct.get_webview_window("mini_player") {
                                if vis_ct.load(Ordering::Acquire) {
                                    set_mini_click_through(&win, false);
                                } else {
                                    set_mini_click_through(&win, true);
                                }
                            }
                        });
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
                                        let app_ct = app_handle.clone();
                                        let vis_ct = mini_visible.clone();
                                        let _ = app_handle.run_on_main_thread(move || {
                                            if let Some(mw) = app_ct.get_webview_window("mini_player") {
                                                if vis_ct.load(Ordering::Acquire) {
                                                    set_mini_click_through(&mw, desired_ignore);
                                                } else {
                                                    set_mini_click_through(&mw, true);
                                                }
                                            }
                                        });
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

            // Mini 灵动岛切窗 / 切屏 / Win+D / 退出全屏稳定性轻检守护（零抢焦、极低开销）
            if is_mini_vis {
                let (fg_now, display_sig) = unsafe {
                    (
                        GetForegroundWindow(),
                        (
                            GetSystemMetrics(SM_CMONITORS),
                            GetSystemMetrics(SM_XVIRTUALSCREEN),
                            GetSystemMetrics(SM_YVIRTUALSCREEN),
                            GetSystemMetrics(SM_CXVIRTUALSCREEN),
                            GetSystemMetrics(SM_CYVIRTUALSCREEN),
                        ),
                    )
                };
                let fg_changed = fg_now != last_fg_hwnd;
                let display_changed = last_display_sig != (0, 0, 0, 0, 0) && display_sig != last_display_sig;
                last_fg_hwnd = fg_now;
                last_display_sig = display_sig;

                let mut mini_state_broken = false;
                if let Some(win) = app_handle.get_webview_window("mini_player") {
                    if let Ok(raw_h) = win.hwnd() {
                        let mh = raw_h.0 as isize;
                        if mh != 0 {
                            unsafe {
                                let iconic = IsIconic(mh) != 0;
                                let visible = IsWindowVisible(mh) != 0;
                                let ex = GetWindowLongW(mh, GWL_EXSTYLE);
                                let lost_topmost = (ex & WS_EX_TOPMOST) == 0;
                                mini_state_broken = iconic || !visible || lost_topmost;
                            }
                        }
                    }
                }

                let elapsed_ms = last_mini_heal_at.elapsed().as_millis();
                let should_heal = display_changed
                    || (fg_changed && elapsed_ms >= 180)
                    || (mini_state_broken && elapsed_ms >= 300)
                    || elapsed_ms >= 1500;

                if should_heal {
                    last_mini_heal_at = std::time::Instant::now();
                    let app_heal = app_handle.clone();
                    let force_resnap = display_changed;
                    let _ = app_handle.run_on_main_thread(move || {
                        let st = app_heal.state::<AppState>();
                        heal_mini_player_window(&app_heal, &st, force_resnap);
                    });
                }
            } else {
                last_fg_hwnd = 0;
                last_display_sig = (0, 0, 0, 0, 0);
            }

            let need_cursor_poll = (is_vis && is_lck) || (is_mini_vis && !is_mini_exp);
            let wait_timeout_ms = if !need_cursor_poll && !is_mini_vis {
                // 无穿透检测及 Mini 守护需求时彻底阻塞休眠：0 次/秒 CPU 唤醒，热键与状态消息到达即刻瞬时唤醒
                INFINITE
            } else if !need_cursor_poll && is_mini_vis {
                // Mini 展开态/自由悬浮态仍保留 350ms 轻检，确保切屏/切窗/退出全屏及时守护
                350
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
    // 0. 进程级单实例互斥守卫（必须在 WebView2、AppStore、托盘与 Tauri Builder 初始化之前执行）：
    //    小窗模式、最小化至托盘或已有实例在后台运行时，再次点击任务栏固定图标 / 桌面快捷方式会立即将前台焦点权限
    //    移交给已有主实例并唤醒恢复主窗口，本进程在数毫秒内静默退出，彻底杜绝无上限多开进程与重复托盘图标。
    let single_instance_guard = match single_instance::SingleInstanceGuard::acquire() {
        single_instance::AcquireOutcome::Primary(guard) => Arc::new(guard),
        single_instance::AcquireOutcome::SecondaryWokeExisting { .. } => {
            return;
        }
    };
    let single_instance_for_setup = single_instance_guard.clone();

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
    let mini_snap_anim_gen = Arc::new(AtomicU64::new(0));
    let mini_yielded_for_fullscreen = Arc::new(AtomicBool::new(false));
    let mini_player_ready = Arc::new(AtomicBool::new(false));
    let mini_exiting_anim = Arc::new(AtomicBool::new(false));
    let mini_exiting_anim_for_setup = mini_exiting_anim.clone();
    let main_was_maximized = Arc::new(AtomicBool::new(false));
    let main_was_maximized_for_event = main_was_maximized.clone();
    let main_transition_gen = Arc::new(AtomicU64::new(0));
    let motion_level = Arc::new(std::sync::Mutex::new("best".to_string()));
    let cover_accent_rgb = Arc::new(std::sync::Mutex::new(((94u8, 106u8, 210u8), (45u8, 212u8, 191u8))));
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
            mini_snap_anim_gen,
            mini_yielded_for_fullscreen,
            mini_player_ready,
            mini_exiting_anim,
            main_was_maximized,
            main_transition_gen,
            motion_level,
            cover_accent_rgb,
            main_low_memory_active,
            hotkey_thread_id,
            active_download_cancels,
            authorized_font_import_paths,
        })
        .setup(move |app| {
            {
                let app_wake = app.handle().clone();
                single_instance_for_setup.attach_wake_handler(move || {
                    let st = app_wake.state::<AppState>();
                    restore_main_from_mini(&app_wake, &st);
                });
            }
            if let Some(w) = app.get_webview_window("main") {
                harden_desktop_webview2(&w);
                install_main_window_nc_paint_subclass(&w);
                apply_main_window_zero_border_dwm(&w);
                apply_main_window_bg_color(&w, 12, 14, 20);
            }
            // 启动 1.8s 后空闲后台静默预热隐藏态 mini_player 子窗，彻底消除首次进小窗的 WebView2 冷启动空档；
            // 加载就绪后自动切低内存并 TrySuspend，平时零活跃开销。
            {
                let app_warm = app.handle().clone();
                let mini_vis_warm = mini_player_visible_for_setup.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
                    let app_ui = app_warm.clone();
                    let vis_flag = mini_vis_warm.clone();
                    let _ = app_warm.run_on_main_thread(move || {
                        if app_ui.get_webview_window("mini_player").is_none() {
                            if let Some(mini_win) = ensure_mini_player_window(&app_ui) {
                                if !vis_flag.load(Ordering::Acquire) {
                                    // 仅做 HWND 隐藏+穿透+去置顶，保留 WebView2 活跃直到 CMD_MINI_PLAYER_READY（或 2.4s 兜底）完成挂起
                                    hide_mini_player_hwnd_only(&mini_win);
                                    // 兜底 2.4s 后若用户仍未进入小窗则强制挂起省内存
                                    let app_susp = app_ui.clone();
                                    let vis_susp = vis_flag.clone();
                                    tauri::async_runtime::spawn(async move {
                                        tokio::time::sleep(std::time::Duration::from_millis(2400))
                                            .await;
                                        if !vis_susp.load(Ordering::Acquire) {
                                            schedule_subwindow_suspend(
                                                &app_susp,
                                                "mini_player",
                                                vis_susp,
                                            );
                                        }
                                    });
                                }
                            }
                        }
                    });
                });
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
                mini_exiting_anim_for_setup.clone(),
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
                            let is_max = window
                                .app_handle()
                                .get_webview_window("main")
                                .map(|mw| is_hwnd_zoomed(&mw))
                                .unwrap_or_else(|| window.is_maximized().unwrap_or(false));
                            main_was_maximized_for_event.store(is_max, Ordering::Relaxed);
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
                            if let Some(mw) = window.app_handle().get_webview_window("main") {
                                let st = window.app_handle().state::<AppState>();
                                hide_main_window_cleanly(&mw, &st);
                            } else {
                                let _ = window.hide();
                            }
                        } else {
                            window.app_handle().exit(0);
                        }
                    }
                    _ => {}
                }
            } else if window.label() == "mini_player" {
                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let app = window.app_handle();
                        let state = app.state::<AppState>();
                        restore_main_from_mini(&app, &state);
                    }
                    WindowEvent::Focused(false) => {
                        let app = window.app_handle().clone();
                        let st = app.state::<AppState>();
                        if st.mini_player_visible.load(Ordering::Acquire) {
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(35));
                                let app_ui = app.clone();
                                let _ = app.run_on_main_thread(move || {
                                    let st_inner = app_ui.state::<AppState>();
                                    heal_mini_player_window(&app_ui, &st_inner, false);
                                });
                            });
                        }
                    }
                    WindowEvent::Resized(_) => {
                        let app = window.app_handle().clone();
                        let st = app.state::<AppState>();
                        if st.mini_player_visible.load(Ordering::Acquire)
                            && window.is_visible().unwrap_or(false)
                        {
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(35));
                                let app_ui = app.clone();
                                let _ = app.run_on_main_thread(move || {
                                    let st_inner = app_ui.state::<AppState>();
                                    heal_mini_player_window(&app_ui, &st_inner, false);
                                });
                            });
                        }
                    }
                    WindowEvent::ScaleFactorChanged { .. } => {
                        let app = window.app_handle().clone();
                        let st = app.state::<AppState>();
                        if st.mini_player_visible.load(Ordering::Relaxed) {
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(45));
                                let app_ui = app.clone();
                                let _ = app.run_on_main_thread(move || {
                                    let st_inner = app_ui.state::<AppState>();
                                    heal_mini_player_window(&app_ui, &st_inner, true);
                                });
                            });
                        }
                    }
                    _ => {}
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
            enrich_song_for_share_cmd,
            enrich_playlist_for_share_cmd,
            open_external_url_cmd,
            read_clipboard_text_cmd,
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
            sync_motion_level_cmd,
            hide_main_to_tray_cmd,
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

    #[test]
    fn test_mini_rect_visibility_and_dock_clamping_across_monitors() {
        let primary = PhysRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let secondary = PhysRect {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1040,
        };

        // 正常位于主屏顶边
        let on_top = PhysRect {
            left: 720,
            top: 0,
            right: 1200,
            bottom: 228,
        };
        assert!(is_rect_visible_in_any_work_area(on_top, &[primary], 36));

        // 副屏拔出后坐标落在负屏幕外 -> 判定不可见
        let orphaned_on_unplugged_secondary = PhysRect {
            left: -1200,
            top: 0,
            right: -720,
            bottom: 228,
        };
        assert!(!is_rect_visible_in_any_work_area(
            orphaned_on_unplugged_secondary,
            &[primary],
            36
        ));
        assert!(is_rect_visible_in_any_work_area(
            orphaned_on_unplugged_secondary,
            &[primary, secondary],
            36
        ));

        // 贴靠顶边重吸附到主屏安全区域内
        assert_eq!(
            compute_mini_dock_or_clamped_pos(-1200, 300, 480, 228, "top", 22, primary),
            (-22, 0)
        );
        // 贴靠右边重吸附
        assert_eq!(
            compute_mini_dock_or_clamped_pos(2400, 1200, 480, 228, "right", 22, primary),
            (1440, 834)
        );
        // 自由悬浮越界钳制
        assert_eq!(
            compute_mini_dock_or_clamped_pos(2500, -300, 480, 228, "none", 22, primary),
            (1462, -22)
        );
    }

    #[test]
    fn test_maximized_client_needs_reconcile_detects_bottom_gap_and_bounds_drift() {
        let wa = PhysRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        // 像素级对齐 + WebView2 满铺 -> 无需重入
        assert!(!maximized_client_needs_reconcile(
            wa,
            wa,
            Some(PhysRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040
            })
        ));
        // 自动隐藏任务栏保留 1px 容差 -> 无需循环重入
        assert!(!maximized_client_needs_reconcile(
            PhysRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1039
            },
            wa,
            Some(PhysRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1039
            })
        ));
        // 底部被 tao 错扣 8px（经典白线场景）-> 必须检出并触发无闪烁重置
        assert!(maximized_client_needs_reconcile(
            PhysRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1032
            },
            wa,
            None
        ));
        // 客户区正确但 WebView2 Bounds 滞后 2px -> 必须检出并强制 SetBounds
        assert!(maximized_client_needs_reconcile(
            wa,
            wa,
            Some(PhysRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1038
            })
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_nc_paint_subclass_eliminates_bottom_white_line_on_maximized_restore() {
        #[repr(C)]
        #[derive(Default, Clone, Copy)]
        struct WinPoint {
            x: i32,
            y: i32,
        }
        #[repr(C)]
        #[derive(Default, Clone, Copy)]
        struct WinRect {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }
        #[repr(C)]
        struct WinNcCalcSizeParams {
            rgrc: [WinRect; 3],
            lppos: usize,
        }
        #[repr(C)]
        #[derive(Default)]
        struct WinMonitorInfo {
            cb_size: u32,
            rc_monitor: WinRect,
            rc_work: WinRect,
            dw_flags: u32,
        }
        #[repr(C)]
        struct WinClassExW {
            cb_size: u32,
            style: u32,
            lpfn_wnd_proc:
                Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
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

        #[link(name = "kernel32")]
        extern "system" {
            fn GetModuleHandleW(lp_module_name: *const u16) -> isize;
        }
        #[link(name = "user32")]
        extern "system" {
            fn RegisterClassExW(lpwcx: *const WinClassExW) -> u16;
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
            fn ShowWindow(hwnd: isize, n_cmd_show: i32) -> i32;
            fn SetWindowLongW(hwnd: isize, n_index: i32, dw_new_long: i32) -> i32;
            fn SetWindowPos(
                hwnd: isize,
                hwnd_insert_after: isize,
                x: i32,
                y: i32,
                cx: i32,
                cy: i32,
                u_flags: u32,
            ) -> i32;
            fn IsZoomed(hwnd: isize) -> i32;
            fn MonitorFromWindow(hwnd: isize, dw_flags: u32) -> isize;
            fn GetMonitorInfoW(h_monitor: isize, lpmi: *mut WinMonitorInfo) -> i32;
            fn GetWindowRect(hwnd: isize, lp_rect: *mut WinRect) -> i32;
            fn GetClientRect(hwnd: isize, lp_rect: *mut WinRect) -> i32;
            fn ClientToScreen(hwnd: isize, lp_point: *mut WinPoint) -> i32;
            fn GetWindowDC(hwnd: isize) -> isize;
            fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
            fn PrintWindow(hwnd: isize, hdc_blt: isize, n_flags: u32) -> i32;
        }
        #[link(name = "gdi32")]
        extern "system" {
            fn CreateCompatibleDC(hdc: isize) -> isize;
            fn CreateCompatibleBitmap(hdc: isize, cx: i32, cy: i32) -> isize;
            fn SelectObject(hdc: isize, h: isize) -> isize;
            fn DeleteObject(ho: isize) -> i32;
            fn DeleteDC(hdc: isize) -> i32;
            fn GetPixel(hdc: isize, x: i32, y: i32) -> u32;
        }

        unsafe extern "system" fn mock_tao_wnd_proc(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: isize,
        ) -> isize {
            const WM_NCCALCSIZE: u32 = 0x0083;
            if msg == WM_NCCALCSIZE && IsZoomed(hwnd) != 0 {
                let hmon = MonitorFromWindow(hwnd, 2);
                let mut mi = WinMonitorInfo {
                    cb_size: std::mem::size_of::<WinMonitorInfo>() as u32,
                    ..Default::default()
                };
                if hmon != 0 && GetMonitorInfoW(hmon, &mut mi) != 0 {
                    let mut rc = mi.rc_work;
                    // 模拟自动隐藏任务栏留出 1px 非客户区底边
                    rc.bottom -= 1;
                    if wparam == 0 {
                        *(lparam as *mut WinRect) = rc;
                    } else {
                        (*(lparam as *mut WinNcCalcSizeParams)).rgrc[0] = rc;
                    }
                    return 0;
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let cls_name: Vec<u16> = "BmTestNcSubclassWindow\0".encode_utf16().collect();
            let wc = WinClassExW {
                cb_size: std::mem::size_of::<WinClassExW>() as u32,
                style: 0x0023,
                lpfn_wnd_proc: Some(mock_tao_wnd_proc),
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance: hinst,
                h_icon: 0,
                h_cursor: 0,
                hbr_background: 0,
                lpsz_menu_name: std::ptr::null(),
                lpsz_class_name: cls_name.as_ptr(),
                h_icon_sm: 0,
            };
            let _ = RegisterClassExW(&wc);

            let base_style: i32 = 0x00C00000 | 0x04000000 | 0x00080000 | 0x00040000 | 0x00010000 | 0x00020000;
            let ws_visible: i32 = 0x10000000;
            let ws_maximize: i32 = 0x01000000;

            let hwnd = CreateWindowExW(
                0x00040110,
                cls_name.as_ptr(),
                cls_name.as_ptr(),
                base_style as u32,
                100,
                100,
                800,
                600,
                0,
                0,
                hinst,
                0,
            );
            assert!(hwnd != 0);
            MAIN_BG_COLORREF.store(12u32 | (14u32 << 8) | (20u32 << 16), Ordering::Relaxed);
            install_nc_paint_subclass_on_hwnd(hwnd);

            // 1. 首次显示并最大化
            let _ = ShowWindow(hwnd, 5);
            let _ = ShowWindow(hwnd, 3);
            let _ = SetWindowLongW(hwnd, -16, base_style | ws_visible | ws_maximize);
            let _ = SetWindowPos(hwnd, 0, 0, 0, 0, 0, 0x0037);
            let _ = ShowWindow(hwnd, 5);

            // 2. 最大化态隐藏（进入 Mini 模式）
            let _ = ShowWindow(hwnd, 0);
            let _ = SetWindowLongW(hwnd, -16, base_style | ws_maximize);
            let _ = SetWindowPos(hwnd, 0, 0, 0, 0, 0, 0x0037);

            // 3. 复现 tao 的 show() 恢复序列
            let _ = ShowWindow(hwnd, 3);
            let _ = SetWindowLongW(hwnd, -16, base_style | ws_maximize);
            let _ = SetWindowPos(hwnd, 0, 0, 0, 0, 0, 0x0037);
            let _ = ShowWindow(hwnd, 5);

            // 4. 通过 PrintWindow 校验底部非客户区 1px 绝非纯白 (0x00FFFFFF)，而是主题深色 (12, 14, 20)
            let mut wr = WinRect::default();
            let mut cr = WinRect::default();
            assert!(GetWindowRect(hwnd, &mut wr) != 0);
            assert!(GetClientRect(hwnd, &mut cr) != 0);
            let mut tl = WinPoint { x: 0, y: 0 };
            assert!(ClientToScreen(hwnd, &mut tl) != 0);
            let w = wr.right - wr.left;
            let h = wr.bottom - wr.top;
            let client_bot_in_win = (tl.y - wr.top) + (cr.bottom - cr.top);

            let hdc_win = GetWindowDC(hwnd);
            let hdc_mem = CreateCompatibleDC(hdc_win);
            let hbmp = CreateCompatibleBitmap(hdc_win, w, h);
            let old = SelectObject(hdc_mem, hbmp);
            let pw_ok = PrintWindow(hwnd, hdc_mem, 0);
            assert_eq!(pw_ok, 1);

            let nc_pixel = GetPixel(hdc_mem, w / 2, client_bot_in_win);
            assert_ne!(nc_pixel, 0x00FF_FFFF, "Maximized bottom non-client row must never be white");
            assert_eq!(
                nc_pixel,
                12u32 | (14u32 << 8) | (20u32 << 16),
                "Maximized bottom non-client row must match active theme background color"
            );

            let _ = SelectObject(hdc_mem, old);
            let _ = DeleteObject(hbmp);
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(hwnd, hdc_win);
            let _ = DestroyWindow(hwnd);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_nc_paint_subclass_preserves_unmaximized_dwm_insets_after_maximized_cycle() {
        use std::sync::atomic::AtomicUsize;

        #[repr(C)]
        #[derive(Default, Clone, Copy)]
        struct WinRect {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }
        #[repr(C)]
        #[derive(Default, Clone, Copy)]
        struct WinPoint {
            x: i32,
            y: i32,
        }
        #[repr(C)]
        struct WinNcCalcSizeParams {
            rgrc: [WinRect; 3],
            lppos: usize,
        }
        #[repr(C)]
        #[derive(Default)]
        struct WinMonitorInfo {
            cb_size: u32,
            rc_monitor: WinRect,
            rc_work: WinRect,
            dw_flags: u32,
        }
        #[repr(C)]
        struct WinClassExW {
            cb_size: u32,
            style: u32,
            lpfn_wnd_proc:
                Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
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

        #[link(name = "kernel32")]
        extern "system" {
            fn GetModuleHandleW(lp_module_name: *const u16) -> isize;
        }
        #[link(name = "user32")]
        extern "system" {
            fn RegisterClassExW(lpwcx: *const WinClassExW) -> u16;
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
            fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
            fn ShowWindow(hwnd: isize, n_cmd_show: i32) -> i32;
            fn SetWindowLongW(hwnd: isize, n_index: i32, dw_new_long: i32) -> i32;
            fn SetWindowPos(
                hwnd: isize,
                hwnd_insert_after: isize,
                x: i32,
                y: i32,
                cx: i32,
                cy: i32,
                u_flags: u32,
            ) -> i32;
            fn IsZoomed(hwnd: isize) -> i32;
            fn MonitorFromWindow(hwnd: isize, dw_flags: u32) -> isize;
            fn GetMonitorInfoW(h_monitor: isize, lpmi: *mut WinMonitorInfo) -> i32;
            fn GetWindowRect(hwnd: isize, lp_rect: *mut WinRect) -> i32;
            fn GetClientRect(hwnd: isize, lp_rect: *mut WinRect) -> i32;
            fn ClientToScreen(hwnd: isize, lp_point: *mut WinPoint) -> i32;
            fn GetWindowDC(hwnd: isize) -> isize;
            fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
            fn PrintWindow(hwnd: isize, hdc_blt: isize, n_flags: u32) -> i32;
            fn GetDpiForWindow(hwnd: isize) -> u32;
            fn GetSystemMetricsForDpi(n_index: i32, dpi: u32) -> i32;
        }
        #[link(name = "gdi32")]
        extern "system" {
            fn CreateCompatibleDC(hdc: isize) -> isize;
            fn CreateCompatibleBitmap(hdc: isize, cx: i32, cy: i32) -> isize;
            fn SelectObject(hdc: isize, h: isize) -> isize;
            fn DeleteObject(ho: isize) -> i32;
            fn DeleteDC(hdc: isize) -> i32;
            fn GetPixel(hdc: isize, x: i32, y: i32) -> u32;
        }
        #[link(name = "dwmapi")]
        extern "system" {
            fn DwmSetWindowAttribute(
                hwnd: isize,
                dw_attribute: u32,
                pv_attribute: *const std::ffi::c_void,
                cb_attribute: u32,
            ) -> i32;
            fn DwmGetWindowAttribute(
                hwnd: isize,
                dw_attribute: u32,
                pv_attribute: *mut std::ffi::c_void,
                cb_attribute: u32,
            ) -> i32;
            fn DwmFlush() -> i32;
        }

        static NCPAINT_DELEGATED_COUNT: AtomicUsize = AtomicUsize::new(0);
        static NCACTIVATE_LPARAM_PRESERVED: AtomicUsize = AtomicUsize::new(0);

        unsafe extern "system" fn mock_tao_shadow_wnd_proc(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: isize,
        ) -> isize {
            const WM_NCCALCSIZE: u32 = 0x0083;
            const WM_NCPAINT: u32 = 0x0085;
            const WM_NCACTIVATE: u32 = 0x0086;

            if msg == WM_NCCALCSIZE {
                let rect_ptr = if wparam == 0 {
                    lparam as *mut WinRect
                } else {
                    &mut (*(lparam as *mut WinNcCalcSizeParams)).rgrc[0] as *mut WinRect
                };
                if IsZoomed(hwnd) != 0 {
                    let hmon = MonitorFromWindow(hwnd, 2);
                    let mut mi = WinMonitorInfo {
                        cb_size: std::mem::size_of::<WinMonitorInfo>() as u32,
                        ..Default::default()
                    };
                    if hmon != 0 && GetMonitorInfoW(hmon, &mut mi) != 0 {
                        let mut rc = mi.rc_work;
                        // 同步模拟自动隐藏任务栏 1px 留边，确保最大化无白线与非最大化无蓝边同时成立
                        rc.bottom -= 1;
                        *rect_ptr = rc;
                        return 0;
                    }
                } else {
                    let dpi = GetDpiForWindow(hwnd).max(96);
                    let frame_thick =
                        GetSystemMetricsForDpi(32, dpi) + GetSystemMetricsForDpi(92, dpi);
                    let top_inset = ((dpi as f32) / 96.0).round() as i32;
                    (*rect_ptr).left += frame_thick;
                    (*rect_ptr).top += top_inset;
                    (*rect_ptr).right -= frame_thick;
                    (*rect_ptr).bottom -= frame_thick;
                    return 0;
                }
            } else if msg == WM_NCPAINT {
                NCPAINT_DELEGATED_COUNT.fetch_add(1, Ordering::Relaxed);
            } else if msg == WM_NCACTIVATE && lparam != -1 {
                NCACTIVATE_LPARAM_PRESERVED.fetch_add(1, Ordering::Relaxed);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        unsafe fn pump_window_msgs(ms: u64) {
            use windows::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
            };
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
            let mut m = MSG::default();
            while std::time::Instant::now() < deadline {
                while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&m);
                    let _ = DispatchMessageW(&m);
                }
                std::thread::sleep(std::time::Duration::from_millis(8));
            }
            let _ = DwmFlush();
        }

        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let cls_name: Vec<u16> = "BmTestUnmaxDwmInsetPreserveV2\0".encode_utf16().collect();
            let wc = WinClassExW {
                cb_size: std::mem::size_of::<WinClassExW>() as u32,
                style: 0x0023,
                lpfn_wnd_proc: Some(mock_tao_shadow_wnd_proc),
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance: hinst,
                h_icon: 0,
                h_cursor: 0,
                hbr_background: 0,
                lpsz_menu_name: std::ptr::null(),
                lpsz_class_name: cls_name.as_ptr(),
                h_icon_sm: 0,
            };
            let _ = RegisterClassExW(&wc);

            let base_style: u32 =
                0x00C00000 | 0x04000000 | 0x00080000 | 0x00040000 | 0x00010000 | 0x00020000;
            let hwnd = CreateWindowExW(
                0x00040100,
                cls_name.as_ptr(),
                cls_name.as_ptr(),
                base_style,
                140,
                120,
                760,
                500,
                0,
                0,
                hinst,
                0,
            );
            assert!(hwnd != 0);

            const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;
            const DWMWA_CLOAK: u32 = 13;
            const DWMWA_BORDER_COLOR: u32 = 34;
            const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
            const PW_RENDERFULLCONTENT: u32 = 2;

            let border_none = DWMWA_COLOR_NONE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &border_none as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            MAIN_BG_COLORREF.store(12u32 | (14u32 << 8) | (20u32 << 16), Ordering::Relaxed);
            install_nc_paint_subclass_on_hwnd(hwnd);

            let _ = ShowWindow(hwnd, 5); // SW_SHOW
            pump_window_msgs(80);

            // 记录初始正常默认窗口的 DWM 透明扩展帧厚度（Windows 11 上左右底应为 ~7px）
            let mut wr0 = WinRect::default();
            let mut ext0 = WinRect::default();
            assert!(GetWindowRect(hwnd, &mut wr0) != 0);
            let dwm_ok0 = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut ext0 as *mut WinRect as *mut _,
                std::mem::size_of::<WinRect>() as u32,
            ) == 0;
            let init_left_inset = ext0.left - wr0.left;

            // 覆盖四套主题（D1 / L1 / L2 / D2）反复完整循环：
            // 最大化 -> 隐身进小窗 -> 隐身一步恢复最大化 -> 校验底边 1px 零白线 -> 还原回默认窗口大小 -> 校验 DWM 扩展帧与边缘像素无 Aero Basic 蓝框
            let theme_colorrefs: [u32; 4] = [
                12u32 | (14u32 << 8) | (20u32 << 16),   // D1 #0C0E14
                246u32 | (248u32 << 8) | (252u32 << 16), // L1 #F6F8FC
                232u32 | (224u32 << 8) | (208u32 << 16), // L2 #E8E0D0
                26u32 | (17u32 << 8) | (22u32 << 16),   // D2 #1A1116
            ];

            for &theme_bg in &theme_colorrefs {
                MAIN_BG_COLORREF.store(theme_bg, Ordering::Relaxed);
                NCPAINT_DELEGATED_COUNT.store(0, Ordering::Relaxed);
                NCACTIVATE_LPARAM_PRESERVED.store(0, Ordering::Relaxed);

                let _ = ShowWindow(hwnd, 3); // SW_SHOWMAXIMIZED
                pump_window_msgs(60);
                assert_ne!(IsZoomed(hwnd), 0);

                // 模拟进入 Mini 小窗：Cloak + SW_HIDE
                let one: u32 = 1;
                let zero: u32 = 0;
                let _ = DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &one as *const u32 as *const _, 4);
                let _ = ShowWindow(hwnd, 0);
                let _ =
                    DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &zero as *const u32 as *const _, 4);
                pump_window_msgs(40);

                // 模拟退出小窗回最大化：Cloak + SW_SHOWMAXIMIZED + 解除 Cloak
                let _ = DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &one as *const u32 as *const _, 4);
                let _ = ShowWindow(hwnd, 3);
                paint_main_window_non_client_area(hwnd);
                pump_window_msgs(40);
                let _ =
                    DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &zero as *const u32 as *const _, 4);
                pump_window_msgs(60);
                assert_ne!(IsZoomed(hwnd), 0);

                // 校验 1：最大化态自动隐藏任务栏 1px 留边必须匹配当前主题底色，绝不漏纯白
                let mut wr_m = WinRect::default();
                let mut cr_m = WinRect::default();
                let mut tl_m = WinPoint::default();
                assert!(GetWindowRect(hwnd, &mut wr_m) != 0);
                assert!(GetClientRect(hwnd, &mut cr_m) != 0);
                assert!(ClientToScreen(hwnd, &mut tl_m) != 0);
                let wm = wr_m.right - wr_m.left;
                let hm = wr_m.bottom - wr_m.top;
                let gap_y = ((tl_m.y - wr_m.top) + (cr_m.bottom - cr_m.top)).min(hm - 1);
                let hdc_win_m = GetWindowDC(hwnd);
                let hdc_mem_m = CreateCompatibleDC(hdc_win_m);
                let hbmp_m = CreateCompatibleBitmap(hdc_win_m, wm, hm);
                let old_m = SelectObject(hdc_mem_m, hbmp_m);
                assert_eq!(PrintWindow(hwnd, hdc_mem_m, PW_RENDERFULLCONTENT), 1);
                let max_gap_px = GetPixel(hdc_mem_m, wm / 2, gap_y);
                assert_eq!(
                    max_gap_px, theme_bg,
                    "最大化态底部 1px 非客户区必须精准匹配当前主题底色，杜绝白线"
                );
                let _ = SelectObject(hdc_mem_m, old_m);
                let _ = DeleteObject(hbmp_m);
                let _ = DeleteDC(hdc_mem_m);
                let _ = ReleaseDC(hwnd, hdc_win_m);

                // 模拟 tao unmaximize() 完整序列
                let _ = ShowWindow(hwnd, 9); // SW_RESTORE
                let _ = SetWindowLongW(hwnd, -16, (base_style | 0x1000_0000) as i32);
                let _ = SetWindowPos(hwnd, 0, 0, 0, 0, 0, 0x0037);
                let _ = ShowWindow(hwnd, 5);
                let _ = SendMessageW(hwnd, 0x0085, 1, 0); // WM_NCPAINT
                let _ = SendMessageW(hwnd, 0x0086, 1, 0); // WM_NCACTIVATE
                pump_window_msgs(90);
                assert_eq!(IsZoomed(hwnd), 0);

                // 校验 2：无论最大化还是还原态，WM_NCPAINT 与 WM_NCACTIVATE 都必须透传给原生 DefWindowProcW
                assert!(
                    NCPAINT_DELEGATED_COUNT.load(Ordering::Relaxed) >= 1,
                    "WM_NCPAINT 必须始终先透传给原生 DefWindowProcW，绝不能吞掉消息导致 DWM 退化"
                );
                assert!(
                    NCACTIVATE_LPARAM_PRESERVED.load(Ordering::Relaxed) >= 1,
                    "WM_NCACTIVATE 必须始终保留原生 lparam 透传给 DefWindowProcW"
                );

                // 校验 3：Windows 11 原生 DWM 扩展阴影帧（DWMWA_EXTENDED_FRAME_BOUNDS）绝不能坍塌归零！
                let mut wr = WinRect::default();
                let mut ext = WinRect::default();
                assert!(GetWindowRect(hwnd, &mut wr) != 0);
                if dwm_ok0 && init_left_inset > 0 {
                    assert_eq!(
                        DwmGetWindowAttribute(
                            hwnd,
                            DWMWA_EXTENDED_FRAME_BOUNDS,
                            &mut ext as *mut WinRect as *mut _,
                            std::mem::size_of::<WinRect>() as u32,
                        ),
                        0
                    );
                    assert_eq!(
                        ext.left - wr.left,
                        init_left_inset,
                        "还原为默认窗口大小后左侧 DWM 透明阴影帧厚度必须完整保留，禁止退化归零"
                    );
                    assert_eq!(
                        wr.right - ext.right,
                        init_left_inset,
                        "还原为默认窗口大小后右侧 DWM 透明阴影帧厚度必须完整保留"
                    );
                    assert_eq!(
                        wr.bottom - ext.bottom,
                        init_left_inset,
                        "还原为默认窗口大小后底部 DWM 透明阴影帧厚度必须完整保留"
                    );
                }

                // 校验 4：采样还原态左右/底边非客户区像素，严禁出现 Windows 7 Aero Basic 蓝灰边框 (186,210,234) 或涂色色带！
                let w = wr.right - wr.left;
                let h = wr.bottom - wr.top;
                let hdc_win = GetWindowDC(hwnd);
                let hdc_mem = CreateCompatibleDC(hdc_win);
                let hbmp = CreateCompatibleBitmap(hdc_win, w, h);
                let old = SelectObject(hdc_mem, hbmp);
                assert_eq!(PrintWindow(hwnd, hdc_mem, PW_RENDERFULLCONTENT), 1);
                let left_px = GetPixel(hdc_mem, 3, h / 2);
                let right_px = GetPixel(hdc_mem, w - 4, h / 2);
                let bottom_px = GetPixel(hdc_mem, w / 2, h - 4);
                assert_eq!(
                    (left_px, right_px, bottom_px),
                    (0, 0, 0),
                    "还原为默认窗口大小后左右底边阴影热区必须保持纯粹透明（0,0,0），禁止出现 Aero Basic 蓝框或实体色带"
                );
                let _ = SelectObject(hdc_mem, old);
                let _ = DeleteObject(hbmp);
                let _ = DeleteDC(hdc_mem);
                let _ = ReleaseDC(hwnd, hdc_win);
            }

            let _ = DestroyWindow(hwnd);
        }
    }

    #[test]
    fn test_is_safe_external_open_url() {
        assert!(is_safe_external_open_url("https://h5.kugou.com/v2/v-5a15aeb1/index.html?hash=abc"));
        assert!(is_safe_external_open_url("https://m.kugou.com/share/?action=single&hash=abc"));
        assert!(is_safe_external_open_url("kugou://start.weixin?%7B%22cmd%22%3A212%7D"));
        assert!(is_safe_external_open_url("kugouyouth://start.weixin?%7B%22cmd%22%3A103301%7D"));
        assert!(!is_safe_external_open_url("file:///C:/Windows/System32/calc.exe"));
        assert!(!is_safe_external_open_url("javascript:alert(1)"));
        assert!(!is_safe_external_open_url("http://127.0.0.1:8080/admin"));
        assert!(!is_safe_external_open_url("https://user:pass@m.kugou.com/share/"));
        assert!(!is_safe_external_open_url("https://m.kugou.com/share/\" && calc"));
    }
}
