#!/bin/bash

echo "=== Démarrage RAM Agent ==="

# Répertoire du script (tout est relatif à ce dossier)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENV_FILE="$SCRIPT_DIR/.venv_name"
API_DIR="$SCRIPT_DIR/api"

# ── Gestion environnement virtuel Python ──
if [ -f "$VENV_FILE" ]; then
    VENV_NAME=$(cat "$VENV_FILE")
    echo "[venv] Environnement trouvé : $VENV_NAME"
else
    VENV_NAME="ram_agent_env"
    echo "[venv] Création de l'environnement : $VENV_NAME"
    python3 -m venv "$API_DIR/$VENV_NAME"
    echo "$VENV_NAME" > "$VENV_FILE"
fi

VENV_BIN="$API_DIR/$VENV_NAME/bin"
echo "[venv] Installation des dépendances..."
"$VENV_BIN/pip" install --quiet -r "$API_DIR/requirements.txt"
echo "[venv] Dépendances OK"

# ── Nettoyage des instances précédentes ──
echo "[cleanup] Libération des ports 7878 et 8000..."
fuser -k 7878/tcp 2>/dev/null || true
fuser -k 8000/tcp 2>/dev/null || true
sleep 1

# ── Compilation Rust ──
echo "[1] Compilation Rust..."
cd "$SCRIPT_DIR/ram-agent"
cargo build --release

# ── Lancement agent Rust ──
echo "[2] Lancement agent Rust..."
./target/release/ram-agent &
RUST_PID=$!
echo "    PID Rust : $RUST_PID"

sleep 2

# ── Lancement API Python ──
echo "[3] Lancement API Python..."
cd "$API_DIR"
"$VENV_BIN/python3" -m uvicorn main:app --host 0.0.0.0 --port 8000 &
PYTHON_PID=$!
echo "    PID Python : $PYTHON_PID"

echo ""
echo "=== Tout est lancé ==="
echo "Agent Rust  : PID $RUST_PID (port 7878)"
echo "API Python  : PID $PYTHON_PID (port 8000)"
echo "Env virtuel : $VENV_NAME"
echo ""
echo "Pour arrêter : kill $RUST_PID $PYTHON_PID"

trap "echo 'Arrêt...'; kill $RUST_PID $PYTHON_PID 2>/dev/null; deactivate 2>/dev/null; exit" SIGINT SIGTERM
wait
