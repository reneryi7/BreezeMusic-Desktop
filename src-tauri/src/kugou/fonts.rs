//! Windows 系统字体扫描、纯 Rust OpenType/TTF/TTC 元数据解析与自定义字体文件持久化管理
//! =======================================================================================
//! 特性：
//! 1. 零外部依赖：通过 Win32 Registry (`HKLM`/`HKCU` Fonts) + 系统/用户字体目录全量枚举电脑已安装字体；
//! 2. 极速流式表解析：仅读取字体文件头部 Table Directory + `name` / `OS/2` / `fvar` 表，精确识别中英文家族名、
//!    真实字重集合（100~900）、可变字体 `wght` 轴范围以及 CJK 中文字符集覆盖度；
//! 3. 手动导入持久化：支持 `.ttf` / `.otf` / `.woff2` / `.woff` / `.ttc` / `.otc` 导入至 `<数据根目录>/fonts/`；
//!    对 Chromium `FontFace` 不原生支持的 `.ttc`/`.otc` 自动拆包提取主字面为标准 `.ttf` 并重算 `head` 校验和。

use crate::kugou::store::resolve_custom_fonts_dir;
use base64::{engine::general_purpose::STANDARD as BASE64_STD, Engine as _};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// 单字体文件导入体积安全上限 (64 MiB，足以覆盖超大全字符集 CJK 可变字体与 TTC)
const MAX_IMPORT_FONT_BYTES: u64 = 64 * 1024 * 1024;
/// 解析单张 `name` 表最大读取字节 (1 MiB)
const MAX_NAME_TABLE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemFontFamilyInfo {
    /// CSS `font-family` 使用的规范家族名（优先英文标准名，无英文时用中文名）
    pub family_name: String,
    /// 面向中文用户的展示名（优先简体中文名，如「微软雅黑」「霞鹜文楷」）
    pub display_name: String,
    /// 该家族在电脑上实际安装拥有的字重值列表（升序，如 [300, 400, 700]）
    pub weights: Vec<u16>,
    /// 是否包含 OpenType `fvar` `wght` 可变字重轴
    pub is_variable: bool,
    pub weight_min: u16,
    pub weight_max: u16,
    /// 是否支持常用汉字 (CJK)
    pub supports_cjk: bool,
    /// 是否等宽字体（基于 `OS/2` Panose 或名称识别）
    pub is_monospace: bool,
    /// 该家族关联的字体文件数量
    pub file_count: u32,
    /// 来源标记 ("system" | "user")
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFontRecord {
    pub id: String,
    /// 注册到 `document.fonts` 使用的唯一安全家族名（如 `BMCustom_xxx`）
    pub css_family_name: String,
    /// 原始解析出的字体家族名
    pub family_name: String,
    /// 界面展示名称
    pub display_name: String,
    /// 存储在 `<数据根目录>/fonts/` 下的文件名
    pub file_name: String,
    /// 原始来源文件名（便于用户辨认）
    pub original_file_name: String,
    /// 实际格式 ("ttf" | "otf" | "woff2" | "woff")
    pub format: String,
    pub file_size_bytes: u64,
    pub weights: Vec<u16>,
    pub is_variable: bool,
    pub weight_min: u16,
    pub weight_max: u16,
    pub supports_cjk: bool,
    pub extracted_from_ttc: bool,
    pub added_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFontsPayload {
    pub system_fonts: Vec<SystemFontFamilyInfo>,
    pub custom_fonts: Vec<CustomFontRecord>,
    pub fonts_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFontBinaryPayload {
    pub id: String,
    pub css_family_name: String,
    pub format: String,
    pub mime_type: String,
    pub base64_data: String,
    pub weights: Vec<u16>,
    pub is_variable: bool,
    pub weight_min: u16,
    pub weight_max: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CustomFontManifest {
    pub fonts: Vec<CustomFontRecord>,
}

#[derive(Debug, Clone)]
struct ParsedFaceMeta {
    family_en: String,
    family_zh: String,
    weight_class: u16,
    is_variable: bool,
    weight_min: u16,
    weight_max: u16,
    supports_cjk: bool,
    is_monospace: bool,
}

fn is_safe_flat_file_name(name: &str) -> bool {
    let t = name.trim();
    if t.is_empty() || t == "." || t == ".." || t.contains('/') || t.contains('\\') || t.contains('\0') {
        return false;
    }
    let p = Path::new(t);
    let mut comps = p.components();
    matches!(comps.next(), Some(std::path::Component::Normal(_))) && comps.next().is_none()
}

fn manifest_path() -> PathBuf {
    resolve_custom_fonts_dir().join("custom_fonts.json")
}

fn load_custom_manifest() -> CustomFontManifest {
    let path = manifest_path();
    if !path.exists() {
        return CustomFontManifest::default();
    }
    let fonts_dir = resolve_custom_fonts_dir();
    let mut manifest = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<CustomFontManifest>(&s).ok())
        .unwrap_or_default();
    // 剔除非法文件名（防路径穿越）或磁盘文件已被手动删除的失效条目
    manifest.fonts.retain(|rec| {
        is_safe_flat_file_name(&rec.file_name) && fonts_dir.join(&rec.file_name).is_file()
    });
    manifest
}

fn save_custom_manifest(manifest: &CustomFontManifest) -> Result<(), String> {
    let path = manifest_path();
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    if fs::rename(&tmp, &path).is_err() {
        fs::write(&path, &json).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

#[inline]
fn read_u16_be(b: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([b[off], b[off + 1]])
}

#[inline]
fn read_u32_be(b: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

#[inline]
fn read_i32_be(b: &[u8], off: usize) -> i32 {
    i32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

fn decode_utf16_be(raw: &[u8]) -> String {
    let mut units = Vec::with_capacity(raw.len() / 2);
    for chunk in raw.chunks_exact(2) {
        let u = u16::from_be_bytes([chunk[0], chunk[1]]);
        if u != 0 {
            units.push(u);
        }
    }
    String::from_utf16_lossy(&units).trim().to_string()
}

fn decode_mac_roman(raw: &[u8]) -> String {
    raw.iter()
        .filter(|&&b| b >= 0x20 && b < 0x7F)
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string()
}

fn contains_cjk_char(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0x4E00..=0x9FFF).contains(&u)
            || (0x3400..=0x4DBF).contains(&u)
            || (0xF900..=0xFAFF).contains(&u)
    })
}

fn name_hints_cjk(name: &str) -> bool {
    if contains_cjk_char(name) {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    const CJK_HINTS: [&str; 22] = [
        "yahei",
        "simsun",
        "simhei",
        "simkai",
        "fangsong",
        "kaiti",
        "dengxian",
        "youyuan",
        "lishu",
        "stsong",
        "sthei",
        "stkai",
        "stfangsong",
        "pingfang",
        "harmonyos",
        "misans",
        "source han",
        "noto sans sc",
        "noto serif sc",
        "noto sans cjk",
        "noto serif cjk",
        "lxgw",
    ];
    CJK_HINTS.iter().any(|h| lower.contains(h))
        || lower.ends_with(" sc")
        || lower.ends_with(" tc")
        || lower.ends_with(" cn")
        || lower.contains("chinese")
        || lower.contains("gbk")
        || lower.contains("gb2312")
}

fn name_hints_mono(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("mono")
        || lower.contains("consola")
        || lower.contains("courier")
        || lower.contains("code")
        || lower.contains("sarasa")
        || lower.contains("fixed")
}

fn read_exact_at(file: &mut File, offset: u64, len: usize) -> Option<Vec<u8>> {
    if len == 0 || len > MAX_NAME_TABLE_BYTES {
        return None;
    }
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn parse_single_sfnt_face(file: &mut File, sfnt_offset: u64, file_len: u64) -> Option<ParsedFaceMeta> {
    if sfnt_offset.saturating_add(12) > file_len {
        return None;
    }
    let header = read_exact_at(file, sfnt_offset, 12)?;
    let tag = &header[0..4];
    let valid_sfnt = tag == [0x00, 0x01, 0x00, 0x00]
        || tag == b"OTTO"
        || tag == b"true"
        || tag == b"typ1";
    if !valid_sfnt {
        return None;
    }
    let num_tables = read_u16_be(&header, 4) as usize;
    if num_tables == 0 || num_tables > 512 {
        return None;
    }
    let dir_bytes_len = num_tables * 16;
    if sfnt_offset.saturating_add(12 + dir_bytes_len as u64) > file_len {
        return None;
    }
    let dir_buf = read_exact_at(file, sfnt_offset + 12, dir_bytes_len)?;

    let mut name_loc: Option<(u64, usize)> = None;
    let mut os2_loc: Option<(u64, usize)> = None;
    let mut fvar_loc: Option<(u64, usize)> = None;

    for i in 0..num_tables {
        let rec = &dir_buf[i * 16..(i + 1) * 16];
        let t = &rec[0..4];
        let off = read_u32_be(rec, 8) as u64;
        let len = read_u32_be(rec, 12) as usize;
        if off.saturating_add(len as u64) > file_len {
            continue;
        }
        match t {
            b"name" => name_loc = Some((off, len)),
            b"OS/2" => os2_loc = Some((off, len)),
            b"fvar" => fvar_loc = Some((off, len)),
            _ => {}
        }
    }

    // 1. Parse `name` table (NameID 16 Typographic Family > NameID 1 Family)
    let mut family_en_1 = String::new();
    let mut family_en_16 = String::new();
    let mut family_zh_1 = String::new();
    let mut family_zh_16 = String::new();
    let mut family_any = String::new();

    if let Some((off, len)) = name_loc {
        if len >= 6 && len <= MAX_NAME_TABLE_BYTES {
            if let Some(nb) = read_exact_at(file, off, len) {
                let count = read_u16_be(&nb, 2) as usize;
                let str_off = read_u16_be(&nb, 4) as usize;
                if str_off <= nb.len() {
                    for i in 0..count.min(4096) {
                        let r_off = 6 + i * 12;
                        if r_off + 12 > nb.len() {
                            break;
                        }
                        let platform_id = read_u16_be(&nb, r_off);
                        let encoding_id = read_u16_be(&nb, r_off + 2);
                        let language_id = read_u16_be(&nb, r_off + 4);
                        let name_id = read_u16_be(&nb, r_off + 6);
                        if name_id != 1 && name_id != 16 {
                            continue;
                        }
                        let slen = read_u16_be(&nb, r_off + 8) as usize;
                        let soff = read_u16_be(&nb, r_off + 10) as usize;
                        let start = str_off.saturating_add(soff);
                        let end = start.saturating_add(slen);
                        if end > nb.len() || slen == 0 {
                            continue;
                        }
                        let raw = &nb[start..end];
                        let decoded = match platform_id {
                            3 | 0 => decode_utf16_be(raw),
                            1 if encoding_id == 0 => decode_mac_roman(raw),
                            _ => continue,
                        };
                        if decoded.is_empty() || decoded.starts_with('@') {
                            continue;
                        }
                        if family_any.is_empty() {
                            family_any = decoded.clone();
                        }
                        let is_zh = platform_id == 3
                            && matches!(language_id, 0x0804 | 0x0404 | 0x0C04 | 0x1004 | 0x1404);
                        let is_en = (platform_id == 3 && language_id == 0x0409)
                            || (platform_id == 1 && language_id == 0);
                        if is_zh {
                            if name_id == 16 && (family_zh_16.is_empty() || language_id == 0x0804) {
                                family_zh_16 = decoded;
                            } else if name_id == 1 && (family_zh_1.is_empty() || language_id == 0x0804) {
                                family_zh_1 = decoded;
                            }
                        } else if is_en {
                            if name_id == 16 && family_en_16.is_empty() {
                                family_en_16 = decoded;
                            } else if name_id == 1 && family_en_1.is_empty() {
                                family_en_1 = decoded;
                            }
                        }
                    }
                }
            }
        }
    }

    let family_en = if !family_en_16.is_empty() {
        family_en_16
    } else if !family_en_1.is_empty() {
        family_en_1
    } else if !family_zh_16.is_empty() {
        family_zh_16.clone()
    } else if !family_zh_1.is_empty() {
        family_zh_1.clone()
    } else {
        family_any
    };

    if family_en.is_empty() || family_en.starts_with('@') {
        return None;
    }

    let family_zh = if !family_zh_16.is_empty() {
        family_zh_16
    } else if !family_zh_1.is_empty() {
        family_zh_1
    } else {
        family_en.clone()
    };

    // 2. Parse `OS/2` table (weight class, CJK Unicode/CodePage bits, Panose monospace)
    let mut weight_class: u16 = 400;
    let mut supports_cjk = name_hints_cjk(&family_en) || name_hints_cjk(&family_zh);
    let mut is_monospace = name_hints_mono(&family_en);

    if let Some((off, len)) = os2_loc {
        let read_len = len.min(96);
        if read_len >= 68 {
            if let Some(ob) = read_exact_at(file, off, read_len) {
                let version = read_u16_be(&ob, 0);
                let raw_w = read_u16_be(&ob, 4);
                if (100..=950).contains(&raw_w) {
                    weight_class = raw_w.clamp(100, 900);
                } else if (1..=9).contains(&raw_w) {
                    weight_class = raw_w * 100;
                }
                // Panose proportion (byte 35 in OS/2: offset 32..42 is panose[10], index 3 == 9 means Monospaced)
                if ob[35] == 9 {
                    is_monospace = true;
                }
                // ulUnicodeRange2 at offset 46..50: bit 27 (bit 59 overall) = CJK Unified Ideographs
                let ur2 = read_u32_be(&ob, 46);
                if (ur2 & (1u32 << 27)) != 0 {
                    supports_cjk = true;
                }
                if version >= 1 && ob.len() >= 82 {
                    let cp1 = read_u32_be(&ob, 78);
                    // bit 18 = Chinese Simplified (GBK), bit 20 = Chinese Traditional (Big5)
                    if (cp1 & ((1u32 << 18) | (1u32 << 20))) != 0 {
                        supports_cjk = true;
                    }
                }
            }
        }
    }

    // 3. Parse `fvar` table for Variable Font `wght` axis
    let mut is_variable = false;
    let mut weight_min = weight_class;
    let mut weight_max = weight_class;

    if let Some((off, len)) = fvar_loc {
        let read_len = len.min(4096);
        if read_len >= 16 {
            if let Some(fb) = read_exact_at(file, off, read_len) {
                let axes_off = read_u16_be(&fb, 4) as usize;
                let axis_count = read_u16_be(&fb, 8) as usize;
                let axis_size = read_u16_be(&fb, 10) as usize;
                if axis_size >= 20 {
                    for i in 0..axis_count.min(16) {
                        let a_off = axes_off.saturating_add(i * axis_size);
                        if a_off + 20 > fb.len() {
                            break;
                        }
                        if &fb[a_off..a_off + 4] == b"wght" {
                            let min_fix = read_i32_be(&fb, a_off + 4);
                            let max_fix = read_i32_be(&fb, a_off + 12);
                            let min_w = (min_fix >> 16).clamp(100, 900) as u16;
                            let max_w = (max_fix >> 16).clamp(100, 900) as u16;
                            if max_w > min_w {
                                is_variable = true;
                                weight_min = min_w;
                                weight_max = max_w;
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    Some(ParsedFaceMeta {
        family_en,
        family_zh,
        weight_class,
        is_variable,
        weight_min,
        weight_max,
        supports_cjk,
        is_monospace,
    })
}

/// 从磁盘字体文件（.ttf/.otf/.ttc/.otc）流式解析所有包含的字面元数据
fn parse_font_file_faces(path: &Path) -> Vec<ParsedFaceMeta> {
    let Ok(meta) = fs::metadata(path) else {
        return Vec::new();
    };
    let file_len = meta.len();
    if file_len < 16 {
        return Vec::new();
    }
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let mut head = [0u8; 12];
    if file.read_exact(&mut head).is_err() {
        return Vec::new();
    }

    if &head[0..4] == b"ttcf" {
        let num_fonts = read_u32_be(&head, 8).min(32) as usize;
        if num_fonts == 0 {
            return Vec::new();
        }
        let mut offs_buf = vec![0u8; num_fonts * 4];
        if file.read_exact(&mut offs_buf).is_err() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for i in 0..num_fonts {
            let face_off = read_u32_be(&offs_buf, i * 4) as u64;
            if let Some(face) = parse_single_sfnt_face(&mut file, face_off, file_len) {
                out.push(face);
            }
        }
        out
    } else {
        parse_single_sfnt_face(&mut file, 0, file_len)
            .into_iter()
            .collect()
    }
}

/// 纯 Rust 将 `.ttc` / `.otc` 字体集合中的指定字面拆包提取为标准独立 `.ttf` / `.otf` 字节流
/// （解决 Chromium / WebView2 `FontFace` API 原生无法直接加载 `.ttc` 的限制）
pub fn extract_ttc_face_to_sfnt(ttc_bytes: &[u8], face_index: usize) -> Result<Vec<u8>, String> {
    if ttc_bytes.len() < 16 || &ttc_bytes[0..4] != b"ttcf" {
        return Err("非法的 TrueType Collection (.ttc) 文件头".to_string());
    }
    let num_fonts = read_u32_be(ttc_bytes, 8) as usize;
    if face_index >= num_fonts || 12 + num_fonts * 4 > ttc_bytes.len() {
        return Err("TTC 字体集合索引越界".to_string());
    }
    let face_off = read_u32_be(ttc_bytes, 12 + face_index * 4) as usize;
    if face_off + 12 > ttc_bytes.len() {
        return Err("TTC 字面偏移越界".to_string());
    }

    let sfnt_tag = &ttc_bytes[face_off..face_off + 4];
    let num_tables = read_u16_be(ttc_bytes, face_off + 4) as usize;
    if num_tables == 0 || num_tables > 512 {
        return Err("TTC 字面表数量异常".to_string());
    }
    let dir_start = face_off + 12;
    let dir_end = dir_start + num_tables * 16;
    if dir_end > ttc_bytes.len() {
        return Err("TTC 字面表目录截断".to_string());
    }

    struct TableSlice<'a> {
        tag: [u8; 4],
        checksum: u32,
        data: &'a [u8],
    }

    let mut tables: Vec<TableSlice<'_>> = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let rec = &ttc_bytes[dir_start + i * 16..dir_start + (i + 1) * 16];
        let mut tag = [0u8; 4];
        tag.copy_from_slice(&rec[0..4]);
        let checksum = read_u32_be(rec, 4);
        let off = read_u32_be(rec, 8) as usize;
        let len = read_u32_be(rec, 12) as usize;
        if off.saturating_add(len) > ttc_bytes.len() {
            return Err("TTC 表数据越界".to_string());
        }
        tables.push(TableSlice {
            tag,
            checksum,
            data: &ttc_bytes[off..off + len],
        });
    }

    let new_header_len = 12 + num_tables * 16;
    let mut total_size = new_header_len;
    for t in &tables {
        total_size += (t.data.len() + 3) & !3;
    }

    let mut out = Vec::with_capacity(total_size);
    // Offset Table (12 bytes)
    out.extend_from_slice(sfnt_tag);
    out.extend_from_slice(&(num_tables as u16).to_be_bytes());
    let mut entry_selector: u16 = 0;
    let mut max_pow2: u16 = 1;
    while (max_pow2 as usize) * 2 <= num_tables {
        max_pow2 *= 2;
        entry_selector += 1;
    }
    let search_range: u16 = max_pow2 * 16;
    let range_shift: u16 = (num_tables as u16) * 16 - search_range;
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());

    // Table Directory
    let mut cur_offset = new_header_len;
    let mut head_table_out_offset: Option<usize> = None;
    for t in &tables {
        out.extend_from_slice(&t.tag);
        out.extend_from_slice(&t.checksum.to_be_bytes());
        out.extend_from_slice(&(cur_offset as u32).to_be_bytes());
        out.extend_from_slice(&(t.data.len() as u32).to_be_bytes());
        if &t.tag == b"head" {
            head_table_out_offset = Some(cur_offset);
        }
        cur_offset += (t.data.len() + 3) & !3;
    }

    // Table Payloads (4-byte aligned)
    for t in &tables {
        out.extend_from_slice(t.data);
        let pad = (4 - (t.data.len() % 4)) % 4;
        for _ in 0..pad {
            out.push(0);
        }
    }

    // Recompute `head` table checkSumAdjustment so Chromium OTS accepts the extracted SFNT
    if let Some(head_off) = head_table_out_offset {
        if head_off + 12 <= out.len() {
            out[head_off + 8..head_off + 12].copy_from_slice(&0u32.to_be_bytes());
            let mut sum: u32 = 0;
            for chunk in out.chunks_exact(4) {
                sum = sum.wrapping_add(u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
            let adj = 0xB1B0_AFBAu32.wrapping_sub(sum);
            out[head_off + 8..head_off + 12].copy_from_slice(&adj.to_be_bytes());
        }
    }

    Ok(out)
}

#[cfg(target_os = "windows")]
fn enumerate_registry_font_paths() -> Vec<(PathBuf, String)> {
    const HKEY_LOCAL_MACHINE: isize = 0x80000002u32 as isize;
    const HKEY_CURRENT_USER: isize = 0x80000001u32 as isize;
    const KEY_READ: u32 = 0x20019;
    const ERROR_SUCCESS: i32 = 0;
    const ERROR_NO_MORE_ITEMS: i32 = 259;
    const REG_SZ: u32 = 1;
    const REG_EXPAND_SZ: u32 = 2;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            hkey: isize,
            sub_key: *const u16,
            options: u32,
            sam: u32,
            result: *mut isize,
        ) -> i32;
        fn RegEnumValueW(
            hkey: isize,
            index: u32,
            value_name: *mut u16,
            value_name_len: *mut u32,
            reserved: *mut u32,
            reg_type: *mut u32,
            data: *mut u8,
            data_len: *mut u32,
        ) -> i32;
        fn RegCloseKey(hkey: isize) -> i32;
    }

    fn to_wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let win_fonts_dir = std::env::var("windir")
        .map(|w| PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|_| PathBuf::from(r"C:\Windows\Fonts"));
    let user_fonts_dir = std::env::var("LOCALAPPDATA")
        .map(|l| PathBuf::from(l).join("Microsoft").join("Windows").join("Fonts"))
        .ok();

    let subkey = to_wide_null(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts");
    let mut results = Vec::new();

    for (hive, default_dir, source_tag) in [
        (HKEY_LOCAL_MACHINE, Some(win_fonts_dir.clone()), "system"),
        (HKEY_CURRENT_USER, user_fonts_dir.clone(), "user"),
    ] {
        let mut hkey: isize = 0;
        let rc = unsafe { RegOpenKeyExW(hive, subkey.as_ptr(), 0, KEY_READ, &mut hkey) };
        if rc != ERROR_SUCCESS || hkey == 0 {
            continue;
        }
        let mut index = 0u32;
        let mut name_buf = vec![0u16; 512];
        let mut data_buf = vec![0u8; 1024];
        loop {
            let mut name_len = name_buf.len() as u32;
            let mut data_len = data_buf.len() as u32;
            let mut reg_type = 0u32;
            let status = unsafe {
                RegEnumValueW(
                    hkey,
                    index,
                    name_buf.as_mut_ptr(),
                    &mut name_len,
                    std::ptr::null_mut(),
                    &mut reg_type,
                    data_buf.as_mut_ptr(),
                    &mut data_len,
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            index += 1;
            if status != ERROR_SUCCESS || (reg_type != REG_SZ && reg_type != REG_EXPAND_SZ) {
                continue;
            }
            let byte_len = (data_len as usize).min(data_buf.len());
            let mut units = Vec::with_capacity(byte_len / 2);
            for chunk in data_buf[..byte_len].chunks_exact(2) {
                let u = u16::from_le_bytes([chunk[0], chunk[1]]);
                if u == 0 {
                    break;
                }
                units.push(u);
            }
            let raw_val = String::from_utf16_lossy(&units).trim().to_string();
            if raw_val.is_empty() {
                continue;
            }
            let candidate = PathBuf::from(&raw_val);
            let resolved = if candidate.is_absolute() {
                candidate
            } else if let Some(ref d) = default_dir {
                d.join(&raw_val)
            } else {
                win_fonts_dir.join(&raw_val)
            };
            results.push((resolved, source_tag.to_string()));
        }
        unsafe {
            RegCloseKey(hkey);
        }
    }

    results
}

#[cfg(not(target_os = "windows"))]
fn enumerate_registry_font_paths() -> Vec<(PathBuf, String)> {
    Vec::new()
}

fn is_supported_system_font_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc" | "otc"))
        .unwrap_or(false)
}

/// 扫描电脑已安装字体并结合本地持久化导入的自定义字体列表
pub fn list_installed_fonts() -> InstalledFontsPayload {
    let mut seen_paths: HashSet<String> = HashSet::new();
    let mut font_files: Vec<(PathBuf, String)> = Vec::new();

    // 1. 注册表枚举（捕获系统 + 当前用户 + 第三方激活字体）
    for (p, src) in enumerate_registry_font_paths() {
        if is_supported_system_font_ext(&p) && p.is_file() {
            let key = p.to_string_lossy().to_ascii_lowercase();
            if seen_paths.insert(key) {
                font_files.push((p, src));
            }
        }
    }

    // 2. 扫描标准 Windows 系统与用户字体目录兜底
    let mut scan_dirs: Vec<(PathBuf, &str)> = Vec::new();
    if let Ok(windir) = std::env::var("windir") {
        scan_dirs.push((PathBuf::from(windir).join("Fonts"), "system"));
    } else {
        scan_dirs.push((PathBuf::from(r"C:\Windows\Fonts"), "system"));
    }
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        scan_dirs.push((
            PathBuf::from(local_app)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
            "user",
        ));
    }

    for (dir, src) in scan_dirs {
        if let Ok(rd) = fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if is_supported_system_font_ext(&p) && p.is_file() {
                    let key = p.to_string_lossy().to_ascii_lowercase();
                    if seen_paths.insert(key) {
                        font_files.push((p, src.to_string()));
                    }
                }
            }
        }
    }

    // 3. 按家族名聚合全部字重与特征
    struct FamilyAgg {
        family_name: String,
        display_name: String,
        weights: BTreeSet<u16>,
        is_variable: bool,
        weight_min: u16,
        weight_max: u16,
        supports_cjk: bool,
        is_monospace: bool,
        file_count: u32,
        source: String,
    }

    let mut map: BTreeMap<String, FamilyAgg> = BTreeMap::new();

    for (path, src) in font_files {
        let faces = parse_font_file_faces(&path);
        for face in faces {
            let key = face.family_en.to_ascii_lowercase();
            let entry = map.entry(key).or_insert_with(|| FamilyAgg {
                family_name: face.family_en.clone(),
                display_name: face.family_zh.clone(),
                weights: BTreeSet::new(),
                is_variable: false,
                weight_min: face.weight_min,
                weight_max: face.weight_max,
                supports_cjk: face.supports_cjk,
                is_monospace: face.is_monospace,
                file_count: 0,
                source: src.clone(),
            });
            entry.weights.insert(face.weight_class);
            if face.is_variable {
                entry.is_variable = true;
                entry.weight_min = entry.weight_min.min(face.weight_min);
                entry.weight_max = entry.weight_max.max(face.weight_max);
            } else {
                entry.weight_min = entry.weight_min.min(face.weight_class);
                entry.weight_max = entry.weight_max.max(face.weight_class);
            }
            if contains_cjk_char(&face.family_zh) && !contains_cjk_char(&entry.display_name) {
                entry.display_name = face.family_zh.clone();
            }
            if face.supports_cjk {
                entry.supports_cjk = true;
            }
            if face.is_monospace {
                entry.is_monospace = true;
            }
            if src == "user" {
                entry.source = "user".to_string();
            }
            entry.file_count = entry.file_count.saturating_add(1);
        }
    }

    let mut system_fonts: Vec<SystemFontFamilyInfo> = map
        .into_values()
        .map(|agg| {
            let mut weights: Vec<u16> = agg.weights.into_iter().collect();
            if weights.is_empty() {
                weights.push(400);
            }
            SystemFontFamilyInfo {
                family_name: agg.family_name,
                display_name: agg.display_name,
                weights,
                is_variable: agg.is_variable,
                weight_min: agg.weight_min,
                weight_max: agg.weight_max,
                supports_cjk: agg.supports_cjk,
                is_monospace: agg.is_monospace,
                file_count: agg.file_count,
                source: agg.source,
            }
        })
        .collect();

    // 智能排序：中文多字重/可变字体 > 用户安装中文字体 > 系统自带中文字体 > 西文字体
    system_fonts.sort_by(|a, b| {
        let score = |f: &SystemFontFamilyInfo| -> i32 {
            let mut s = 0;
            if f.supports_cjk {
                s += 1000;
            }
            if f.source == "user" {
                s += 120;
            }
            if f.is_variable {
                s += 80;
            }
            if f.weights.len() >= 3 {
                s += 60;
            } else if f.weights.len() >= 2 {
                s += 30;
            }
            let lower = f.family_name.to_ascii_lowercase();
            if lower.contains("harmonyos")
                || lower.contains("misans")
                || lower.contains("source han")
                || lower.contains("noto sans sc")
                || lower.contains("lxgw")
                || lower.contains("microsoft yahei")
                || lower.contains("pingfang")
            {
                s += 90;
            }
            s
        };
        score(b)
            .cmp(&score(a))
            .then_with(|| a.display_name.cmp(&b.display_name))
    });

    let custom_fonts = load_custom_manifest().fonts;
    InstalledFontsPayload {
        system_fonts,
        custom_fonts,
        fonts_dir: resolve_custom_fonts_dir().to_string_lossy().to_string(),
    }
}

fn sanitize_font_file_stem(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "custom_font".to_string()
    } else {
        trimmed.chars().take(48).collect()
    }
}

/// 导入用户手动选择的外部字体文件（支持 .ttf / .otf / .woff2 / .woff / .ttc / .otc）并持久化保存到 `<数据根目录>/fonts/`
pub fn import_custom_font_files(paths: &[String]) -> Result<Vec<CustomFontRecord>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let fonts_dir = resolve_custom_fonts_dir();
    let mut manifest = load_custom_manifest();
    let mut added: Vec<CustomFontRecord> = Vec::new();

    for raw_path in paths.iter().take(24) {
        let p = Path::new(raw_path.trim());
        let meta = fs::symlink_metadata(p)
            .map_err(|_| format!("无法访问字体文件：{}", p.display()))?;
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err(format!("不支持符号链接或目录：{}", p.display()));
        }
        if meta.len() < 16 || meta.len() > MAX_IMPORT_FONT_BYTES {
            return Err(format!(
                "字体文件体积超出允许范围 (16B ~ 64MB)：{}",
                p.display()
            ));
        }

        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "ttf" | "otf" | "woff2" | "woff" | "ttc" | "otc") {
            return Err(format!("不支持的字体文件格式 (.{})", ext));
        }

        let original_bytes = fs::read(p).map_err(|e| format!("读取字体文件失败: {}", e))?;
        if original_bytes.len() < 12 {
            return Err("字体文件内容过短".to_string());
        }
        let magic = &original_bytes[0..4];
        let valid_magic = magic == [0x00, 0x01, 0x00, 0x00]
            || magic == b"OTTO"
            || magic == b"true"
            || magic == b"typ1"
            || magic == b"ttcf"
            || magic == b"wOF2"
            || magic == b"wOFF";
        if !valid_magic {
            return Err(format!("文件头不是合法字体格式：{}", p.display()));
        }

        let orig_name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("custom.ttf")
            .to_string();
        let orig_stem = p
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("CustomFont")
            .to_string();

        let mut extracted_from_ttc = false;
        let (final_bytes, final_format) = if magic == b"ttcf" {
            // 优先选择第一个非 @ 的常规主字面提取为独立 SFNT
            let faces = parse_font_file_faces(p);
            let chosen_idx = faces
                .iter()
                .position(|f| f.weight_class == 400)
                .unwrap_or(0);
            let extracted = extract_ttc_face_to_sfnt(&original_bytes, chosen_idx)?;
            extracted_from_ttc = true;
            let fmt = if extracted.len() >= 4 && &extracted[0..4] == b"OTTO" {
                "otf"
            } else {
                "ttf"
            };
            (extracted, fmt.to_string())
        } else if magic == b"wOF2" {
            (original_bytes, "woff2".to_string())
        } else if magic == b"wOFF" {
            (original_bytes, "woff".to_string())
        } else if magic == b"OTTO" {
            (original_bytes, "otf".to_string())
        } else {
            (original_bytes, "ttf".to_string())
        };

        let id = format!("cf_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let css_family_name = format!("BMCustomFont_{}", &id[3..]);
        let stored_file_name = format!(
            "{}_{}.{}",
            sanitize_font_file_stem(&orig_stem),
            &id[3..],
            final_format
        );
        let stored_path = fonts_dir.join(&stored_file_name);
        fs::write(&stored_path, &final_bytes).map_err(|e| format!("保存字体文件失败: {}", e))?;

        let parsed = if final_format == "ttf" || final_format == "otf" {
            parse_font_file_faces(&stored_path).into_iter().next()
        } else {
            None
        };

        let (family_name, display_name, weights, is_variable, weight_min, weight_max, supports_cjk) =
            if let Some(f) = parsed {
                (
                    f.family_en,
                    f.family_zh,
                    vec![f.weight_class],
                    f.is_variable,
                    f.weight_min,
                    f.weight_max,
                    f.supports_cjk,
                )
            } else {
                let clean_disp = orig_stem.replace(['_', '-'], " ").trim().to_string();
                let disp = if clean_disp.is_empty() {
                    "自定义字体".to_string()
                } else {
                    clean_disp
                };
                let cjk = name_hints_cjk(&disp);
                (disp.clone(), disp, vec![400], false, 400, 400, cjk)
            };

        let rec = CustomFontRecord {
            id,
            css_family_name,
            family_name,
            display_name,
            file_name: stored_file_name,
            original_file_name: orig_name,
            format: final_format,
            file_size_bytes: final_bytes.len() as u64,
            weights,
            is_variable,
            weight_min,
            weight_max,
            supports_cjk,
            extracted_from_ttc,
            added_at_ms: Utc::now().timestamp_millis(),
        };
        manifest.fonts.insert(0, rec.clone());
        added.push(rec);
    }

    save_custom_manifest(&manifest)?;
    Ok(added)
}

/// 按已注册 `font_id` 安全读取自定义字体 Base64 数据（供主窗口、桌面歌词与迷你窗口通过 `FontFace` API 加载）
pub fn read_custom_font_binary(font_id: &str) -> Result<CustomFontBinaryPayload, String> {
    let clean_id = font_id.trim();
    if clean_id.is_empty() {
        return Err("字体 ID 不能为空".to_string());
    }
    let manifest = load_custom_manifest();
    let rec = manifest
        .fonts
        .into_iter()
        .find(|f| f.id == clean_id)
        .ok_or_else(|| format!("未找到已导入的自定义字体：{}", clean_id))?;

    if !is_safe_flat_file_name(&rec.file_name) {
        return Err("非法字体文件名，已拒绝读取".to_string());
    }
    let path = resolve_custom_fonts_dir().join(&rec.file_name);
    let bytes = fs::read(&path).map_err(|e| format!("读取已保存字体失败: {}", e))?;
    let mime_type = match rec.format.as_str() {
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "otf" => "font/otf",
        _ => "font/ttf",
    }
    .to_string();

    Ok(CustomFontBinaryPayload {
        id: rec.id,
        css_family_name: rec.css_family_name,
        format: rec.format,
        mime_type,
        base64_data: BASE64_STD.encode(&bytes),
        weights: rec.weights,
        is_variable: rec.is_variable,
        weight_min: rec.weight_min,
        weight_max: rec.weight_max,
    })
}

/// 卸载并删除指定的用户手动导入字体
pub fn delete_custom_font(font_id: &str) -> Result<Vec<CustomFontRecord>, String> {
    let clean_id = font_id.trim();
    let mut manifest = load_custom_manifest();
    let fonts_dir = resolve_custom_fonts_dir();
    if let Some(pos) = manifest.fonts.iter().position(|f| f.id == clean_id) {
        let removed = manifest.fonts.remove(pos);
        if is_safe_flat_file_name(&removed.file_name) {
            let path = fonts_dir.join(&removed.file_name);
            if path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
        save_custom_manifest(&manifest)?;
    }
    Ok(manifest.fonts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小合法 TrueType SFNT（包含 `head` / `name` / `OS/2` / `fvar`）用于离线单元测试
    fn build_minimal_sfnt(en_name: &str, zh_name: &str, weight: u16, variable: bool) -> Vec<u8> {
        // Build `head` table (54 bytes)
        let head_data = vec![0u8; 54];

        // Build `name` table (2 records: en-US NameID=1, zh-CN NameID=1)
        let en_utf16: Vec<u8> = en_name
            .encode_utf16()
            .flat_map(|u| u.to_be_bytes())
            .collect();
        let zh_utf16: Vec<u8> = zh_name
            .encode_utf16()
            .flat_map(|u| u.to_be_bytes())
            .collect();
        let mut name_data = Vec::new();
        let count = 2u16;
        let str_off = (6 + count * 12) as u16;
        name_data.extend_from_slice(&0u16.to_be_bytes()); // format
        name_data.extend_from_slice(&count.to_be_bytes());
        name_data.extend_from_slice(&str_off.to_be_bytes());
        // Record 1: en-US
        name_data.extend_from_slice(&3u16.to_be_bytes());
        name_data.extend_from_slice(&1u16.to_be_bytes());
        name_data.extend_from_slice(&0x0409u16.to_be_bytes());
        name_data.extend_from_slice(&1u16.to_be_bytes());
        name_data.extend_from_slice(&(en_utf16.len() as u16).to_be_bytes());
        name_data.extend_from_slice(&0u16.to_be_bytes());
        // Record 2: zh-CN
        name_data.extend_from_slice(&3u16.to_be_bytes());
        name_data.extend_from_slice(&1u16.to_be_bytes());
        name_data.extend_from_slice(&0x0804u16.to_be_bytes());
        name_data.extend_from_slice(&1u16.to_be_bytes());
        name_data.extend_from_slice(&(zh_utf16.len() as u16).to_be_bytes());
        name_data.extend_from_slice(&(en_utf16.len() as u16).to_be_bytes());
        name_data.extend_from_slice(&en_utf16);
        name_data.extend_from_slice(&zh_utf16);

        // Build `OS/2` table (86 bytes, version 1, CJK bit set)
        let mut os2_data = vec![0u8; 86];
        os2_data[0..2].copy_from_slice(&1u16.to_be_bytes());
        os2_data[4..6].copy_from_slice(&weight.to_be_bytes());
        os2_data[46..50].copy_from_slice(&(1u32 << 27).to_be_bytes()); // CJK Unified Ideographs
        os2_data[78..82].copy_from_slice(&(1u32 << 18).to_be_bytes()); // GBK

        let mut tables: Vec<([u8; 4], Vec<u8>)> = vec![
            (*b"OS/2", os2_data),
            (*b"head", head_data),
            (*b"name", name_data),
        ];

        if variable {
            let mut fvar_data = Vec::new();
            fvar_data.extend_from_slice(&1u16.to_be_bytes()); // major
            fvar_data.extend_from_slice(&0u16.to_be_bytes()); // minor
            fvar_data.extend_from_slice(&16u16.to_be_bytes()); // axesArrayOffset
            fvar_data.extend_from_slice(&2u16.to_be_bytes()); // reserved
            fvar_data.extend_from_slice(&1u16.to_be_bytes()); // axisCount
            fvar_data.extend_from_slice(&20u16.to_be_bytes()); // axisSize
            fvar_data.extend_from_slice(&0u16.to_be_bytes()); // instanceCount
            fvar_data.extend_from_slice(&4u16.to_be_bytes()); // instanceSize
            // wght axis: min=100, default=400, max=900
            fvar_data.extend_from_slice(b"wght");
            fvar_data.extend_from_slice(&((100i32) << 16).to_be_bytes());
            fvar_data.extend_from_slice(&((400i32) << 16).to_be_bytes());
            fvar_data.extend_from_slice(&((900i32) << 16).to_be_bytes());
            fvar_data.extend_from_slice(&0u16.to_be_bytes());
            fvar_data.extend_from_slice(&256u16.to_be_bytes());
            tables.push((*b"fvar", fvar_data));
        }

        let num_tables = tables.len();
        let header_len = 12 + num_tables * 16;
        let mut out = Vec::new();
        out.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
        out.extend_from_slice(&(num_tables as u16).to_be_bytes());
        out.extend_from_slice(&32u16.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&16u16.to_be_bytes());

        let mut off = header_len;
        for (tag, data) in &tables {
            out.extend_from_slice(tag);
            out.extend_from_slice(&0u32.to_be_bytes());
            out.extend_from_slice(&(off as u32).to_be_bytes());
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            off += (data.len() + 3) & !3;
        }
        for (_, data) in &tables {
            out.extend_from_slice(data);
            let pad = (4 - (data.len() % 4)) % 4;
            out.resize(out.len() + pad, 0);
        }
        out
    }

    #[test]
    fn test_sfnt_name_os2_fvar_parsing_and_ttc_extraction() {
        let sfnt = build_minimal_sfnt("Breeze Test Sans", "微风测试黑体", 500, true);
        let tmp_ttf = std::env::temp_dir().join(format!(
            "bm_font_unit_{}.ttf",
            uuid::Uuid::new_v4().simple()
        ));
        fs::write(&tmp_ttf, &sfnt).unwrap();
        let faces = parse_font_file_faces(&tmp_ttf);
        let _ = fs::remove_file(&tmp_ttf);

        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].family_en, "Breeze Test Sans");
        assert_eq!(faces[0].family_zh, "微风测试黑体");
        assert_eq!(faces[0].weight_class, 500);
        assert!(faces[0].supports_cjk);
        assert!(faces[0].is_variable);
        assert_eq!(faces[0].weight_min, 100);
        assert_eq!(faces[0].weight_max, 900);

        // Wrap single SFNT into a minimal TTC header (offset adjusted by +16)
        let mut ttc = Vec::new();
        ttc.extend_from_slice(b"ttcf");
        ttc.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        ttc.extend_from_slice(&1u32.to_be_bytes());
        ttc.extend_from_slice(&16u32.to_be_bytes()); // face 0 starts at offset 16
        let mut shifted = sfnt.clone();
        let num_tables = read_u16_be(&shifted, 4) as usize;
        for i in 0..num_tables {
            let off_pos = 12 + i * 16 + 8;
            let orig_off = read_u32_be(&shifted, off_pos);
            shifted[off_pos..off_pos + 4].copy_from_slice(&(orig_off + 16).to_be_bytes());
        }
        ttc.extend_from_slice(&shifted);

        let extracted = extract_ttc_face_to_sfnt(&ttc, 0).expect("TTC extraction should succeed");
        let tmp_ext = std::env::temp_dir().join(format!(
            "bm_font_ttc_ext_{}.ttf",
            uuid::Uuid::new_v4().simple()
        ));
        fs::write(&tmp_ext, &extracted).unwrap();
        let ext_faces = parse_font_file_faces(&tmp_ext);
        let _ = fs::remove_file(&tmp_ext);

        assert_eq!(ext_faces.len(), 1);
        assert_eq!(ext_faces[0].family_en, "Breeze Test Sans");
        assert_eq!(ext_faces[0].family_zh, "微风测试黑体");
        assert!(ext_faces[0].is_variable);
    }

    #[test]
    fn test_windows_system_fonts_enumeration_non_empty() {
        let payload = list_installed_fonts();
        #[cfg(target_os = "windows")]
        {
            assert!(
                !payload.system_fonts.is_empty(),
                "Expected Windows system fonts to be detected"
            );
            assert!(
                payload.system_fonts.iter().any(|f| f.supports_cjk),
                "Expected at least one CJK font on Windows"
            );
        }
    }
}
