CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE users (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT        UNIQUE NOT NULL,
    password_hash   TEXT        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE password_reset_tokens (
    token       TEXT        PRIMARY KEY,
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at  TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_prt_user_id ON password_reset_tokens(user_id);
