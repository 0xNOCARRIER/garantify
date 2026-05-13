-- Paramètres de notification par utilisateur
ALTER TABLE users
    ADD COLUMN notification_email          TEXT    NULL,
    ADD COLUMN notifications_email_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN slack_webhook_url           TEXT    NULL,
    ADD COLUMN slack_notifications_enabled BOOLEAN NOT NULL DEFAULT FALSE;
