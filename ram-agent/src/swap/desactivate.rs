use crate::swap::common::{SwapBlock, SwapState, run_cmd};

pub fn desactivate(bloc: &mut SwapBlock) -> Result<(), String> {
    if !bloc.est_actif() {
        return Err(format!("Bloc {} déjà inactif", bloc.id));
    }

    // 1. Désactiver le swap
    if let Some(ref loop_dev) = bloc.loop_device {
        run_cmd("swapoff", &[loop_dev])?;
        // 2. Détacher le loop device
        run_cmd("losetup", &["-d", loop_dev])?;
    }

    bloc.loop_device = None;
    bloc.etat = SwapState::Inactif;
    Ok(())
}
