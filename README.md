# RAM Agent — Documentation

## Vue d'ensemble

RAM Agent est un système distribué de swap RAM entre nœuds Proxmox. Quand un nœud est en saturation mémoire, il emprunte de la RAM disponible sur un nœud voisin via NBD (Network Block Device), monté comme espace swap.

```
Nœud receveur (saturé)          Nœud donneur (RAM disponible)
┌──────────────────────┐         ┌──────────────────────────┐
│  Agent Rust :7878    │◄───────►│  Agent Rust :7878        │
│  API Python :8000    │  RPC    │  nbd-server :10809+      │
│  /dev/nbd0 (swap)    │◄────────│  tmpfs /mnt/swapram/...  │
└──────────────────────┘   NBD   └──────────────────────────┘
```

---

## Architecture

### Composants

| Composant | Rôle |
|---|---|
| `ram-agent` (Rust) | Agent principal — surveillance RAM, gestion swap, RPC |
| `api/main.py` (Python) | API REST HTTP sur port 8000 — interface de contrôle |
| `nbd-server` | Expose un fichier tmpfs en bloc réseau (côté donneur) |
| `nbd-client` | Connecte le bloc réseau et l'active comme swap (côté receveur) |

### Flux d'un swap

```
1. Receveur détecte RAM saturée (> 80%)
2. Receveur interroge ses peers → collecte RAM disponible
3. Receveur choisit un donneur (plus de RAM dispo) et calcule la taille
4. Receveur envoie RPC creer_swap → donneur crée tmpfs + nbd-server
5. Receveur connecte nbd-client → mkswap + swapon /dev/nbdX
6. Swap actif — watcher receveur surveille l'utilisation

Libération (côté receveur) :
7a. Si swap vide depuis 2 min → swapoff + nbd-client -d + RPC liberer_swap

Libération (côté donneur — DonneurWatcher) :
7b. Après 3 min (test) / 10 min (prod), donneur demande au receveur : ram_saturee ?
    → Si non saturé : donneur envoie liberer_swap au receveur + arrête nbd-server
    → Si encore saturé : retry dans 30s
```

---

## Configuration — `.env`

```env
NODE_IP=192.168.249.47          # IP de ce nœud (agent P2P + API Proxmox)
PROXMOX_USER=root@pam           # Utilisateur API Proxmox
PROXMOX_TOKEN_NAME=root-1       # Nom du token API
PROXMOX_TOKEN_VALUE=<uuid>      # Valeur du token API
PROXMOX_NODE=pve                # Nom du nœud Proxmox ciblé (optionnel)
PEER_IPS=192.168.249.45,192.168.249.46,192.168.249.47   # Nœuds voisins
RPC_PORT=7878                   # Port RPC entre agents
```

---

## Méthodes RPC (port 7878)

| Méthode | Params | Description |
|---|---|---|
| `ram_available` | — | RAM disponible en Mo sur ce nœud |
| `ram_saturee` | — | `true` si usage RAM > 80% |
| `peers_ram` | — | RAM disponible sur chaque peer |
| `creer_swap` | `taille_mb`, `receveur_ip`, `receveur_port_rpc` | Crée et expose un swap NBD |
| `liberer_swap` | `swap_id` | Arrête le nbd-server et libère le tmpfs |
| `liste_swaps` | — | Liste des swaps actifs surveillés |
| `utilisation_swap` | `swap_id` | Utilisation en Ko d'un swap |
| `statut` | — | État complet : RAM + swaps |
| `get_vm_status` | `proxmox_ip`, `token_*`, `node_name` | RAM libre par VM Proxmox |

---

## API REST (port 8000)

| Méthode | Endpoint | Description |
|---|---|---|
| GET | `/ram` | RAM disponible |
| GET | `/ram/saturee` | Saturation RAM |
| GET | `/peers` | RAM des peers |
| POST | `/swap/creer/{taille_mb}` | Créer un swap de N Mo |
| DELETE | `/swap/{swap_id}` | Libérer un swap |
| GET | `/swap/liste` | Lister les swaps actifs |
| GET | `/swap/{swap_id}/utilisation` | Utilisation d'un swap |
| GET | `/statut` | Statut complet |
| GET | `/vms` | État RAM des VMs Proxmox |

---

## Démarrage

```bash
# Sur chaque nœud
cd /etc/swap-agent
./start.sh
```

Le script :
1. Crée/met à jour le venv Python et installe les dépendances
2. Libère les ports 7878 et 8000 si occupés
3. Compile l'agent Rust (`cargo build --release`)
4. Lance l'agent Rust en arrière-plan
5. Lance l'API Python en arrière-plan

---

## Tests de saturation

### Saturer la RAM pour déclencher un swap distant

```bash
# Saturer avec 4500 Mo de RAM (adapter selon la RAM dispo sur le nœud)
stress-ng --vm 2 --vm-bytes 4500M --timeout 60s
```

> **Paramètres utiles :**
> - `--vm 2` : 2 workers mémoire en parallèle
> - `--vm-bytes 4500M` : chaque worker alloue 4500 Mo (total = 9000 Mo ici)
> - Pour cibler un total précis : `--vm 1 --vm-bytes <total>M`
> - `--timeout 60s` : durée du test

**Observer les logs de l'agent en temps réel :**
```bash
journalctl -u swap-agent -f
# ou si lancé manuellement :
./start.sh 2>&1 | grep -E "\[main\]|\[watcher\]|\[donneur\]|\[nbd\]"
```

### Vérifier que le swap distant est actif

```bash
swapon -s
```

Sortie attendue quand un swap NBD est actif :
```
Filename        Type        Size      Used    Priority
/dev/nbd0       partition   3686396   102400  100
```

| Colonne | Description |
|---|---|
| `Filename` | Device NBD monté comme swap |
| `Type` | `partition` (bloc device) |
| `Size` | Taille totale en Ko |
| `Used` | Ko actuellement utilisés |
| `Priority` | 100 = swap distant (priorité haute) |

### Vérifier l'état complet via l'API

```bash
# Statut RAM + swaps actifs
curl http://localhost:8000/statut | python3 -m json.tool

# Liste des swaps surveillés
curl http://localhost:8000/swap/liste | python3 -m json.tool

# RAM disponible sur les peers
curl http://localhost:8000/peers | python3 -m json.tool
```

---

## Libération automatique du swap

### Côté receveur (SwapWatcher)
- Vérifie toutes les 10s l'utilisation du swap via `/proc/swaps`
- Si `used_kb == 0` pendant **2 minutes** → swapoff + libération

### Côté donneur (DonneurWatcher)
- Vérifie toutes les **30 secondes**
- Après **3 minutes** (test) / modifier `DonneurWatcher::new(180)` dans `main.rs` pour la prod
- Interroge le receveur : `ram_saturee ?`
  - Non saturé → envoie `liberer_swap` au receveur + arrête `nbd-server` localement
  - Encore saturé → log + retry dans 30s

### Libération manuelle

```bash
# Lister les swaps actifs
curl http://localhost:8000/swap/liste

# Libérer un swap spécifique
curl -X DELETE http://localhost:8000/swap/swap_1779211620086
```

---

## Diagnostics

```bash
# Vérifier les processus nbd-server actifs (sur le donneur)
ps aux | grep nbd-server

# Vérifier les tmpfs montés (sur le donneur)
df -h | grep swapram

# Vérifier les devices NBD connectés (sur le receveur)
ls -la /dev/nbd*
cat /proc/swaps

# Vérifier la connectivité RPC entre deux nœuds
echo '{"jsonrpc":"2.0","method":"ram_available","params":{},"id":1}' \
  | nc 192.168.249.45 7878

# Libérer manuellement les ports en cas de crash
fuser -k 7878/tcp
fuser -k 8000/tcp
```

---

## Paramètres de tuning (dans `main.rs`)

| Paramètre | Valeur actuelle | Description |
|---|---|---|
| `SwapWatcher::new(120)` | 120s | Délai avant libération si swap vide (receveur) |
| `DonneurWatcher::new(180)` | 180s (test) | Délai avant vérification pression (donneur) |
| `seuil_ram_confortable` | 50% | Seuil RAM pour considérer la pression relâchée |
| `ram_saturee()` | > 80% | Seuil de saturation RAM |
| `marge_donneur` | 500 Mo | RAM minimale à laisser au donneur |
| `marge_swap` | 5 Mo | Marge ajoutée à la taille du swap demandé |
| taille max swap | 4096 Mo | Plafond par swap (dans `nbd/server.rs`) |
