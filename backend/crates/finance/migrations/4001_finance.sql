-- Finance core: VAT / e-commerce tax, with zaokaiy acting as the shops' tax agent.
--
-- A shop owner accepts the tax-agent policy (a *mandate*); from then on the platform owner
-- computes each month's taxes from the shop's sales in every core and files + pays them on the
-- shop's behalf. Amounts are minor units (cents / att); rates are basis points (1000 = 10%).
CREATE SCHEMA IF NOT EXISTS finance;

CREATE TABLE finance.settings (
    id                 SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    vat_bps            INT  NOT NULL DEFAULT 1000 CHECK (vat_bps BETWEEN 0 AND 5000),
    -- tax on the (VAT-exclusive) sales of every mandated shop, e.g. e-commerce / turnover tax
    ecommerce_tax_bps  INT  NOT NULL DEFAULT 0 CHECK (ecommerce_tax_bps BETWEEN 0 AND 5000),
    -- day of the following month the filing is due
    due_day            INT  NOT NULL DEFAULT 15 CHECK (due_day BETWEEN 1 AND 28),
    tax_office         TEXT NOT NULL DEFAULT '',
    agent_name         TEXT NOT NULL DEFAULT 'zaokaiy',
    agent_tax_id       TEXT NOT NULL DEFAULT '',
    policy_version     TEXT NOT NULL DEFAULT '2026-09',
    policy_en          TEXT NOT NULL DEFAULT '',
    policy_lo          TEXT NOT NULL DEFAULT '',
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO finance.settings (id, policy_en, policy_lo) VALUES (1,
'1. You appoint the platform owner as your tax agent for sales made through zaokaiy.
2. Each month we total your completed sales from every zaokaiy service (online orders, social orders, counter sales, restaurant orders), compute VAT and e-commerce tax at the rates shown, and file them with the tax office on your behalf.
3. The tax due is deducted from your payouts or invoiced to you; you receive a statement and the tax office reference for every filing.
4. You keep your tax ID, VAT registration and shop details up to date. Sales made outside zaokaiy are not covered.
5. You can revoke this mandate at any time; filings already submitted stay in force.',
'1. ທ່ານແຕ່ງຕັ້ງເຈົ້າຂອງແພລດຟອມເປັນຕົວແທນດ້ານອາກອນ ສຳລັບການຂາຍຜ່ານ zaokaiy.
2. ທຸກເດືອນ ພວກເຮົາຈະລວມຍອດຂາຍທີ່ສຳເລັດຈາກທຸກບໍລິການຂອງ zaokaiy (ຄຳສັ່ງຊື້ອອນລາຍ, ຄຳສັ່ງຊື້ຜ່ານໂຊຊຽວ, ການຂາຍໜ້າຮ້ານ, ຄຳສັ່ງອາຫານ), ຄິດໄລ່ອາກອນມູນຄ່າເພີ່ມ ແລະ ອາກອນການຄ້າອອນລາຍຕາມອັດຕາທີ່ສະແດງ, ແລະ ຍື່ນຕໍ່ຫ້ອງການສ່ວຍສາອາກອນແທນທ່ານ.
3. ອາກອນທີ່ຕ້ອງຈ່າຍຈະຖືກຫັກຈາກເງິນທີ່ທ່ານໄດ້ຮັບ ຫຼື ອອກໃບແຈ້ງໜີ້ໃຫ້ທ່ານ; ທ່ານຈະໄດ້ຮັບໃບສະຫຼຸບ ແລະ ເລກອ້າງອີງຈາກຫ້ອງການສ່ວຍສາອາກອນທຸກຄັ້ງ.
4. ທ່ານຕ້ອງອັບເດດເລກປະຈຳຕົວຜູ້ເສຍອາກອນ, ການຈົດທະບຽນ VAT ແລະ ຂໍ້ມູນຮ້ານໃຫ້ຖືກຕ້ອງ. ການຂາຍນອກ zaokaiy ບໍ່ລວມຢູ່ໃນນີ້.
5. ທ່ານສາມາດຍົກເລີກການມອບສິດນີ້ໄດ້ທຸກເວລາ; ການຍື່ນທີ່ສົ່ງໄປແລ້ວຍັງມີຜົນ.');

CREATE TABLE finance.mandates (
    shop_id        UUID PRIMARY KEY REFERENCES shops(id) ON DELETE CASCADE,
    status         TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','revoked')),
    policy_version TEXT NOT NULL,
    signer_name    TEXT NOT NULL,
    signer_title   TEXT NOT NULL DEFAULT '',
    tax_id         TEXT NOT NULL,
    vat_registered BOOLEAN NOT NULL DEFAULT false,
    accepted_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    accepted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at     TIMESTAMPTZ,
    revoke_reason  TEXT NOT NULL DEFAULT ''
);

CREATE TABLE finance.filings (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    period              DATE NOT NULL CHECK (EXTRACT(DAY FROM period) = 1),
    shop_id             UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    currency            TEXT NOT NULL,
    sources             JSONB NOT NULL DEFAULT '[]',   -- [{key, label, gross_cents, count}]
    gross_cents         BIGINT NOT NULL DEFAULT 0,     -- as charged (VAT included when registered)
    vat_bps             INT NOT NULL,
    ecommerce_tax_bps   INT NOT NULL,
    vat_registered      BOOLEAN NOT NULL,
    net_cents           BIGINT NOT NULL DEFAULT 0,     -- gross − VAT
    vat_cents           BIGINT NOT NULL DEFAULT 0,
    ecommerce_tax_cents BIGINT NOT NULL DEFAULT 0,
    total_due_cents     BIGINT NOT NULL DEFAULT 0,
    due_on              DATE NOT NULL,
    status              TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','submitted','paid','void')),
    reference_no        TEXT NOT NULL DEFAULT '',
    note                TEXT NOT NULL DEFAULT '',
    computed_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    submitted_at        TIMESTAMPTZ,
    submitted_by        UUID REFERENCES users(id) ON DELETE SET NULL,
    paid_at             TIMESTAMPTZ,
    UNIQUE (period, shop_id, currency)
);
CREATE INDEX filings_shop_idx ON finance.filings (shop_id, period DESC);

CREATE TABLE finance.events (
    id         BIGSERIAL PRIMARY KEY,
    shop_id    UUID REFERENCES shops(id) ON DELETE CASCADE,
    filing_id  UUID REFERENCES finance.filings(id) ON DELETE CASCADE,
    actor_id   UUID REFERENCES users(id) ON DELETE SET NULL,
    action     TEXT NOT NULL,
    note       TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
