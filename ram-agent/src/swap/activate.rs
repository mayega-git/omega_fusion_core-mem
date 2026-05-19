use std::process::Command;
use crate::swap::common::{SwapBlock, SwapState};

pub fn activate(bloc: &mut SwapBlock, priorite: i32) -> Result<(), String> {
    if bloc.est_actif() {
        return Err(format!("Bloc {} déjà actif", bloc.id));
    }

    // 1. Créer un loop device
    let output = Command::new("losetup")
        .args(&["-f", "--show", &bloc.chemin_fichier()])
        .output()
        .map_err(|e| format!("losetup échoué : {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("losetup échoué : {}", stderr.trim()));
    }

    let loop_dev = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // 2. Activer le swap sur le loop device
    let prio_str = priorite.to_string();
    let result = Command::new("swapon")
        .args(&["-p", &prio_str, &loop_dev])
        .output()
        .map_err(|e| format!("swapon échoué : {}", e))?;

    if !result.status.success() {
        // Nettoyage : détacher le loop si swapon échoue
        let _ = Command::new("losetup").args(&["-d", &loop_dev]).output();
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!("swapon échoué : {}", stderr.trim()));
    }

    bloc.loop_device = Some(loop_dev);
    bloc.etat = SwapState::Actif;
    Ok(())
}
