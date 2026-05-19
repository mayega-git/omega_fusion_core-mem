use std::fs;
use std::process::Command;
use crate::remote_ram::common::{RamDistante, REMOTE_RAM_PATH};

pub fn connecter(donneur_ip: &str, export_id: &str, taille_mb: u64) -> Result<RamDistante, String> {
    let chemin_distant = format!("/mnt/shared-ram/{}", export_id);
    let chemin_local = format!("{}/{}", REMOTE_RAM_PATH, export_id);
    println!("[ram-client] 1/2 mkdir {}", chemin_local);
    fs::create_dir_all(&chemin_local).map_err(|e| format!("mkdir échoué : {}", e))?;
    println!("[ram-client] 2/2 mount NFS {}:{}", donneur_ip, chemin_distant);
    let source = format!("{}:{}", donneur_ip, chemin_distant);
    run("mount", &["-t", "nfs", &source, &chemin_local])?;
    let chemin_fichier = format!("{}/data", chemin_local);
    println!("[ram-client] RAM distante montée : {}", chemin_local);
    Ok(RamDistante { id: export_id.to_string(), donneur_ip: donneur_ip.to_string(), chemin_local, chemin_fichier, taille_mb })
}

pub fn deconnecter(distant: &RamDistante) -> Result<(), String> {
    let _ = run("umount", &[&distant.chemin_local]);
    let _ = fs::remove_dir_all(&distant.chemin_local);
    println!("[ram-client] Nettoyé : {}", distant.id);
    Ok(())
}

pub fn chemin_mmap(distant: &RamDistante) -> String {
    distant.chemin_fichier.clone()
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(cmd).args(args).output().map_err(|e| format!("{} impossible : {}", cmd, e))?;
    if output.status.success() { Ok(()) } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} échoué : {}", cmd, stderr.trim()))
    }
}
