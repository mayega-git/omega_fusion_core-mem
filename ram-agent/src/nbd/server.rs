use std::fs;
use std::process::Command;
use crate::nbd::common::{SwapExpose, BASE_PATH, CONFIG_PATH};

pub fn exposer(swap_id: &str, taille_mb: u64, port: u16, receveur_ip: &str) -> Result<SwapExpose, String> {
    let chemin_dir = format!("{}/{}", BASE_PATH, swap_id);
    let chemin_fichier = format!("{}/swapfile", chemin_dir);
    let chemin_config = format!("{}/{}.conf", CONFIG_PATH, swap_id);

    // Limiter la taille max à 4096 Mo
    let taille_effective = if taille_mb > 4096 { 4096 } else { taille_mb };

    println!("[nbd-server] 1/6 mkdir {}", chemin_dir);
    fs::create_dir_all(&chemin_dir)
        .map_err(|e| format!("mkdir échoué : {}", e))?;

    println!("[nbd-server] 2/6 mount tmpfs {}M", taille_effective);
    let taille_arg = format!("size={}M", taille_effective);
    run("mount", &["-t", "tmpfs", "-o", &taille_arg, "tmpfs", &chemin_dir])?;

    println!("[nbd-server] 3/6 dd {}M", taille_effective);
    let count = taille_effective.to_string();
    run("dd", &[
        "if=/dev/zero",
        &format!("of={}", chemin_fichier),
        "bs=1M",
        &format!("count={}", count),
    ])?;

    // Tuer l'ancien nbd-server sur ce port s'il existe
    println!("[nbd-server] 4/6 nettoyage port {}", port);
    let _ = Command::new("fuser")
        .args(&["-k", &format!("{}/tcp", port)])
        .output();
    std::thread::sleep(std::time::Duration::from_secs(1));

    println!("[nbd-server] 5/6 config NBD");
    fs::create_dir_all(CONFIG_PATH)
        .map_err(|e| format!("mkdir config échoué : {}", e))?;

    let config = format!(
        "[generic]\n    user = root\n    group = root\n\n[{}]\n    exportname = {}\n    port = {}\n",
        swap_id, chemin_fichier, port
    );
    fs::write(&chemin_config, &config)
        .map_err(|e| format!("Écriture config échouée : {}", e))?;

    println!("[nbd-server] 6/6 lancement nbd-server port {}", port);
    run("nbd-server", &["-C", &chemin_config])?;

    // Attendre que nbd-server écoute vraiment
    println!("[nbd-server] Attente démarrage sur port {}...", port);
    let mut pret = false;
    for _ in 0..15 {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let check = Command::new("ss")
            .args(&["-tlnp"])
            .output();
        if let Ok(output) = check {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains(&format!(":{}", port)) {
                println!("[nbd-server] Port {} ouvert", port);
                pret = true;
                break;
            }
        }
    }
    if !pret {
        // Nettoyage en cas d'échec
        let _ = run("umount", &[&chemin_dir]);
        let _ = fs::remove_dir_all(&chemin_dir);
        let _ = fs::remove_file(&chemin_config);
        return Err(format!("nbd-server n'a pas démarré sur le port {}", port));
    }

    Ok(SwapExpose {
        swap_id: swap_id.to_string(),
        port_nbd: port,
        receveur_ip: receveur_ip.to_string(),
        taille_mb: taille_effective,
        chemin_fichier,
    })
}

pub fn arreter(expose: &SwapExpose) -> Result<(), String> {
    let chemin_dir = format!("{}/{}", BASE_PATH, expose.swap_id);
    let chemin_config = format!("{}/{}.conf", CONFIG_PATH, expose.swap_id);

    println!("[nbd-server] Arrêt port {}", expose.port_nbd);
    let _ = Command::new("fuser")
        .args(&["-k", &format!("{}/tcp", expose.port_nbd)])
        .output();

    let _ = run("umount", &[&chemin_dir]);
    let _ = fs::remove_dir_all(&chemin_dir);
    let _ = fs::remove_file(&chemin_config);

    println!("[nbd-server] Nettoyé : {}", expose.swap_id);
    Ok(())
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{} impossible : {}", cmd, e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} échoué : {}", cmd, stderr.trim()))
    }
}
