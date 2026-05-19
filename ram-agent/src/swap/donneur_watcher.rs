use std::time::{Instant, Duration};
use crate::nbd::SwapExpose;

pub struct SwapDonneurSuivi {
    pub expose: SwapExpose,
    pub receveur_ip: String,
    pub receveur_port_rpc: u16,
    pub cree_a: Instant,
}

pub struct DonneurWatcher {
    pub suivis: Vec<SwapDonneurSuivi>,
    pub delai: Duration,
}

impl DonneurWatcher {
    pub fn new(delai_secs: u64) -> Self {
        DonneurWatcher {
            suivis: Vec::new(),
            delai: Duration::from_secs(delai_secs),
        }
    }

    pub fn ajouter(&mut self, expose: SwapExpose, receveur_ip: String, receveur_port_rpc: u16) {
        println!("[donneur-watcher] Suivi {} → receveur {} (libération dans {}s)",
            expose.swap_id, receveur_ip, self.delai.as_secs());
        self.suivis.push(SwapDonneurSuivi {
            expose,
            receveur_ip,
            receveur_port_rpc,
            cree_a: Instant::now(),
        });
    }

    /// Retourne un snapshot des swaps dont le délai est écoulé (swap_id, receveur_ip, port)
    pub fn snapshot_expires(&self) -> Vec<(String, String, u16)> {
        self.suivis.iter()
            .filter(|s| s.cree_a.elapsed() >= self.delai)
            .map(|s| (s.expose.swap_id.clone(), s.receveur_ip.clone(), s.receveur_port_rpc))
            .collect()
    }

    /// Retire un suivi par swap_id et retourne l'expose pour pouvoir appeler arreter()
    pub fn retirer(&mut self, swap_id: &str) -> Option<SwapDonneurSuivi> {
        if let Some(pos) = self.suivis.iter().position(|s| s.expose.swap_id == swap_id) {
            Some(self.suivis.remove(pos))
        } else {
            None
        }
    }

    pub fn nb_suivis(&self) -> usize {
        self.suivis.len()
    }
}
