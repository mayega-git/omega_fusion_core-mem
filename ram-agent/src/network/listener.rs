use std::net::TcpListener;
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU16, Ordering};
use std::thread;
use crate::network::agent::Agent;
use crate::network::message::{RpcRequest, RpcResponse};
use crate::swap::{SwapWatcher, DonneurWatcher};
use crate::nbd;

static PROCHAIN_PORT: AtomicU16 = AtomicU16::new(10809);

pub fn start(agent: Arc<Mutex<Agent>>, watcher: Arc<Mutex<SwapWatcher>>, donneur_watcher: Arc<Mutex<DonneurWatcher>>, port: u16) {
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            println!("[listener] Impossible d'écouter sur {} : {}", addr, e);
            return;
        }
    };

    println!("[listener] Écoute sur {}", addr);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let agent_clone = Arc::clone(&agent);
                let watcher_clone = Arc::clone(&watcher);
                let donneur_clone = Arc::clone(&donneur_watcher);
                thread::spawn(move || {
                    gerer_connexion(stream, agent_clone, watcher_clone, donneur_clone);
                });
            }
            Err(e) => {
                println!("[listener] Erreur connexion : {}", e);
            }
        }
    }
}

fn gerer_connexion(mut stream: std::net::TcpStream, agent: Arc<Mutex<Agent>>, watcher: Arc<Mutex<SwapWatcher>>, donneur_watcher: Arc<Mutex<DonneurWatcher>>) {
    let reader_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let reader = BufReader::new(reader_stream);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let response = match RpcRequest::from_json(&line) {
            Ok(req) => {
                println!("[listener] Reçu: {} (id={})", req.method, req.id);
                let agent = agent.lock().unwrap();
                let watcher = watcher.lock().unwrap();
                let mut dw = donneur_watcher.lock().unwrap();
                traiter(&agent, &watcher, &mut dw, &req)
            }
            Err(e) => RpcResponse::error(0, -32700, &format!("JSON invalide: {}", e)),
        };

        let json = response.to_json().unwrap_or_default();
        if writeln!(stream, "{}", json).is_err() {
            break;
        }
    }
}

fn traiter(agent: &Agent, watcher: &SwapWatcher, donneur_watcher: &mut DonneurWatcher, req: &RpcRequest) -> RpcResponse {
    match req.method.as_str() {
        "ram_available" => handle_ram_available(agent, req),
        "ram_saturee" => handle_ram_saturee(agent, req),
        "peers_ram" => handle_peers_ram(agent, req),
        "creer_swap" => handle_creer_swap(agent, donneur_watcher, req),
        "liberer_swap" => handle_liberer_swap(req),
        "liste_swaps" => handle_liste_swaps(watcher, req),
        "utilisation_swap" => handle_utilisation_swap(watcher, req),
        "statut" => handle_statut(agent, watcher, req),
               "get_vm_status" => handle_vm_status(agent, req),
        _ => RpcResponse::error(req.id, -32601, "Méthode inconnue"),
    }
}

// ── 1. RAM disponible ──
fn handle_ram_available(agent: &Agent, req: &RpcRequest) -> RpcResponse {
    let mb = agent.ram_available_mb();
    RpcResponse::success(req.id, serde_json::json!({ "available_mb": mb }))
}

// ── 2. RAM saturée ──
fn handle_ram_saturee(agent: &Agent, req: &RpcRequest) -> RpcResponse {
    RpcResponse::success(req.id, serde_json::json!({ "saturee": agent.ram_saturee() }))
}

// ── 3. RAM des peers ──
fn handle_peers_ram(agent: &Agent, req: &RpcRequest) -> RpcResponse {
    let peers = agent.ram_peers();
    RpcResponse::success(req.id, serde_json::json!({ "peers": peers }))
}

// ── 4. Créer swap ──
fn handle_creer_swap(agent: &Agent, donneur_watcher: &mut DonneurWatcher, req: &RpcRequest) -> RpcResponse {
    let taille_mb = match req.params["taille_mb"].as_u64() {
        Some(t) => t,
        None => return RpcResponse::error(req.id, -32602, "taille_mb manquant"),
    };
    let receveur_ip = req.params["receveur_ip"].as_str().unwrap_or("").to_string();
    let receveur_port_rpc = req.params["receveur_port_rpc"].as_u64().unwrap_or(7878) as u16;

    let available = agent.ram_available_mb();
    if available < taille_mb + 500 {
        return RpcResponse::error(req.id, -1,
            &format!("RAM insuffisante : {} Mo dispo, {} Mo demandés + 500 Mo marge", available, taille_mb));
    }

    let swap_id = format!("swap_{}", chrono::Utc::now().timestamp_millis());
    let port = PROCHAIN_PORT.fetch_add(1, Ordering::SeqCst);

    match nbd::exposer(&swap_id, taille_mb, port, &receveur_ip) {
        Ok(expose) => {
            println!("[listener] Swap {} créé et exposé sur port {}", swap_id, port);
            donneur_watcher.ajouter(expose, receveur_ip, receveur_port_rpc);
            RpcResponse::success(req.id, serde_json::json!({
                "port_nbd": port,
                "swap_id": swap_id
            }))
        }
        Err(e) => RpcResponse::error(req.id, -1, &format!("Création échouée : {}", e)),
    }
}

// ── 5. Libérer swap ──
fn handle_liberer_swap(req: &RpcRequest) -> RpcResponse {
    let swap_id = match req.params["swap_id"].as_str() {
        Some(id) => id,
        None => return RpcResponse::error(req.id, -32602, "swap_id manquant"),
    };

    let expose = nbd::SwapExpose {
        swap_id: swap_id.to_string(),
        port_nbd: 0,
        receveur_ip: String::new(),
        taille_mb: 0,
        chemin_fichier: String::new(),
    };

    match nbd::arreter(&expose) {
        Ok(()) => {
            println!("[listener] Swap {} libéré", swap_id);
            RpcResponse::success(req.id, serde_json::json!({ "ok": true }))
        }
        Err(e) => RpcResponse::error(req.id, -1, &format!("Libération échouée : {}", e)),
    }
}

// ── 6. Liste des swaps surveillés ──
fn handle_liste_swaps(watcher: &SwapWatcher, req: &RpcRequest) -> RpcResponse {
    let liste: Vec<serde_json::Value> = watcher.suivis.iter().map(|s| {
        serde_json::json!({
            "swap_id": s.swap_id,
            "device": s.device,
            "donneur_ip": s.donneur_ip,
            "vide": s.vide_depuis.is_some()
        })
    }).collect();

    RpcResponse::success(req.id, serde_json::json!({
        "nb_swaps": liste.len(),
        "swaps": liste
    }))
}

// ── 7. Utilisation d'un swap ──
fn handle_utilisation_swap(watcher: &SwapWatcher, req: &RpcRequest) -> RpcResponse {
    let swap_id = match req.params["swap_id"].as_str() {
        Some(id) => id,
        None => return RpcResponse::error(req.id, -32602, "swap_id manquant"),
    };

    let suivi = watcher.suivis.iter().find(|s| s.swap_id == swap_id);

    match suivi {
        Some(s) => {
            let usage = crate::swap::SwapWatcher::lire_usage_pub(&s.device);
            RpcResponse::success(req.id, serde_json::json!({
                "swap_id": s.swap_id,
                "device": s.device,
                "used_kb": usage.unwrap_or(0),
                "vide": s.vide_depuis.is_some()
            }))
        }
        None => RpcResponse::error(req.id, -1, "Swap introuvable"),
    }
}

// ── 8. Statut complet ──
fn handle_statut(agent: &Agent, watcher: &SwapWatcher, req: &RpcRequest) -> RpcResponse {
    let available = agent.ram_available_mb();
    let saturee = agent.ram_saturee();

    let swaps: Vec<serde_json::Value> = watcher.suivis.iter().map(|s| {
        let usage = crate::swap::SwapWatcher::lire_usage_pub(&s.device);
        serde_json::json!({
            "swap_id": s.swap_id,
            "device": s.device,
            "donneur_ip": s.donneur_ip,
            "used_kb": usage.unwrap_or(0)
        })
    }).collect();

    RpcResponse::success(req.id, serde_json::json!({
        "ip": agent.ip,
        "available_mb": available,
        "saturee": saturee,
        "nb_swaps": swaps.len(),
        "swaps": swaps
    }))
}




// ── 9. Statut VMs Proxmox ──
fn handle_vm_status(agent: &Agent, req: &RpcRequest) -> RpcResponse {
    let proxmox_ip = req.params["proxmox_ip"].as_str().unwrap_or("192.168.123.101");
    let token_user = req.params["token_user"].as_str().unwrap_or("root@pam");
    let token_name = req.params["token_name"].as_str().unwrap_or("monitoring");
    let token_value = req.params["token_value"].as_str().unwrap_or("");
    let node_name = req.params["node_name"].as_str();
    let node_ip = req.params["node_ip"].as_str();

    let ram_list = agent.ram_vms_proxmox(proxmox_ip, token_user, token_name, token_value, node_name, node_ip);

    let result: Vec<serde_json::Value> = ram_list.iter()
        .map(|ram| serde_json::json!({ "ram_free": ram }))
        .collect();

    RpcResponse::success(req.id, serde_json::json!({ "vms": result }))
}
