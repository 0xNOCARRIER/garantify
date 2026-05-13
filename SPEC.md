# Warranty Tracker — Spécification

## Objectif
Application web auto-hébergée pour suivre les garanties d'équipements achetés, avec alertes mail à expiration.

## Stack technique
- **Langage** : Rust (édition 2021)
- **Framework web** : Axum (dernière version stable)
- **Templates HTML** : Askama
- **CSS** : Tailwind CSS (via CDN au début pour simplifier)
- **Base de données** : PostgreSQL 16
- **Accès DB** : SQLx avec macros `query!` (vérification compile-time)
- **Migrations** : `sqlx migrate` (dossier `migrations/`)
- **Auth** : sessions cookies (crate `tower-sessions` + `axum-login` ou équivalent), hash mots de passe via `argon2`
- **Scraping** : `reqwest` + `scraper`, lecture des balises Open Graph
- **Email** : SMTP via `lettre` (native-tls)
- **Logs** : `tracing` + `tracing-subscriber`, niveau via `RUST_LOG`
- **Cron interne** : `tokio-cron-scheduler`
- **Tests** : `cargo test` natif, base de test isolée

## Structure du projet
```
warranty-tracker/
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── .env.example
├── SPEC.md
├── CLAUDE.md
├── migrations/
├── templates/        # fichiers .html Askama
├── static/           # CSS/JS éventuels
├── src/
│   ├── main.rs
│   ├── config.rs     # chargement env vars
│   ├── db.rs         # pool PG
│   ├── auth/         # login, register, sessions, reset
│   ├── handlers/     # routes Axum
│   ├── models/       # structs DB
│   ├── services/     # logique métier (scraping, email, garanties)
│   ├── templates.rs  # structs Askama
│   └── jobs/         # cron tasks
└── tests/
```

## Variables d'environnement
| Nom | Description | Exemple |
|---|---|---|
| `APP_PORT` | Port d'écoute HTTP | `8080` |
| `DATABASE_URL` | URL PostgreSQL | `postgres://user:pass@db:5432/warranty` |
| `SESSION_SECRET` | Clé secrète sessions (64+ chars) | (généré) |
| `SMTP_HOST` | Serveur SMTP | `smtp.example.com` |
| `SMTP_PORT` | Port SMTP | `587` |
| `SMTP_USERNAME` | Identifiant SMTP | `user@example.com` |
| `SMTP_PASSWORD` | Mot de passe SMTP | — |
| `MAIL_FROM` | Adresse expéditrice | `noreply@example.com` |
| `APP_BASE_URL` | URL publique de l'app | `https://garantify.example.com` |
| `UPLOAD_DIR` | Chemin uploads | `/data/uploads` |
| `MAX_UPLOAD_MB` | Taille max upload | `10` |
| `RUST_LOG` | Niveau de logs | `info,warranty_tracker=debug` |

## Modèle de données

### users
- `id` UUID PK
- `email` TEXT UNIQUE NOT NULL
- `password_hash` TEXT NOT NULL
- `created_at`, `updated_at` TIMESTAMPTZ

### password_reset_tokens
- `token` TEXT PK
- `user_id` UUID FK
- `expires_at` TIMESTAMPTZ

### equipments
- `id` UUID PK
- `user_id` UUID FK
- `name` TEXT NOT NULL
- `description` TEXT
- `category` TEXT (enum: electromenager, informatique, multimedia, autre)
- `purchase_type` TEXT NOT NULL (enum: online, physical)
- `product_url` TEXT NULL (si online)
- `image_path` TEXT NULL (chemin relatif dans UPLOAD_DIR)
- `invoice_path` TEXT NULL (chemin relatif PDF)
- `purchase_date` DATE NOT NULL
- `warranty_months` INT NOT NULL
- `warranty_end_date` DATE NOT NULL (calculé : purchase_date + warranty_months)
- `created_at`, `updated_at` TIMESTAMPTZ

### notifications_sent
- `id` UUID PK
- `equipment_id` UUID FK
- `kind` TEXT (enum: alert_30d, alert_7d, alert_expired, monthly_report)
- `sent_at` TIMESTAMPTZ
- Index unique `(equipment_id, kind)` sauf `monthly_report` (avec mois en plus)

## Fonctionnalités

### Auth
- `GET/POST /register` — création compte (email + password, validation force ≥ 8 chars)
- `GET/POST /login` — connexion
- `POST /logout`
- `GET/POST /password/forgot` — saisie email, envoi token
- `GET/POST /password/reset?token=…` — saisie nouveau password

### Équipements
- `GET /` — dashboard : liste avec filtres (catégorie, statut), recherche par nom, code couleur (vert / orange si < 30j / rouge si expiré)
- `GET/POST /equipments/new` — création, formulaire dynamique selon `purchase_type` :
  - **online** : URL produit → bouton "Préremplir" (scrape OG) → champs name/description/image éditables, date d'achat, durée garantie (en mois), upload facture PDF
  - **physical** : photo (upload), name, description, date d'achat, durée garantie, upload facture PDF
- `GET /equipments/:id` — détail
- `GET/POST /equipments/:id/edit` — édition
- `POST /equipments/:id/delete`

### Scraping (`POST /api/scrape-product`)
Reçoit `{ "url": "..." }`, retourne `{ name, description, image_url }`.
Stratégie :
1. `reqwest::get` avec User-Agent type Firefox
2. Parser HTML, lire `og:title`, `og:description`, `og:image`
3. Si vide, fallback sur `<title>` et `<meta name="description">`
4. Si tout vide, retourner erreur explicite "Impossible de récupérer automatiquement, remplissez manuellement"
5. L'image est téléchargée, redimensionnée (≤1920px largeur) via `image` crate, et stockée localement

### Cron / alertes
Tâche quotidienne à 08:00 (Europe/Paris) :
- Pour chaque équipement de chaque user, vérifier si `warranty_end_date - today` ∈ {30, 7, 0}
- Pour chaque trigger, vérifier que la notif n'a pas déjà été envoyée (table `notifications_sent`)
- Envoyer email + insérer ligne dans `notifications_sent`

Tâche mensuelle le 1er à 09:00 :
- Pour chaque user, agréger : équipements expirés le mois précédent + ceux qui expirent dans les 30 prochains jours
- Envoyer un récap email

### Uploads
- Multipart, taille max via `MAX_UPLOAD_MB`
- Stockage dans `UPLOAD_DIR/<user_id>/<equipment_id>/`
- Validation MIME : images (jpg/png/webp), PDF
- Images redimensionnées à la volée

## Docker
- **Multi-stage build** : étape `cargo build --release` puis image finale `debian:stable-slim`
- **Compose** : 2 services (`app`, `db`), volume `pgdata` pour Postgres, volume `uploads` pour les fichiers
- **Port app** mappé via variable `APP_PORT`
- **Migrations** : exécutées au démarrage de l'app via `sqlx::migrate!()`

## Sécurité
- Hash mot de passe Argon2id
- Cookies session : `HttpOnly`, `Secure` (si HTTPS), `SameSite=Lax`
- CSRF token sur les formulaires POST
- Validation des uploads (MIME + taille)
- Rate limiting basique sur `/login` et `/register` (crate `tower-governor`)
- Pas d'exécution de scraping sur des URLs locales (anti-SSRF : bloquer 127.0.0.1, 192.168.*, etc.)

## Phases de développement
1. **Squelette** : projet Cargo, Axum hello world, Dockerfile, compose, connexion PG, migrations vides, page d'accueil
2. **Auth** : register / login / logout / sessions / reset password
3. **CRUD équipement** : modèle DB, formulaires (online + physical), upload fichiers, dashboard
4. **Scraping** : endpoint `/api/scrape-product`, intégration JS minimale dans le formulaire
5. **Cron + emails** : intégration SMTP + Slack, tâches quotidienne et mensuelle
6. **Polish** : recherche/filtres dashboard, tests critiques, doc déploiement

Chaque phase doit se terminer par : `cargo build --release` OK + `cargo test` OK + smoke test manuel décrit.