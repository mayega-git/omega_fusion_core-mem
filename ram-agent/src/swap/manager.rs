use crate::memory::MemInfo;
use crate::swap::common::SwapBlock;
use crate::swap::{create, activate, delete};

pub struct SwapManager {
    pub blocs: Vec<SwapBlock>,
    pub min_available_mb: u64,
    pub max_total_swap_mb: u64,
    pub taille_bloc_mb: u64,
}

impl SwapManager {
    pub fn new(taille_bloc_mb: u64) -> Self {
        SwapManager {
            blocs: Vec::new(),
            min_available_mb: 300,
            max_total_swap_mb: 1024,
            taille_bloc_mb,
        }
    }

    pub fn total_swap_mb(&self) -> u64 {
        self.blocs.iter().map(|b| b.taille_mb).sum()
    }

    pub fn nb_actifs(&self) -> usize {
        self.blocs.iter().filter(|b| b.est_actif()).count()
    }

    pub fn ajouter(&mut self) -> Result<String, String> {
        if self.total_swap_mb() + self.taille_bloc_mb > self.max_total_swap_mb {
            return Err(format!(
                "Limite atteinte : {} Mo / {} Mo max",
                self.total_swap_mb(), self.max_total_swap_mb
            ));
        }

        let mut bloc = create::create(self.taille_bloc_mb)?;
        let priorite = (10 - self.blocs.len() as i32).max(1);
        activate::activate(&mut bloc, priorite)?;

        let id = bloc.id.clone();
        println!("[manager] Bloc {} créé et activé ({} Mo)", id, bloc.taille_mb);
        self.blocs.push(bloc);
        Ok(id) // retourne l'id du bloc créé
    }
     

     /// Retourne le pourcentage d'utilisation total du swap géré
       /// Retourne le pourcentage d'utilisation d'un bloc spécifique
pub fn utilisation(&self, id: &str) -> Result<f64, String> {
    let bloc = self.blocs.iter()
        .find(|b| b.id == id)
        .ok_or(format!("Bloc {} introuvable", id))?;

    Ok(bloc.utilisation_pct())
}

    /// Retirer un bloc par son id
    pub fn retirer(&mut self, id: &str) -> Result<(), String> {
        // Chercher la position du bloc
        let pos = self.blocs.iter()
            .position(|b| b.id == id)
            .ok_or(format!("Bloc {} introuvable", id))?;

        let bloc = self.blocs.remove(pos);
        delete::delete(bloc)?;

        println!("[manager] Bloc {} supprimé", id);
        Ok(())
    }

    /// Retirer le plus ancien
    pub fn retirer_ancien(&mut self) -> Result<(), String> {
        if self.blocs.is_empty() {
            return Err("Aucun bloc à retirer".to_string());
        }
        let id = self.blocs[0].id.clone();
        self.retirer(&id)
    }

    pub fn lister(&self) {
        if self.blocs.is_empty() {
            println!("[manager] Aucun bloc de swap actif");
            return;
        }
        println!("[manager] {} bloc(s), total {} Mo :", self.blocs.len(), self.total_swap_mb());
        for bloc in &self.blocs {
            let etat = if bloc.est_actif() { "ACTIF" } else { "INACTIF" };
            println!("  - {} | {} Mo | {}", bloc.id, bloc.taille_mb, etat);
        }
    }

    /// Retourne les ids des blocs créés ou supprimés
    pub fn ajuster(&mut self, info: &MemInfo) -> (Vec<String>, Vec<String>) {
        let available_mb = info.available_kb / 1024;
        let mut crees: Vec<String> = Vec::new();
        let mut supprimes: Vec<String> = Vec::new();

        // Assez de RAM → créer
        if available_mb > self.min_available_mb
            && self.total_swap_mb() + self.taille_bloc_mb <= self.max_total_swap_mb
        {
            match self.ajouter() {
                Ok(id) => crees.push(id),
                Err(e) => println!("[manager] Création refusée : {}", e),
            }
        }

        // RAM trop basse → supprimer le plus ancien
        if available_mb < self.min_available_mb / 2 && !self.blocs.is_empty() {
            let id = self.blocs[0].id.clone();
            match self.retirer(&id) {
                Ok(()) => supprimes.push(id),
                Err(e) => println!("[manager] Suppression échouée : {}", e),
            }
        }

        (crees, supprimes)
    }
}
