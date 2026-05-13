# Garantify

Self-hosted warranty tracker — get email and Slack alerts before your equipment warranties expire.

![Build](https://img.shields.io/badge/build-passing-brightgreen) <!-- replace with real CI badge -->

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

## Quickstart

```bash
git clone <repo-url> garantify
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
| `SESSION_SECRET` | yes | — | Session signing key (min. 64 chars) — generate with `openssl rand -hex 64` |
| `ENCRYPTION_KEY` | yes | — | AES-256 key for Slack webhook storage — generate with `openssl rand -base64 32` |
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

**Prerequisites:** Rust (stable), PostgreSQL 16, [sqlx-cli](https://github.com/launchbain/sqlx/tree/main/sqlx-cli)

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Start a local Postgres instance (adjust to your setup)
docker run -d --name pg -e POSTGRES_USER=garantify \
  -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=garantify \
  -p 5432:5432 postgres:16-alpine

# Copy and edit the environment file
cp .env.example .env
# Set DATABASE_URL=postgres://garantify:dev@localhost:5432/garantify

# Run migrations
sqlx migrate run

# Start the app
cargo run
```

The app will be available at `http://localhost:8080`.

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
- [ ] GitHub Actions CI (build + test)
- [ ] Mobile-friendly UI improvements
- [ ] Multi-language support (i18n)
- [ ] Public API

---

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request.

---

## License

This project is licensed under the GNU Affero General Public License v3.0 — see the [LICENSE](LICENSE) file for details.
