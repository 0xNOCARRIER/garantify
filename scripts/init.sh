#!/bin/sh
# init.sh — Configuration initiale de Garantify
# Usage : ./scripts/init.sh
set -e

BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info()    { printf "${BLUE}[info]${NC}  %s\n" "$1"; }
success() { printf "${GREEN}[ok]${NC}    %s\n" "$1"; }
warn()    { printf "${YELLOW}[warn]${NC}  %s\n" "$1"; }
error()   { printf "${RED}[error]${NC} %s\n" "$1"; exit 1; }

# ── Vérification des prérequis ──────────────────────────────────

command -v docker >/dev/null 2>&1 || error "Docker n'est pas installé. Voir https://docs.docker.com/get-docker/"

if docker compose version >/dev/null 2>&1; then
    COMPOSE="docker compose"
elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE="docker-compose"
else
    error "Docker Compose n'est pas disponible. Voir https://docs.docker.com/compose/install/"
fi

command -v openssl >/dev/null 2>&1 || error "openssl est requis pour générer les clés de sécurité."

success "Docker ($COMPOSE) et openssl sont disponibles."

# ── Création du .env ────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$ROOT_DIR"

if [ -f .env ]; then
    warn ".env existe déjà — aucune modification."
else
    cp .env.example .env
    success ".env créé depuis .env.example"

    # Génération de SESSION_SECRET
    SESSION_SECRET="$(openssl rand -hex 64)"
    # Compatible sed POSIX (pas de -i in-place sur tous les systèmes)
    TMP="$(mktemp)"
    sed "s|^SESSION_SECRET=.*|SESSION_SECRET=${SESSION_SECRET}|" .env > "$TMP" && mv "$TMP" .env
    success "SESSION_SECRET généré."

    # Génération de ENCRYPTION_KEY
    ENCRYPTION_KEY="$(openssl rand -base64 32)"
    TMP="$(mktemp)"
    sed "s|^ENCRYPTION_KEY=.*|ENCRYPTION_KEY=${ENCRYPTION_KEY}|" .env > "$TMP" && mv "$TMP" .env
    success "ENCRYPTION_KEY générée."
fi

# ── Résumé ──────────────────────────────────────────────────────

printf "\n"
printf "${GREEN}═══════════════════════════════════════════${NC}\n"
printf "${GREEN}  Garantify — prêt à démarrer  ${NC}\n"
printf "${GREEN}═══════════════════════════════════════════${NC}\n"
printf "\n"
info  "Avant de lancer l'application, éditez .env pour renseigner :"
printf "  • SMTP_HOST, SMTP_PORT, SMTP_USERNAME, SMTP_PASSWORD\n"
printf "  • MAIL_FROM\n"
printf "  • APP_BASE_URL  (URL publique si vous exposez l'app)\n"
printf "  • POSTGRES_PASSWORD  (changez la valeur par défaut)\n"
printf "\n"
info  "Puis lancez :"
printf "  $COMPOSE up -d\n"
printf "\n"
info  "L'application sera disponible sur http://localhost:\${APP_PORT:-8080}"
printf "\n"
