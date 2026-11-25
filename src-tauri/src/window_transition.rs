//! BreezeMusic For Windows — 原生 DirectComposition GPU 空间过渡引擎与零白线主窗口生命周期管理
//!
//! 设计原则：
//! 1. 彻底绕开 tao-0.37 在 Windows 上最大化窗口 hide() -> show() 时篡改 GWL_STYLE(去掉 WS_VISIBLE) +
//!    SWP_FRAMECHANGED 导致底部白线的底层缺陷，也彻底摒弃会造成「先变回默认窗口大小再快速最大化」
//!    的 SW_RESTORE -> SW_SHOWMAXIMIZED 强拉补丁。
//! 2. 所有主窗口隐藏/恢复均在 DWMWA_CLOAK = 1 与 DWMWA_TRANSITIONS_FORCEDISABLED = 1 隐身保护下原子完成，
//!    最大化状态一步到位 SW_SHOWMAXIMIZED + 隐身帧/WebView2 Bounds 对齐，视觉上 0 闪烁、0 白线、0 弹跳。
//! 3. 进出 Mini 小窗使用独立 D3D11 + DirectComposition (WS_EX_NOREDIRECTIONBITMAP) GPU 合成舞台：
//!    - 等比缩放（拒绝把 16:9 主窗口压扁畸变成细长胶囊）+ 指向 Mini 位置的空间牵引位移
//!    - 硬件抗锯齿动态圆角裁剪（IDCompositionRectangleClip：最大化 0px <-> 22px，默认窗 8px <-> 22px）
//!    - IDCompositionAnimation 分段三次 Hermite 样条精准拟合 Apple 高阻尼曲线，交由 Windows DWM 合成器
//!      以显示器原生刷新率（60/120/144/240Hz）零掉帧独立驱动。

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectPx {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RectPx {
    pub fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    pub fn center_x(self) -> f32 {
        self.left as f32 + self.width() as f32 * 0.5
    }

    pub fn center_y(self) -> f32 {
        self.top as f32 + self.height() as f32 * 0.5
    }

    pub fn is_valid(self) -> bool {
        self.width() >= 16 && self.height() >= 16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionMotionStyle {
    /// 对应 motionLevel = "best"：完整 Apple 高阻尼空间势能交接
    Rich,
    /// 对应 motionLevel = "standard"：明快克制短距过渡
    Standard,
    /// 对应 motionLevel = "none" 或系统减弱动态效果：0ms 零闪烁原子切换
    None,
}

impl TransitionMotionStyle {
    pub fn from_level_str(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "none" | "off" | "reduced" => Self::None,
            "standard" | "normal" => Self::Standard,
            _ => Self::Rich,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    EnterMini,
    ExitMini,
    RestoreFromTray,
}

/// 窗口过渡的空间几何与时序规划（纯数据，便于跨平台单元测试）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialTransitionPlan {
    pub kind: TransitionKind,
    pub duration_ms: u64,
    /// 退出小窗时 GPU 舞台抵达终态后与真实主窗口交叉溶解揭幕的时长（ms）
    pub reveal_dissolve_ms: u64,
    /// 进入小窗时 Mini 窗口在主窗口收束进行到多少毫秒后接力浮现（ms）
    pub mini_handoff_delay_ms: u64,
    pub start_scale: f32,
    pub end_scale: f32,
    pub start_dx: f32,
    pub start_dy: f32,
    pub end_dx: f32,
    pub end_dy: f32,
    pub start_opacity: f32,
    pub end_opacity: f32,
    pub start_radius_px: f32,
    pub end_radius_px: f32,
    /// 传给 Mini 前端的微位移方向提示（逻辑像素，用于入场/退场物理衔接）
    pub mini_hint_offset_x: f32,
    pub mini_hint_offset_y: f32,
    pub motion_style: TransitionMotionStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicPolySegment {
    pub begin_offset_sec: f64,
    pub constant: f32,
    pub linear: f32,
    pub quadratic: f32,
    pub cubic: f32,
}

/// 标准化三次贝塞尔曲线求值（用于精确拟合 CSS cubic-bezier）
fn eval_bezier_1d(t: f64, p1: f64, p2: f64) -> f64 {
    let u = 1.0 - t;
    3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
}

fn eval_bezier_deriv_1d(t: f64, p1: f64, p2: f64) -> f64 {
    let u = 1.0 - t;
    3.0 * u * u * p1 + 6.0 * u * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

/// 给定时间进度 x ∈ [0, 1]，使用牛顿迭代 + 二分兜底精确求 cubic-bezier(x1, y1, x2, y2) 的 y(x) 与 dy/dx
pub fn sample_cubic_bezier(x: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> (f64, f64) {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.0 {
        let slope = if x1.abs() > 1e-6 { y1 / x1 } else { 0.0 };
        return (0.0, slope);
    }
    if x >= 1.0 {
        let slope = if (1.0 - x2).abs() > 1e-6 {
            (1.0 - y2) / (1.0 - x2)
        } else {
            0.0
        };
        return (1.0, slope);
    }

    let mut lo = 0.0;
    let mut hi = 1.0;
    let mut t = x;
    for _ in 0..8 {
        let bx = eval_bezier_1d(t, x1, x2) - x;
        if bx.abs() < 1e-7 {
            break;
        }
        let d = eval_bezier_deriv_1d(t, x1, x2);
        if d.abs() < 1e-6 {
            break;
        }
        let next = t - bx / d;
        if next > lo && next < hi {
            t = next;
        } else {
            if bx < 0.0 {
                lo = t;
            } else {
                hi = t;
            }
            t = 0.5 * (lo + hi);
        }
    }
    for _ in 0..16 {
        let bx = eval_bezier_1d(t, x1, x2);
        if (bx - x).abs() < 1e-7 {
            break;
        }
        if bx < x {
            lo = t;
        } else {
            hi = t;
        }
        t = 0.5 * (lo + hi);
    }

    let y = eval_bezier_1d(t, y1, y2);
    let dx = eval_bezier_deriv_1d(t, x1, x2);
    let dy = eval_bezier_deriv_1d(t, y1, y2);
    let slope = if dx.abs() > 1e-5 { dy / dx } else { 0.0 };
    (y, slope)
}

/// 将任意 cubic-bezier(x1, y1, x2, y2) 在 [0, duration_sec] 上用多段 C¹ 连续三次 Hermite 样条精准展开，
/// 直接喂给 DirectComposition IDCompositionAnimation::AddCubic，使 GPU 合成线程以 120Hz/144Hz 精确还原。
pub fn build_hermite_cubic_segments(
    from: f32,
    to: f32,
    duration_sec: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    segments: usize,
) -> Vec<CubicPolySegment> {
    let n = segments.max(2);
    let dur = duration_sec.max(0.001);
    let delta = (to - from) as f64;
    let mut out = Vec::with_capacity(n);

    for i in 0..n {
        let u0 = i as f64 / n as f64;
        let u1 = (i + 1) as f64 / n as f64;
        let h = (u1 - u0) * dur;
        let (p0, s0) = sample_cubic_bezier(u0, x1, y1, x2, y2);
        let (p1, s1) = sample_cubic_bezier(u1, x1, y1, x2, y2);

        let v0 = from as f64 + delta * p0;
        let v1 = from as f64 + delta * p1;
        // 将无量纲导数 dy/du 转换为真实时间导数 dv/dt (单位/秒)
        let m0 = (delta * s0) / dur;
        let m1 = (delta * s1) / dur;

        // 局部时间 s ∈ [0, h] 上的三次 Hermite 多项式：
        // P(s) = d + c*s + b*s^2 + a*s^3
        let d = v0;
        let c = m0;
        let b = (3.0 * (v1 - v0) - (2.0 * m0 + m1) * h) / (h * h);
        let a = (2.0 * (v0 - v1) + (m0 + m1) * h) / (h * h * h);

        out.push(CubicPolySegment {
            begin_offset_sec: u0 * dur,
            constant: d as f32,
            linear: c as f32,
            quadratic: b as f32,
            cubic: a as f32,
        });
    }

    out
}

/// 根据主窗口状态（最大化 vs 默认窗口）、Mini 目标位置与动效档位，生成统一优雅的空间势能交接方案
pub fn compute_spatial_transition_plan(
    kind: TransitionKind,
    main_rect: RectPx,
    mini_rect: Option<RectPx>,
    main_is_maximized: bool,
    same_monitor: bool,
    scale_factor: f64,
    motion_style: TransitionMotionStyle,
) -> SpatialTransitionPlan {
    let sf = scale_factor.clamp(1.0, 4.0) as f32;

    // 计算指向 Mini 小窗的单位方向向量与空间位移向量（仅当同屏且坐标有效时牵引）
    let (raw_dx, raw_dy) = match (same_monitor, mini_rect) {
        (true, Some(mr)) if main_rect.is_valid() && mr.is_valid() => (
            mr.center_x() - main_rect.center_x(),
            mr.center_y() - main_rect.center_y(),
        ),
        _ => (0.0, 0.0),
    };
    let dist = (raw_dx * raw_dx + raw_dy * raw_dy).sqrt();
    let (dir_x, dir_y) = if dist > 1.0 {
        (raw_dx / dist, raw_dy / dist)
    } else {
        (0.0, -0.5)
    };

    // 给 Mini 前端的微位移提示（逻辑像素，范围柔和克制在 ±18px 内）
    let hint_mag = match motion_style {
        TransitionMotionStyle::Rich => 18.0,
        TransitionMotionStyle::Standard => 10.0,
        TransitionMotionStyle::None => 0.0,
    };
    // 进入小窗时，Mini 从「偏向主窗口的一侧」滑入落位 -> 偏移方向与主窗->Mini方向相反
    // 退出小窗时，Mini 朝「主窗口中心方向」微移消隐 -> 同理指向主窗方向 (-dir_x, -dir_y)
    let mini_hint_offset_x = -dir_x * hint_mag;
    let mini_hint_offset_y = -dir_y * hint_mag;

    if motion_style == TransitionMotionStyle::None {
        return SpatialTransitionPlan {
            kind,
            duration_ms: 0,
            reveal_dissolve_ms: 0,
            mini_handoff_delay_ms: 0,
            start_scale: 1.0,
            end_scale: 1.0,
            start_dx: 0.0,
            start_dy: 0.0,
            end_dx: 0.0,
            end_dy: 0.0,
            start_opacity: 1.0,
            end_opacity: 1.0,
            start_radius_px: 0.0,
            end_radius_px: 0.0,
            mini_hint_offset_x: 0.0,
            mini_hint_offset_y: 0.0,
            motion_style,
        };
    }

    let is_rich = motion_style == TransitionMotionStyle::Rich;
    // 最大化窗口贴齐屏幕四边，起始/终态圆角必须为 0px；非最大化普通窗对齐 Win11 系统 8px 圆角
    let rest_radius = if main_is_maximized { 0.0 } else { 8.0 * sf };
    let enter_morph_radius = if is_rich { 36.0 * sf } else { 22.0 * sf };
    let exit_morph_radius = if is_rich { 24.0 * sf } else { 16.0 * sf };

    match kind {
        TransitionKind::EnterMini => {
            // 进入小窗：允许更大范围的方向牵引，让主窗口清晰优雅地朝灵动岛/小窗实体飞掠收束
            let max_pull_x =
                (main_rect.width() as f32 * if is_rich { 0.42 } else { 0.24 }).max(64.0 * sf);
            let max_pull_y =
                (main_rect.height() as f32 * if is_rich { 0.44 } else { 0.26 }).max(64.0 * sf);
            let pull_ratio = match (main_is_maximized, is_rich) {
                (true, true) => 0.54,
                (false, true) => 0.58,
                (true, false) => 0.30,
                (false, false) => 0.34,
            };
            let pull_dx = (raw_dx * pull_ratio).clamp(-max_pull_x, max_pull_x);
            let pull_dy = (raw_dy * pull_ratio).clamp(-max_pull_y, max_pull_y);

            let end_scale = match (main_is_maximized, is_rich) {
                (true, true) => 0.54,
                (false, true) => 0.48,
                (true, false) => 0.72,
                (false, false) => 0.68,
            };
            SpatialTransitionPlan {
                kind,
                duration_ms: if is_rich { 310 } else { 210 },
                reveal_dissolve_ms: 0,
                mini_handoff_delay_ms: if is_rich { 125 } else { 78 },
                start_scale: 1.0,
                end_scale,
                start_dx: 0.0,
                start_dy: 0.0,
                end_dx: pull_dx,
                end_dy: pull_dy,
                start_opacity: 1.0,
                end_opacity: 0.0,
                start_radius_px: rest_radius,
                end_radius_px: enter_morph_radius,
                mini_hint_offset_x,
                mini_hint_offset_y,
                motion_style,
            }
        }
        TransitionKind::ExitMini => {
            let max_pull_x =
                (main_rect.width() as f32 * if is_rich { 0.20 } else { 0.10 }).max(42.0 * sf);
            let max_pull_y =
                (main_rect.height() as f32 * if is_rich { 0.22 } else { 0.11 }).max(42.0 * sf);
            let pull_ratio = match (main_is_maximized, is_rich) {
                (true, true) => 0.22,
                (false, true) => 0.28,
                (true, false) => 0.12,
                (false, false) => 0.15,
            };
            let pull_dx = (raw_dx * pull_ratio).clamp(-max_pull_x, max_pull_x);
            let pull_dy = (raw_dy * pull_ratio).clamp(-max_pull_y, max_pull_y);

            let start_scale = match (main_is_maximized, is_rich) {
                (true, true) => 0.86,
                (false, true) => 0.82,
                (true, false) => 0.93,
                (false, false) => 0.91,
            };
            SpatialTransitionPlan {
                kind,
                duration_ms: if is_rich { 260 } else { 170 },
                reveal_dissolve_ms: if is_rich { 65 } else { 40 },
                mini_handoff_delay_ms: 0,
                start_scale,
                end_scale: 1.0,
                start_dx: pull_dx * 0.85,
                start_dy: pull_dy * 0.85,
                end_dx: 0.0,
                end_dy: 0.0,
                start_opacity: 0.0,
                end_opacity: 1.0,
                start_radius_px: exit_morph_radius,
                end_radius_px: rest_radius,
                mini_hint_offset_x,
                mini_hint_offset_y,
                motion_style,
            }
        }
        TransitionKind::RestoreFromTray => {
            let start_scale = match (main_is_maximized, is_rich) {
                (true, true) => 0.945,
                (false, true) => 0.925,
                (true, false) => 0.965,
                (false, false) => 0.955,
            };
            let drop_y = if is_rich { 18.0 * sf } else { 10.0 * sf };
            SpatialTransitionPlan {
                kind,
                duration_ms: if is_rich { 215 } else { 140 },
                reveal_dissolve_ms: if is_rich { 55 } else { 35 },
                mini_handoff_delay_ms: 0,
                start_scale,
                end_scale: 1.0,
                start_dx: 0.0,
                start_dy: drop_y,
                end_dx: 0.0,
                end_dy: 0.0,
                start_opacity: 0.0,
                end_opacity: 1.0,
                start_radius_px: exit_morph_radius * 0.75,
                end_radius_px: rest_radius,
                mini_hint_offset_x: 0.0,
                mini_hint_offset_y: 0.0,
                motion_style,
            }
        }
    }
}

/// 缓存最近一次高质量主窗口客户区快照，作为退出小窗且 DWMWA_CLOAK 期间无法抓实时帧时的无缝降级纹理
#[derive(Clone)]
pub struct CachedMainSnapshot {
    pub width: u32,
    pub height: u32,
    pub bg_colorref: u32,
    pub bgra_premultiplied: Vec<u8>,
}

static LAST_MAIN_SNAPSHOT: once_cell::sync::Lazy<Mutex<Option<CachedMainSnapshot>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

/// 标记当前主窗口是否被我们通过 DWMWA_CLOAK 临时隐身（防止异常路径漏恢复）
static MAIN_CLOAKED_FLAG: AtomicBool = AtomicBool::new(false);

pub fn store_cached_main_snapshot(snap: CachedMainSnapshot) {
    if let Ok(mut g) = LAST_MAIN_SNAPSHOT.lock() {
        *g = Some(snap);
    }
}

pub fn load_cached_main_snapshot(
    expect_w: u32,
    expect_h: u32,
    expect_bg_colorref: u32,
) -> Option<CachedMainSnapshot> {
    let guard = LAST_MAIN_SNAPSHOT.lock().ok()?;
    let snap = guard.as_ref()?;
    // 只有尺寸基本一致且主题底色未大改（避免浅色<->深色切换时露出旧主题）时才复用
    let dw = (snap.width as i64 - expect_w as i64).abs();
    let dh = (snap.height as i64 - expect_h as i64).abs();
    if dw <= 8 && dh <= 8 && snap.bg_colorref == expect_bg_colorref {
        Some(snap.clone())
    } else {
        None
    }
}

pub fn clear_cached_main_snapshot() {
    if let Ok(mut g) = LAST_MAIN_SNAPSHOT.lock() {
        *g = None;
    }
}

/// 生成一张与当前四主题底色 + 封面双主色柔光完全一致的高定无框渐变玻璃卡面（BGRA 预乘，绝不闪黑屏）
pub fn synthesize_theme_aura_card_bgra(
    width: u32,
    height: u32,
    bg_rgb: (u8, u8, u8),
    accent1_rgb: (u8, u8, u8),
    accent2_rgb: (u8, u8, u8),
) -> Vec<u8> {
    let w = width.max(1) as usize;
    let h = height.max(1) as usize;
    let mut buf = vec![0u8; w * h * 4];
    let inv_w = 1.0f32 / (w.max(1) as f32);
    let inv_h = 1.0f32 / (h.max(1) as f32);

    let (br, bg, bb) = (bg_rgb.0 as f32, bg_rgb.1 as f32, bg_rgb.2 as f32);
    let (a1r, a1g, a1b) = (accent1_rgb.0 as f32, accent1_rgb.1 as f32, accent1_rgb.2 as f32);
    let (a2r, a2g, a2b) = (accent2_rgb.0 as f32, accent2_rgb.1 as f32, accent2_rgb.2 as f32);
    let is_light = (br * 0.299 + bg * 0.587 + bb * 0.114) > 155.0;
    let glow_strength = if is_light { 0.11f32 } else { 0.16f32 };

    for y in 0..h {
        let ny = y as f32 * inv_h;
        let dy1 = ny - 0.18;
        let dy2 = ny - 0.84;
        let row_off = y * w * 4;
        for x in 0..w {
            let nx = x as f32 * inv_w;
            let dx1 = nx - 0.18;
            let dx2 = nx - 0.84;
            let d1 = (dx1 * dx1 + dy1 * dy1).min(1.0);
            let d2 = (dx2 * dx2 + dy2 * dy2).min(1.0);
            let w1 = (1.0 - d1 * 1.45).max(0.0);
            let w2 = (1.0 - d2 * 1.45).max(0.0);
            let k1 = w1 * w1 * glow_strength;
            let k2 = w2 * w2 * (glow_strength * 0.82);

            let r = (br * (1.0 - k1 - k2) + a1r * k1 + a2r * k2).clamp(0.0, 255.0) as u8;
            let g = (bg * (1.0 - k1 - k2) + a1g * k1 + a2g * k2).clamp(0.0, 255.0) as u8;
            let b = (bb * (1.0 - k1 - k2) + a1b * k1 + a2b * k2).clamp(0.0, 255.0) as u8;

            let idx = row_off + x * 4;
            buf[idx] = b;
            buf[idx + 1] = g;
            buf[idx + 2] = r;
            buf[idx + 3] = 255;
        }
    }
    buf
}

#[cfg(target_os = "windows")]
pub mod win_impl {
    use super::*;
    use std::cell::RefCell;
    use windows::core::{w, Interface, PCWSTR};
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL,
        D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_11_0,
    };
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
        D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
        D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC, D3D11_USAGE_IMMUTABLE,
    };
    use windows::Win32::Graphics::DirectComposition::{
        DCompositionCreateDevice, IDCompositionAnimation, IDCompositionClip, IDCompositionDevice,
        IDCompositionEffect, IDCompositionEffectGroup, IDCompositionRectangleClip,
        IDCompositionScaleTransform, IDCompositionSurface, IDCompositionTarget,
        IDCompositionTransform, IDCompositionTranslateTransform, IDCompositionVisual,
        DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR, DCOMPOSITION_BORDER_MODE_SOFT,
    };
    use windows::Win32::Graphics::Dwm::{
        DwmFlush, DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CLOAK,
        DWMWA_CLOAKED, DWMWA_COLOR_NONE, DWMWA_EXTENDED_FRAME_BOUNDS,
        DWMWA_TRANSITIONS_FORCEDISABLED,
    };
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
    };
    use windows::Win32::Graphics::Dxgi::IDXGIDevice;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, ClientToScreen, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject,
        GetDC, GetMonitorInfoW, MonitorFromWindow, RedrawWindow, ReleaseDC, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, MONITORINFO,
        MONITOR_DEFAULTTONEAREST, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE,
        RDW_UPDATENOW, SRCCOPY,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, CreateWindowExW, DefWindowProcW, GetClassNameW, GetClientRect,
        GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowPlacement, GetWindowRect,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, RegisterClassExW,
        SendMessageW, SetForegroundWindow, SetWindowLongW, SetWindowPos, ShowWindow, GWL_EXSTYLE,
        GW_HWNDPREV, HTTRANSPARENT, HWND_NOTOPMOST, HWND_TOPMOST, SC_RESTORE, SWP_FRAMECHANGED,
        SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
        SW_HIDE, SW_RESTORE, SW_SHOW, SW_SHOWMAXIMIZED, SW_SHOWNOACTIVATE, WINDOWPLACEMENT,
        WM_ERASEBKGND, WM_NCHITTEST, WM_SYSCOMMAND, WNDCLASSEXW, WPF_RESTORETOMAXIMIZED,
        WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    const PW_CLIENTONLY: u32 = 0x0000_0001;
    const PW_RENDERFULLCONTENT: u32 = 0x0000_0002;

    #[link(name = "user32")]
    extern "system" {
        fn PrintWindow(hwnd: isize, hdc_blt: isize, n_flags: u32) -> i32;
    }

    pub fn is_main_cloaked() -> bool {
        MAIN_CLOAKED_FLAG.load(Ordering::Acquire)
    }

    /// 原子切换透明悬浮子窗（如 Mini 灵动岛）的点击穿透态：
    /// 始终保留 WS_EX_TOOLWINDOW 并剥离 WS_EX_APPWINDOW，绝不调用 ShowWindow 或触发 tao apply_diff 篡改可见态。
    /// 仅当 TOOLWINDOW/APPWINDOW 帧属性确实发生变化时才附带 SWP_FRAMECHANGED，避免纯穿透切换反复触发
    /// WM_MOUSELEAVE 冲掉 WebView2 悬停态或引发 DWM 非客户区重算抖动。
    pub fn set_overlay_click_through_raw(hwnd_raw: isize, ignore: bool) {
        if hwnd_raw == 0 {
            return;
        }
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            let base = (ex | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0;
            let frame_bits = WS_EX_TOOLWINDOW.0 | WS_EX_APPWINDOW.0;
            let frame_style_changed = (ex & frame_bits) != (base & frame_bits);
            let mask = WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0;
            let next = if ignore { base | mask } else { base & !mask };
            if next != ex {
                let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, next as i32);
                if frame_style_changed {
                    let _ = SetWindowPos(
                        hwnd,
                        None,
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

    /// 查询窗口当前是否处于 DWM Cloaked 隐身状态（用于准确过滤挂起 UWP 窗与检验全屏礼让状态）
    pub fn is_window_cloaked(hwnd_raw: isize) -> bool {
        if hwnd_raw == 0 {
            return false;
        }
        let mut cloaked: u32 = 0;
        unsafe {
            DwmGetWindowAttribute(
                HWND(hwnd_raw as *mut _),
                DWMWA_CLOAKED,
                &mut cloaked as *mut u32 as *mut _,
                std::mem::size_of::<u32>() as u32,
            )
            .is_ok()
                && cloaked != 0
        }
    }

    /// 强制隐藏透明悬浮子窗（如 Mini 灵动岛）并立即开启穿透、剥离 HWND_TOPMOST 与可见态：
    /// 彻底绕开 tao WindowFlags::VISIBLE 缓存不同步导致 win.hide() 静默空操作（产生不可见点击遮挡幽灵窗）的问题。
    pub fn hide_overlay_window_native(hwnd_raw: isize) {
        if hwnd_raw == 0 {
            return;
        }
        set_overlay_click_through_raw(hwnd_raw, true);
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_HIDEWINDOW,
            );
        }
        set_window_cloak(hwnd_raw, false);
    }

    /// 原子显示透明悬浮子窗并维持 HWND_TOPMOST（零抢焦、清除 DWM Cloak、保 WS_EX_TOOLWINDOW 与目标点击穿透态）
    pub fn show_overlay_window_native(hwnd_raw: isize, click_through: bool) {
        if hwnd_raw == 0 {
            return;
        }
        set_window_cloak(hwnd_raw, false);
        set_overlay_click_through_raw(hwnd_raw, click_through);
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    /// 在 DWMWA_CLOAK = 1 完全隐身掩护下先唤醒显示悬浮子窗（如刚从 TrySuspend 恢复的 Mini 小窗）：
    /// 让 WebView2 在屏幕 0 像素可见的前提下完成 entering(opacity=0) 首帧与最新歌曲封面合成，
    /// 彻底杜绝 ShowWindow 瞬间闪现上一帧再变透明的“双闪”现象。
    pub fn prepare_overlay_window_cloaked_native(hwnd_raw: isize) {
        if hwnd_raw == 0 {
            return;
        }
        set_overlay_click_through_raw(hwnd_raw, true);
        set_window_cloak(hwnd_raw, true);
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    /// 真全屏独占应用（F11 浏览器 / 全屏视频 / 全屏游戏 / PPT 放映）礼让：
    /// 原位通过 DWMWA_CLOAK = 1 零闪烁隐身 + 开启点击穿透 + 剥离 HWND_TOPMOST，
    /// 既 0ms 彻底退让不遮挡画面或拦截鼠标，又不调用 SW_HIDE 污染 tao WindowFlags::VISIBLE 缓存。
    pub fn yield_overlay_window_for_fullscreen_native(hwnd_raw: isize) {
        if hwnd_raw == 0 {
            return;
        }
        set_overlay_click_through_raw(hwnd_raw, true);
        set_window_cloak(hwnd_raw, true);
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    /// 纯原生无抢焦重申 HWND_TOPMOST（自动解除可能残留的 DWM Cloak），绝不经由 tao 修改其他窗口属性
    pub fn reassert_overlay_topmost_native(hwnd_raw: isize) {
        if hwnd_raw == 0 {
            return;
        }
        if is_window_cloaked(hwnd_raw) {
            set_window_cloak(hwnd_raw, false);
        }
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    /// 轻量 Z-order 遮挡探针：沿 GW_HWNDPREV 向上扫描至多 24 层，检测是否存在与目标视觉区域相交的
    /// 非穿透普通应用窗口压盖在悬浮窗上方（自动忽略不可见/Cloaked/穿透窗/本应用子窗及系统菜单与任务栏壳层）。
    pub fn is_overlay_obscured_in_zorder(hwnd_raw: isize, ignore_hwnds: &[isize]) -> bool {
        if hwnd_raw == 0 {
            return false;
        }
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                return false;
            }
            let Some(target_rect) = get_window_visual_rect(hwnd_raw) else {
                return false;
            };
            let mut cur = hwnd;
            let mut steps = 0usize;
            while steps < 24 {
                steps += 1;
                let Ok(prev) = GetWindow(cur, GW_HWNDPREV) else {
                    break;
                };
                if prev.0.is_null() || prev == cur || prev == hwnd {
                    break;
                }
                cur = prev;
                let prev_raw = prev.0 as isize;
                if ignore_hwnds.contains(&prev_raw) {
                    continue;
                }
                if !IsWindowVisible(prev).as_bool()
                    || IsIconic(prev).as_bool()
                    || is_window_cloaked(prev_raw)
                {
                    continue;
                }
                let ex = GetWindowLongW(prev, GWL_EXSTYLE) as u32;
                if (ex & WS_EX_TRANSPARENT.0) != 0 {
                    continue;
                }
                let mut cls_buf = [0u16; 128];
                let len = GetClassNameW(prev, &mut cls_buf);
                if len > 0 {
                    let cls = String::from_utf16_lossy(&cls_buf[..len as usize]);
                    if matches!(
                        cls.as_str(),
                        "#32768"
                            | "tooltips_class32"
                            | "Shell_TrayWnd"
                            | "Shell_SecondaryTrayWnd"
                            | "NotifyIconOverflowWindow"
                            | "Windows.UI.Core.CoreWindow"
                            | "TaskListThumbnailWnd"
                            | "MultitaskingViewFrame"
                            | "XamlExplorerHostIslandWindow"
                            | "Auto-Suggest Dropdown"
                            | "Windows.UI.Input.InputSite.WindowClass"
                            | "ApplicationManager_DesktopShellWindow"
                            | "CEF-OSC-WIDGET"
                    ) {
                        continue;
                    }
                }
                let Some(prev_rect) = get_window_visual_rect(prev_raw) else {
                    continue;
                };
                let inter_w = target_rect.right.min(prev_rect.right)
                    - target_rect.left.max(prev_rect.left);
                let inter_h = target_rect.bottom.min(prev_rect.bottom)
                    - target_rect.top.max(prev_rect.top);
                if inter_w >= 4 && inter_h >= 4 {
                    return true;
                }
            }
        }
        false
    }

    pub fn set_window_cloak(hwnd_raw: isize, cloaked: bool) {
        if hwnd_raw == 0 {
            return;
        }
        let val: u32 = if cloaked { 1 } else { 0 };
        unsafe {
            let _ = DwmSetWindowAttribute(
                HWND(hwnd_raw as *mut _),
                DWMWA_CLOAK,
                &val as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }

    pub fn set_window_dwm_transitions_disabled(hwnd_raw: isize, disabled: bool) {
        if hwnd_raw == 0 {
            return;
        }
        let val: u32 = if disabled { 1 } else { 0 };
        unsafe {
            let _ = DwmSetWindowAttribute(
                HWND(hwnd_raw as *mut _),
                DWMWA_TRANSITIONS_FORCEDISABLED,
                &val as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }

    pub fn dwm_flush() {
        unsafe {
            let _ = DwmFlush();
        }
    }

    /// 获取窗口客户区在屏幕物理坐标系中的精确矩形
    pub fn get_window_client_screen_rect(hwnd_raw: isize) -> Option<RectPx> {
        if hwnd_raw == 0 {
            return None;
        }
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let mut cr = RECT::default();
            if GetClientRect(hwnd, &mut cr).is_err() {
                return None;
            }
            let mut tl = POINT {
                x: cr.left,
                y: cr.top,
            };
            let mut br = POINT {
                x: cr.right,
                y: cr.bottom,
            };
            if !ClientToScreen(hwnd, &mut tl).as_bool() || !ClientToScreen(hwnd, &mut br).as_bool()
            {
                return None;
            }
            let r = RectPx {
                left: tl.x,
                top: tl.y,
                right: br.x,
                bottom: br.y,
            };
            if r.is_valid() {
                Some(r)
            } else {
                None
            }
        }
    }

    /// 获取窗口视觉外框（优先 DWMWA_EXTENDED_FRAME_BOUNDS，剔除不可见的透明拖拽阴影边）
    pub fn get_window_visual_rect(hwnd_raw: isize) -> Option<RectPx> {
        if hwnd_raw == 0 {
            return None;
        }
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let mut rc = RECT::default();
            if DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut rc as *mut RECT as *mut _,
                std::mem::size_of::<RECT>() as u32,
            )
            .is_ok()
            {
                let r = RectPx {
                    left: rc.left,
                    top: rc.top,
                    right: rc.right,
                    bottom: rc.bottom,
                };
                if r.is_valid() {
                    return Some(r);
                }
            }
            if GetWindowRect(hwnd, &mut rc).is_ok() {
                let r = RectPx {
                    left: rc.left,
                    top: rc.top,
                    right: rc.right,
                    bottom: rc.bottom,
                };
                if r.is_valid() {
                    return Some(r);
                }
            }
        }
        None
    }

    /// 获取窗口所在显示器的整屏与工作区物理矩形
    pub fn get_monitor_rects_for_hwnd(hwnd_raw: isize) -> Option<(RectPx, RectPx)> {
        if hwnd_raw == 0 {
            return None;
        }
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            if hmon.0.is_null() {
                return None;
            }
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(hmon, &mut mi).as_bool() {
                return None;
            }
            let mon = RectPx {
                left: mi.rcMonitor.left,
                top: mi.rcMonitor.top,
                right: mi.rcMonitor.right,
                bottom: mi.rcMonitor.bottom,
            };
            let work = RectPx {
                left: mi.rcWork.left,
                top: mi.rcWork.top,
                right: mi.rcWork.right,
                bottom: mi.rcWork.bottom,
            };
            Some((mon, work))
        }
    }

    pub fn is_same_monitor(hwnd_a: isize, hwnd_b: isize) -> bool {
        if hwnd_a == 0 || hwnd_b == 0 {
            return false;
        }
        unsafe {
            let ma = MonitorFromWindow(HWND(hwnd_a as *mut _), MONITOR_DEFAULTTONEAREST);
            let mb = MonitorFromWindow(HWND(hwnd_b as *mut _), MONITOR_DEFAULTTONEAREST);
            !ma.0.is_null() && ma == mb
        }
    }

    pub fn is_hwnd_visible_and_not_minimized(hwnd_raw: isize) -> bool {
        if hwnd_raw == 0 {
            return false;
        }
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() }
    }

    pub fn is_hwnd_maximized_native(hwnd_raw: isize) -> bool {
        if hwnd_raw == 0 {
            return false;
        }
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            if IsZoomed(hwnd).as_bool() {
                return true;
            }
            let mut wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            if GetWindowPlacement(hwnd, &mut wp).is_ok() {
                if wp.showCmd == SW_SHOWMAXIMIZED.0 as u32 {
                    return true;
                }
                if IsIconic(hwnd).as_bool() && (wp.flags.0 & WPF_RESTORETOMAXIMIZED.0) != 0 {
                    return true;
                }
            }
            false
        }
    }

    /// 检查采集到的 BGRA 图像是否包含真实画面（避免全黑/纯单色空帧）
    fn has_meaningful_frame_variance(bgra: &[u8], w: u32, h: u32) -> bool {
        if bgra.len() < (w as usize) * (h as usize) * 4 || w < 16 || h < 16 {
            return false;
        }
        let mut first_rgb: Option<(u8, u8, u8)> = None;
        let mut non_zero_count = 0usize;
        let mut diff_count = 0usize;
        let grid = 7usize;
        for gy in 1..grid {
            let y = (h as usize * gy) / grid;
            for gx in 1..grid {
                let x = (w as usize * gx) / grid;
                let idx = (y * w as usize + x) * 4;
                let rgb = (bgra[idx + 2], bgra[idx + 1], bgra[idx]);
                if rgb.0 > 4 || rgb.1 > 4 || rgb.2 > 4 {
                    non_zero_count += 1;
                }
                match first_rgb {
                    None => first_rgb = Some(rgb),
                    Some(f) => {
                        let dr = (rgb.0 as i32 - f.0 as i32).abs();
                        let dg = (rgb.1 as i32 - f.1 as i32).abs();
                        let db = (rgb.2 as i32 - f.2 as i32).abs();
                        if dr + dg + db > 14 {
                            diff_count += 1;
                        }
                    }
                }
            }
        }
        non_zero_count >= 3 && diff_count >= 2
    }

    /// 采集主窗口客户区 32-bit BGRA 预乘位图（优先 PrintWindow(PW_CLIENTONLY | PW_RENDERFULLCONTENT)，
    /// 可选在窗口可见前景时用屏幕 BitBlt 兜底，确保绝不返回全黑帧）。
    pub fn capture_window_client_bgra(
        hwnd_raw: isize,
        allow_screen_blt_fallback: bool,
    ) -> Option<(u32, u32, Vec<u8>)> {
        let client_rect = get_window_client_screen_rect(hwnd_raw)?;
        let w = client_rect.width() as u32;
        let h = client_rect.height() as u32;
        if w < 16 || h < 16 || w > 8192 || h > 8192 {
            return None;
        }

        let try_capture = |use_screen_blt: bool| -> Option<Vec<u8>> {
            unsafe {
                let screen_dc = GetDC(None);
                if screen_dc.is_invalid() {
                    return None;
                }
                let mem_dc = CreateCompatibleDC(Some(screen_dc));
                if mem_dc.is_invalid() {
                    let _ = ReleaseDC(None, screen_dc);
                    return None;
                }

                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w as i32,
                        biHeight: -(h as i32), // top-down DIB
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };

                let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
                let hbmp =
                    CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)
                        .ok();

                let mut captured: Option<Vec<u8>> = None;
                if let Some(bmp) = hbmp {
                    if !bmp.is_invalid() && !bits_ptr.is_null() {
                        let old = SelectObject(mem_dc, bmp.into());
                        let ok = if use_screen_blt {
                            BitBlt(
                                mem_dc,
                                0,
                                0,
                                w as i32,
                                h as i32,
                                Some(screen_dc),
                                client_rect.left,
                                client_rect.top,
                                SRCCOPY,
                            )
                            .is_ok()
                        } else {
                            PrintWindow(
                                hwnd_raw,
                                mem_dc.0 as isize,
                                PW_CLIENTONLY | PW_RENDERFULLCONTENT,
                            ) != 0
                        };

                        if ok {
                            let total_bytes = (w as usize) * (h as usize) * 4;
                            let mut pixels = vec![0u8; total_bytes];
                            std::ptr::copy_nonoverlapping(
                                bits_ptr as *const u8,
                                pixels.as_mut_ptr(),
                                total_bytes,
                            );
                            // 强制 Alpha = 255（预乘完全不透明），防止 GDI/WebView2 子像素 alpha=0 透底
                            for chunk in pixels.chunks_exact_mut(4) {
                                chunk[3] = 255;
                            }
                            if has_meaningful_frame_variance(&pixels, w, h) {
                                captured = Some(pixels);
                            }
                        }

                        let _ = SelectObject(mem_dc, old);
                        let _ = DeleteObject(bmp.into());
                    }
                }

                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(None, screen_dc);
                captured
            }
        };

        // 当窗口当前前景可见且允许屏幕捕获时（如 EnterMini 点击切小窗按钮的瞬间），
        // 若未被其他非本应用实窗遮挡，优先使用极速屏幕 BitBlt（~1.5ms，100% 精准还原 WebView2/WebGL2 真画面，
        // 彻底规避对 WebView2 调用 PrintWindow(PW_RENDERFULLCONTENT) 带来的 40~100ms 卡顿与重绘闪屏！）。
        let prefer_fast_screen_blt = allow_screen_blt_fallback
            && is_hwnd_visible_and_not_minimized(hwnd_raw)
            && !is_window_cloaked(hwnd_raw)
            && !is_overlay_obscured_in_zorder(hwnd_raw, &[]);

        if prefer_fast_screen_blt {
            if let Some(px) = try_capture(true) {
                return Some((w, h, px));
            }
            if let Some(px) = try_capture(false) {
                return Some((w, h, px));
            }
        } else {
            if let Some(px) = try_capture(false) {
                return Some((w, h, px));
            }
            if allow_screen_blt_fallback {
                if let Some(px) = try_capture(true) {
                    return Some((w, h, px));
                }
            }
        }
        None
    }

    /// 纯原生 Win32 隐藏主窗口：在 DWMWA_CLOAK = 1 保护下直接 ShowWindow(SW_HIDE)，
    /// 彻底绕过 tao::set_visible(false) 对 GWL_STYLE / WS_VISIBLE 的破坏，完整保留最大化状态与非客户区完整性。
    pub fn hide_main_window_native(win: &tauri::WebviewWindow) {
        let Ok(raw) = win.hwnd() else {
            return;
        };
        let hwnd_raw = raw.0 as isize;
        if hwnd_raw == 0 {
            return;
        }
        set_window_dwm_transitions_disabled(hwnd_raw, true);
        set_window_cloak(hwnd_raw, true);
        MAIN_CLOAKED_FLAG.store(true, Ordering::Release);
        unsafe {
            let _ = ShowWindow(HWND(hwnd_raw as *mut _), SW_HIDE);
        }
        // 隐藏后再解除 Cloak 标记（因为已 SW_HIDE，屏幕上依然完全不可见），防止任务栏/系统误判永久 Cloak
        set_window_cloak(hwnd_raw, false);
        MAIN_CLOAKED_FLAG.store(false, Ordering::Release);
    }

    /// 在完全隐身保护下（DWMWA_CLOAK = 1 + DWMWA_TRANSITIONS_FORCEDISABLED = 1）原子恢复主窗口并校准
    /// 最大化客户区、无描边 DWM 属性、WRY_WEBVIEW 子窗口与 WebView2 Bounds。
    /// 核心承诺：全程 0 次 SW_RESTORE 还原抖动，屏幕 0 像素可见闪烁，绝不出现底部白线。
    pub fn restore_main_window_cloaked_native(
        win: &tauri::WebviewWindow,
        expect_maximized: bool,
        paint_nc_fn: fn(isize),
    ) -> Option<RectPx> {
        let Ok(raw) = win.hwnd() else {
            return None;
        };
        let hwnd_raw = raw.0 as isize;
        if hwnd_raw == 0 {
            return None;
        }
        let hwnd = HWND(hwnd_raw as *mut _);

        set_window_dwm_transitions_disabled(hwnd_raw, true);
        set_window_cloak(hwnd_raw, true);
        MAIN_CLOAKED_FLAG.store(true, Ordering::Release);

        unsafe {
            let border_none: u32 = DWMWA_COLOR_NONE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &border_none as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // 一步到位恢复目标显示状态：最大化直接 SW_SHOWMAXIMIZED，绝不先 SW_RESTORE 缩回默认窗口！
            if IsIconic(hwnd).as_bool() {
                // 若此前处于最小化，先通知 tao 清除内部 WindowFlags::MINIMIZED，再一步落到目标态
                let _ = SendMessageW(hwnd, WM_SYSCOMMAND, Some(WPARAM(SC_RESTORE as usize)), Some(LPARAM(0)));
            }
            if expect_maximized {
                let _ = ShowWindow(hwnd, SW_SHOWMAXIMIZED);
            } else if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }

            // 若预期最大化但因切屏/自动隐藏任务栏导致客户区偏离工作区，在隐身态下原地 SWP_FRAMECHANGED 校准
            if expect_maximized {
                if let (Some(client_rect), Some((_mon, work))) = (
                    get_window_client_screen_rect(hwnd_raw),
                    get_monitor_rects_for_hwnd(hwnd_raw),
                ) {
                    let drift = (client_rect.left - work.left).abs() > 1
                        || (client_rect.top - work.top).abs() > 1
                        || (work.right - client_rect.right).abs() > 1
                        || (work.bottom - client_rect.bottom).abs() > 1;
                    if drift {
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE
                                | SWP_NOSIZE
                                | SWP_NOZORDER
                                | SWP_NOACTIVATE
                                | SWP_FRAMECHANGED,
                        );
                    }
                }
            }
        }

        // 强制同步 WRY_WEBVIEW 子窗口与 WebView2 Controller Bounds 至最新客户区
        let _ = win.with_webview(move |webview| unsafe {
            let hwnd_in = HWND(hwnd_raw as *mut _);
            let controller = webview.controller();
            let _ = controller.SetIsVisible(true);
            let mut cr = RECT::default();
            if GetClientRect(hwnd_in, &mut cr).is_ok() && cr.right > cr.left && cr.bottom > cr.top {
                let cw = cr.right - cr.left;
                let ch = cr.bottom - cr.top;
                let mut parent_wv_hwnd = std::mem::zeroed();
                if controller.ParentWindow(&mut parent_wv_hwnd).is_ok() {
                    let wry_child_raw = parent_wv_hwnd.0 as isize;
                    if wry_child_raw != 0 && wry_child_raw != hwnd_raw {
                        let _ = SetWindowPos(
                            HWND(wry_child_raw as *mut _),
                            None,
                            0,
                            0,
                            cw,
                            ch,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                }
                let mut bounds = std::mem::zeroed();
                if controller.Bounds(&mut bounds).is_ok() {
                    bounds.left = 0;
                    bounds.top = 0;
                    bounds.right = cw;
                    bounds.bottom = ch;
                    let _ = controller.SetBounds(bounds);
                    let _ = controller.NotifyParentWindowPositionChanged();
                }
            }
            let _ = RedrawWindow(
                Some(hwnd_in),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
            if IsZoomed(hwnd_in).as_bool() {
                paint_nc_fn(hwnd_raw);
            }
        });

        unsafe {
            if IsZoomed(hwnd).as_bool() {
                paint_nc_fn(hwnd_raw);
            }
        }
        get_window_client_screen_rect(hwnd_raw)
    }

    /// 将主窗口可靠置前并赋予前台激活焦点（兼容任务栏固定图标、桌面快捷方式单实例唤醒与系统托盘唤醒，避免仅任务栏闪烁）
    unsafe fn force_foreground_main_hwnd(hwnd: HWND) {
        #[link(name = "user32")]
        extern "system" {
            fn AttachThreadInput(id_attach: u32, id_attach_to: u32, f_attach: i32) -> i32;
            fn SetActiveWindow(hwnd: isize) -> isize;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentThreadId() -> u32;
        }

        let _ = BringWindowToTop(hwnd);
        if SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd {
            return;
        }

        let fg = GetForegroundWindow();
        let fg_tid = if !fg.0.is_null() {
            GetWindowThreadProcessId(fg, None)
        } else {
            0
        };
        let cur_tid = GetCurrentThreadId();
        let attached = fg_tid != 0 && fg_tid != cur_tid && AttachThreadInput(fg_tid, cur_tid, 1) != 0;
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetActiveWindow(hwnd.0 as isize);
        if attached {
            let _ = AttachThreadInput(fg_tid, cur_tid, 0);
        }
    }

    /// 解除主窗口隐身斗篷并恢复焦点与正常 DWM 动画属性
    pub fn reveal_main_window_native(
        win: &tauri::WebviewWindow,
        focus: bool,
        paint_nc_fn: fn(isize),
    ) {
        let Ok(raw) = win.hwnd() else {
            return;
        };
        let hwnd_raw = raw.0 as isize;
        if hwnd_raw == 0 {
            return;
        }
        let hwnd = HWND(hwnd_raw as *mut _);
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
            if IsZoomed(hwnd).as_bool() {
                paint_nc_fn(hwnd_raw);
            }
        }
        set_window_cloak(hwnd_raw, false);
        MAIN_CLOAKED_FLAG.store(false, Ordering::Release);
        set_window_dwm_transitions_disabled(hwnd_raw, false);
        if focus {
            unsafe {
                force_foreground_main_hwnd(hwnd);
            }
            let _ = win.set_focus();
        }
    }

    // =========================================================================
    // DirectComposition GPU 硬件过渡舞台（独立、零 GDI 重定向、120Hz+ 硬件圆角缩放）
    // =========================================================================

    unsafe extern "system" fn stage_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_ERASEBKGND => LRESULT(1),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    #[derive(Clone, Copy)]
    struct LockedStageGeom {
        tex_w: u32,
        tex_h: u32,
        cw: f32,
        ch: f32,
        fit_sx: f32,
        fit_sy: f32,
    }

    struct DCompStage {
        hwnd: HWND,
        d3d_device: ID3D11Device,
        d3d_ctx: ID3D11DeviceContext,
        dcomp_device: IDCompositionDevice,
        target: IDCompositionTarget,
        root_visual: IDCompositionVisual,
        // 保持当前动画周期引用的 COM 对象存活直到动画结束
        active_surface: Option<IDCompositionSurface>,
        active_effect: Option<IDCompositionEffectGroup>,
        active_clip: Option<IDCompositionRectangleClip>,
        active_fit_scale: Option<IDCompositionScaleTransform>,
        active_scale: Option<IDCompositionScaleTransform>,
        active_translate: Option<IDCompositionTranslateTransform>,
        active_transform_group: Option<IDCompositionTransform>,
        locked_geom: Option<LockedStageGeom>,
        stage_rect: RectPx,
    }

    impl DCompStage {
        fn create() -> windows::core::Result<Self> {
            unsafe {
                let hinst = GetModuleHandleW(None)?;
                let cls_name = w!("BmDcompSpatialTransitionStage");
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(stage_wnd_proc),
                    hInstance: hinst.into(),
                    lpszClassName: cls_name,
                    ..Default::default()
                };
                let _ = RegisterClassExW(&wc);

                // WS_EX_NOREDIRECTIONBITMAP: 零 GDI 底板，绝不黑屏，全原生 DirectComposition 透明合成
                let ex_style = WS_EX_NOREDIRECTIONBITMAP
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE
                    | WS_EX_TRANSPARENT;
                let hwnd = CreateWindowExW(
                    ex_style,
                    cls_name,
                    PCWSTR::null(),
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    None,
                    None,
                    Some(hinst.into()),
                    None,
                )?;

                let disable_trans: u32 = 1;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_TRANSITIONS_FORCEDISABLED,
                    &disable_trans as *const u32 as *const _,
                    std::mem::size_of::<u32>() as u32,
                );
                let border_none: u32 = DWMWA_COLOR_NONE;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_BORDER_COLOR,
                    &border_none as *const u32 as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 优先硬件 D3D11，失败时自动降级 WARP 软渲染，确保 100% 设备兼容
                let feature_levels = [D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_0];
                let mut d3d_dev_opt: Option<ID3D11Device> = None;
                let mut d3d_ctx_opt: Option<ID3D11DeviceContext> = None;
                let mut got_level = D3D_FEATURE_LEVEL(0);

                let hw_res = D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    Default::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&feature_levels),
                    D3D11_SDK_VERSION,
                    Some(&mut d3d_dev_opt),
                    Some(&mut got_level),
                    Some(&mut d3d_ctx_opt),
                );
                if hw_res.is_err() || d3d_dev_opt.is_none() || d3d_ctx_opt.is_none() {
                    d3d_dev_opt = None;
                    d3d_ctx_opt = None;
                    D3D11CreateDevice(
                        None,
                        D3D_DRIVER_TYPE_WARP,
                        Default::default(),
                        D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                        Some(&feature_levels),
                        D3D11_SDK_VERSION,
                        Some(&mut d3d_dev_opt),
                        Some(&mut got_level),
                        Some(&mut d3d_ctx_opt),
                    )?;
                }

                let d3d_device = d3d_dev_opt.unwrap();
                let d3d_ctx = d3d_ctx_opt.unwrap();
                let dxgi_device: IDXGIDevice = d3d_device.cast()?;
                let dcomp_device: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
                let target = dcomp_device.CreateTargetForHwnd(hwnd, true)?;
                let root_visual = dcomp_device.CreateVisual()?;
                root_visual.SetBitmapInterpolationMode(
                    DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR,
                )?;
                root_visual.SetBorderMode(DCOMPOSITION_BORDER_MODE_SOFT)?;
                target.SetRoot(&root_visual)?;
                dcomp_device.Commit()?;

                Ok(Self {
                    hwnd,
                    d3d_device,
                    d3d_ctx,
                    dcomp_device,
                    target,
                    root_visual,
                    active_surface: None,
                    active_effect: None,
                    active_clip: None,
                    active_fit_scale: None,
                    active_scale: None,
                    active_translate: None,
                    active_transform_group: None,
                    locked_geom: None,
                    stage_rect: RectPx {
                        left: 0,
                        top: 0,
                        right: 1,
                        bottom: 1,
                    },
                })
            }
        }

        fn build_bezier_animation(
            &self,
            from: f32,
            to: f32,
            duration_ms: u64,
            x1: f64,
            y1: f64,
            x2: f64,
            y2: f64,
        ) -> windows::core::Result<IDCompositionAnimation> {
            unsafe {
                let anim = self.dcomp_device.CreateAnimation()?;
                let dur_sec = (duration_ms.max(1) as f64) / 1000.0;
                if (to - from).abs() < 1e-4 {
                    anim.AddCubic(0.0, to, 0.0, 0.0, 0.0)?;
                    anim.End(dur_sec, to)?;
                    return Ok(anim);
                }
                let segs = build_hermite_cubic_segments(from, to, dur_sec, x1, y1, x2, y2, 6);
                for seg in segs {
                    anim.AddCubic(
                        seg.begin_offset_sec,
                        seg.constant,
                        seg.linear,
                        seg.quadratic,
                        seg.cubic,
                    )?;
                }
                anim.End(dur_sec, to)?;
                Ok(anim)
            }
        }

        /// 进入小窗专属「两段式实体保持」透明度动画：
        /// - 前 40% 行程维持极高实体感（1.0 -> 0.88），让人眼清晰追踪主窗口圆角化并优雅缩小飞向灵动岛；
        /// - 后 60% 行程柔和加速消融至 0.0，与灵动岛弹簧浮现无缝接棒，彻底告别“第 1 帧就发虚像闪屏”。
        fn build_enter_opacity_animation(
            &self,
            duration_ms: u64,
        ) -> windows::core::Result<IDCompositionAnimation> {
            unsafe {
                let anim = self.dcomp_device.CreateAnimation()?;
                let dur_sec = (duration_ms.max(16) as f64) / 1000.0;
                let split_sec = dur_sec * 0.40;
                let tail_sec = (dur_sec - split_sec).max(0.005);

                // 阶段 1 [0, split_sec]：1.0 -> 0.88 缓柔微降，保持强烈视觉锚定
                let segs_head = build_hermite_cubic_segments(
                    1.0,
                    0.88,
                    split_sec,
                    0.35,
                    0.0,
                    0.65,
                    1.0,
                    2,
                );
                for seg in segs_head {
                    anim.AddCubic(
                        seg.begin_offset_sec,
                        seg.constant,
                        seg.linear,
                        seg.quadratic,
                        seg.cubic,
                    )?;
                }

                // 阶段 2 [split_sec, dur_sec]：0.88 -> 0.0 丝滑加速融化消隐
                let segs_tail = build_hermite_cubic_segments(
                    0.88,
                    0.0,
                    tail_sec,
                    0.32,
                    0.0,
                    0.58,
                    1.0,
                    4,
                );
                for seg in segs_tail {
                    anim.AddCubic(
                        split_sec + seg.begin_offset_sec,
                        seg.constant,
                        seg.linear,
                        seg.quadratic,
                        seg.cubic,
                    )?;
                }
                anim.End(dur_sec, 0.0)?;
                Ok(anim)
            }
        }

        fn upload_bgra_surface(
            &self,
            width: u32,
            height: u32,
            bgra: &[u8],
        ) -> windows::core::Result<IDCompositionSurface> {
            unsafe {
                let surface = self.dcomp_device.CreateSurface(
                    width,
                    height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_ALPHA_MODE_PREMULTIPLIED,
                )?;
                let mut offset = POINT::default();
                let dest_tex: ID3D11Texture2D = surface.BeginDraw(None, &mut offset)?;
                let dst_x = offset.x.max(0) as u32;
                let dst_y = offset.y.max(0) as u32;

                // 优先采用 MSDN 官方推荐路径：构建 IMMUTABLE 源纹理 + CopySubresourceRegion 精准拷贝到 BeginDraw 返回的
                // DComp Atlas 偏移处并 Flush，100% 兼容所有硬件显卡驱动与 WARP 软渲染。
                let src_desc = D3D11_TEXTURE2D_DESC {
                    Width: width,
                    Height: height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_IMMUTABLE,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    CPUAccessFlags: 0,
                    MiscFlags: 0,
                };
                let init_data = D3D11_SUBRESOURCE_DATA {
                    pSysMem: bgra.as_ptr() as *const _,
                    SysMemPitch: width * 4,
                    SysMemSlicePitch: 0,
                };
                let mut src_tex_opt: Option<ID3D11Texture2D> = None;
                if self
                    .d3d_device
                    .CreateTexture2D(&src_desc, Some(&init_data), Some(&mut src_tex_opt))
                    .is_ok()
                {
                    if let Some(src_tex) = src_tex_opt {
                        let dest_res: ID3D11Resource = dest_tex.cast()?;
                        let src_res: ID3D11Resource = src_tex.cast()?;
                        let src_box = D3D11_BOX {
                            left: 0,
                            top: 0,
                            front: 0,
                            right: width,
                            bottom: height,
                            back: 1,
                        };
                        self.d3d_ctx.CopySubresourceRegion(
                            &dest_res,
                            0,
                            dst_x,
                            dst_y,
                            0,
                            &src_res,
                            0,
                            Some(&src_box),
                        );
                    }
                } else {
                    let dst_box = D3D11_BOX {
                        left: dst_x,
                        top: dst_y,
                        front: 0,
                        right: dst_x + width,
                        bottom: dst_y + height,
                        back: 1,
                    };
                    self.d3d_ctx.UpdateSubresource(
                        &dest_tex,
                        0,
                        Some(&dst_box),
                        bgra.as_ptr() as *const _,
                        width * 4,
                        0,
                    );
                }
                self.d3d_ctx.Flush();
                surface.EndDraw()?;
                Ok(surface)
            }
        }

        /// 阶段 1（静态锁帧接管）：先上传纹理并以 100% 不透明、1.0x 原位原尺寸提交到 DirectComposition，
        /// 遵循「先 Commit 有效首帧、再 SetWindowPos(SWP_SHOWWINDOW) + DwmFlush」铁律，
        /// 确保 WS_EX_NOREDIRECTIONBITMAP 舞台 0 黑闪/0 空帧完美盖住下方主窗口。
        fn lock_static_frame(
            &mut self,
            stage_rect: RectPx,
            content_rect: RectPx,
            tex_w: u32,
            tex_h: u32,
            bgra: &[u8],
            rest_radius_px: f32,
        ) -> windows::core::Result<()> {
            unsafe {
                let surface = self.upload_bgra_surface(tex_w, tex_h, bgra)?;

                let base_x = (content_rect.left - stage_rect.left) as f32;
                let base_y = (content_rect.top - stage_rect.top) as f32;
                let cw = content_rect.width().max(1) as f32;
                let ch = content_rect.height().max(1) as f32;
                let fit_sx = cw / tex_w.max(1) as f32;
                let fit_sy = ch / tex_h.max(1) as f32;

                self.root_visual.SetOffsetX2(base_x)?;
                self.root_visual.SetOffsetY2(base_y)?;
                self.root_visual.SetContent(&surface)?;

                let clip = self.dcomp_device.CreateRectangleClip()?;
                clip.SetLeft2(0.0)?;
                clip.SetTop2(0.0)?;
                clip.SetRight2(tex_w as f32)?;
                clip.SetBottom2(tex_h as f32)?;
                let rx = (rest_radius_px / fit_sx.max(0.01)).max(0.0);
                let ry = (rest_radius_px / fit_sy.max(0.01)).max(0.0);
                clip.SetTopLeftRadiusX2(rx)?;
                clip.SetTopLeftRadiusY2(ry)?;
                clip.SetTopRightRadiusX2(rx)?;
                clip.SetTopRightRadiusY2(ry)?;
                clip.SetBottomLeftRadiusX2(rx)?;
                clip.SetBottomLeftRadiusY2(ry)?;
                clip.SetBottomRightRadiusX2(rx)?;
                clip.SetBottomRightRadiusY2(ry)?;
                let clip_base: IDCompositionClip = clip.cast()?;
                self.root_visual.SetClip(&clip_base)?;

                // 固定原点缩放把纹理铺满 [0..cw, 0..ch]，彻底杜绝降采样纹理时的 25% 左上偏移！
                let fit_scale_tf = self.dcomp_device.CreateScaleTransform()?;
                fit_scale_tf.SetCenterX2(0.0)?;
                fit_scale_tf.SetCenterY2(0.0)?;
                fit_scale_tf.SetScaleX2(fit_sx)?;
                fit_scale_tf.SetScaleY2(fit_sy)?;
                let tf_fit_base: IDCompositionTransform = fit_scale_tf.cast()?;
                self.root_visual.SetTransform(&tf_fit_base)?;

                let effect_group = self.dcomp_device.CreateEffectGroup()?;
                effect_group.SetOpacity2(1.0)?;
                let effect_base: IDCompositionEffect = effect_group.cast()?;
                self.root_visual.SetEffect(&effect_base)?;

                // 先 Commit 静态满屏不透明首帧，再显示舞台窗，彻底根治 WS_EX_NOREDIRECTIONBITMAP 首帧黑闪
                self.dcomp_device.Commit()?;

                if self.stage_rect != stage_rect || !IsWindowVisible(self.hwnd).as_bool() {
                    let _ = SetWindowPos(
                        self.hwnd,
                        Some(HWND_TOPMOST),
                        stage_rect.left,
                        stage_rect.top,
                        stage_rect.width().max(1),
                        stage_rect.height().max(1),
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                    self.stage_rect = stage_rect;
                } else {
                    let _ = SetWindowPos(
                        self.hwnd,
                        Some(HWND_TOPMOST),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
                // 窗口从隐藏/1x1 展开至显示器全尺寸后补发一次轻量 Commit，确保 DWM 在同一 VSync 帧采样完整视觉树
                let _ = self.dcomp_device.Commit();
                let _ = DwmFlush();

                self.active_surface = Some(surface);
                self.active_effect = Some(effect_group);
                self.active_clip = Some(clip);
                self.active_fit_scale = Some(fit_scale_tf);
                self.active_scale = None;
                self.active_translate = None;
                self.active_transform_group = None;
                self.locked_geom = Some(LockedStageGeom {
                    tex_w,
                    tex_h,
                    cw,
                    ch,
                    fit_sx,
                    fit_sy,
                });
                Ok(())
            }
        }

        /// 阶段 3（在真实主窗口已撤离屏幕后干净起飞）：复用已静态锁帧的 GPU 纹理与坐标，
        /// 挂载 120Hz+ 动态圆角、等比缩放、方向牵引与两段式实体保持透明度动画并立即 Commit。
        fn launch_locked_animation(
            &mut self,
            plan: SpatialTransitionPlan,
        ) -> windows::core::Result<()> {
            unsafe {
                let Some(geom) = self.locked_geom else {
                    return Ok(());
                };
                if self.active_surface.is_none() {
                    return Ok(());
                }

                let dur = plan.duration_ms.max(16);
                let (mx1, my1, mx2, my2) = match plan.kind {
                    TransitionKind::EnterMini => (0.24, 0.12, 0.20, 1.0),
                    TransitionKind::ExitMini | TransitionKind::RestoreFromTray => {
                        (0.16, 0.96, 0.26, 1.0)
                    }
                };

                let clip = self.dcomp_device.CreateRectangleClip()?;
                clip.SetLeft2(0.0)?;
                clip.SetTop2(0.0)?;
                clip.SetRight2(geom.tex_w as f32)?;
                clip.SetBottom2(geom.tex_h as f32)?;
                let radius_anim_x = self.build_bezier_animation(
                    plan.start_radius_px / geom.fit_sx.max(0.01),
                    plan.end_radius_px / geom.fit_sx.max(0.01),
                    dur,
                    mx1,
                    my1,
                    mx2,
                    my2,
                )?;
                let radius_anim_y = self.build_bezier_animation(
                    plan.start_radius_px / geom.fit_sy.max(0.01),
                    plan.end_radius_px / geom.fit_sy.max(0.01),
                    dur,
                    mx1,
                    my1,
                    mx2,
                    my2,
                )?;
                clip.SetTopLeftRadiusX(&radius_anim_x)?;
                clip.SetTopLeftRadiusY(&radius_anim_y)?;
                clip.SetTopRightRadiusX(&radius_anim_x)?;
                clip.SetTopRightRadiusY(&radius_anim_y)?;
                clip.SetBottomLeftRadiusX(&radius_anim_x)?;
                clip.SetBottomLeftRadiusY(&radius_anim_y)?;
                clip.SetBottomRightRadiusX(&radius_anim_x)?;
                clip.SetBottomRightRadiusY(&radius_anim_y)?;
                let clip_base: IDCompositionClip = clip.cast()?;
                self.root_visual.SetClip(&clip_base)?;

                // 三级独立变换：fit_scale (0,0) -> anim_scale (cw/2, ch/2) -> translate (dx, dy)
                let fit_scale_tf = self.dcomp_device.CreateScaleTransform()?;
                fit_scale_tf.SetCenterX2(0.0)?;
                fit_scale_tf.SetCenterY2(0.0)?;
                fit_scale_tf.SetScaleX2(geom.fit_sx)?;
                fit_scale_tf.SetScaleY2(geom.fit_sy)?;

                let scale_tf = self.dcomp_device.CreateScaleTransform()?;
                scale_tf.SetCenterX2(geom.cw * 0.5)?;
                scale_tf.SetCenterY2(geom.ch * 0.5)?;
                let sx_anim = self.build_bezier_animation(
                    plan.start_scale,
                    plan.end_scale,
                    dur,
                    mx1,
                    my1,
                    mx2,
                    my2,
                )?;
                let sy_anim = self.build_bezier_animation(
                    plan.start_scale,
                    plan.end_scale,
                    dur,
                    mx1,
                    my1,
                    mx2,
                    my2,
                )?;
                scale_tf.SetScaleX(&sx_anim)?;
                scale_tf.SetScaleY(&sy_anim)?;

                let trans_tf = self.dcomp_device.CreateTranslateTransform()?;
                let tx_anim = self.build_bezier_animation(
                    plan.start_dx,
                    plan.end_dx,
                    dur,
                    mx1,
                    my1,
                    mx2,
                    my2,
                )?;
                let ty_anim = self.build_bezier_animation(
                    plan.start_dy,
                    plan.end_dy,
                    dur,
                    mx1,
                    my1,
                    mx2,
                    my2,
                )?;
                trans_tf.SetOffsetX(&tx_anim)?;
                trans_tf.SetOffsetY(&ty_anim)?;

                let tf_fit_base: IDCompositionTransform = fit_scale_tf.cast()?;
                let tf_scale_base: IDCompositionTransform = scale_tf.cast()?;
                let tf_trans_base: IDCompositionTransform = trans_tf.cast()?;
                let transforms = [Some(tf_fit_base), Some(tf_scale_base), Some(tf_trans_base)];
                let tf_group = self.dcomp_device.CreateTransformGroup(&transforms)?;
                self.root_visual.SetTransform(&tf_group)?;

                let effect_group = self.dcomp_device.CreateEffectGroup()?;
                let opacity_anim = if plan.kind == TransitionKind::EnterMini {
                    self.build_enter_opacity_animation(dur)?
                } else {
                    self.build_bezier_animation(
                        plan.start_opacity,
                        plan.end_opacity,
                        dur,
                        0.12,
                        0.88,
                        0.28,
                        1.0,
                    )?
                };
                effect_group.SetOpacity(&opacity_anim)?;
                let effect_base: IDCompositionEffect = effect_group.cast()?;
                self.root_visual.SetEffect(&effect_base)?;

                self.dcomp_device.Commit()?;

                self.active_effect = Some(effect_group);
                self.active_clip = Some(clip);
                self.active_fit_scale = Some(fit_scale_tf);
                self.active_scale = Some(scale_tf);
                self.active_translate = Some(trans_tf);
                self.active_transform_group = Some(tf_group);
                Ok(())
            }
        }

        /// 单步呈现完整 GPU 动画（用于 ExitMini / RestoreFromTray：先 Commit 首帧再显示舞台，杜绝首帧黑闪）
        fn start_spatial_animation(
            &mut self,
            stage_rect: RectPx,
            content_rect: RectPx,
            tex_w: u32,
            tex_h: u32,
            bgra: &[u8],
            plan: SpatialTransitionPlan,
        ) -> windows::core::Result<()> {
            unsafe {
                let surface = self.upload_bgra_surface(tex_w, tex_h, bgra)?;

                let base_x = (content_rect.left - stage_rect.left) as f32;
                let base_y = (content_rect.top - stage_rect.top) as f32;
                let cw = content_rect.width().max(1) as f32;
                let ch = content_rect.height().max(1) as f32;
                let fit_sx = cw / tex_w.max(1) as f32;
                let fit_sy = ch / tex_h.max(1) as f32;

                self.root_visual.SetOffsetX2(base_x)?;
                self.root_visual.SetOffsetY2(base_y)?;
                self.root_visual.SetContent(&surface)?;

                self.active_surface = Some(surface);
                self.locked_geom = Some(LockedStageGeom {
                    tex_w,
                    tex_h,
                    cw,
                    ch,
                    fit_sx,
                    fit_sy,
                });

                self.launch_locked_animation(plan)?;

                if self.stage_rect != stage_rect || !IsWindowVisible(self.hwnd).as_bool() {
                    let _ = SetWindowPos(
                        self.hwnd,
                        Some(HWND_TOPMOST),
                        stage_rect.left,
                        stage_rect.top,
                        stage_rect.width().max(1),
                        stage_rect.height().max(1),
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                    self.stage_rect = stage_rect;
                }
                let _ = DwmFlush();
                let _ = &self.target;
                Ok(())
            }
        }

        /// 退出小窗抵达 100% 位置重合后，执行 55~65ms 丝滑无痕揭幕，将舞台淡出至 0 露出下方真实活跃主窗口
        fn start_cross_dissolve_reveal(&mut self, dissolve_ms: u64) -> windows::core::Result<()> {
            unsafe {
                let Some(effect_group) = self.active_effect.as_ref() else {
                    return Ok(());
                };
                let anim = self.build_bezier_animation(
                    1.0,
                    0.0,
                    dissolve_ms.max(16),
                    0.22,
                    0.0,
                    0.36,
                    1.0,
                )?;
                effect_group.SetOpacity(&anim)?;
                self.dcomp_device.Commit()?;
                Ok(())
            }
        }

        fn hide_and_release_surface(&mut self) {
            unsafe {
                let _ = self.root_visual.SetContent(None);
                let _ = self.root_visual.SetTransform(None);
                let _ = self.root_visual.SetEffect(None);
                let _ = self.root_visual.SetClip(None);
                let _ = self.dcomp_device.Commit();
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND_NOTOPMOST),
                    0,
                    0,
                    1,
                    1,
                    SWP_NOACTIVATE | SWP_HIDEWINDOW,
                );
            }
            self.stage_rect = RectPx {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            };
            self.active_surface = None;
            self.active_effect = None;
            self.active_clip = None;
            self.active_fit_scale = None;
            self.active_scale = None;
            self.active_translate = None;
            self.active_transform_group = None;
            self.locked_geom = None;
        }
    }

    // 过渡舞台绑定在 UI 主线程（所有 Win32 HWND 与 DirectComposition 均在 Tauri 主线程安全访问）；
    // 使用 ManuallyDrop 彻底避免进程/线程退出时在 Windows DLL_THREAD_DETACH Loader Lock 中 Drop ID3D11Device 引发死锁
    thread_local! {
        static STAGE_CELL: RefCell<std::mem::ManuallyDrop<Option<DCompStage>>> =
            const { RefCell::new(std::mem::ManuallyDrop::new(None)) };
    }

    fn with_stage<R>(f: impl FnOnce(&mut DCompStage) -> R) -> Option<R> {
        STAGE_CELL.with(|cell| {
            let mut opt = cell.borrow_mut();
            if opt.is_none() {
                match DCompStage::create() {
                    Ok(stage) => **opt = Some(stage),
                    Err(_) => return None,
                }
            }
            opt.as_mut().map(f)
        })
    }

    /// 启动空闲期静默预热 DirectComposition GPU 过渡舞台（仅建设备与 1x1 隐藏空窗，0 显存纹理占用），
    /// 彻底消除首次点击进入小窗时的 D3D11/DComp 初始化卡顿与闪屏。
    pub fn warm_gpu_stage_on_main_thread() {
        let _ = with_stage(|_| {});
    }

    /// 阶段 1：以 100% 不透明静态真画面精确覆盖主窗口区域并同步至 DWM（零重影、零黑闪）
    pub fn lock_static_gpu_stage_on_main_thread(
        stage_rect: RectPx,
        content_rect: RectPx,
        tex_w: u32,
        tex_h: u32,
        bgra: &[u8],
        rest_radius_px: f32,
    ) -> bool {
        with_stage(|stage| {
            stage
                .lock_static_frame(stage_rect, content_rect, tex_w, tex_h, bgra, rest_radius_px)
                .is_ok()
        })
        .unwrap_or(false)
    }

    /// 阶段 3：真实主窗口撤离屏幕后，直接基于已锁帧的静态 GPU 表面起飞播放 120Hz+ 收束动效
    pub fn launch_locked_gpu_transition_on_main_thread(plan: SpatialTransitionPlan) -> bool {
        with_stage(|stage| stage.launch_locked_animation(plan).is_ok()).unwrap_or(false)
    }

    pub fn play_gpu_transition_on_main_thread(
        stage_rect: RectPx,
        content_rect: RectPx,
        tex_w: u32,
        tex_h: u32,
        bgra: &[u8],
        plan: SpatialTransitionPlan,
    ) -> bool {
        with_stage(|stage| {
            stage
                .start_spatial_animation(stage_rect, content_rect, tex_w, tex_h, bgra, plan)
                .is_ok()
        })
        .unwrap_or(false)
    }

    pub fn start_gpu_cross_dissolve_on_main_thread(dissolve_ms: u64) {
        let _ = with_stage(|stage| {
            let _ = stage.start_cross_dissolve_reveal(dissolve_ms);
        });
    }

    pub fn cancel_and_hide_gpu_stage_on_main_thread() {
        STAGE_CELL.with(|cell| {
            if let Some(stage) = cell.borrow_mut().as_mut() {
                stage.hide_and_release_surface();
            }
        });
    }

    pub fn shutdown_gpu_stage_on_main_thread() {
        STAGE_CELL.with(|cell| {
            if let Some(mut stage) = cell.borrow_mut().take() {
                stage.hide_and_release_surface();
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(stage.hwnd);
                }
            }
        });
    }

    pub fn colorref_to_rgb(colorref: u32) -> (u8, u8, u8) {
        let r = (colorref & 0xFF) as u8;
        let g = ((colorref >> 8) & 0xFF) as u8;
        let b = ((colorref >> 16) & 0xFF) as u8;
        (r, g, b)
    }

    pub fn rgb_to_colorref(r: u8, g: u8, b: u8) -> u32 {
        COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16)).0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hermite_cubic_segments_match_apple_bezier_accurately() {
        let dur = 0.245;
        let (x1, y1, x2, y2) = (0.16, 0.96, 0.26, 1.0);
        let segs = build_hermite_cubic_segments(0.895, 1.0, dur, x1, y1, x2, y2, 6);
        assert_eq!(segs.len(), 6);
        assert!((segs[0].begin_offset_sec - 0.0).abs() < 1e-6);
        assert!((segs[0].constant - 0.895).abs() < 1e-4);

        // 逐段验证中点与终点拟合误差 < 0.002 且单调递增
        let mut prev_val = 0.895f64;
        for (i, seg) in segs.iter().enumerate() {
            let next_begin = if i + 1 < segs.len() {
                segs[i + 1].begin_offset_sec
            } else {
                dur
            };
            let h = next_begin - seg.begin_offset_sec;
            for frac in [0.25, 0.5, 0.75, 1.0] {
                let s = h * frac;
                let approx = seg.constant as f64
                    + seg.linear as f64 * s
                    + seg.quadratic as f64 * s * s
                    + seg.cubic as f64 * s * s * s;
                let u = (seg.begin_offset_sec + s) / dur;
                let (exact_y, _) = sample_cubic_bezier(u, x1, y1, x2, y2);
                let exact = 0.895 + (1.0 - 0.895) * exact_y;
                assert!(
                    (approx - exact).abs() < 0.002,
                    "segment {} frac {} mismatch: approx={} exact={}",
                    i,
                    frac,
                    approx,
                    exact
                );
                assert!(approx + 1e-4 >= prev_val);
                prev_val = approx;
            }
        }
        assert!((prev_val - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_spatial_transition_plan_maximized_and_normal_and_cross_monitor() {
        let max_rect = RectPx {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let top_island = RectPx {
            left: 794,
            top: 0,
            right: 1126,
            bottom: 34,
        };

        // 1. 最大化进入顶部灵动岛：起始圆角 0 -> 36px，明显向上牵引与有力收束缩放，不拉扁等比缩放
        let enter_max = compute_spatial_transition_plan(
            TransitionKind::EnterMini,
            max_rect,
            Some(top_island),
            true,
            true,
            1.0,
            TransitionMotionStyle::Rich,
        );
        assert!(enter_max.duration_ms >= 280);
        assert_eq!(enter_max.start_radius_px, 0.0);
        assert!(enter_max.end_radius_px >= 32.0);
        assert!(enter_max.end_scale >= 0.45 && enter_max.end_scale <= 0.62);
        assert!(enter_max.end_dy < -120.0, "应朝顶部灵动岛方向明显牵引飞掠");
        assert!(enter_max.mini_handoff_delay_ms >= 100);

        // 2. 最大化退出小窗：起始圆角 22px -> 0px（贴齐满屏），带交叉溶解揭幕
        let exit_max = compute_spatial_transition_plan(
            TransitionKind::ExitMini,
            max_rect,
            Some(top_island),
            true,
            true,
            1.0,
            TransitionMotionStyle::Rich,
        );
        assert!(exit_max.start_radius_px >= 20.0);
        assert_eq!(exit_max.end_radius_px, 0.0);
        assert_eq!(exit_max.end_scale, 1.0);
        assert_eq!(exit_max.end_dx, 0.0);
        assert_eq!(exit_max.end_dy, 0.0);
        assert!(exit_max.reveal_dissolve_ms >= 50);

        // 3. 普通默认窗口：保留 8px 系统圆角
        let normal_rect = RectPx {
            left: 340,
            top: 120,
            right: 1580,
            bottom: 920,
        };
        let enter_normal = compute_spatial_transition_plan(
            TransitionKind::EnterMini,
            normal_rect,
            Some(top_island),
            false,
            true,
            1.25,
            TransitionMotionStyle::Rich,
        );
        assert!((enter_normal.start_radius_px - 10.0).abs() < 0.01);

        // 4. 跨屏时牵引位移归零（原地居中优雅收束，绝不跨显示器边界撕裂）
        let enter_cross = compute_spatial_transition_plan(
            TransitionKind::EnterMini,
            normal_rect,
            Some(top_island),
            false,
            false,
            1.0,
            TransitionMotionStyle::Rich,
        );
        assert_eq!(enter_cross.end_dx, 0.0);
        assert_eq!(enter_cross.end_dy, 0.0);

        // 5. 无动效档位：0ms 零延迟精准切态
        let none_plan = compute_spatial_transition_plan(
            TransitionKind::ExitMini,
            max_rect,
            Some(top_island),
            true,
            true,
            1.0,
            TransitionMotionStyle::None,
        );
        assert_eq!(none_plan.duration_ms, 0);
        assert_eq!(none_plan.reveal_dissolve_ms, 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_dcomp_gpu_transition_stage_initializes_and_commits_cleanly() {
        let stage_rect = RectPx {
            left: 0,
            top: 0,
            right: 640,
            bottom: 360,
        };
        let content_rect = RectPx {
            left: 40,
            top: 30,
            right: 360,
            bottom: 230,
        };
        let bgra = synthesize_theme_aura_card_bgra(
            320,
            200,
            (12, 14, 20),
            (94, 106, 210),
            (45, 212, 191),
        );
        let plan = compute_spatial_transition_plan(
            TransitionKind::EnterMini,
            content_rect,
            None,
            false,
            false,
            1.0,
            TransitionMotionStyle::Rich,
        );
        // 1. 静态锁帧接管 -> 复用表面起飞播放收束动画（完整验证三段式零闪屏管线 + 1/2 降采样三级变换对齐）
        win_impl::warm_gpu_stage_on_main_thread();
        let locked = win_impl::lock_static_gpu_stage_on_main_thread(
            stage_rect,
            content_rect,
            160,
            100,
            &synthesize_theme_aura_card_bgra(160, 100, (12, 14, 20), (94, 106, 210), (45, 212, 191)),
            plan.start_radius_px,
        );
        assert!(locked, "静态锁帧 GPU 舞台必须成功提交首帧");
        let launched = win_impl::launch_locked_gpu_transition_on_main_thread(plan);
        assert!(launched, "静态锁帧后必须能够复用 GPU 纹理无闪烁起飞播放动画");
        win_impl::cancel_and_hide_gpu_stage_on_main_thread();

        // 2. 单步展开路径（ExitMini）
        let ok = win_impl::play_gpu_transition_on_main_thread(
            stage_rect,
            content_rect,
            320,
            200,
            &bgra,
            plan,
        );
        assert!(ok, "DirectComposition GPU transition stage must initialize and commit on Windows");
        win_impl::start_gpu_cross_dissolve_on_main_thread(45);
        win_impl::cancel_and_hide_gpu_stage_on_main_thread();
        win_impl::shutdown_gpu_stage_on_main_thread();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_native_hide_and_cloaked_maximized_restore_never_shrinks_to_normal() {
        use windows::core::w;
        use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
        use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, IsZoomed, RegisterClassExW, ShowWindow,
            NCCALCSIZE_PARAMS, SW_HIDE, SW_SHOW, SW_SHOWMAXIMIZED, WM_NCCALCSIZE, WM_SIZE,
            WNDCLASSEXW, WS_CAPTION, WS_CLIPSIBLINGS, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SIZEBOX,
            WS_SYSMENU,
        };

        static SAW_NON_MAX_RESTORE_EVENT: AtomicBool = AtomicBool::new(false);
        static TRACK_RESTORE_BOUNCE: AtomicBool = AtomicBool::new(false);

        unsafe extern "system" fn test_wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            if msg == WM_NCCALCSIZE && IsZoomed(hwnd).as_bool() {
                let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut mi = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if !hmon.0.is_null() && GetMonitorInfoW(hmon, &mut mi).as_bool() {
                    if wparam.0 == 0 {
                        *(lparam.0 as *mut RECT) = mi.rcWork;
                    } else {
                        (*(lparam.0 as *mut NCCALCSIZE_PARAMS)).rgrc[0] = mi.rcWork;
                    }
                    return LRESULT(0);
                }
            } else if msg == WM_SIZE && TRACK_RESTORE_BOUNCE.load(Ordering::Relaxed) {
                // SIZE_RESTORED == 0，SIZE_MAXIMIZED == 2
                if wparam.0 == 0 {
                    SAW_NON_MAX_RESTORE_EVENT.store(true, Ordering::Relaxed);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        unsafe {
            let hinst = GetModuleHandleW(None).unwrap();
            let cls = w!("BmNativeMaxRestoreZeroBounceTest");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(test_wnd_proc),
                hInstance: hinst.into(),
                lpszClassName: cls,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);
            let style = WS_CAPTION | WS_CLIPSIBLINGS | WS_SYSMENU | WS_SIZEBOX | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
            let hwnd = CreateWindowExW(
                Default::default(),
                cls,
                cls,
                style,
                120,
                120,
                900,
                600,
                None,
                None,
                Some(hinst.into()),
                None,
            )
            .unwrap();

            // 1. 首次显示并最大化
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = ShowWindow(hwnd, SW_SHOWMAXIMIZED);
            assert!(IsZoomed(hwnd).as_bool());

            // 2. 开启跳变监控，执行原生 Cloak 隐藏 -> 原生 Cloak 一步恢复最大化
            SAW_NON_MAX_RESTORE_EVENT.store(false, Ordering::Relaxed);
            TRACK_RESTORE_BOUNCE.store(true, Ordering::Relaxed);

            win_impl::set_window_dwm_transitions_disabled(hwnd.0 as isize, true);
            win_impl::set_window_cloak(hwnd.0 as isize, true);
            let _ = ShowWindow(hwnd, SW_HIDE);
            win_impl::set_window_cloak(hwnd.0 as isize, false);

            // 恢复：一步 SW_SHOWMAXIMIZED，绝不经过 SW_RESTORE
            win_impl::set_window_cloak(hwnd.0 as isize, true);
            let _ = ShowWindow(hwnd, SW_SHOWMAXIMIZED);
            win_impl::set_window_cloak(hwnd.0 as isize, false);
            win_impl::set_window_dwm_transitions_disabled(hwnd.0 as isize, false);

            TRACK_RESTORE_BOUNCE.store(false, Ordering::Relaxed);
            assert!(IsZoomed(hwnd).as_bool(), "窗口恢复后必须直接处于最大化态");
            assert!(
                !SAW_NON_MAX_RESTORE_EVENT.load(Ordering::Relaxed),
                "最大化恢复期间绝不允许经过 SIZE_RESTORED(默认尺寸) 弹跳"
            );

            let _ = DestroyWindow(hwnd);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_overlay_native_show_click_through_and_hide_leaves_zero_ghost_window() {
        use windows::core::w;
        use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongW, IsWindowVisible,
            RegisterClassExW, GWL_EXSTYLE, WNDCLASSEXW, WS_EX_APPWINDOW, WS_EX_LAYERED,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
        };

        unsafe extern "system" fn overlay_test_wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        unsafe {
            let hinst = GetModuleHandleW(None).unwrap();
            let cls = w!("BmMiniOverlayGhostRegressionTest");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(overlay_test_wnd_proc),
                hInstance: hinst.into(),
                lpszClassName: cls,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                cls,
                cls,
                WS_POPUP,
                160,
                160,
                480,
                228,
                None,
                None,
                Some(hinst.into()),
                None,
            )
            .unwrap();
            let raw = hwnd.0 as isize;
            assert!(!IsWindowVisible(hwnd).as_bool());

            // 1. 显示悬浮小窗（交互态）：可见 + 置顶 + TOOLWINDOW + 无点击穿透
            win_impl::show_overlay_window_native(raw, false);
            assert!(IsWindowVisible(hwnd).as_bool(), "Mini 小窗显示后必须物理可见");
            let ex_vis = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            assert_ne!(ex_vis & WS_EX_TOOLWINDOW.0, 0);
            assert_eq!(ex_vis & WS_EX_APPWINDOW.0, 0);
            assert_ne!(ex_vis & WS_EX_TOPMOST.0, 0);
            assert_eq!(ex_vis & WS_EX_TRANSPARENT.0, 0);

            // 2. 动态切换点击穿透（模拟贴边收起态热区检测）：严禁误隐藏窗口，且保持 TOOLWINDOW
            win_impl::set_overlay_click_through_raw(raw, true);
            assert!(IsWindowVisible(hwnd).as_bool(), "切换点击穿透绝不能意外隐藏窗口");
            let ex_ct = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            assert_ne!(ex_ct & WS_EX_TRANSPARENT.0, 0);
            assert_ne!(ex_ct & WS_EX_LAYERED.0, 0);
            assert_ne!(ex_ct & WS_EX_TOOLWINDOW.0, 0);
            assert_eq!(ex_ct & WS_EX_APPWINDOW.0, 0);

            win_impl::set_overlay_click_through_raw(raw, false);
            assert!(IsWindowVisible(hwnd).as_bool());
            let ex_interactive = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            assert_eq!(ex_interactive & WS_EX_TRANSPARENT.0, 0);

            // 3. 原生强力隐藏（无论 tao 内部 WindowFlags 是否不同步）：
            //    必须保证物理不可见、剥离 HWND_TOPMOST、且开启点击穿透，彻底杜绝透明遮挡幽灵窗！
            win_impl::hide_overlay_window_native(raw);
            assert!(
                !IsWindowVisible(hwnd).as_bool(),
                "退出小窗后 HWND 必须彻底隐藏，绝不允许残留不可见遮挡窗口"
            );
            let ex_hidden = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            assert_eq!(
                ex_hidden & WS_EX_TOPMOST.0,
                0,
                "隐藏后的小窗必须剥离 WS_EX_TOPMOST"
            );
            assert_ne!(
                ex_hidden & (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0),
                0,
                "隐藏后的小窗必须处于点击穿透兜底态"
            );
            assert!(
                !win_impl::is_window_cloaked(raw),
                "隐藏后的小窗不得残留 DWM Cloak 状态"
            );

            // 4. 真全屏礼让（DWM Cloak 隐身 + 点击穿透 + 剥离 TOPMOST）及退出全屏瞬间恢复
            win_impl::show_overlay_window_native(raw, false);
            assert!(IsWindowVisible(hwnd).as_bool());
            assert!(!win_impl::is_window_cloaked(raw));

            win_impl::yield_overlay_window_for_fullscreen_native(raw);
            assert!(
                IsWindowVisible(hwnd).as_bool(),
                "全屏礼让不得调用 SW_HIDE 污染 tao WindowFlags::VISIBLE 缓存"
            );
            assert!(
                win_impl::is_window_cloaked(raw),
                "全屏礼让必须通过 DWMWA_CLOAK 立即隐身"
            );
            let ex_yielded = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            assert_eq!(ex_yielded & WS_EX_TOPMOST.0, 0, "全屏礼让必须剥离 WS_EX_TOPMOST");
            assert_ne!(
                ex_yielded & WS_EX_TRANSPARENT.0,
                0,
                "全屏礼让必须开启点击穿透防止拦截鼠标"
            );

            win_impl::show_overlay_window_native(raw, false);
            assert!(
                !win_impl::is_window_cloaked(raw),
                "退出全屏恢复后必须立即解除 DWMWA_CLOAK"
            );
            let ex_restored = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            assert_ne!(ex_restored & WS_EX_TOPMOST.0, 0, "退出全屏后必须恢复 WS_EX_TOPMOST");
            assert_eq!(ex_restored & WS_EX_TRANSPARENT.0, 0, "退出全屏交互态必须恢复点击响应");

            win_impl::hide_overlay_window_native(raw);
            assert!(!IsWindowVisible(hwnd).as_bool());
            assert!(!win_impl::is_window_cloaked(raw));

            let _ = DestroyWindow(hwnd);
        }
    }
}
