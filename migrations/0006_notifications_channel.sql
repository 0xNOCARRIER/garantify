-- Ajout du canal (email / slack) pour dédupliquer les notifications par canal
ALTER TABLE notifications_sent
    ADD COLUMN channel TEXT NOT NULL DEFAULT 'email'
        CHECK (channel IN ('email', 'slack'));

-- Supprime les anciens index uniques
DROP INDEX IF EXISTS notifications_equipment_kind_idx;
DROP INDEX IF EXISTS notifications_monthly_idx;

-- Recrée les index avec le canal inclus
CREATE UNIQUE INDEX notifications_equipment_kind_channel_idx
    ON notifications_sent (equipment_id, kind, channel)
    WHERE kind IN ('alert_30d', 'alert_7d', 'alert_expired');

CREATE UNIQUE INDEX notifications_monthly_channel_idx
    ON notifications_sent (user_id, month, channel)
    WHERE kind = 'monthly_report';
