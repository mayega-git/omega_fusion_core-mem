use std::fs;
use std::process::Command;
use crate::remote_ram::common::{RamExport, SHARED_RAM_PATH};

pub fn exporter(id: &str, taille_mb: u64) -> Result<RamExport, String> {
    let chemin = format!("{}/{}", SHARED_RAM_PATH, id);
    println!("[ram-server] 1/4 mkdir {}", chemin);
    fs::create_dir_all(&chemin).map_err(|e| format!("mkdir échoué : {}", e))?;
    println!("[ram-server] 2/4 mount tmpfs {}M", taille_mb);
    let taille_arg = format!("size={}M", taille_mb);
    run("mount", &["-t", "tmpfs", "-o", &taille_arg, "tmpfs", &chemin])?;
    println!("[ram-server] 3/4 création fichier {}M", taille_mb);
    let fichier = format!("{}/data", chemin);
    let count = taille_mb.to_string();
    run("dd", &["if=/dev/zero", &format!("of={}", fichier), "bs=1M", &format!("count={}", count)])?;
    println!("[ram-server] 4/4 export NFS");
    let export_line = format!("{} *(rw,sync,no_subtree_check,no_root_squash)\n", chemin);
    fs::OpenOptions::new().append(true).open("/etc/exports")
        .and_then(|mut f| { use std::io::Write; f.write_all(export_line.as_bytes()) })
        .map_err(|e| format!("Écriture exports échouée : {}", e))?;
    run("exportfs", &["-ra"])?;
    println!("[ram-server] Export prêt : {}", chemin);
    Ok(RamExport { id: id.to_string(), taille_mb, chemin })
}

pub fn arreter(export: &RamExport) -> Result<(), String> {
    if let Ok(contenu) = fs::read_to_string("/etc/exports") {
        let nouveau: String = contenu.lines().filter(|l| !l.contains(&export.chemin)).collect::<Vec<&str>>().join("\n");
        let _ = fs::write("/etc/exports", nouveau + "\n");
    }
    let _ = run("exportfs", &["-ra"]);
    let _ = run("umount", &[&export.chemin]);
    let _ = fs::remove_dir_all(&export.chemin);
    println!("[ram-server] Nettoyé : {}", export.id);
    Ok(())
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(cmd).args(args).output().map_err(|e| format!("{} impossible : {}", cmd, e))?;
    if output.status.success() { Ok(()) } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} échoué : {}", cmd, stderr.trim()))
    }
}
