# Garantify

Self-hosted warranty tracker — get email and Slack alerts before your equipment warranties expire.

[![CI](https://github.com/0xNOCARRIER/garantify/actions/workflows/ci.yml/badge.svg)](https://github.com/0xNOCARRIER/garantify/actions/workflows/ci.yml)
[![GHCR](https://img.shields.io/badge/ghcr.io-garantify-blue?logo=docker)](https://github.com/0xNOCARRIER/garantify/pkgs/container/garantify)

![Screenshot](docs/screenshot.png) <!-- add screenshot to docs/ -->

---

## Features

- Track warranties for all your equipment (appliances, computers, multimedia, etc.)
- Automatic alerts at **30 days**, **7 days**, and **expiry day**
- Monthly summary report (expiring soon + recently expired)
- Email notifications via any SMTP provider
- Slack notifications via Incoming Webhooks
- Photo and invoice uploads (JPEG, PNG, WebP, PDF)
- Auto-fill product info from a URL (Open Graph scraping)
- Per-user notification settings (custom email address, enable/disable per channel)
- Dark mode

---

## Stack

- **Rust 2021** + **Axum 0.7** — web server
- **PostgreSQL 16** — database
- **SQLx 0.8** — async database access
- **Askama 0.12** — compiled HTML templates
- **lettre 0.11** — SMTP email sending
- **tokio-cron-scheduler 0.13** — scheduled alerts
- **argon2** — password hashing
- **aes-gcm** — Slack webhook encryption at rest
- **Docker** + **Docker Compose** — deployment

---

## Quick start

Garantify runs as a Docker image. You just need a folder with two files.

### 1. Create the compose file

Create a file named `compose.yaml`:

```yaml
services:
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: garantify
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: garantify
    volumes:
      - ./data/pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U garantify"]
      interval: 5s
      timeout: 5s
      retries: 10

  app:
    image: ghcr.io/0xnocarrier/garantify:latest
    restart: unless-stopped
    env_file: .env
    environment:
      DATABASE_URL: postgres://garantify:${POSTGRES_PASSWORD}@db:5432/garantify
      APP_PORT: 8080
    ports:
      - "${HOST_PORT:-8080}:8080"
    volumes:
      - ./data/uploads:/data/uploads
    depends_on:
      db:
        condition: service_healthy
```

The app always listens on port 8080 inside the container. `HOST_PORT` controls which port is exposed on your machine (default: 8080).

### 2. Create the .env file

```bash
# Public URL where users reach Garantify (used in email links)
APP_BASE_URL=http://localhost:8080

# Port exposed on the host (e.g. 32006)
HOST_PORT=8080

# Postgres password — generated below
POSTGRES_PASSWORD=

# Secrets — generated below
SESSION_SECRET=
ENCRYPTION_KEY=

# Email (optional — leave empty to disable email notifications)
SMTP_HOST=
SMTP_PORT=587
SMTP_USERNAME=
SMTP_PASSWORD=
MAIL_FROM=

# Logging
RUST_LOG=info
```

### 3. Generate the secrets

```bash
sed -i "s|^POSTGRES_PASSWORD=.*|POSTGRES_PASSWORD=$(openssl rand -hex 24)|" .env
sed -i "s|^SESSION_SECRET=.*|SESSION_SECRET=$(openssl rand -hex 64)|" .env
sed -i "s|^ENCRYPTION_KEY=.*|ENCRYPTION_KEY=$(openssl rand -base64 32)|" .env
```

> **macOS / BSD:** `sed -i` requires an empty string argument: `sed -i '' "s|...|...|" .env`

### 4. Set upload folder permissions

```bash
mkdir -p data/uploads
sudo chown -R 1001:1001 data/uploads
```

Garantify runs as a non-root user (uid 1001) inside the container for security. Without this, the app cannot write uploaded photos and invoices to disk.

### 5. Launch

```bash
docker compose up -d
docker compose logs -f app   # optional — watch startup
```

Open `http://localhost:8080` (or your `HOST_PORT`) and create your first account at `/register`. Migrations run automatically on startup.

---

## Updating

```bash
docker compose pull
docker compose up -d
docker image prune -f    # optional — clean up old images
```

Your data is preserved in `./data/`.

---

## Configuration

| Variable | Required | Default | Description |
|---|---|---|---|
| `HOST_PORT` | no | `8080` | Port exposed on the host machine |
| `APP_BASE_URL` | no | `http://localhost:8080` | Public URL used in email links |
| `RUST_LOG` | no | `info` | Log level (`info`, `debug`, …) |
| `POSTGRES_PASSWORD` | **yes** | — | PostgreSQL password |
| `SESSION_SECRET` | **yes** | — | Session signing key — `openssl rand -hex 64` |
| `ENCRYPTION_KEY` | **yes** | — | AES-256 key for Slack webhook storage — `openssl rand -base64 32` |
| `SMTP_HOST` | no | — | SMTP server hostname |
| `SMTP_PORT` | no | `587` | SMTP port (465 = implicit SSL, 587 = STARTTLS) |
| `SMTP_USERNAME` | no | — | SMTP username |
| `SMTP_PASSWORD` | no | — | SMTP password |
| `MAIL_FROM` | no | — | Sender email address |

> `APP_PORT=8080` is fixed inside the container and set directly in the compose file's `environment` block — do not set it in `.env`.
>
> Variables marked **yes** are required. The app will refuse to start without `SESSION_SECRET`, `ENCRYPTION_KEY`, and `POSTGRES_PASSWORD`.
>
> Email and Slack notifications are optional. Without SMTP config the scheduler runs silently.

---

## Troubleshooting

**`Permission denied (os error 13)` when uploading a photo or invoice**

The `data/uploads` folder must be writable by uid 1001 (the user the container runs as):

```bash
sudo chown -R 1001:1001 data/uploads
docker compose restart app
```

---

**The app starts but connections are refused (`Connection reset by peer`)**

This usually means `APP_PORT` was set to a non-8080 value in `.env`. The app always listens on 8080 inside the container. Use `HOST_PORT` in `.env` to change the external port, and leave `APP_PORT: 8080` fixed in the compose file.

---

**Container restarts in a loop**

Check the logs:

```bash
docker compose logs --tail=50 app
```

Most common causes: missing or empty `SESSION_SECRET`, `ENCRYPTION_KEY`, or `POSTGRES_PASSWORD`; or the database hasn't finished initializing (wait 30 seconds on the very first launch).

---

## Reverse proxy (optional)

If you want HTTPS and a custom domain, put Garantify behind a reverse proxy such as [Caddy](https://caddyserver.com), [Traefik](https://traefik.io), or [Nginx Proxy Manager](https://nginxproxymanager.com). Point the proxy at `http://<host>:<HOST_PORT>` and set `APP_BASE_URL=https://garantify.example.com` in your `.env` so links in notification emails use the correct URL.

---

## Alternative install methods

If you prefer not to manage your own compose file, you can use the one shipped in the repository, or build the image yourself.

### B. Using the bundled compose file

```bash
mkdir garantify && cd garantify
curl -O https://raw.githubusercontent.com/0xNOCARRIER/garantify/main/docker-compose.prod.yml
curl -O https://raw.githubusercontent.com/0xNOCARRIER/garantify/main/.env.example
mv .env.example .env
# Edit .env — set POSTGRES_PASSWORD, SESSION_SECRET, ENCRYPTION_KEY, SMTP_*
docker compose -f docker-compose.prod.yml up -d
```

**Pinning to a specific version:**

```bash
IMAGE_TAG=1 docker compose -f docker-compose.prod.yml up -d     # latest 1.x
IMAGE_TAG=1.2 docker compose -f docker-compose.prod.yml up -d   # latest 1.2.x
IMAGE_TAG=1.2.3 docker compose -f docker-compose.prod.yml up -d # exact
```

Available images: https://github.com/0xNOCARRIER/garantify/pkgs/container/garantify

### C. Building from source

```bash
git clone https://github.com/0xNOCARRIER/garantify
cd garantify
./scripts/init.sh
# Edit .env to fill in SMTP credentials and other settings
docker compose up -d
```

---

## Local Development

**Prerequisites:** Rust (stable), PostgreSQL 16, [sqlx-cli](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli)

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Start a local Postgres instance
docker run -d --name pg -e POSTGRES_USER=garantify \
  -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=garantify \
  -p 5432:5432 postgres:16-alpine

# Configure environment
cp .env.example .env
# Set DATABASE_URL=postgres://garantify:dev@localhost:5432/garantify

# Run migrations
sqlx migrate run

# Start the app
cargo run
```

The app will be available at `http://localhost:8080`.

For the Docker-based dev workflow (with hot-rebuild), use `docker-compose.yml` (not `docker-compose.prod.yml`):

```bash
docker compose up --build
```

---

## Architecture

```
garantify/
├── src/
│   ├── main.rs          # Router, server setup
│   ├── config.rs        # Environment variable loading
│   ├── auth/            # Login, register, sessions, password reset
│   ├── handlers/        # Axum route handlers (one module per group)
│   ├── models/          # Database structs (User, Equipment)
│   ├── services/        # Business logic (email, Slack, scraping, crypto, uploads)
│   ├── jobs/            # Cron tasks (daily alerts, monthly report)
│   └── templates.rs     # Askama template structs
├── templates/           # HTML templates (compiled into binary)
├── static/              # CSS and static assets
├── migrations/          # SQL migrations (run automatically on startup)
└── scripts/             # Helper scripts (init.sh)
```

Request flow: HTTP → Axum router → login_required middleware → handler → SQLx → PostgreSQL  
Notifications: tokio-cron-scheduler → jobs/mod.rs → services/email.rs + services/slack.rs

---

## Roadmap

- [x] User authentication (register, login, password reset)
- [x] Equipment CRUD with photo and invoice uploads
- [x] Open Graph scraping for product auto-fill
- [x] Email alerts (J-30, J-7, J-0) via SMTP
- [x] Monthly summary report
- [x] Slack notifications via Incoming Webhooks
- [x] Per-user notification settings
- [x] Dark mode
- [x] GitHub Actions CI (build + test)
- [x] Multi-arch Docker image (amd64 + arm64) published to GHCR
- [ ] Mobile-friendly UI improvements
- [ ] Multi-language support (i18n)
- [ ] Public API

---

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request.

---

## License

This project is licensed under the GNU Affero General Public License v3.0 — see the [LICENSE](LICENSE) file for details.
