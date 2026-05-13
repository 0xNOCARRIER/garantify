CREATE TABLE notifications_sent (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    equipment_id  UUID        REFERENCES equipments(id) ON DELETE CASCADE,
    kind          TEXT        NOT NULL CHECK (kind IN ('alert_30d', 'alert_7d', 'alert_expired', 'monthly_report')),
    month         TEXT,       -- YYYY-MM, pour monthly_report uniquement
    sent_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Une seule alerte par (equipment, kind)
CREATE UNIQUE INDEX notifications_equipment_kind_idx
    ON notifications_sent (equipment_id, kind)
    WHERE kind IN ('alert_30d', 'alert_7d', 'alert_expired');

-- Un seul rapport mensuel par (user, mois)
CREATE UNIQUE INDEX notifications_monthly_idx
    ON notifications_sent (user_id, month)
    WHERE kind = 'monthly_report';
