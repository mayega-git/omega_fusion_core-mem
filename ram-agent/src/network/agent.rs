use std::process::Command;
use std::collections::HashMap;
use crate::memory;
use crate::nbd;

pub struct Agent {
    pub ip: String,
    pub port_rpc: u16,
    pub peers: Vec<String>,
}

impl Agent {
    pub fn new(ip: String, port_rpc: u16, peers: Vec<String>) -> Self {
        Agent {
            ip,
            port_rpc,
            peers,
        }
    }

    pub fn ram_saturee(&self) -> bool {
        match memory::read_meminfo() {
            Ok(info) => info.used_pct > 80.0,
            Err(_) => false,
        }
    }

    pub fn ram_available_mb(&self) -> u64 {
        match memory::read_meminfo() {
            Ok(info) => info.available_kb / 1024,
            Err(_) => 0,
        }
    }

    pub fn ram_peers(&self) -> HashMap<String, u64> {
        let mut resultats: HashMap<String, u64> = HashMap::new();

        for peer in &self.peers {
            match crate::network::demander_ram(peer, self.port_rpc) {
                Ok(mb) => {
                    println!("[agent] {} a {} Mo disponibles", peer, mb);
                    resultats.insert(peer.clone(), mb);
                }
                Err(e) => {
                    println!("[agent] {} injoignable : {}", peer, e);
                }
            }
        }

        resultats
    }

    pub fn ram_vms_proxmox(
        &self,
        proxmox_ip: &str,
        token_user: &str,
        token_name: &str,
        token_value: &str,
        node_name: Option<&str>,
        node_ip: Option<&str>,
    ) -> Vec<u64> {
        let auth = format!("PVEAPIToken={}!{}={}", token_user, token_name, token_value);
        let mut ram_free_list: Vec<u64> = Vec::new();

        let target_node = if let Some(name) = node_name {
            Some(name.to_string())
        } else if let Some(ip) = node_ip {
            Self::resolve_node_name_from_ip(proxmox_ip, &auth, ip)
        } else {
            None
        };

        if let Some(node_name) = target_node.as_deref() {
            let vm_url = format!(
                "https://{}:8006/api2/json/nodes/{}/qemu",
                proxmox_ip, node_name
            );

            let vm_output = Command::new("curl")
                .args(&["-s", "-k", "-H", &format!("Authorization: {}", auth), &vm_url])
                .output();

            match vm_output {
                Ok(o) => {
                    let vm_json = String::from_utf8_lossy(&o.stdout);
                    if let Ok(vms) = serde_json::from_str::<serde_json::Value>(&vm_json) {
                        if let Some(vm_list) = vms["data"].as_array() {
                            for vm in vm_list {
                                let mem_used = vm["mem"].as_u64().unwrap_or(0) / (1024 * 1024);
                                let mem_total = vm["maxmem"].as_u64().unwrap_or(0) / (1024 * 1024);
                                let mem_free = mem_total.saturating_sub(mem_used);
                                ram_free_list.push(mem_free);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("[proxmox] Erreur connexion au noeud {}: {}", node_name, e);
                    return vec![];
                }
            }
        } else {
            let url = format!("https://{}:8006/api2/json/nodes", proxmox_ip);

            println!("[proxmox] GET {}", url);
            let nodes_output = Command::new("curl")
                .args(&["-s", "-k", "-H", &format!("Authorization: {}", auth), &url])
                .output();

            let nodes_json = match nodes_output {
                Ok(o) => {
                    if !o.stderr.is_empty() {
                        println!("[proxmox] curl stderr: {}", String::from_utf8_lossy(&o.stderr));
                    }
                    String::from_utf8_lossy(&o.stdout).to_string()
                }
                Err(e) => {
                    println!("[proxmox] Erreur curl nodes : {}", e);
                    return vec![];
                }
            };

            println!("[proxmox] Réponse nodes: {}", &nodes_json[..nodes_json.len().min(300)]);

            let nodes: serde_json::Value = match serde_json::from_str(&nodes_json) {
                Ok(v) => v,
                Err(e) => {
                    println!("[proxmox] JSON invalide (nodes) : {}", e);
                    return vec![];
                }
            };

            if let Some(data) = nodes["data"].as_array() {
                println!("[proxmox] {} nœud(s) trouvé(s)", data.len());
                for node in data {
                    if let Some(node_name) = node["node"].as_str() {
                        let vm_url = format!(
                            "https://{}:8006/api2/json/nodes/{}/qemu",
                            proxmox_ip, node_name
                        );

                        println!("[proxmox] GET {}", vm_url);
                        let vm_output = Command::new("curl")
                            .args(&["-s", "-k", "-H", &format!("Authorization: {}", auth), &vm_url])
                            .output();

                        match vm_output {
                            Ok(o) => {
                                let vm_json = String::from_utf8_lossy(&o.stdout).to_string();
                                println!("[proxmox] Réponse VMs ({}): {}", node_name, &vm_json[..vm_json.len().min(300)]);
                                match serde_json::from_str::<serde_json::Value>(&vm_json) {
                                    Ok(vms) => {
                                        if let Some(vm_list) = vms["data"].as_array() {
                                            println!("[proxmox] {} VM(s) sur {}", vm_list.len(), node_name);
                                            for vm in vm_list {
                                                let vmid = vm["vmid"].as_u64().unwrap_or(0);
                                                let status = vm["status"].as_str().unwrap_or("?");
                                                let mem_used = vm["mem"].as_u64().unwrap_or(0) / (1024 * 1024);
                                                let mem_total = vm["maxmem"].as_u64().unwrap_or(0) / (1024 * 1024);
                                                let mem_free = mem_total.saturating_sub(mem_used);
                                                println!("[proxmox] VM {} ({}) : {}Mo utilisés / {}Mo total → {}Mo libre", vmid, status, mem_used, mem_total, mem_free);
                                                ram_free_list.push(mem_free);
                                            }
                                        } else {
                                            println!("[proxmox] Champ 'data' absent dans la réponse VMs");
                                        }
                                    }
                                    Err(e) => println!("[proxmox] JSON invalide (VMs {}) : {}", node_name, e),
                                }
                            }
                            Err(e) => println!("[proxmox] Erreur curl VMs ({}) : {}", node_name, e),
                        }
                    }
                }
            } else {
                println!("[proxmox] Champ 'data' absent dans la réponse nodes — contenu: {}", &nodes_json[..nodes_json.len().min(300)]);
            }
        }

        println!("[proxmox] RAM libre VMs : {:?}", ram_free_list);
        ram_free_list
    }

    fn resolve_node_name_from_ip(proxmox_ip: &str, auth: &str, node_ip: &str) -> Option<String> {
        let url = format!("https://{}:8006/api2/json/nodes", proxmox_ip);
        let nodes_output = Command::new("curl")
            .args(&["-s", "-k", "-H", &format!("Authorization: {}", auth), &url])
            .output()
            .ok()?;

        let nodes_json = String::from_utf8_lossy(&nodes_output.stdout);
        let nodes: serde_json::Value = serde_json::from_str(&nodes_json).ok()?;
        let data = nodes["data"].as_array()?;

        for node in data {
            if let Some(name) = node["node"].as_str() {
                if name == node_ip {
                    return Some(name.to_string());
                }
            }
            if let Some(address) = node["address"].as_str() {
                if address == node_ip {
                    return node["node"].as_str().map(|s| s.to_string());
                }
            }
            if let Some(ip_field) = node["ip"].as_str() {
                if ip_field == node_ip {
                    return node["node"].as_str().map(|s| s.to_string());
                }
            }
            if let Some(fqdn) = node["fqdn"].as_str() {
                if fqdn == node_ip {
                    return node["node"].as_str().map(|s| s.to_string());
                }
            }
        }

        println!("[proxmox] Aucun noeud trouvé pour l'adresse IP {}", node_ip);
        None
    }

    pub fn choisir_offre(&self, peers: &HashMap<String, u64>) -> Option<(String, u64)> {
        peers.iter()
            .max_by_key(|(_, mb)| *mb)
            .map(|(ip, mb)| (ip.clone(), *mb))
    }

    pub fn choisir_taille(&self, mem: &(String, u64), tailles: &[u64]) -> Option<(String, u64)> {
        let (ip, available) = mem;

        let mut tailles_triees = tailles.to_vec();
        tailles_triees.sort_by(|a, b| b.cmp(a));

        for taille in tailles_triees {
            if taille <= *available {
                return Some((ip.clone(), taille));
            }
        }

        None
    }

    pub fn apparier_offres(
        &self,
        offres: &HashMap<String, u64>,
        ram_manquante: &[u64],
        marge_donneur: u64,
        marge_swap: u64,
    ) -> Vec<(String, u64)> {
        let mut besoins = ram_manquante.to_vec();
        besoins.sort_by(|a, b| b.cmp(a));

        let mut offres_triees: Vec<(String, u64)> = offres.iter()
            .map(|(ip, mb)| (ip.clone(), *mb))
            .collect();
        offres_triees.sort_by(|a, b| b.1.cmp(&a.1));

        let mut paires: Vec<(String, u64)> = Vec::new();
        let mut offres_utilisees: Vec<bool> = vec![false; offres_triees.len()];

        for besoin in &besoins {
            let besoin_plafonne = if *besoin > 4096 { 4096 } else { *besoin };
            let taille_swap = besoin_plafonne + marge_swap;
            let mut trouve = false;

            for (i, (ip, dispo)) in offres_triees.iter().enumerate() {
                if !offres_utilisees[i] && *dispo >= taille_swap + marge_donneur {
                    println!("[agent] Paire : besoin {} Mo (plafonné {}) → swap {} Mo ← {} ({} Mo dispo)",
                        besoin, besoin_plafonne, taille_swap, ip, dispo);
                    paires.push((ip.clone(), taille_swap));
                    offres_utilisees[i] = true;
                    trouve = true;
                    break;
                }
            }

            if !trouve {
                println!("[agent] Pas d'offre pour {} Mo (swap {} Mo), ignoré", besoin, taille_swap);
            }
        }

        paires
    }

    pub fn demander_swap(&self, donneur_ip: &str, taille_mb: u64) -> Result<(u16, String), String> {
        let req = crate::network::RpcRequest::new(
            "creer_swap",
            serde_json::json!({
                "taille_mb": taille_mb,
                "receveur_ip": self.ip,
                "receveur_port_rpc": self.port_rpc
            }),
            2,
        );

        let resp = crate::network::sender::envoyer(donneur_ip, self.port_rpc, &req)?;

        match resp.result {
            Some(r) => {
                let port = r["port_nbd"].as_u64()
                    .ok_or("port_nbd manquant")? as u16;
                let swap_id = r["swap_id"].as_str()
                    .ok_or("swap_id manquant")?.to_string();
                Ok((port, swap_id))
            }
            None => {
                let msg = resp.error
                    .map(|e| e.message)
                    .unwrap_or("Erreur inconnue".to_string());
                Err(msg)
            }
        }
    }

    pub fn configurer_swap_local(&self, donneur_ip: &str, port: u16, swap_id: &str, taille_mb: u64)
        -> Result<nbd::SwapDistant, String>
    {
        let device = nbd::trouver_device_libre()?;
        nbd::connecter(donneur_ip, port, &device, swap_id, taille_mb)
    }
}

pub fn detect_ip() -> String {
    let output = Command::new("hostname")
        .args(&["-I"])
        .output();

    match output {
        Ok(o) => {
            let ips = String::from_utf8_lossy(&o.stdout);
            ips.split_whitespace()
                .next()
                .unwrap_or("127.0.0.1")
                .to_string()
        }
        Err(_) => "127.0.0.1".to_string(),
    }
}
