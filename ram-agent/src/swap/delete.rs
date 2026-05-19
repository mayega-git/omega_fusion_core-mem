use std::fs;
use crate::swap::common::{SwapBlock, run_cmd};
use crate::swap::desactivate;

/// Supprime un bloc de swap complètement
/// 1. swapoff si actif
/// 2. umount le tmpfs
/// 3. rmdir le dossier
/// Prend le bloc par valeur → il est consommé, plus utilisable après
pub fn delete(mut bloc: SwapBlock) -> Result<(), String> {
    // Désactiver si encore actif
    if bloc.est_actif() {
        desactivate::desactivate(&mut bloc)?;
    }

    // Démonter le tmpfs
    run_cmd("umount", &[&bloc.chemin_tmpfs()])?;

    // Supprimer le dossier
    fs::remove_dir_all(bloc.chemin_tmpfs())
        .map_err(|e| format!("rmdir échoué : {}", e))?;

    // bloc est consommé ici — Rust interdit de l'utiliser après cette fonction
    Ok(())
}
