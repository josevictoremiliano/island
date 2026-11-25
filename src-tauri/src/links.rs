//! Your own sites and programs in the island: a click opens them.

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
