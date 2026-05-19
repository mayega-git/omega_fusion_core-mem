use std::process::Command;
use crate::nbd::common::SwapDistant;

/// Se connecte à un swap distant et l'active
pub fn connecter(donneur_ip: &str, port: u16, device: &str, swap_id: &str, taille_mb: u64) -> Result<SwapDistant, String> {
     
     let ip = donneur_ip.trim().trim_end_matches(',');

    // 1. Charger le module nbd
    println!("[nbd-client] 1/4 modprobe nbd");
    run("modprobe", &["nbd"])?;

    // 2. Connecter au donneur
    println!("[nbd-client] 2/4 connexion {}:{} → {}", donneur_ip, port, device);
    run("nbd-client", &[ip, &port.to_string(), "-N", swap_id, device])?;

    // 3. Formater en swap
    println!("[nbd-client] 3/4 mkswap {}", device);
    run("mkswap", &[device])?;

    // 4. Activer avec priorité haute
    println!("[nbd-client] 4/4 swapon -p 100 {}", device);
    run("swapon", &["-p", "100", device])?;

    Ok(SwapDistant {
        swap_id: swap_id.to_string(),
        donneur_ip: donneur_ip.to_string(),
        port_nbd: port,
        device: device.to_string(),
        taille_mb,
    })
}

/// Déconnecte un swap distant
pub fn deconnecter(distant: &SwapDistant) -> Result<(), String> {
    // 1. Désactiver le swap (rapatrie toutes les pages)
    println!("[nbd-client] 1/2 swapoff {}", distant.device);
    run("swapoff", &[&distant.device])?;

    // 2. Déconnecter le nbd-client
    println!("[nbd-client] 2/2 déconnexion {}", distant.device);
    run("nbd-client", &["-d", &distant.device])?;

    println!("[nbd-client] Nettoyé : {}", distant.swap_id);
    Ok(())
}

/// Trouver le premier /dev/nbdX libre
pub fn trouver_device_libre() -> Result<String, String> {
    for i in 0..16 {
        let device = format!("/dev/nbd{}", i);
        let output = Command::new("blockdev")
            .args(&["--getsize64", &device])
            .output()
            .map_err(|e| format!("blockdev échoué : {}", e))?;

        let size: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap_or(0);

        if size == 0 {
            return Ok(device);
        }
    }
    Err("Aucun /dev/nbdX libre".to_string())
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
