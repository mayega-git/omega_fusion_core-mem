pub const SHARED_RAM_PATH: &str = "/mnt/shared-ram";
pub const REMOTE_RAM_PATH: &str = "/mnt/remote-ram";

pub struct RamExport {
    pub id: String,
    pub taille_mb: u64,
    pub chemin: String,
}

pub struct RamDistante {
    pub id: String,
    pub donneur_ip: String,
    pub chemin_local: String,
    pub chemin_fichier: String,
    pub taille_mb: u64,
}
