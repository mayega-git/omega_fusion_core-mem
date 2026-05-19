use std::fs;
use crate::swap::common::{SwapBlock, SwapState, run_cmd};

pub fn create(taille_mb: u64) -> Result<SwapBlock, String> {
    let id = format!("swap_{}", chrono::Utc::now().timestamp_millis());

    let bloc = SwapBlock {
        id,
        taille_mb,
        etat: SwapState::Inactif,
        loop_device: None,
    };

    println!("[create] 1/5 mkdir {}", bloc.chemin_tmpfs());
    fs::create_dir_all(bloc.chemin_tmpfs())
        .map_err(|e| format!("mkdir échoué : {}", e))?;

    println!("[create] 2/5 mount tmpfs {}M", taille_mb);
    let taille_arg = format!("size={}M", taille_mb);
    run_cmd("mount", &["-t", "tmpfs", "-o", &taille_arg, "tmpfs", &bloc.chemin_tmpfs()])?;

    println!("[create] 3/5 dd {}M", taille_mb);
    let count = taille_mb.to_string();
    run_cmd("dd", &["if=/dev/zero", &format!("of={}", bloc.chemin_fichier()), "bs=1M", &format!("count={}", count)])?;

    println!("[create] 4/5 chmod 600");
    run_cmd("chmod", &["600", &bloc.chemin_fichier()])?;

    println!("[create] 5/5 mkswap");
    run_cmd("mkswap", &[&bloc.chemin_fichier()])?;

    Ok(bloc)
}
