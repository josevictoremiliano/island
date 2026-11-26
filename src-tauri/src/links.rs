//! Your own sites and programs in the island: a click opens them.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use std::collections::HashMap;
use std::sync::Mutex;

/// A web address (youtube.com, https://…) or a program / file path.
pub fn is_web(target: &str) -> bool {
    let t = target.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        return true;
    }
    // "youtube.com", "mail.google.com/x": a dot, no backslash and no drive letter
    !t.contains('\\') && !t.contains(':') && t.contains('.') && !t.ends_with(".exe") && !t.ends_with(".lnk")
}

pub fn normalize(target: &str) -> String {
    let t = target.trim();
    if is_web(t) && !t.starts_with("http") {
        format!("https://{t}")
    } else {
        t.to_string()
    }
}

pub fn open(target: &str) -> Result<(), String> {
    let t = normalize(target);
    if t.is_empty() {
        return Err("Endereço vazio.".into());
    }
    if !is_web(&t) && !std::path::Path::new(&t).exists() {
        return Err("Esse programa não existe mais nesse lugar.".into());
    }
    // explorer opens a web address in the default browser and a program as if double-clicked
    std::process::Command::new("explorer.exe").arg(&t).spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ---- icons ----

static ICONS: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

/// The icon of a site or program as a data URL (None: the island shows the first letter).
/// Cached for the whole run, failures too, so a broken site is asked only once.
pub async fn icon(target: &str) -> Option<String> {
    let t = normalize(target);
    if let Some(hit) = ICONS.lock().ok()?.get_or_insert_with(HashMap::new).get(&t) {
        return hit.clone();
    }
    let found = if is_web(&t) {
        favicon(&t).await
    } else {
        let p = t.clone();
        tauri::async_runtime::spawn_blocking(move || program_icon(&p)).await.ok().flatten()
    };
    ICONS.lock().ok()?.get_or_insert_with(HashMap::new).insert(t, found.clone());
    found
}

/// Asks the site itself: the icon its page names (the biggest kind first), else /favicon.ico.
async fn favicon(url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Ilha")
        .build()
        .ok()?;
    let page = client.get(url).send().await.ok()?;
    let base = page.url().clone();
    let html = page.text().await.unwrap_or_default();
    let mut tries: Vec<reqwest::Url> = icon_links(&html).iter().filter_map(|h| base.join(h).ok()).collect();
    if let Ok(u) = base.join("/favicon.ico") {
        tries.push(u);
    }
    for u in tries {
        let Ok(r) = client.get(u).send().await else { continue };
        if !r.status().is_success() {
            continue;
        }
        let ct = r.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
        if ct.starts_with("text/html") {
            continue;
        }
        let Ok(bytes) = r.bytes().await else { continue };
        if bytes.len() < 50 || bytes.len() > 600_000 {
            continue;
        }
        let mime = if ct.starts_with("image/") { ct.split(';').next().unwrap_or("image/png").trim().to_string() } else { sniff(&bytes).to_string() };
        return Some(format!("data:{mime};base64,{}", B64.encode(&bytes)));
    }
    None
}

fn sniff(b: &[u8]) -> &'static str {
    if b.starts_with(b"\x89PNG") {
        "image/png"
    } else if b.starts_with(b"\xff\xd8") {
        "image/jpeg"
    } else if b.starts_with(b"GIF8") {
        "image/gif"
    } else if b.starts_with(b"RIFF") {
        "image/webp"
    } else if b.starts_with(b"<svg") || b.starts_with(b"<?xml") {
        "image/svg+xml"
    } else {
        "image/x-icon"
    }
}

/// hrefs of <link rel="…icon…">, apple-touch-icon (big and sharp) first.
fn icon_links(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out: Vec<(u8, String)> = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find("<link") {
        let start = from + i;
        let end = lower[start..].find('>').map(|e| start + e).unwrap_or(lower.len());
        let (tag, ltag) = (&html[start..end], &lower[start..end]);
        from = end;
        let Some(rel) = attr(ltag, tag, "rel") else { continue };
        let rel = rel.to_ascii_lowercase();
        if !rel.contains("icon") || rel.contains("mask") {
            continue;
        }
        let Some(href) = attr(ltag, tag, "href") else { continue };
        let rank = if rel.contains("apple-touch") { 0 } else if href.ends_with(".svg") { 1 } else { 2 };
        out.push((rank, href.replace("&amp;", "&")));
    }
    out.sort_by_key(|(r, _)| *r);
    out.into_iter().map(|(_, h)| h).collect()
}

/// The value of attribute `name` in one tag (`lower_tag` is `tag` in ASCII lowercase, same byte offsets).
fn attr(lower_tag: &str, tag: &str, name: &str) -> Option<String> {
    let mut from = 0;
    loop {
        let i = from + lower_tag[from..].find(name)?;
        from = i + name.len();
        if !lower_tag[..i].ends_with(|c: char| c.is_whitespace()) {
            continue;
        }
        let rest = lower_tag[from..].trim_start();
        let Some(after_eq) = rest.strip_prefix('=') else { continue };
        let v = tag[tag.len() - after_eq.len()..].trim_start();
        let (body, end) = match v.chars().next()? {
            q @ ('"' | '\'') => (&v[1..], v[1..].find(q)?),
            _ => (v, v.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(v.len())),
        };
        return Some(body[..end].trim().to_string());
    }
}

/// The icon Windows shows for this program or shortcut (what a shortcut points to), as a PNG data URL.
#[cfg(windows)]
fn program_icon(path: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY};

    if !std::path::Path::new(path).exists() {
        return None;
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    const PX: i32 = 64;
    let (w, h, rgba) = unsafe {
        // the shell resolves .lnk files through COM; harmless if this thread already has it
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;
        let hbm = factory.GetImage(SIZE { cx: PX, cy: PX }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK).ok()?;
        let mut bm = BITMAP::default();
        let got = GetObjectW(hbm.into(), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut _));
        let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
        let px = if got != 0 && w > 0 && h > 0 && w <= 512 && h <= 512 {
            let mut bi = BITMAPINFO::default();
            bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = w;
            bi.bmiHeader.biHeight = -h; // top-down rows
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            bi.bmiHeader.biCompression = BI_RGB.0;
            let mut buf = vec![0u8; (w * h * 4) as usize];
            let dc = GetDC(None);
            let lines = GetDIBits(dc, hbm, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
            ReleaseDC(None, dc);
            (lines > 0).then_some(buf)
        } else {
            None
        };
        let _ = DeleteObject(hbm.into());
        let mut px = px?;
        let has_alpha = px.chunks(4).any(|p| p[3] != 0);
        for p in px.chunks_mut(4) {
            p.swap(0, 2); // BGRA -> RGBA
            if !has_alpha {
                p[3] = 255;
            } else if p[3] > 0 && p[3] < 255 {
                // the shell hands premultiplied colors
                let a = p[3] as u32;
                for c in &mut p[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        (w as u32, h as u32, px)
    };
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().ok()?;
        wr.write_image_data(&rgba).ok()?;
    }
    Some(format!("data:image/png;base64,{}", B64.encode(&png_bytes)))
}

#[cfg(not(windows))]
fn program_icon(_: &str) -> Option<String> {
    None
}

