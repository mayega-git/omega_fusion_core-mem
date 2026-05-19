use std::process::Command;

/// Chemin de base où seront montés les tmpfs
pub const BASE_PATH: &str = "/mnt/swapram";

/// État possible d'un bloc de swap
pub enum SwapState {
    Inactif,
    Actif,
}

/// Représente un espace de swap en RAM
pub struct SwapBlock {
    pub id: String,
    pub taille_mb: u64,
    pub etat: SwapState,
    pub loop_device: Option<String>,
}

impl SwapBlock {
    /// Chemin du dossier tmpfs : /mnt/swapram/<id>
    pub fn chemin_tmpfs(&self) -> String {
        format!("{}/{}", BASE_PATH, self.id)
    }

    /// Chemin du fichier swap : /mnt/swapram/<id>/swapfile
    pub fn chemin_fichier(&self) -> String {
        format!("{}/{}/swapfile", BASE_PATH, self.id)
    }

    /// Vérifie si le bloc est actif
    pub fn est_actif(&self) -> bool {
        matches!(self.etat, SwapState::Actif)
    }

   /// Vérifie l'utilisation du swap de ce bloc
/// Retourne (utilisé_kb, total_kb) ou None si pas trouvé
pub fn utilisation(&self) -> Option<(u64, u64)> {
    let contenu = std::fs::read_to_string("/proc/swaps").ok()?;

    for line in contenu.lines().skip(1) { // skip l'en-tête
        // Chercher notre loop device dans la ligne
        if let Some(ref loop_dev) = self.loop_device {
            if line.starts_with(loop_dev) {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() >= 4 {
                    let total: u64 = cols[2].parse().unwrap_or(0);
                    let used: u64 = cols[3].parse().unwrap_or(0);
                    return Some((used, total));
                }
            }
        }
    }
    None
}

/// Pourcentage d'utilisation du swap
pub fn utilisation_pct(&self) -> f64 {
    match self.utilisation() {
        Some((used, total)) if total > 0 => (used as f64 / total as f64) * 100.0,
        _ => 0.0,
    }
}


}

/// Exécute une commande système et retourne Ok ou l'erreur
/// C'est la fonction utilitaire que create, activate, etc. utiliseront
pub fn run_cmd(programme: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(programme)
        .args(args)
        .output()
        .map_err(|e| format!("Impossible de lancer {} : {}", programme, e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} a échoué : {}", programme, stderr.trim()))
    }
}
