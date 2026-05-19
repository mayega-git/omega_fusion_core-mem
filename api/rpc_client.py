import socket
import json

def envoyer_rpc(ip, port, method, params, id=1):
    requete = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": id,
    }
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5)
    try:
        sock.connect((ip, port))
        data = json.dumps(requete) + "\n"
        sock.sendall(data.encode())
        reponse = b""
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            reponse += chunk
            if b"\n" in reponse:
                break
        return json.loads(reponse.decode().strip())
    finally:
        sock.close()
