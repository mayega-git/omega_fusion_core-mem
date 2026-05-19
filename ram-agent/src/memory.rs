use std::fs;

/// Equivalent d'un dictionnaire Python — chaque champ est typé
pub struct MemInfo {
    pub total_kb: u64,
    pub free_kb: u64,
    pub available_kb: u64,
    pub buffers_kb: u64,
    pub cached_kb: u64,
    pub used_kb: u64,
    pub used_pct: f64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
    pub swap_used_kb: u64,
}


impl MemInfo {
    pub fn available_pct(&self) -> f64 {
        if self.total_kb == 0 { return 0.0; }
        (self.available_kb as f64 / self.total_kb as f64) * 100.0
    }
}


/// Lit /proc/meminfo et retourne une struct MemInfo
pub fn read_meminfo() -> Result<MemInfo, String> {
    let content = fs::read_to_string("/proc/meminfo")
        .map_err(|e| format!("Erreur lecture /proc/meminfo: {}", e))?;

    let total_kb = extract_field(&content, "MemTotal");
    let free_kb = extract_field(&content, "MemFree");
    let available_kb = extract_field(&content, "MemAvailable");
    let buffers_kb = extract_field(&content, "Buffers");
    let cached_kb = extract_field(&content, "Cached");
    let swap_total_kb = extract_field(&content, "SwapTotal");
    let swap_free_kb = extract_field(&content, "SwapFree");

    let used_kb = total_kb - free_kb - buffers_kb - cached_kb;
    let used_pct = if total_kb > 0 {
        (used_kb as f64 / total_kb as f64) * 100.0
    } else {
        0.0
    };

    Ok(MemInfo {
        total_kb,
        free_kb,
        available_kb,
        buffers_kb,
        cached_kb,
        used_kb,
        used_pct,
        swap_total_kb,
        swap_free_kb,
        swap_used_kb: swap_total_kb - swap_free_kb,
    })
}

fn extract_field(content: &str, field: &str) -> u64 {
    for line in content.lines() {
        if line.starts_with(field) {
            let value_part = match line.split(':').nth(1) {
                Some(v) => v,
                None => continue,
            };
            let number_str = value_part.trim().replace(" kB", "");
            return number_str.parse().unwrap_or(0);
        }
    }
    0
}
