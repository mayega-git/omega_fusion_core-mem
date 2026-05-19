mod memory;
mod swap;
mod network;
mod nbd;

use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::env;
use dotenvy::dotenv;
use crate::network::{Agent, detect_ip, start_listener};
use crate::swap::{SwapWatcher, DonneurWatcher};

fn env_var_or_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_list(key: &str) -> Vec<String> {
    env::var(key)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn main() {
    println!("=== RAM Agent ===\n");

    dotenv().ok();
    nbd::init();

    let local_port = env_var_or_default("RPC_PORT", "7878")
        .parse::<u16>()
        .unwrap_or(7878);
    let neighbour_ips = env_list("PEER_IPS");
    let local_ip = env::var("NODE_IP").unwrap_or_else(|_| detect_ip());

    let agent = Agent::new(local_ip.clone(), local_port, neighbour_ips.clone());
    let watcher = Arc::new(Mutex::new(SwapWatcher::new(120)));

    let agent_shared = Arc::new(Mutex::new(Agent::new(local_ip.clone(), local_port, neighbour_ips.clone())));

    // 3 minutes pour les tests (à passer à 600+ en production)
    let donneur_watcher = Arc::new(Mutex::new(DonneurWatcher::new(180)));

    let agent_listener = Arc::clone(&agent_shared);
    let watcher_listener = Arc::clone(&watcher);
    let donneur_listener = Arc::clone(&donneur_watcher);
    thread::spawn(move || {
        start_listener(agent_listener, watcher_listener, donneur_listener, local_port);
    });

    // Thread de surveillance côté donneur
    let donneur_bg = Arc::clone(&donneur_watcher);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(30));

            let expires = donneur_bg.lock().unwrap().snapshot_expires();
            if expires.is_empty() {
                continue;
            }

            for (swap_id, receveur_ip, receveur_port) in expires {
                // Demander au receveur si la pression RAM a diminué
                let req = network::RpcRequest::new("ram_saturee", serde_json::json!({}), 99);
                let encore_saturee = match network::envoyer(&receveur_ip, receveur_port, &req) {
                    Ok(resp) => resp.result
                        .and_then(|r| r["saturee"].as_bool())
                        .unwrap_or(false),
                    Err(e) => {
                        println!("[donneur-watcher] {} injoignable ({}) → libération forcée", receveur_ip, e);
                        false
                    }
                };

                if !encore_saturee {
                    println!("[donneur-watcher] RAM de {} OK → libération de {}", receveur_ip, swap_id);

                    // Notifier le receveur pour qu'il déconnecte son nbd-client
                    let req_lib = network::RpcRequest::new(
                        "liberer_swap",
                        serde_json::json!({ "swap_id": swap_id }),
                        100,
                    );
                    let _ = network::envoyer(&receveur_ip, receveur_port, &req_lib);

                    // Libérer l'espace swap côté donneur
                    if let Some(suivi) = donneur_bg.lock().unwrap().retirer(&swap_id) {
                        let _ = nbd::arreter(&suivi.expose);
                    }
                } else {
                    println!("[donneur-watcher] {} encore saturé → swap {} maintenu, retry dans 30s",
                        receveur_ip, swap_id);
                }
            }
        }
    });

    thread::sleep(Duration::from_secs(1));

    
    loop {

          // Récupérer la RAM manquante des VMs via Proxmox
    let proxmox_user = env_var_or_default("PROXMOX_USER", "root@pam");
    let proxmox_token_name = env_var_or_default("PROXMOX_TOKEN_NAME", "root-1");
    let proxmox_token_value = env_var_or_default("PROXMOX_TOKEN_VALUE", "31e59e7d-a573-4282-ae6d-e749e980320b");
    let proxmox_node = env::var("PROXMOX_NODE").ok();

    let mut vms_ram = agent.ram_vms_proxmox(
        &local_ip,
        &proxmox_user,
        &proxmox_token_name,
        &proxmox_token_value,
        proxmox_node.as_deref(),
        None,
    );
    vms_ram.sort_by(|a, b| b.cmp(a));
    println!("[main] RAM manquante VMs (décroissant) : {:?}", vms_ram);


        let info = match memory::read_meminfo() {
            Ok(i) => i,
            Err(e) => {
                eprintln!("[main] Erreur mémoire : {}", e);
                thread::sleep(Duration::from_secs(10));
                continue;
            }
        };

        let available_mb = info.available_kb / 1024;
        println!("[main] RAM : {} Mo dispo ({:.1}% utilisée)", available_mb, info.used_pct);

        if agent.ram_saturee() {
            // Vérifier s'il y a déjà un swap actif
            let nb_swaps = watcher.lock().unwrap().nb_suivis();
            if nb_swaps >= 1 {
                println!("[main] Swap déjà actif, pas de nouvelle création");
            } else {
                println!("[main] RAM SATURÉE — recherche de swap distant...");

                let offres = agent.ram_peers();
                let paires = agent.apparier_offres(&offres, &vms_ram, 500, 5);

                if paires.is_empty() {
                    println!("[main] Aucune paire possible");
                }

                for (donneur_ip, taille) in &paires {
                    println!("[main] Demande {} Mo à {}", taille, donneur_ip);

                    match agent.demander_swap(donneur_ip, *taille) {
                        Ok((port, swap_id)) => {
                            match agent.configurer_swap_local(donneur_ip, port, &swap_id, *taille) {
                                Ok(distant) => {
                                    println!("[main] Swap distant ACTIF : {} sur {}", distant.device, distant.donneur_ip);
                                    let mut w = watcher.lock().unwrap();
                                    w.ajouter(&distant.swap_id, &distant.device, &distant.donneur_ip, local_port);
                                    break; // Un seul swap à la fois
                                }
                                Err(e) => println!("[main] Config locale échouée : {}", e),
                            }
                        }
                        Err(e) => println!("[main] Demande refusée par {} : {}", donneur_ip, e),
                    }
                }
            }
        }

        // Surveillance
        {
            let mut w = watcher.lock().unwrap();
            let a_liberer = w.verifier();
            for suivi in &a_liberer {
                println!("[main] Libération de {} (vide depuis 2 min)", suivi.swap_id);

                let distant = nbd::SwapDistant {
                    swap_id: suivi.swap_id.clone(),
                    donneur_ip: suivi.donneur_ip.clone(),
                    port_nbd: 0,
                    device: suivi.device.clone(),
                    taille_mb: 0,
                };
                let _ = nbd::deconnecter(&distant);

                let req = network::RpcRequest::new(
                    "liberer_swap",
                    serde_json::json!({ "swap_id": suivi.swap_id }),
                    3,
                );
                let _ = network::envoyer(&suivi.donneur_ip, suivi.port_rpc, &req);
            }

            if w.nb_suivis() > 0 {
                println!("[watcher] {} swap(s) sous surveillance", w.nb_suivis());
            }
        }

        thread::sleep(Duration::from_secs(10));
    }
}
