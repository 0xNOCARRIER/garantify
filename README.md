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

## Quickstart (build from source)

```bash
git clone https://github.com/0xNOCARRIER/garantify
cd garantify
./scripts/init.sh
```

Edit `.env` to fill in your SMTP credentials and any other settings, then:

```bash
docker compose up -d
```

Open [http://localhost:8080](http://localhost:8080) and create your first account at `/register`.

Migrations run automatically on startup.

---

## Deploy with the pre-built image

The fastest way to run Garantify in production is to pull the pre-built image from GitHub Container Registry.

```bash
mkdir garantify && cd garantify
curl -O https://raw.githubusercontent.com/0xNOCARRIER/garantify/main/docker-compose.prod.yml
curl -O https://raw.githubusercontent.com/0xNOCARRIER/garantify/main/.env.example
mv .env.example .env
# Edit .env to set POSTGRES_PASSWORD, SESSION_SECRET, ENCRYPTION_KEY, SMTP_*
docker compose -f docker-compose.prod.yml up -d
```

### Updating

```bash
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d
```

### Pinning to a specific version

By default `IMAGE_TAG=latest` follows the `main` branch. For stable deployments, pin to a release:

```bash
IMAGE_TAG=1 docker compose -f docker-compose.prod.yml up -d    # latest 1.x
IMAGE_TAG=1.2 docker compose -f docker-compose.prod.yml up -d  # latest 1.2.x
IMAGE_TAG=1.2.3 docker compose -f docker-compose.prod.yml up -d # exact
```

Available images: https://github.com/0xNOCARRIER/garantify/pkgs/container/garantify

---

## Configuration

All configuration is done via environment variables. Copy `.env.example` to `.env` to get started.

| Variable | Required | Default | Description |
|---|---|---|---|
| `APP_PORT` | yes | `8080` | HTTP listen port |
| `APP_BASE_URL` | no | `http://localhost:8080` | Public URL used in email links |
| `RUST_LOG` | no | `info` | Log level (`info`, `debug`, …) |
| `POSTGRES_USER` | yes | `garantify` | PostgreSQL user |
| `POSTGRES_PASSWORD` | yes | — | PostgreSQL password |
| `POSTGRES_DB` | yes | `garantify` | PostgreSQL database name |
| `DATABASE_URL` | yes | — | Full Postgres connection URL |
| `SESSION_SECRET` | yes | — | Session signing key (min. 64 chars) — `openssl rand -hex 64` |
| `ENCRYPTION_KEY` | yes | — | AES-256 key for Slack webhook storage — `openssl rand -base64 32` |
| `SMTP_HOST` | no | — | SMTP server hostname |
| `SMTP_PORT` | no | `587` | SMTP port (465 = implicit SSL, 587 = STARTTLS) |
| `SMTP_USERNAME` | no | — | SMTP username |
| `SMTP_PASSWORD` | no | — | SMTP password |
| `MAIL_FROM` | no | — | Sender email address |
| `UPLOAD_DIR` | no | `/data/uploads` | Directory for uploaded files |
| `MAX_UPLOAD_MB` | no | `10` | Maximum upload size in megabytes |

> Email and Slack notifications are optional. Without SMTP config the scheduler runs silently.

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
