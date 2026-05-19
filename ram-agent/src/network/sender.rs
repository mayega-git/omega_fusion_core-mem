use std::net::TcpStream;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use crate::network::message::{RpcRequest, RpcResponse};

pub fn envoyer(ip: &str, port: u16, request: &RpcRequest) -> Result<RpcResponse, String> {
    let ip = ip.trim().trim_end_matches(',');
    let addr = format!("{}:{}", ip, port);
    println!("[debug] addr brute : {:?}", addr);
    let socket_addr = addr.parse()
        .map_err(|e| format!("Adresse invalide: {}", e))?;

    let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(10))
        .map_err(|e| format!("Connexion à {} échouée: {}", addr, e))?;

    stream.set_read_timeout(Some(Duration::from_secs(120)))
        .map_err(|e| e.to_string())?;

    let json = request.to_json()?;
    writeln!(stream, "{}", json).map_err(|e| e.to_string())?;

    let reader = BufReader::new(stream);
    let line = reader.lines()
        .next()
        .ok_or("Pas de réponse")?
        .map_err(|e| e.to_string())?;

    RpcResponse::from_json(&line)
}

/// Demande à un noeud distant sa RAM disponible
pub fn demander_ram(ip: &str, port: u16) -> Result<u64, String> {
    let req = RpcRequest::new("ram_available", serde_json::json!({}), 1);
    let resp = envoyer(ip, port, &req)?;

    match resp.result {
        Some(r) => r["available_mb"].as_u64().ok_or("Champ manquant".to_string()),
        None => Err(resp.error.map(|e| e.message).unwrap_or("Erreur".to_string())),
    }
}
