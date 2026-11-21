use base64::{engine::general_purpose::STANDARD as BASE64_STD, Engine as _};
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::picture::{MimeType, PictureType};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

pub struct AudioTagMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub publish_date: String,
    pub song_hash: String,
    pub quality_label: String,
    pub cover_bytes: Option<Vec<u8>>,
    pub lrc_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioContainerFormat {
    Flac,
    Mp3,
    Ogg,
    Other,
}

/// 原子写入辅助函数：先写入同目录临时文件再执行 rename，防止断电或中途异常损坏原音频文件
fn write_file_atomically(path: &Path, data: &[u8]) -> Result<(), String> {
    let tmp_path = path.with_extension("tmp_breeze_tag");
    if let Err(e) = fs::write(&tmp_path, data) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.to_string());
    }
    if fs::rename(&tmp_path, path).is_err() {
        let res = fs::write(path, data).map_err(|e| e.to_string());
        let _ = fs::remove_file(&tmp_path);
        return res;
    }
    Ok(())
}

/// 单文件元数据重写最大允许体积上限 (512 MiB)，防止恶意超大文件触发 OOM (SEC-09)
const MAX_METADATA_REWRITE_BYTES: u64 = 512 * 1024 * 1024;

pub fn detect_format(path: &Path, fallback_ext: &str) -> AudioContainerFormat {
    if let Ok(mut file) = fs::File::open(path) {
        let mut magic = [0u8; 16];
        if let Ok(n) = std::io::Read::read(&mut file, &mut magic) {
            if n >= 4 {
                if &magic[0..4] == b"fLaC" {
                    return AudioContainerFormat::Flac;
                }
                if &magic[0..4] == b"OggS" {
                    return AudioContainerFormat::Ogg;
                }
                if &magic[0..3] == b"ID3" {
                    return AudioContainerFormat::Mp3;
                }
                if magic[0] == 0xFF && (magic[1] & 0xE0) == 0xE0 {
                    return AudioContainerFormat::Mp3;
                }
            }
        }
    }
    let ext = if !fallback_ext.trim().is_empty() {
        fallback_ext.trim().trim_start_matches('.').to_lowercase()
    } else {
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
    };
    match ext.as_str() {
        "flac" => AudioContainerFormat::Flac,
        "mp3" => AudioContainerFormat::Mp3,
        "ogg" | "oga" | "opus" => AudioContainerFormat::Ogg,
        _ => AudioContainerFormat::Other,
    }
}

#[derive(Debug, Clone, Default)]
pub struct StreamHeaderAnalysis {
    pub codec: String,
    pub sample_rate_hz: i32,
    pub bit_depth: i32,
    pub channels: i32,
    pub bitrate_kbps: i32,
    pub publish_date: String,
    pub publish_company: String,
    pub language: String,
    pub genre: String,
    pub lyricist: String,
    pub composer: String,
    pub arranger: String,
    pub replay_gain_db: Option<f32>,
}

fn fill_from_vorbis_map(map: &BTreeMap<String, String>, out: &mut StreamHeaderAnalysis) {
    let pick = |keys: &[&str]| -> String {
        for k in keys {
            if let Some(v) = map.get(*k) {
                let t = v.trim();
                if !t.is_empty() {
                    return t.to_string();
                }
            }
        }
        String::new()
    };
    if out.publish_date.is_empty() {
        let dt = pick(&["DATE", "ORIGINALDATE", "YEAR"]);
        if !dt.is_empty() {
            out.publish_date = dt.chars().take(10).collect();
        }
    }
    if out.publish_company.is_empty() {
        out.publish_company = pick(&["ORGANIZATION", "PUBLISHER", "LABEL", "COPYRIGHT"]);
    }
    if out.language.is_empty() {
        out.language = pick(&["LANGUAGE"]);
    }
    if out.genre.is_empty() {
        out.genre = pick(&["GENRE", "STYLE"]);
    }
    if out.lyricist.is_empty() {
        out.lyricist = pick(&["LYRICIST", "WRITER", "AUTHOR"]);
    }
    if out.composer.is_empty() {
        out.composer = pick(&["COMPOSER"]);
    }
    if out.arranger.is_empty() {
        out.arranger = pick(&["ARRANGER"]);
    }
    if out.replay_gain_db.is_none() {
        let rg = pick(&["REPLAYGAIN_TRACK_GAIN", "R128_TRACK_GAIN"]);
        if !rg.is_empty() {
            let cleaned: String = rg
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '+' || *c == '.')
                .collect();
            if let Ok(v) = cleaned.parse::<f32>() {
                if (-18.0..=12.0).contains(&v) {
                    out.replay_gain_db = Some(v.clamp(-12.0, 8.0));
                }
            }
        }
    }
}

fn decode_id3_text_payload(payload: &[u8]) -> String {
    if payload.len() < 2 {
        return String::new();
    }
    let encoding = payload[0];
    let body = &payload[1..];
    let raw = match encoding {
        0 => body.iter().map(|&b| b as char).collect::<String>(),
        1 | 2 => {
            if body.len() < 2 {
                String::new()
            } else {
                let (is_le, slice) = if body[0] == 0xFF && body[1] == 0xFE {
                    (true, &body[2..])
                } else if body[0] == 0xFE && body[1] == 0xFF {
                    (false, &body[2..])
                } else {
                    (encoding == 1, body)
                };
                let units: Vec<u16> = slice
                    .chunks_exact(2)
                    .map(|c| {
                        if is_le {
                            u16::from_le_bytes([c[0], c[1]])
                        } else {
                            u16::from_be_bytes([c[0], c[1]])
                        }
                    })
                    .take_while(|&u| u != 0)
                    .collect();
                String::from_utf16_lossy(&units)
            }
        }
        3 => String::from_utf8_lossy(body).to_string(),
        _ => String::new(),
    };
    raw.trim_matches('\0').trim().to_string()
}

/// 解析音频流头部字节（支持 FLAC / OGG Vorbis / Opus / MP3 + ID3v2 / WAV / M4A AAC/ALAC）
pub fn parse_stream_header_analysis(bytes: &[u8], ext_hint: &str) -> StreamHeaderAnalysis {
    let mut out = StreamHeaderAnalysis::default();
    let ext_lower = ext_hint.trim().trim_start_matches('.').to_lowercase();

    // 1. FLAC: "fLaC" + STREAMINFO + Vorbis Comment
    if bytes.len() >= 42 && &bytes[0..4] == b"fLaC" {
        out.codec = "FLAC".to_string();
        let mut offset = 4usize;
        let mut is_last = false;
        while !is_last && offset + 4 <= bytes.len() {
            let hdr = bytes[offset];
            is_last = (hdr & 0x80) != 0;
            let btype = hdr & 0x7F;
            let len = ((bytes[offset + 1] as usize) << 16)
                | ((bytes[offset + 2] as usize) << 8)
                | (bytes[offset + 3] as usize);
            offset += 4;
            if offset + len > bytes.len() {
                break;
            }
            let blk = &bytes[offset..offset + len];
            if btype == 0 && blk.len() >= 18 {
                let b10 = blk[10] as i32;
                let b11 = blk[11] as i32;
                let b12 = blk[12] as i32;
                let b13 = blk[13] as i32;
                let sr = (b10 << 12) | (b11 << 4) | ((b12 & 0xF0) >> 4);
                let ch = ((b12 & 0x0E) >> 1) + 1;
                let bd = (((b12 & 0x01) << 4) | ((b13 & 0xF0) >> 4)) + 1;
                if (8000..=384_000).contains(&sr) && (8..=32).contains(&bd) {
                    out.sample_rate_hz = sr;
                    out.bit_depth = bd;
                    out.channels = ch.clamp(1, 8);
                }
            } else if btype == 4 {
                let (_, map) = parse_vorbis_comment_block_with_vendor(blk);
                fill_from_vorbis_map(&map, &mut out);
            }
            offset += len;
        }
        if out.sample_rate_hz > 0 {
            return out;
        }
    }

    // 2. OGG Vorbis / Opus: "OggS"
    if bytes.len() >= 48 && &bytes[0..4] == b"OggS" {
        for i in 0..bytes.len().saturating_sub(16) {
            if &bytes[i..i + 7] == b"\x01vorbis" && i + 28 <= bytes.len() {
                out.codec = "OGG Vorbis".to_string();
                let ch = bytes[i + 11] as i32;
                let sr = u32::from_le_bytes([
                    bytes[i + 12],
                    bytes[i + 13],
                    bytes[i + 14],
                    bytes[i + 15],
                ]) as i32;
                let nom_br = i32::from_le_bytes([
                    bytes[i + 20],
                    bytes[i + 21],
                    bytes[i + 22],
                    bytes[i + 23],
                ]);
                if (8000..=192_000).contains(&sr) {
                    out.sample_rate_hz = sr;
                    out.bit_depth = 16;
                    out.channels = ch.clamp(1, 8);
                    if nom_br > 0 {
                        out.bitrate_kbps = (nom_br / 1000).clamp(32, 1000);
                    }
                }
            } else if i + 19 <= bytes.len() && &bytes[i..i + 8] == b"OpusHead" {
                out.codec = "Opus".to_string();
                out.channels = (bytes[i + 9] as i32).clamp(1, 8);
                out.sample_rate_hz = 48_000;
                out.bit_depth = 16;
            } else if i + 7 <= bytes.len() && &bytes[i..i + 7] == b"\x03vorbis" {
                let (_, map) = parse_vorbis_comment_block_with_vendor(&bytes[i + 7..]);
                fill_from_vorbis_map(&map, &mut out);
            } else if i + 8 <= bytes.len() && &bytes[i..i + 8] == b"OpusTags" {
                let (_, map) = parse_vorbis_comment_block_with_vendor(&bytes[i + 8..]);
                fill_from_vorbis_map(&map, &mut out);
            }
        }
        if out.sample_rate_hz > 0 {
            return out;
        }
    }

    // 3. WAV / RIFF: "RIFF"...."WAVE"
    if bytes.len() >= 44 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        out.codec = "WAV".to_string();
        let mut pos = 12usize;
        while pos + 8 <= bytes.len() {
            let chunk_id = &bytes[pos..pos + 4];
            let chunk_sz = u32::from_le_bytes([
                bytes[pos + 4],
                bytes[pos + 5],
                bytes[pos + 6],
                bytes[pos + 7],
            ]) as usize;
            pos += 8;
            if chunk_id == b"fmt " && chunk_sz >= 16 && pos + 16 <= bytes.len() {
                let ch = u16::from_le_bytes([bytes[pos + 2], bytes[pos + 3]]) as i32;
                let sr = u32::from_le_bytes([
                    bytes[pos + 4],
                    bytes[pos + 5],
                    bytes[pos + 6],
                    bytes[pos + 7],
                ]) as i32;
                let byte_rate = u32::from_le_bytes([
                    bytes[pos + 8],
                    bytes[pos + 9],
                    bytes[pos + 10],
                    bytes[pos + 11],
                ]) as i32;
                let bd = u16::from_le_bytes([bytes[pos + 14], bytes[pos + 15]]) as i32;
                if (8000..=384_000).contains(&sr) {
                    out.sample_rate_hz = sr;
                    out.channels = ch.clamp(1, 8);
                    out.bit_depth = if (8..=32).contains(&bd) { bd } else { 16 };
                    if byte_rate > 0 {
                        out.bitrate_kbps = (byte_rate * 8) / 1000;
                    }
                    return out;
                }
            }
            pos = pos.saturating_add(chunk_sz + (chunk_sz & 1));
        }
    }

    // 4. MP4 / M4A: "ftyp"
    if bytes.len() >= 16 && &bytes[4..8] == b"ftyp" {
        let is_alac = bytes.windows(4).any(|w| w == b"alac");
        out.codec = if is_alac {
            "ALAC".to_string()
        } else {
            "AAC".to_string()
        };
        if is_alac {
            if let Some(pos) = bytes.windows(4).position(|w| w == b"alac") {
                // alac magic cookie may follow sample entry; fallback safe values if truncated
                if pos + 28 <= bytes.len() {
                    let sr = u32::from_be_bytes([
                        bytes[pos + 24],
                        bytes[pos + 25],
                        bytes[pos + 26],
                        bytes[pos + 27],
                    ]) as i32;
                    if (8000..=384_000).contains(&sr) {
                        out.sample_rate_hz = sr;
                    }
                }
            }
            out.bit_depth = 24;
        } else {
            out.bit_depth = 16;
        }
        if out.sample_rate_hz <= 0 {
            out.sample_rate_hz = 44_100;
        }
        out.channels = 2;
        return out;
    }

    // 5. MP3 / ID3v2
    if bytes.len() >= 10 {
        let mut offset = 0usize;
        if &bytes[0..3] == b"ID3" {
            let ver_major = bytes[3];
            let flags = bytes[5];
            let id3_size = decode_synchsafe_u32(&bytes[6..10]);
            let body_end = (10 + id3_size).min(bytes.len());
            if body_end > 10 {
                let preserved = extract_preserved_id3v2_frames(
                    &bytes[10..body_end],
                    ver_major,
                    flags,
                    &HashSet::new(),
                );
                for (fid, payload) in preserved {
                    let val = decode_id3_text_payload(&payload);
                    if val.is_empty() {
                        continue;
                    }
                    match &fid {
                        b"TPUB" if out.publish_company.is_empty() => out.publish_company = val,
                        b"TCON" if out.genre.is_empty() => {
                            out.genre = val.trim_matches(|c| c == '(' || c == ')').to_string();
                        }
                        b"TLAN" if out.language.is_empty() => out.language = val,
                        b"TEXT" if out.lyricist.is_empty() => out.lyricist = val,
                        b"TCOM" if out.composer.is_empty() => out.composer = val,
                        b"TPE4" if out.arranger.is_empty() => out.arranger = val,
                        b"TDRC" | b"TYER" if out.publish_date.is_empty() => {
                            out.publish_date = val.chars().take(10).collect();
                        }
                        _ => {}
                    }
                }
            }
            offset = (10 + id3_size).min(bytes.len());
        }

        for i in offset..bytes.len().saturating_sub(4) {
            if bytes[i] == 0xFF && (bytes[i + 1] & 0xE0) == 0xE0 {
                let version_bits = (bytes[i + 1] >> 3) & 0x03;
                let layer_bits = (bytes[i + 1] >> 1) & 0x03;
                let br_idx = ((bytes[i + 2] >> 4) & 0x0F) as usize;
                let sr_idx = ((bytes[i + 2] >> 2) & 0x03) as usize;
                let ch_mode = (bytes[i + 3] >> 6) & 0x03;
                if version_bits != 1 && layer_bits != 0 && sr_idx < 3 {
                    let base_sr = match sr_idx {
                        0 => 44_100,
                        1 => 48_000,
                        _ => 32_000,
                    };
                    let sr = match version_bits {
                        3 => base_sr,
                        2 => base_sr / 2,
                        0 => base_sr / 4,
                        _ => 44_100,
                    };
                    let ch = if ch_mode == 3 { 1 } else { 2 };
                    if version_bits == 3 && layer_bits == 1 && (1..=14).contains(&br_idx) {
                        const MPEG1_L3_KBPS: [i32; 15] = [
                            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
                        ];
                        out.bitrate_kbps = MPEG1_L3_KBPS[br_idx];
                    }
                    out.codec = "MP3".to_string();
                    out.sample_rate_hz = sr;
                    out.bit_depth = 16;
                    out.channels = ch;
                    return out;
                }
            }
        }
    }

    // Fallback by extension hint
    match ext_lower.as_str() {
        "flac" => {
            out.codec = "FLAC".to_string();
            out.sample_rate_hz = 48_000;
            out.bit_depth = 24;
            out.channels = 2;
        }
        "ogg" | "oga" => {
            out.codec = "OGG Vorbis".to_string();
            out.sample_rate_hz = 44_100;
            out.bit_depth = 16;
            out.channels = 2;
        }
        "opus" => {
            out.codec = "Opus".to_string();
            out.sample_rate_hz = 48_000;
            out.bit_depth = 16;
            out.channels = 2;
        }
        "m4a" | "aac" => {
            out.codec = "AAC".to_string();
            out.sample_rate_hz = 44_100;
            out.bit_depth = 16;
            out.channels = 2;
        }
        "wav" => {
            out.codec = "WAV".to_string();
            out.sample_rate_hz = 44_100;
            out.bit_depth = 16;
            out.channels = 2;
        }
        _ => {
            out.codec = if ext_lower.is_empty() {
                "MP3".to_string()
            } else {
                ext_lower.to_uppercase()
            };
            out.sample_rate_hz = 44_100;
            out.bit_depth = 16;
            out.channels = 2;
        }
    }
    out
}

pub fn detect_image_mime_type(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 4
        && bytes[0] == 0x89
        && bytes[1] == 0x50
        && bytes[2] == 0x4E
        && bytes[3] == 0x47
    {
        "image/png"
    } else {
        "image/jpeg"
    }
}

pub fn embed_metadata(path: &Path, meta: &AudioTagMetadata, fallback_ext: &str) -> bool {
    let Ok(file_meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if file_meta.file_type().is_symlink()
        || !file_meta.is_file()
        || file_meta.len() > MAX_METADATA_REWRITE_BYTES
    {
        return false;
    }
    match detect_format(path, fallback_ext) {
        AudioContainerFormat::Flac => embed_flac_metadata(path, meta).unwrap_or(false),
        AudioContainerFormat::Mp3 => embed_mp3_id3v23_metadata(path, meta).unwrap_or(false),
        AudioContainerFormat::Ogg => embed_ogg_metadata(path, meta).unwrap_or(false),
        AudioContainerFormat::Other => false,
    }
}

struct FlacBlock {
    block_type: u8,
    data: Vec<u8>,
}

fn apply_common_vorbis_comments(
    existing_comments: &mut BTreeMap<String, String>,
    meta: &AudioTagMetadata,
) {
    if !meta.title.trim().is_empty() {
        existing_comments.insert("TITLE".to_string(), meta.title.trim().to_string());
    }
    if !meta.artist.trim().is_empty() {
        existing_comments.insert("ARTIST".to_string(), meta.artist.trim().to_string());
        existing_comments.insert("ALBUMARTIST".to_string(), meta.artist.trim().to_string());
    }
    if !meta.album.trim().is_empty() {
        existing_comments.insert("ALBUM".to_string(), meta.album.trim().to_string());
    }
    if !meta.publish_date.trim().is_empty() {
        let dt = meta.publish_date.trim();
        existing_comments.insert("DATE".to_string(), dt.to_string());
        existing_comments.insert("YEAR".to_string(), dt.chars().take(4).collect());
    }

    let mut comment_parts = Vec::new();
    if !meta.quality_label.trim().is_empty() {
        comment_parts.push(meta.quality_label.trim().to_string());
    }
    if !meta.song_hash.trim().is_empty() {
        comment_parts.push(format!("Hash:{}", meta.song_hash.trim().to_uppercase()));
    }
    if !comment_parts.is_empty() {
        existing_comments.insert("COMMENT".to_string(), comment_parts.join(" | "));
    }
    if !meta.lrc_text.trim().is_empty() {
        existing_comments.insert("LYRICS".to_string(), meta.lrc_text.clone());
        existing_comments.insert("UNSYNCEDLYRICS".to_string(), meta.lrc_text.clone());
    }
}

fn embed_flac_metadata(path: &Path, meta: &AudioTagMetadata) -> Result<bool, String> {
    let raw_bytes = fs::read(path).map_err(|e| e.to_string())?;
    if raw_bytes.len() < 8 || &raw_bytes[0..4] != b"fLaC" {
        return Ok(false);
    }

    let mut kept_blocks: Vec<FlacBlock> = Vec::new();
    let mut existing_comments: BTreeMap<String, String> = BTreeMap::new();
    let mut vendor_string = String::from("BreezeMusic Kugou Concept");
    let mut offset = 4usize;
    let mut is_last_block = false;

    while !is_last_block && offset + 4 <= raw_bytes.len() {
        let header_byte = raw_bytes[offset];
        is_last_block = (header_byte & 0x80) != 0;
        let block_type = header_byte & 0x7F;
        let length = ((raw_bytes[offset + 1] as usize) << 16)
            | ((raw_bytes[offset + 2] as usize) << 8)
            | (raw_bytes[offset + 3] as usize);
        offset += 4;
        if offset + length > raw_bytes.len() {
            return Ok(false);
        }
        let block_data = raw_bytes[offset..offset + length].to_vec();
        offset += length;

        match block_type {
            4 => {
                let (parsed_vendor, parsed_map) =
                    parse_vorbis_comment_block_with_vendor(&block_data);
                if !parsed_vendor.trim().is_empty() {
                    vendor_string = parsed_vendor;
                }
                for (k, v) in parsed_map {
                    existing_comments.insert(k, v);
                }
            }
            6 => {
                if meta.cover_bytes.as_ref().map(|b| b.is_empty()).unwrap_or(true) {
                    kept_blocks.push(FlacBlock {
                        block_type,
                        data: block_data,
                    });
                }
            }
            1 => {
                // Skip old padding block
            }
            _ => {
                kept_blocks.push(FlacBlock {
                    block_type,
                    data: block_data,
                });
            }
        }
    }

    let audio_frames_start = offset;
    if audio_frames_start > raw_bytes.len() {
        return Ok(false);
    }

    apply_common_vorbis_comments(&mut existing_comments, meta);

    let vorbis_data = build_vorbis_comment_block(&vendor_string, &existing_comments);
    if vorbis_data.len() <= 0x00FF_FFFF {
        kept_blocks.push(FlacBlock {
            block_type: 4,
            data: vorbis_data,
        });
    }

    if let Some(ref cover) = meta.cover_bytes {
        if !cover.is_empty() {
            let mime = detect_image_mime_type(cover);
            let pic_data = build_flac_picture_block(cover, mime, "Cover");
            if pic_data.len() <= 0x00FF_FFFF {
                kept_blocks.push(FlacBlock {
                    block_type: 6,
                    data: pic_data,
                });
            }
        }
    }

    let valid_blocks: Vec<&FlacBlock> = kept_blocks
        .iter()
        .filter(|b| b.data.len() <= 0x00FF_FFFF)
        .collect();
    if valid_blocks.is_empty() {
        return Ok(false);
    }

    let mut out = Vec::with_capacity(raw_bytes.len() + 65536);
    out.extend_from_slice(b"fLaC");
    let last_idx = valid_blocks.len().saturating_sub(1);
    for (i, blk) in valid_blocks.iter().enumerate() {
        let is_last = i == last_idx;
        let first_byte = (if is_last { 0x80 } else { 0x00 }) | (blk.block_type & 0x7F);
        let len = blk.data.len();
        out.push(first_byte);
        out.push(((len >> 16) & 0xFF) as u8);
        out.push(((len >> 8) & 0xFF) as u8);
        out.push((len & 0xFF) as u8);
        out.extend_from_slice(&blk.data);
    }
    out.extend_from_slice(&raw_bytes[audio_frames_start..]);
    write_file_atomically(path, &out)?;
    Ok(true)
}

fn parse_vorbis_comment_block_with_vendor(data: &[u8]) -> (String, BTreeMap<String, String>) {
    let mut map = BTreeMap::new();
    if data.len() < 8 {
        return (String::new(), map);
    }
    let vendor_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if 4 + vendor_len + 4 > data.len() {
        return (String::new(), map);
    }
    let vendor = std::str::from_utf8(&data[4..4 + vendor_len])
        .unwrap_or("")
        .to_string();
    let mut pos = 4 + vendor_len;
    let count =
        u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
    pos += 4;
    for _ in 0..count.min(2000) {
        if pos + 4 > data.len() {
            break;
        }
        let item_len =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if pos + item_len > data.len() {
            break;
        }
        if let Ok(s) = std::str::from_utf8(&data[pos..pos + item_len]) {
            if let Some(eq_idx) = s.find('=') {
                if eq_idx > 0 {
                    map.insert(s[..eq_idx].to_uppercase(), s[eq_idx + 1..].to_string());
                }
            }
        }
        pos += item_len;
    }
    (vendor, map)
}

fn build_vorbis_comment_block(vendor: &str, comments: &BTreeMap<String, String>) -> Vec<u8> {
    let mut out = Vec::new();
    let vendor_bytes = vendor.as_bytes();
    out.extend_from_slice(&(vendor_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(vendor_bytes);

    let entries: Vec<Vec<u8>> = comments
        .iter()
        .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
        .map(|(k, v)| format!("{}={}", k.to_uppercase(), v).into_bytes())
        .collect();

    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in entries {
        out.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        out.extend_from_slice(&entry);
    }
    out
}

fn build_flac_picture_block(image_bytes: &[u8], mime_type: &str, description: &str) -> Vec<u8> {
    let mime_bytes = mime_type.as_bytes();
    let desc_bytes = description.as_bytes();
    let mut out = Vec::with_capacity(32 + mime_bytes.len() + desc_bytes.len() + image_bytes.len());
    out.extend_from_slice(&3u32.to_be_bytes()); // Picture type 3: Front Cover
    out.extend_from_slice(&(mime_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(mime_bytes);
    out.extend_from_slice(&(desc_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(desc_bytes);
    out.extend_from_slice(&0u32.to_be_bytes()); // width
    out.extend_from_slice(&0u32.to_be_bytes()); // height
    out.extend_from_slice(&24u32.to_be_bytes()); // color depth
    out.extend_from_slice(&0u32.to_be_bytes()); // indexed colors
    out.extend_from_slice(&(image_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(image_bytes);
    out
}

fn decode_synchsafe_u32(b: &[u8]) -> usize {
    if b.len() < 4 {
        return 0;
    }
    (((b[0] & 0x7F) as usize) << 21)
        | (((b[1] & 0x7F) as usize) << 14)
        | (((b[2] & 0x7F) as usize) << 7)
        | ((b[3] & 0x7F) as usize)
}

fn is_valid_id3_frame_id(id: &[u8]) -> bool {
    id.len() == 4
        && id
            .iter()
            .all(|&c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// 解析旧 ID3v2.3 / ID3v2.4 标签中未被本次写入覆盖的扩展帧（如 TRCK / TPOS / TCON / TCOM / TXXX 等）
fn extract_preserved_id3v2_frames(
    tag_slice: &[u8],
    ver_major: u8,
    flags: u8,
    overwritten_ids: &HashSet<[u8; 4]>,
) -> Vec<([u8; 4], Vec<u8>)> {
    let mut preserved = Vec::new();
    // 若开启了全局 unsynchronisation (0x80) 或版本非 2.3/2.4，则跳过旧帧提取以避免解码失真
    if (flags & 0x80) != 0 || (ver_major != 3 && ver_major != 4) || tag_slice.len() < 10 {
        return preserved;
    }

    let mut pos = 0usize;
    // 处理 Extended Header (0x40)
    if (flags & 0x40) != 0 {
        if tag_slice.len() < 4 {
            return preserved;
        }
        let ext_size = if ver_major == 4 {
            decode_synchsafe_u32(&tag_slice[0..4])
        } else {
            u32::from_be_bytes([tag_slice[0], tag_slice[1], tag_slice[2], tag_slice[3]]) as usize
                + 4
        };
        if ext_size == 0 || ext_size > tag_slice.len() {
            return preserved;
        }
        pos += ext_size;
    }

    while pos + 10 <= tag_slice.len() {
        let frame_id_slice = &tag_slice[pos..pos + 4];
        if frame_id_slice[0] == 0 {
            break; // 遇到 Padding 零字节区域
        }
        if !is_valid_id3_frame_id(frame_id_slice) {
            break;
        }
        let frame_size = if ver_major == 4 {
            decode_synchsafe_u32(&tag_slice[pos + 4..pos + 8])
        } else {
            u32::from_be_bytes([
                tag_slice[pos + 4],
                tag_slice[pos + 5],
                tag_slice[pos + 6],
                tag_slice[pos + 7],
            ]) as usize
        };
        let format_flags = tag_slice[pos + 9];
        pos += 10;

        if frame_size == 0 || pos + frame_size > tag_slice.len() {
            break;
        }

        let mut id_arr = [0u8; 4];
        id_arr.copy_from_slice(frame_id_slice);
        // 仅保留未加密、未压缩（format_flags == 0）且不在本次覆盖集合中的标准帧
        if format_flags == 0 && !overwritten_ids.contains(&id_arr) {
            preserved.push((id_arr, tag_slice[pos..pos + frame_size].to_vec()));
        }
        pos += frame_size;
    }

    preserved
}

fn embed_mp3_id3v23_metadata(path: &Path, meta: &AudioTagMetadata) -> Result<bool, String> {
    let raw_bytes = fs::read(path).map_err(|e| e.to_string())?;
    if raw_bytes.is_empty() {
        return Ok(false);
    }

    let mut overwritten_ids: HashSet<[u8; 4]> = HashSet::new();
    if !meta.title.trim().is_empty() {
        overwritten_ids.insert(*b"TIT2");
    }
    if !meta.artist.trim().is_empty() {
        overwritten_ids.insert(*b"TPE1");
        overwritten_ids.insert(*b"TPE2");
    }
    if !meta.album.trim().is_empty() {
        overwritten_ids.insert(*b"TALB");
    }
    if !meta.publish_date.trim().is_empty() {
        overwritten_ids.insert(*b"TYER");
        overwritten_ids.insert(*b"TDAT");
        overwritten_ids.insert(*b"TDRC");
    }
    if !meta.quality_label.trim().is_empty() || !meta.song_hash.trim().is_empty() {
        overwritten_ids.insert(*b"COMM");
    }
    if !meta.lrc_text.trim().is_empty() {
        overwritten_ids.insert(*b"USLT");
    }
    if meta.cover_bytes.as_ref().map(|b| !b.is_empty()).unwrap_or(false) {
        overwritten_ids.insert(*b"APIC");
    }

    let mut audio_start_offset = 0usize;
    let mut preserved_frames: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    if raw_bytes.len() >= 10 && &raw_bytes[0..3] == b"ID3" {
        let ver_major = raw_bytes[3];
        let flags = raw_bytes[5];
        let synchsafe_size = decode_synchsafe_u32(&raw_bytes[6..10]);
        let footer_size = if (flags & 0x10) != 0 { 10 } else { 0 };
        let total_old_id3 = 10 + synchsafe_size + footer_size;
        if (10..=raw_bytes.len()).contains(&total_old_id3) {
            let tag_body_end = (10 + synchsafe_size).min(raw_bytes.len());
            preserved_frames = extract_preserved_id3v2_frames(
                &raw_bytes[10..tag_body_end],
                ver_major,
                flags,
                &overwritten_ids,
            );
            audio_start_offset = total_old_id3;
        }
    }

    let mut frames_out: Vec<u8> = Vec::new();
    let mut write_frame = |frame_id: &[u8; 4], payload: &[u8]| {
        if payload.is_empty() {
            return;
        }
        frames_out.extend_from_slice(frame_id);
        frames_out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frames_out.extend_from_slice(&[0u8, 0u8]);
        frames_out.extend_from_slice(payload);
    };

    let encode_utf16le_with_bom = |text: &str| -> Vec<u8> {
        let mut v = vec![0xFF, 0xFE];
        for code_unit in text.encode_utf16() {
            v.extend_from_slice(&code_unit.to_le_bytes());
        }
        v
    };

    let build_text_payload = |text: &str| -> Vec<u8> {
        let mut v = vec![1u8]; // UTF-16 with BOM
        v.extend_from_slice(&encode_utf16le_with_bom(text));
        v
    };

    if !meta.title.trim().is_empty() {
        write_frame(b"TIT2", &build_text_payload(meta.title.trim()));
    }
    if !meta.artist.trim().is_empty() {
        write_frame(b"TPE1", &build_text_payload(meta.artist.trim()));
        write_frame(b"TPE2", &build_text_payload(meta.artist.trim()));
    }
    if !meta.album.trim().is_empty() {
        write_frame(b"TALB", &build_text_payload(meta.album.trim()));
    }
    if !meta.publish_date.trim().is_empty() {
        let trimmed_date = meta.publish_date.trim();
        let year: String = trimmed_date.chars().take(4).collect();
        if year.len() == 4 && year.chars().all(|c| c.is_ascii_digit()) {
            write_frame(b"TYER", &build_text_payload(&year));
        }
        // ID3v2.3 标准使用 TDAT (DDMM 四位格式) 记录月日，而非 ID3v2.4 专属的 TDRC 帧
        let digits_only: String = trimmed_date
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        if digits_only.len() >= 8 {
            let mm = &digits_only[4..6];
            let dd = &digits_only[6..8];
            let tdat_val = format!("{}{}", dd, mm);
            write_frame(b"TDAT", &build_text_payload(&tdat_val));
        }
    }

    let mut comment_parts = Vec::new();
    if !meta.quality_label.trim().is_empty() {
        comment_parts.push(meta.quality_label.trim().to_string());
    }
    if !meta.song_hash.trim().is_empty() {
        comment_parts.push(format!("Hash:{}", meta.song_hash.trim().to_uppercase()));
    }
    if !comment_parts.is_empty() {
        let comment_text = comment_parts.join(" | ");
        let mut comm_payload = vec![1u8]; // UTF-16 with BOM
        comm_payload.extend_from_slice(b"eng");
        comm_payload.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x00]); // Empty UTF-16 desc
        comm_payload.extend_from_slice(&encode_utf16le_with_bom(&comment_text));
        write_frame(b"COMM", &comm_payload);
    }

    if !meta.lrc_text.trim().is_empty() {
        let mut uslt_payload = vec![1u8];
        uslt_payload.extend_from_slice(b"chi");
        uslt_payload.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x00]);
        uslt_payload.extend_from_slice(&encode_utf16le_with_bom(&meta.lrc_text));
        write_frame(b"USLT", &uslt_payload);
    }

    // 写回原文件中未被覆盖的扩展帧（如 TRCK / TPOS / TCON / TCOM / TXXX 等）
    for (frame_id, payload) in &preserved_frames {
        write_frame(frame_id, payload);
    }

    if let Some(ref cover) = meta.cover_bytes {
        if !cover.is_empty() {
            let mime = detect_image_mime_type(cover);
            let mut apic_payload = vec![0u8]; // ISO-8859-1
            apic_payload.extend_from_slice(mime.as_bytes());
            apic_payload.push(0);
            apic_payload.push(3); // Front Cover
            apic_payload.extend_from_slice(b"Cover");
            apic_payload.push(0);
            apic_payload.extend_from_slice(cover);
            write_frame(b"APIC", &apic_payload);
        }
    }

    let tag_size = frames_out.len();
    // 校验 ID3v2 28-bit Synchsafe Integer 上限 (0x0FFFFFFF = 256MB)
    if tag_size > 0x0FFF_FFFF {
        return Err("ID3v2 tag payload exceeds 256MB synchsafe integer limit".to_string());
    }

    let mut audio_end_offset = raw_bytes.len();
    // 若文件末尾包含旧版 128 字节定长 ID3v1 ("TAG") 块且我们已经写入完整标题，自动剥离防冲突
    if !meta.title.trim().is_empty()
        && audio_end_offset >= audio_start_offset + 128
        && &raw_bytes[audio_end_offset - 128..audio_end_offset - 125] == b"TAG"
    {
        audio_end_offset -= 128;
    }

    let mut out = Vec::with_capacity(10 + tag_size + (audio_end_offset - audio_start_offset));
    out.extend_from_slice(b"ID3");
    out.push(3); // v2.3
    out.push(0);
    out.push(0);
    out.push(((tag_size >> 21) & 0x7F) as u8);
    out.push(((tag_size >> 14) & 0x7F) as u8);
    out.push(((tag_size >> 7) & 0x7F) as u8);
    out.push((tag_size & 0x7F) as u8);
    out.extend_from_slice(&frames_out);
    out.extend_from_slice(&raw_bytes[audio_start_offset..audio_end_offset]);
    write_file_atomically(path, &out)?;
    Ok(true)
}

/// RFC 3533 Ogg 封装容器使用的直接型 MSB-first CRC-32 校验算法（生成多项式 0x04C11DB7，初值 0，无反射）
fn compute_ogg_page_crc32(page_bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    for (idx, &b) in page_bytes.iter().enumerate() {
        let byte = if (22..26).contains(&idx) { 0u8 } else { b };
        crc ^= (byte as u32) << 24;
        for _ in 0..8 {
            if (crc & 0x8000_0000) != 0 {
                crc = (crc << 1) ^ 0x04C1_1DB7;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[derive(Clone)]
struct ParsedOggPage {
    header_type: u8,
    granule_pos: i64,
    serial: u32,
    seq_no: u32,
    segments: Vec<u8>,
    body: Vec<u8>,
}

impl ParsedOggPage {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(27 + self.segments.len() + self.body.len());
        out.extend_from_slice(b"OggS");
        out.push(0); // stream_structure_version = 0
        out.push(self.header_type);
        out.extend_from_slice(&self.granule_pos.to_le_bytes());
        out.extend_from_slice(&self.serial.to_le_bytes());
        out.extend_from_slice(&self.seq_no.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // CRC placeholder
        out.push(self.segments.len() as u8);
        out.extend_from_slice(&self.segments);
        out.extend_from_slice(&self.body);
        let crc = compute_ogg_page_crc32(&out);
        out[22..26].copy_from_slice(&crc.to_le_bytes());
        out
    }
}

fn parse_ogg_pages(raw: &[u8]) -> Option<Vec<ParsedOggPage>> {
    let mut pages = Vec::new();
    let mut offset = 0usize;
    while offset < raw.len() {
        if offset + 27 > raw.len() || &raw[offset..offset + 4] != b"OggS" {
            return None;
        }
        let version = raw[offset + 4];
        if version != 0 {
            return None;
        }
        let header_type = raw[offset + 5];
        let granule_pos = i64::from_le_bytes([
            raw[offset + 6],
            raw[offset + 7],
            raw[offset + 8],
            raw[offset + 9],
            raw[offset + 10],
            raw[offset + 11],
            raw[offset + 12],
            raw[offset + 13],
        ]);
        let serial = u32::from_le_bytes([
            raw[offset + 14],
            raw[offset + 15],
            raw[offset + 16],
            raw[offset + 17],
        ]);
        let seq_no = u32::from_le_bytes([
            raw[offset + 18],
            raw[offset + 19],
            raw[offset + 20],
            raw[offset + 21],
        ]);
        let num_segments = raw[offset + 26] as usize;
        let seg_start = offset + 27;
        let seg_end = seg_start + num_segments;
        if seg_end > raw.len() {
            return None;
        }
        let segments = raw[seg_start..seg_end].to_vec();
        let body_len: usize = segments.iter().map(|&s| s as usize).sum();
        let body_end = seg_end + body_len;
        if body_end > raw.len() {
            return None;
        }
        let body = raw[seg_end..body_end].to_vec();
        pages.push(ParsedOggPage {
            header_type,
            granule_pos,
            serial,
            seq_no,
            segments,
            body,
        });
        offset = body_end;
    }
    if pages.is_empty() {
        None
    } else {
        Some(pages)
    }
}

/// 纯 Rust OGG Vorbis / Opus 注释包（\x03vorbis / OpusTags）与 METADATA_BLOCK_PICTURE 封面重打包引擎
fn embed_ogg_metadata(path: &Path, meta: &AudioTagMetadata) -> Result<bool, String> {
    let raw_bytes = fs::read(path).map_err(|e| e.to_string())?;
    let Some(pages) = parse_ogg_pages(&raw_bytes) else {
        return Ok(false);
    };
    if pages.len() < 2 {
        return Ok(false);
    }

    // 第 0 页为 BOS Identification Header (\x01vorbis 或 OpusHead)，从第 1 页起重组 Header Packets
    let stream_serial = pages[0].serial;
    let mut header_end_page_idx = 1usize;
    let mut packets: Vec<Vec<u8>> = Vec::new();
    let mut current_pkt = Vec::new();

    for (idx, page) in pages.iter().enumerate().skip(1) {
        header_end_page_idx = idx;
        let mut body_offset = 0usize;
        for &lacing in &page.segments {
            let seg_len = lacing as usize;
            current_pkt.extend_from_slice(&page.body[body_offset..body_offset + seg_len]);
            body_offset += seg_len;
            if lacing < 255 {
                packets.push(std::mem::take(&mut current_pkt));
            }
        }
        // 当首包（Comment Header）完整闭合且当前页没有未闭合的跨页包（如 Vorbis Setup Header）时即可停止扫描
        if !packets.is_empty() && current_pkt.is_empty() {
            break;
        }
    }

    if packets.is_empty() || !current_pkt.is_empty() {
        return Ok(false);
    }

    let first_pkt = &packets[0];
    let (prefix_bytes, has_framing_bit, comment_slice) =
        if first_pkt.len() >= 7 && &first_pkt[0..7] == b"\x03vorbis" {
            let end = if first_pkt.last() == Some(&1) {
                first_pkt.len() - 1
            } else {
                first_pkt.len()
            };
            (&b"\x03vorbis"[..], true, &first_pkt[7..end])
        } else if first_pkt.len() >= 8 && &first_pkt[0..8] == b"OpusTags" {
            (&b"OpusTags"[..], false, &first_pkt[8..])
        } else {
            return Ok(false);
        };

    let (parsed_vendor, mut existing_comments) =
        parse_vorbis_comment_block_with_vendor(comment_slice);
    let vendor = if parsed_vendor.trim().is_empty() {
        "BreezeMusic Kugou Concept".to_string()
    } else {
        parsed_vendor
    };

    apply_common_vorbis_comments(&mut existing_comments, meta);

    if let Some(ref cover) = meta.cover_bytes {
        if !cover.is_empty() {
            let mime = detect_image_mime_type(cover);
            let pic_block = build_flac_picture_block(cover, mime, "Cover");
            existing_comments.insert(
                "METADATA_BLOCK_PICTURE".to_string(),
                BASE64_STD.encode(&pic_block),
            );
        }
    }

    let rebuilt_comment_body = build_vorbis_comment_block(&vendor, &existing_comments);
    let mut rebuilt_comment_pkt =
        Vec::with_capacity(prefix_bytes.len() + rebuilt_comment_body.len() + 1);
    rebuilt_comment_pkt.extend_from_slice(prefix_bytes);
    rebuilt_comment_pkt.extend_from_slice(&rebuilt_comment_body);
    if has_framing_bit {
        rebuilt_comment_pkt.push(1u8);
    }
    packets[0] = rebuilt_comment_pkt;

    // 将重构后的包序列（Comment Packet + 可能同页的 Setup Packet）转换为 255 字节 Lacing 分段并分页
    let mut all_lacing: Vec<(u8, Vec<u8>, bool)> = Vec::new();
    for pkt in &packets {
        let mut pos = 0usize;
        while pos + 255 <= pkt.len() {
            all_lacing.push((255u8, pkt[pos..pos + 255].to_vec(), false));
            pos += 255;
        }
        let rem = pkt.len() - pos;
        all_lacing.push((rem as u8, pkt[pos..].to_vec(), true));
    }

    let mut out_pages: Vec<ParsedOggPage> = Vec::new();
    out_pages.push(pages[0].clone());
    let mut next_seq_no = pages[0].seq_no + 1;
    let mut continued = false;

    for chunk in all_lacing.chunks(255) {
        let mut seg_table = Vec::with_capacity(chunk.len());
        let mut page_body = Vec::new();
        let mut any_pkt_completed = false;
        for (lacing, seg_bytes, completed) in chunk {
            seg_table.push(*lacing);
            page_body.extend_from_slice(seg_bytes);
            if *completed {
                any_pkt_completed = true;
            }
        }
        let header_type = if continued { 0x01 } else { 0x00 };
        let granule_pos = if any_pkt_completed { 0 } else { -1 };
        out_pages.push(ParsedOggPage {
            header_type,
            granule_pos,
            serial: stream_serial,
            seq_no: next_seq_no,
            segments: seg_table,
            body: page_body,
        });
        next_seq_no += 1;
        continued = chunk.last().map(|(l, _, _)| *l == 255).unwrap_or(false);
    }

    // 拼接后续音频数据页并重排连续的 page_sequence_number + 重算 CRC32
    for mut audio_page in pages.into_iter().skip(header_end_page_idx + 1) {
        audio_page.seq_no = next_seq_no;
        next_seq_no += 1;
        out_pages.push(audio_page);
    }

    let mut final_bytes = Vec::with_capacity(raw_bytes.len() + 65536);
    for p in out_pages {
        final_bytes.extend_from_slice(&p.encode());
    }
    write_file_atomically(path, &final_bytes)?;
    Ok(true)
}

/// 专业本地音频全量内嵌元数据解析结果（支持 FLAC/MP3/M4A/AAC/OGG/OPUS/WAV/AIFF/APE/WV）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalAudioParsedMeta {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub publish_date: String,
    pub publish_company: String,
    pub language: String,
    pub genre: String,
    pub lyricist: String,
    pub composer: String,
    pub arranger: String,
    pub codec: String,
    pub duration_ms: i64,
    pub sample_rate_hz: i32,
    pub bit_depth: i32,
    pub channels: i32,
    pub bitrate_kbps: i32,
    pub is_lossless: bool,
    pub is_hi_res: bool,
    pub has_cover: bool,
    pub replay_gain_db: Option<f32>,
}

const MAX_LOCAL_COVER_BYTES: usize = 10 * 1024 * 1024;

fn sniff_image_mime(bytes: &[u8], hinted: Option<&MimeType>) -> &'static str {
    if let Some(m) = hinted {
        match m {
            MimeType::Jpeg => return "image/jpeg",
            MimeType::Png => return "image/png",
            MimeType::Gif => return "image/gif",
            MimeType::Bmp => return "image/bmp",
            MimeType::Tiff => return "image/tiff",
            _ => {}
        }
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return "image/jpeg";
    }
    if bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" {
        return "image/png";
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    if bytes.len() >= 6 && (&bytes[0..6] == b"GIF89a" || &bytes[0..6] == b"GIF87a") {
        return "image/gif";
    }
    if bytes.len() >= 2 && &bytes[0..2] == b"BM" {
        return "image/bmp";
    }
    "image/jpeg"
}

fn find_sidecar_cover_path(audio_path: &Path) -> Option<std::path::PathBuf> {
    let parent = audio_path.parent()?;
    let exts = ["jpg", "jpeg", "png", "webp"];
    // 1. 优先同名外置封面（如 `周杰伦 - 晴天.jpg`）
    if let Some(stem) = audio_path.file_stem().and_then(|s| s.to_str()) {
        for ext in exts {
            let cand = parent.join(format!("{}.{}", stem, ext));
            if cand.is_file() {
                if let Ok(m) = fs::metadata(&cand) {
                    if m.len() > 64 && (m.len() as usize) <= MAX_LOCAL_COVER_BYTES {
                        return Some(cand);
                    }
                }
            }
        }
    }
    // 2. 回退专辑目录通用封面（cover / folder / front / album）
    for base in ["cover", "folder", "front", "album", "Cover", "Folder", "Front"] {
        for ext in exts {
            let cand = parent.join(format!("{}.{}", base, ext));
            if cand.is_file() {
                if let Ok(m) = fs::metadata(&cand) {
                    if m.len() > 64 && (m.len() as usize) <= MAX_LOCAL_COVER_BYTES {
                        return Some(cand);
                    }
                }
            }
        }
    }
    None
}

fn pick_tag_string(tags: &[&lofty::tag::Tag], keys: &[ItemKey]) -> String {
    for tag in tags {
        for key in keys {
            if let Some(val) = tag.get_string(*key) {
                let t = val.trim();
                if !t.is_empty() {
                    return t.to_string();
                }
            }
        }
    }
    String::new()
}

fn pick_tag_joined_artists(tags: &[&lofty::tag::Tag]) -> String {
    for tag in tags {
        let mut names: Vec<String> = Vec::new();
        for val in tag.get_strings(ItemKey::TrackArtist) {
            let cleaned = val.trim();
            if !cleaned.is_empty() && !names.iter().any(|x| x == cleaned) {
                names.push(cleaned.to_string());
            }
        }
        if !names.is_empty() {
            return names.join("、");
        }
        if let Some(a) = tag.artist() {
            let cleaned = a.trim();
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }
    }
    pick_tag_string(tags, &[ItemKey::AlbumArtist])
}

fn parse_replay_gain_str(raw: &str) -> Option<f32> {
    let cleaned: String = raw
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '+' || *c == '.')
        .collect();
    let v = cleaned.parse::<f32>().ok()?;
    if (-18.0..=12.0).contains(&v) {
        Some(v.clamp(-12.0, 8.0))
    } else {
        None
    }
}

fn codec_and_lossless_from_file_type(ft: FileType, ext_lower: &str, bitrate_kbps: i32, bit_depth: i32) -> (String, bool) {
    match ft {
        FileType::Flac => ("FLAC".to_string(), true),
        FileType::Wav => ("WAV".to_string(), true),
        FileType::Aiff => ("AIFF".to_string(), true),
        FileType::Ape => ("APE".to_string(), true),
        FileType::WavPack => ("WV".to_string(), true),
        FileType::Mpeg => ("MP3".to_string(), false),
        FileType::Opus => ("OPUS".to_string(), false),
        FileType::Vorbis => ("OGG".to_string(), false),
        FileType::Mp4 => {
            // M4A 既可能是 AAC 有损，也可能是 ALAC 无损（有 bit_depth 或极高码率）
            let is_alac = ext_lower == "alac" || bit_depth >= 16 && bitrate_kbps >= 600;
            if is_alac {
                ("ALAC".to_string(), true)
            } else {
                ("AAC".to_string(), false)
            }
        }
        FileType::Aac => ("AAC".to_string(), false),
        _ => match ext_lower {
            "flac" => ("FLAC".to_string(), true),
            "wav" => ("WAV".to_string(), true),
            "alac" => ("ALAC".to_string(), true),
            "aiff" | "aif" => ("AIFF".to_string(), true),
            "ape" => ("APE".to_string(), true),
            "wv" => ("WV".to_string(), true),
            "ogg" | "oga" => ("OGG".to_string(), false),
            "opus" => ("OPUS".to_string(), false),
            "m4a" | "aac" | "mp4" => ("AAC".to_string(), false),
            _ => ("MP3".to_string(), false),
        },
    }
}

/// 完整解析本地音频文件的全部内嵌标签、技术流参数、ReplayGain 以及是否存在封面
pub fn read_local_audio_full_metadata(path: &Path) -> LocalAudioParsedMeta {
    let mut out = LocalAudioParsedMeta::default();
    let ext_lower = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3")
        .to_lowercase();
    let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let mut probed_ok = false;
    if let Ok(probe) = Probe::open(path) {
        let probe_guessed = probe.guess_file_type().ok();
        if let Some(Ok(tagged)) = probe_guessed.map(|p| p.read()) {
            probed_ok = true;
            let props = tagged.properties();
            let dur_ms = props.duration().as_millis() as i64;
            let sr = props.sample_rate().unwrap_or(0) as i32;
            let bd = props.bit_depth().unwrap_or(0) as i32;
            let ch = props.channels().unwrap_or(2) as i32;
            let mut br = props
                .audio_bitrate()
                .or_else(|| props.overall_bitrate())
                .unwrap_or(0) as i32;
            if br <= 0 && dur_ms >= 3_000 && file_size > 0 {
                br = (((file_size as f64) * 8.0) / ((dur_ms as f64) / 1000.0) / 1000.0).round() as i32;
            }

            let (codec, is_lossless_container) =
                codec_and_lossless_from_file_type(tagged.file_type(), &ext_lower, br, bd);

            out.duration_ms = dur_ms;
            out.sample_rate_hz = if sr > 0 { sr } else { 44_100 };
            out.bit_depth = if bd > 0 { bd } else { 16 };
            out.channels = if ch > 0 { ch } else { 2 };
            out.bitrate_kbps = br;
            out.codec = codec;
            out.is_hi_res = is_lossless_container && (out.sample_rate_hz > 48_000 || out.bit_depth > 16);
            out.is_lossless = out.is_hi_res || is_lossless_container;

            // 聚合 primary_tag 与全部标签，避免 ID3v1 掩盖 ID3v2 或副标签漏读
            let mut ordered_tags: Vec<&lofty::tag::Tag> = Vec::new();
            if let Some(pt) = tagged.primary_tag() {
                ordered_tags.push(pt);
            }
            for t in tagged.tags() {
                if !ordered_tags.iter().any(|existing| std::ptr::eq(*existing, t)) {
                    ordered_tags.push(t);
                }
            }

            out.title = pick_tag_string(&ordered_tags, &[ItemKey::TrackTitle]);
            if out.title.is_empty() {
                if let Some(t) = ordered_tags.first().and_then(|tg| tg.title()) {
                    out.title = t.trim().to_string();
                }
            }
            out.artist = pick_tag_joined_artists(&ordered_tags);
            out.album = pick_tag_string(&ordered_tags, &[ItemKey::AlbumTitle]);
            if out.album.is_empty() {
                if let Some(a) = ordered_tags.first().and_then(|tg| tg.album()) {
                    out.album = a.trim().to_string();
                }
            }

            let raw_date = pick_tag_string(
                &ordered_tags,
                &[
                    ItemKey::RecordingDate,
                    ItemKey::ReleaseDate,
                    ItemKey::OriginalReleaseDate,
                    ItemKey::Year,
                ],
            );
            if !raw_date.is_empty() {
                out.publish_date = raw_date.chars().take(10).collect();
            }
            out.publish_company = pick_tag_string(&ordered_tags, &[ItemKey::Publisher, ItemKey::Label]);
            out.language = pick_tag_string(&ordered_tags, &[ItemKey::Language]);
            out.genre = pick_tag_string(&ordered_tags, &[ItemKey::Genre]);
            out.lyricist = pick_tag_string(&ordered_tags, &[ItemKey::Lyricist, ItemKey::Writer]);
            out.composer = pick_tag_string(&ordered_tags, &[ItemKey::Composer]);
            out.arranger = pick_tag_string(&ordered_tags, &[ItemKey::Arranger]);

            let rg_raw = pick_tag_string(&ordered_tags, &[ItemKey::ReplayGainTrackGain]);
            if !rg_raw.is_empty() {
                out.replay_gain_db = parse_replay_gain_str(&rg_raw);
            }

            let has_embedded_pic = ordered_tags
                .iter()
                .any(|t| t.pictures().iter().any(|p| p.data().len() > 64));
            out.has_cover = has_embedded_pic || find_sidecar_cover_path(path).is_some();
        }
    }

    // 若 lofty 遇到极少数非标准封装文件，回退至内置二进制头分析兜底
    if !probed_ok {
        let mut head_buf = vec![0u8; 65_536];
        if let Ok(mut f) = fs::File::open(path) {
            if let Ok(n) = std::io::Read::read(&mut f, &mut head_buf) {
                head_buf.truncate(n);
                let analysis = parse_stream_header_analysis(&head_buf, &ext_lower);
                out.codec = analysis.codec;
                out.sample_rate_hz = analysis.sample_rate_hz;
                out.bit_depth = analysis.bit_depth;
                out.channels = analysis.channels;
                out.bitrate_kbps = analysis.bitrate_kbps;
                out.publish_date = analysis.publish_date;
                out.publish_company = analysis.publish_company;
                out.language = analysis.language;
                out.genre = analysis.genre;
                out.lyricist = analysis.lyricist;
                out.composer = analysis.composer;
                out.arranger = analysis.arranger;
                out.replay_gain_db = analysis.replay_gain_db;
            }
        }
        let is_lossless_ext = matches!(ext_lower.as_str(), "flac" | "wav" | "alac" | "aiff" | "aif" | "ape" | "wv");
        out.is_hi_res = is_lossless_ext && (out.sample_rate_hz > 48_000 || out.bit_depth > 16);
        out.is_lossless = out.is_hi_res || is_lossless_ext;
        out.has_cover = find_sidecar_cover_path(path).is_some();
    }

    out
}

/// 提取本地音频内嵌封面（优先 CoverFront，其次任意内嵌图，最后回退同目录外置封面）
pub fn extract_local_cover_art(path: &Path) -> Option<(Vec<u8>, &'static str)> {
    if let Ok(probe) = Probe::open(path) {
        if let Ok(guessed) = probe.guess_file_type() {
            if let Ok(tagged) = guessed.read() {
                let mut all_pics: Vec<&lofty::picture::Picture> = Vec::new();
                if let Some(pt) = tagged.primary_tag() {
                    all_pics.extend(pt.pictures().iter());
                }
                for t in tagged.tags() {
                    for p in t.pictures() {
                        if !all_pics.iter().any(|e| std::ptr::eq(*e, p)) {
                            all_pics.push(p);
                        }
                    }
                }
                let chosen = all_pics
                    .iter()
                    .copied()
                    .find(|p| p.pic_type() == PictureType::CoverFront && p.data().len() > 64 && p.data().len() <= MAX_LOCAL_COVER_BYTES)
                    .or_else(|| {
                        all_pics
                            .iter()
                            .copied()
                            .find(|p| p.data().len() > 64 && p.data().len() <= MAX_LOCAL_COVER_BYTES)
                    });
                if let Some(pic) = chosen {
                    let bytes = pic.data().to_vec();
                    let mime = sniff_image_mime(&bytes, pic.mime_type());
                    return Some((bytes, mime));
                }
            }
        }
    }

    if let Some(sidecar) = find_sidecar_cover_path(path) {
        if let Ok(bytes) = fs::read(&sidecar) {
            if bytes.len() > 64 && bytes.len() <= MAX_LOCAL_COVER_BYTES {
                let mime = sniff_image_mime(&bytes, None);
                return Some((bytes, mime));
            }
        }
    }
    None
}

/// 读取音频文件内嵌歌词标签（ID3v2 USLT / FLAC & Ogg LYRICS / MP4 ©lyr）
pub fn extract_embedded_lyrics(path: &Path) -> Option<String> {
    let probe = Probe::open(path).ok()?;
    let guessed = probe.guess_file_type().ok()?;
    let tagged = guessed.read().ok()?;
    let mut ordered_tags: Vec<&lofty::tag::Tag> = Vec::new();
    if let Some(pt) = tagged.primary_tag() {
        ordered_tags.push(pt);
    }
    for t in tagged.tags() {
        if !ordered_tags.iter().any(|existing| std::ptr::eq(*existing, t)) {
            ordered_tags.push(t);
        }
    }
    let lyr = pick_tag_string(&ordered_tags, &[ItemKey::Lyrics]);
    if lyr.trim().is_empty() {
        None
    } else {
        Some(lyr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> AudioTagMetadata {
        AudioTagMetadata {
            title: "微风测试曲".to_string(),
            artist: "微风乐团".to_string(),
            album: "天际线专辑".to_string(),
            publish_date: "2026-11-01".to_string(),
            song_hash: "0123456789abcdef0123456789abcdef".to_string(),
            quality_label: "Hi-Res".to_string(),
            cover_bytes: Some(vec![0x89, 0x50, 0x4E, 0x47, 1, 2, 3, 4]),
            lrc_text: "[00:01.00]第一句歌词".to_string(),
        }
    }

    #[test]
    fn test_mp3_id3v23_preserves_existing_frames_and_strips_id3v1() {
        let tmp = std::env::temp_dir().join("breeze_test_id3v23.mp3");
        // 构造含 TRCK ("01/12") 与旧 TIT2 的 ID3v2.3 头部 + 模拟 MP3 帧 + 尾部 128 字节 ID3v1
        let mut trck_payload = vec![0u8]; // ISO-8859-1
        trck_payload.extend_from_slice(b"01/12");
        let mut old_frames = Vec::new();
        old_frames.extend_from_slice(b"TRCK");
        old_frames.extend_from_slice(&(trck_payload.len() as u32).to_be_bytes());
        old_frames.extend_from_slice(&[0u8, 0u8]);
        old_frames.extend_from_slice(&trck_payload);

        let mut old_tit2 = vec![0u8];
        old_tit2.extend_from_slice(b"OldTitle");
        old_frames.extend_from_slice(b"TIT2");
        old_frames.extend_from_slice(&(old_tit2.len() as u32).to_be_bytes());
        old_frames.extend_from_slice(&[0u8, 0u8]);
        old_frames.extend_from_slice(&old_tit2);

        let sz = old_frames.len();
        let mut raw = Vec::new();
        raw.extend_from_slice(b"ID3\x03\x00\x00");
        raw.push(((sz >> 21) & 0x7F) as u8);
        raw.push(((sz >> 14) & 0x7F) as u8);
        raw.push(((sz >> 7) & 0x7F) as u8);
        raw.push((sz & 0x7F) as u8);
        raw.extend_from_slice(&old_frames);
        raw.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x64, 0x00, 0x01, 0x02, 0x03]);
        let mut id3v1 = vec![0u8; 128];
        id3v1[0..3].copy_from_slice(b"TAG");
        raw.extend_from_slice(&id3v1);

        fs::write(&tmp, &raw).unwrap();
        assert!(embed_metadata(&tmp, &sample_meta(), "mp3"));

        let updated = fs::read(&tmp).unwrap();
        let _ = fs::remove_file(&tmp);
        assert_eq!(&updated[0..3], b"ID3");
        // 验证原有的 TRCK 音轨帧仍然被完整保留
        assert!(updated.windows(4).any(|w| w == b"TRCK"));
        assert!(updated.windows(5).any(|w| w == b"01/12"));
        // 验证尾部冗余 ID3v1 已被剥离，音频帧结尾保持干净
        assert_eq!(
            &updated[updated.len() - 8..],
            &[0xFF, 0xFB, 0x90, 0x64, 0x00, 0x01, 0x02, 0x03]
        );
    }

    #[test]
    fn test_ogg_vorbis_comment_and_picture_embedding() {
        let tmp = std::env::temp_dir().join("breeze_test_vorbis.ogg");
        // Page 0: BOS Identification header
        let page0 = ParsedOggPage {
            header_type: 0x02,
            granule_pos: 0,
            serial: 0x1234_5678,
            seq_no: 0,
            segments: vec![30],
            body: {
                let mut b = b"\x01vorbis".to_vec();
                b.resize(30, 0);
                b
            },
        };
        // Page 1: Comment header (\x03vorbis) + Setup header (\x05vorbis)
        let mut init_comments = BTreeMap::new();
        init_comments.insert("GENRE".to_string(), "Synthwave".to_string());
        let comment_block = build_vorbis_comment_block("Xiph.Org libVorbis", &init_comments);
        let mut pkt1 = b"\x03vorbis".to_vec();
        pkt1.extend_from_slice(&comment_block);
        pkt1.push(1);
        let pkt2 = b"\x05vorbis_setup_codebooks".to_vec();
        let mut page1_body = Vec::new();
        page1_body.extend_from_slice(&pkt1);
        page1_body.extend_from_slice(&pkt2);
        let page1 = ParsedOggPage {
            header_type: 0x00,
            granule_pos: 0,
            serial: 0x1234_5678,
            seq_no: 1,
            segments: vec![pkt1.len() as u8, pkt2.len() as u8],
            body: page1_body,
        };
        // Page 2: Audio data page
        let page2 = ParsedOggPage {
            header_type: 0x04,
            granule_pos: 44100,
            serial: 0x1234_5678,
            seq_no: 2,
            segments: vec![16],
            body: vec![0xAA; 16],
        };

        let mut raw = Vec::new();
        raw.extend_from_slice(&page0.encode());
        raw.extend_from_slice(&page1.encode());
        raw.extend_from_slice(&page2.encode());
        fs::write(&tmp, &raw).unwrap();

        assert!(embed_metadata(&tmp, &sample_meta(), "ogg"));
        let updated = fs::read(&tmp).unwrap();
        let _ = fs::remove_file(&tmp);

        let reparsed = parse_ogg_pages(&updated).expect("updated OGG pages should be valid");
        assert!(reparsed.len() >= 3);
        // 校验每一页 CRC32 均合法且原有 GENRE 与新写入的 METADATA_BLOCK_PICTURE 均存在
        let full_str = String::from_utf8_lossy(&updated);
        assert!(full_str.contains("GENRE=Synthwave"));
        assert!(full_str.contains("TITLE=微风测试曲"));
        assert!(full_str.contains("METADATA_BLOCK_PICTURE="));
    }

    #[test]
    fn test_parse_stream_header_analysis_flac_and_mp3() {
        // FLAC: 96000Hz (0x17700), 2 channels (bits 001), 24-bit (23 = 0x17) + Vorbis comment block
        let mut flac = Vec::new();
        flac.extend_from_slice(b"fLaC");
        // Block 0: STREAMINFO (34 bytes, not last)
        flac.push(0x00);
        flac.extend_from_slice(&[0, 0, 34]);
        let mut si = [0u8; 34];
        // bytes 10..14 encode SR(20b) CH-1(3b) BD-1(5b)
        // SR=96000 (0x17700) -> b10=0x17, b11=0x70, b12 upper4=0x00
        // CH=2 -> ch-1=1 -> <<1 = 0x02; BD=24 -> bd-1=23 (0x17) -> top bit 1 in b12 bit0, lower4 0x7 in b13 upper4
        si[10] = 0x17;
        si[11] = 0x70;
        si[12] = 0x03;
        si[13] = 0x70;
        flac.extend_from_slice(&si);

        let mut comments = BTreeMap::new();
        comments.insert("PUBLISHER".to_string(), "JVR Music".to_string());
        comments.insert("GENRE".to_string(), "Pop".to_string());
        comments.insert("LANGUAGE".to_string(), "国语".to_string());
        comments.insert("LYRICIST".to_string(), "方文山".to_string());
        comments.insert("COMPOSER".to_string(), "周杰伦".to_string());
        comments.insert("ARRANGER".to_string(), "钟兴民".to_string());
        let vc = build_vorbis_comment_block("reference", &comments);
        flac.push(0x84); // last block, type 4
        let vlen = vc.len();
        flac.push(((vlen >> 16) & 0xFF) as u8);
        flac.push(((vlen >> 8) & 0xFF) as u8);
        flac.push((vlen & 0xFF) as u8);
        flac.extend_from_slice(&vc);

        let spec = parse_stream_header_analysis(&flac, "flac");
        assert_eq!(spec.codec, "FLAC");
        assert_eq!(spec.sample_rate_hz, 96_000);
        assert_eq!(spec.bit_depth, 24);
        assert_eq!(spec.channels, 2);
        assert_eq!(spec.publish_company, "JVR Music");
        assert_eq!(spec.genre, "Pop");
        assert_eq!(spec.language, "国语");
        assert_eq!(spec.lyricist, "方文山");
        assert_eq!(spec.composer, "周杰伦");
        assert_eq!(spec.arranger, "钟兴民");
    }
}
