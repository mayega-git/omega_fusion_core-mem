use std::fs;
use std::time::{Instant, Duration};
use crate::memory;

pub struct SwapSuivi {
    pub swap_id: String,
    pub device: String,
    pub donneur_ip: String,
    pub port_rpc: u16,
    pub vide_depuis: Option<Instant>,
}

pub struct SwapWatcher {
    pub suivis: Vec<SwapSuivi>,
    pub delai_liberation: Duration,
    pub seuil_ram_confortable: f64, // en dessous de ce % → libérer les swaps vides
}

impl SwapWatcher {
    pub fn new(delai_secs: u64) -> Self {
        SwapWatcher {
            suivis: Vec::new(),
            delai_liberation: Duration::from_secs(delai_secs),
            seuil_ram_confortable: 50.0, // si used < 50% → RAM confortable
        }
    }

    pub fn ajouter(&mut self, swap_id: &str, device: &str, donneur_ip: &str, port_rpc: u16) {
        println!("[watcher] Surveillance de {} sur {}", swap_id, device);
        self.suivis.push(SwapSuivi {
            swap_id: swap_id.to_string(),
            device: device.to_string(),
            donneur_ip: donneur_ip.to_string(),
            port_rpc,
            vide_depuis: None,
        });
    }

    fn lire_usage(device: &str) -> Option<u64> {
        let contenu = fs::read_to_string("/proc/swaps").ok()?;
        for line in contenu.lines().skip(1) {
            if line.starts_with(device) {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() >= 4 {
                    return cols[3].parse().ok();
                }
            }
        }
        None
    }

    pub fn lire_usage_pub(device: &str) -> Option<u64> {
        Self::lire_usage(device)
    }

    /// Vérifie si la RAM est confortable (plus besoin de swap distant)
    fn ram_confortable(&self) -> bool {
        match memory::read_meminfo() {
            Ok(info) => info.used_pct < self.seuil_ram_confortable,
            Err(_) => false,
        }
    }

    pub fn verifier(&mut self) -> Vec<SwapSuivi> {
        let mut a_liberer: Vec<usize> = Vec::new();
        let confortable = self.ram_confortable();

        for (i, suivi) in self.suivis.iter_mut().enumerate() {
            match Self::lire_usage(&suivi.device) {
                Some(used_kb) => {
                    if used_kb == 0 {
                        match suivi.vide_depuis {
                            None => {
                                println!("[watcher] {} vide, début du délai", suivi.swap_id);
                                suivi.vide_depuis = Some(Instant::now());
                            }
                            Some(depuis) => {
                                if depuis.elapsed() >= self.delai_liberation {
                                    println!("[watcher] {} vide depuis {:?}, à libérer",
                                        suivi.swap_id, self.delai_liberation);
                                    a_liberer.push(i);
                                }
                            }
                        }
                    } else if confortable && used_kb < 1024 {
                        // RAM confortable ET swap presque vide → libérer
                        println!("[watcher] {} quasi-vide ({} Ko) et RAM confortable, à libérer",
                            suivi.swap_id, used_kb);
                        a_liberer.push(i);
                    } else {
                        if suivi.vide_depuis.is_some() {
                            println!("[watcher] {} de nouveau utilisé ({} Ko)", suivi.swap_id, used_kb);
                        }
                        suivi.vide_depuis = None;
                    }
                }
                None => {
                    println!("[watcher] {} introuvable dans /proc/swaps", suivi.swap_id);
                }
            }
        }

        let mut liberes: Vec<SwapSuivi> = Vec::new();
        a_liberer.sort_by(|a, b| b.cmp(a));
        for i in a_liberer {
            liberes.push(self.suivis.remove(i));
        }

        liberes
    }

    pub fn nb_suivis(&self) -> usize {
        self.suivis.len()
    }
}
