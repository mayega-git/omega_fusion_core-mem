use std::fs;
use std::process::Command;

pub struct SwapExpose {
    pub swap_id: String,
    pub port_nbd: u16,
    pub receveur_ip: String,
    pub taille_mb: u64,
    pub chemin_fichier: String,
}

pub struct SwapDistant {
    pub swap_id: String,
    pub donneur_ip: String,
    pub port_nbd: u16,
    pub device: String,
    pub taille_mb: u64,
}

pub const BASE_PATH: &str = "/mnt/swapram";
pub const PORT_NBD_BASE: u16 = 10809;
pub const CONFIG_PATH: &str = "/etc/nbd-server";

/// Initialise l'environnement NBD au démarrage
/// Nettoie les restes d'une exécution précédente
pub fn init() {
    println!("[nbd] Initialisation...");

    // 1. Tuer les anciens nbd-server
    let _ = Command::new("pkill").arg("nbd-server").output();

    // 2. Déconnecter les nbd-client
    for i in 0..16 {
        let dev = format!("/dev/nbd{}", i);
        let _ = Command::new("swapoff").arg(&dev).output();
        let _ = Command::new("nbd-client").args(&["-d", &dev]).output();
    }

    // 3. Démonter les tmpfs
    if let Ok(entries) = fs::read_dir(BASE_PATH) {
        for entry in entries.flatten() {
            let path = entry.path();
            let _ = Command::new("umount").arg(path.to_str().unwrap_or("")).output();
        }
    }

    // 4. Nettoyer les fichiers
    let _ = fs::remove_dir_all(BASE_PATH);
    let _ = Command::new("rm").args(&["-f"]).arg(format!("{}/swap_*.conf", CONFIG_PATH)).output();

    // 5. Recréer les dossiers
    let _ = fs::create_dir_all(BASE_PATH);
    let _ = fs::create_dir_all(CONFIG_PATH);

    // 6. Charger le module nbd
    let _ = Command::new("modprobe").arg("nbd").output();

    println!("[nbd] Prêt");
}
