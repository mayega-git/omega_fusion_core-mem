from fastapi import FastAPI, HTTPException
from rpc_client import envoyer_rpc

app = FastAPI(title="RAM Agent API")

AGENT_IP = "127.0.0.1"
AGENT_PORT = 7878

@app.get("/ram")
def ram_status():
    resp = envoyer_rpc(AGENT_IP, AGENT_PORT, "ram_available", {})
    if resp.get("error"):
        raise HTTPException(status_code=500, detail=resp["error"]["message"])
    return resp["result"]

@app.get("/ram/saturee")
def ram_saturee():
    resp = envoyer_rpc(AGENT_IP, AGENT_PORT, "ram_saturee", {})
    if resp.get("error"):
        raise HTTPException(status_code=500, detail=resp["error"]["message"])
    return resp["result"]

@app.get("/peers")
def peers_ram():
    resp = envoyer_rpc(AGENT_IP, AGENT_PORT, "peers_ram", {})
    if resp.get("error"):
        raise HTTPException(status_code=500, detail=resp["error"]["message"])
    return resp["result"]

@app.post("/swap/creer/{taille_mb}")
def creer_swap(taille_mb: int):
    resp = envoyer_rpc(AGENT_IP, AGENT_PORT, "creer_swap", {"taille_mb": taille_mb})
    if resp.get("error"):
        raise HTTPException(status_code=503, detail=resp["error"]["message"])
    return resp["result"]

@app.delete("/swap/{swap_id}")
def liberer_swap(swap_id: str):
    resp = envoyer_rpc(AGENT_IP, AGENT_PORT, "liberer_swap", {"swap_id": swap_id})
    if resp.get("error"):
        raise HTTPException(status_code=500, detail=resp["error"]["message"])
    return {"ok": True}

@app.get("/swap/liste")
def liste_swaps():
    resp = envoyer_rpc(AGENT_IP, AGENT_PORT, "liste_swaps", {})
    if resp.get("error"):
        raise HTTPException(status_code=500, detail=resp["error"]["message"])
    return resp["result"]

@app.get("/swap/{swap_id}/utilisation")
def utilisation_swap(swap_id: str):
    resp = envoyer_rpc(AGENT_IP, AGENT_PORT, "utilisation_swap", {"swap_id": swap_id})
    if resp.get("error"):
        raise HTTPException(status_code=500, detail=resp["error"]["message"])
    return resp["result"]

@app.get("/statut")
def statut():
    resp = envoyer_rpc(AGENT_IP, AGENT_PORT, "statut", {})
    if resp.get("error"):
        raise HTTPException(status_code=500, detail=resp["error"]["message"])
    return resp["result"]

@app.get("/vms")
def vms_status():
    resp = envoyer_rpc(AGENT_IP, AGENT_PORT, "get_vm_status", {
        "proxmox_ip": "192.168.123.101",
        "token_name": "monitoring",
        "token_value": "f7c448c2-1870-445f-8a43-4b8bfd587701"
    })
    if resp.get("error"):
        raise HTTPException(status_code=500, detail=resp["error"]["message"])
    return resp["result"]
