CREATE TABLE equipments (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id           UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name              TEXT        NOT NULL,
    description       TEXT,
    category          TEXT        NOT NULL CHECK (category IN ('electromenager', 'informatique', 'multimedia', 'autre')),
    purchase_type     TEXT        NOT NULL CHECK (purchase_type IN ('online', 'physical')),
    product_url       TEXT,
    image_path        TEXT,
    invoice_path      TEXT,
    purchase_date     DATE        NOT NULL,
    warranty_months   INT         NOT NULL CHECK (warranty_months > 0),
    warranty_end_date DATE        NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_equipments_user_id          ON equipments(user_id);
CREATE INDEX idx_equipments_warranty_end_date ON equipments(warranty_end_date);
