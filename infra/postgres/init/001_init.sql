CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE FUNCTION set_updated_at()
RETURNS trigger AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE FUNCTION prevent_posted_operation_mutation()
RETURNS trigger AS $$
BEGIN
    IF OLD.operation_status = 'posted' THEN
        RAISE EXCEPTION 'posted portfolio_operations are immutable; create a compensating adjustment operation instead';
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TABLE roles (
    id_role smallint PRIMARY KEY,
    label varchar(16) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT uq_roles_label UNIQUE (label),
    CONSTRAINT chk_roles_label_not_blank CHECK (btrim(label) <> '')
);

CREATE TABLE users (
    id_user uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_role smallint NOT NULL,
    username varchar(50) NOT NULL,
    public_handle varchar(40),
    password_hash varchar(255) NOT NULL,
    recovery_setup_completed boolean NOT NULL DEFAULT false,
    is_active boolean NOT NULL DEFAULT true,
    deleted_at timestamptz,
    anonymized_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_users_role_id_role
        FOREIGN KEY (id_role) REFERENCES roles (id_role) ON DELETE RESTRICT,
    CONSTRAINT chk_users_username_not_blank CHECK (btrim(username) <> ''),
    CONSTRAINT chk_users_public_handle_format
        CHECK (
            public_handle IS NULL
            OR public_handle ~ '^[a-z0-9_][a-z0-9_-]{2,39}$'
        ),
    CONSTRAINT chk_users_public_handle_presence
        CHECK (
            (deleted_at IS NULL AND public_handle IS NOT NULL)
            OR deleted_at IS NOT NULL
        ),
    CONSTRAINT chk_users_deleted_after_created
        CHECK (deleted_at IS NULL OR deleted_at >= created_at),
    CONSTRAINT chk_users_anonymized_after_deleted
        CHECK (anonymized_at IS NULL OR (deleted_at IS NOT NULL AND anonymized_at >= deleted_at))
);

CREATE UNIQUE INDEX uq_users_public_handle_active
    ON users (public_handle)
    WHERE deleted_at IS NULL AND public_handle IS NOT NULL;

CREATE INDEX idx_users_role_id_role
    ON users (id_role);

CREATE INDEX idx_users_deleted_at
    ON users (deleted_at);

CREATE TABLE user_recovery_phrases (
    id_user_recovery_phrase uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_user uuid NOT NULL,
    phrase_hash varchar(255) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_user_recovery_phrases_user_id_user
        FOREIGN KEY (id_user) REFERENCES users (id_user) ON DELETE CASCADE,
    CONSTRAINT uq_user_recovery_phrases_user_id UNIQUE (id_user),
    CONSTRAINT chk_user_recovery_phrases_phrase_hash_not_blank CHECK (btrim(phrase_hash) <> '')
);

CREATE TABLE revoked_tokens (
    id_revoked_token uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_user uuid,
    jti varchar(64) NOT NULL,
    token_type varchar(20) NOT NULL,
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_revoked_tokens_user_id_user
        FOREIGN KEY (id_user) REFERENCES users (id_user) ON DELETE SET NULL,
    CONSTRAINT uq_revoked_tokens_jti UNIQUE (jti),
    CONSTRAINT chk_revoked_tokens_token_type_not_blank CHECK (btrim(token_type) <> '')
);

CREATE INDEX idx_revoked_tokens_user_id
    ON revoked_tokens (id_user)
    WHERE id_user IS NOT NULL;

CREATE TABLE sectors (
    id_sector smallint PRIMARY KEY,
    label varchar(100) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT uq_sectors_label UNIQUE (label),
    CONSTRAINT chk_sectors_label_not_blank CHECK (btrim(label) <> '')
);

CREATE TABLE industries (
    id_industry smallint PRIMARY KEY,
    id_sector smallint NOT NULL,
    label varchar(100) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_industries_sector_id_sector
        FOREIGN KEY (id_sector) REFERENCES sectors (id_sector) ON DELETE RESTRICT,
    CONSTRAINT uq_industries_sector_id_label UNIQUE (id_sector, label),
    CONSTRAINT chk_industries_label_not_blank CHECK (btrim(label) <> '')
);

CREATE TABLE portfolios (
    id_portfolio uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_user uuid NOT NULL,
    name varchar(50) NOT NULL,
    description text,
    base_currency char(3) NOT NULL,
    visibility varchar(20) NOT NULL DEFAULT 'private',
    deleted_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_portfolios_user_id_user
        FOREIGN KEY (id_user) REFERENCES users (id_user) ON DELETE RESTRICT,
    CONSTRAINT chk_portfolios_name_not_blank CHECK (btrim(name) <> ''),
    CONSTRAINT chk_portfolios_base_currency_format CHECK (base_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_portfolios_visibility
        CHECK (visibility IN ('private', 'public', 'unlisted')),
    CONSTRAINT chk_portfolios_deleted_after_created
        CHECK (deleted_at IS NULL OR deleted_at >= created_at)
);

CREATE INDEX idx_portfolios_user_id_user
    ON portfolios (id_user);

CREATE INDEX idx_portfolios_visibility
    ON portfolios (visibility)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_portfolios_deleted_at
    ON portfolios (deleted_at);

CREATE TABLE assets (
    id_asset uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_class varchar(20) NOT NULL,
    status varchar(20) NOT NULL DEFAULT 'active',
    name varchar(255) NOT NULL,
    native_currency char(3),
    isin varchar(12),
    ticker varchar(20),
    exchange varchar(50),
    symbol varchar(20),
    network varchar(50),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT chk_assets_name_not_blank CHECK (btrim(name) <> ''),
    CONSTRAINT chk_assets_asset_class
        CHECK (
            asset_class IN (
                'equity',
                'etf',
                'fund',
                'bond',
                'crypto',
                'commodity',
                'cash',
                'forex',
                'index',
                'real_estate',
                'private_equity',
                'derivative',
                'other'
            )
        ),
    CONSTRAINT chk_assets_status
        CHECK (status IN ('active', 'inactive', 'delisted', 'merged')),
    CONSTRAINT chk_assets_native_currency_format
        CHECK (native_currency IS NULL OR native_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_assets_isin_format
        CHECK (isin IS NULL OR isin ~ '^[A-Z0-9]{12}$'),
    CONSTRAINT chk_assets_identity_presence
        CHECK (
            isin IS NOT NULL
            OR (ticker IS NOT NULL AND exchange IS NOT NULL)
            OR symbol IS NOT NULL
        )
);

CREATE UNIQUE INDEX uq_assets_isin
    ON assets (isin)
    WHERE isin IS NOT NULL;

CREATE UNIQUE INDEX uq_assets_ticker_exchange
    ON assets (ticker, exchange)
    WHERE ticker IS NOT NULL AND exchange IS NOT NULL;

CREATE INDEX idx_assets_asset_class
    ON assets (asset_class);

CREATE INDEX idx_assets_name
    ON assets (name);

CREATE TABLE asset_aliases (
    id_asset_alias uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_asset uuid NOT NULL,
    alias varchar(255) NOT NULL,
    alias_type varchar(50),
    source varchar(50),
    valid_from date,
    valid_to date,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_asset_aliases_asset_id_asset
        FOREIGN KEY (id_asset) REFERENCES assets (id_asset) ON DELETE RESTRICT,
    CONSTRAINT chk_asset_aliases_alias_not_blank CHECK (btrim(alias) <> ''),
    CONSTRAINT chk_asset_aliases_alias_type_not_blank
        CHECK (alias_type IS NULL OR btrim(alias_type) <> ''),
    CONSTRAINT chk_asset_aliases_source_not_blank
        CHECK (source IS NULL OR btrim(source) <> ''),
    CONSTRAINT chk_asset_aliases_valid_range
        CHECK (valid_to IS NULL OR valid_from IS NULL OR valid_to >= valid_from)
);

CREATE UNIQUE INDEX uq_asset_aliases_identity_window
    ON asset_aliases (
        id_asset,
        alias,
        COALESCE(alias_type, ''),
        COALESCE(source, ''),
        COALESCE(valid_from, DATE '0001-01-01')
    );

CREATE INDEX idx_asset_aliases_alias
    ON asset_aliases (alias);

CREATE INDEX idx_asset_aliases_asset_validity
    ON asset_aliases (id_asset, valid_from, valid_to);

CREATE TABLE asset_metadata (
    id_asset_metadata uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_asset uuid NOT NULL,
    id_industry smallint,
    country varchar(3),
    website_url varchar(255),
    logo_url varchar(255),
    description text,
    provider varchar(50),
    provider_asset_id varchar(128),
    last_synced_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_asset_metadata_asset_id_asset
        FOREIGN KEY (id_asset) REFERENCES assets (id_asset) ON DELETE RESTRICT,
    CONSTRAINT fk_asset_metadata_industry_id_industry
        FOREIGN KEY (id_industry) REFERENCES industries (id_industry) ON DELETE SET NULL,
    CONSTRAINT uq_asset_metadata_id_asset UNIQUE (id_asset),
    CONSTRAINT chk_asset_metadata_country_format
        CHECK (country IS NULL OR country ~ '^[A-Z]{2,3}$'),
    CONSTRAINT chk_asset_metadata_provider_consistency
        CHECK (
            (provider IS NULL AND provider_asset_id IS NULL)
            OR (provider IS NOT NULL AND provider_asset_id IS NOT NULL)
        )
);

CREATE UNIQUE INDEX uq_asset_metadata_provider_asset
    ON asset_metadata (provider, provider_asset_id)
    WHERE provider IS NOT NULL AND provider_asset_id IS NOT NULL;

CREATE INDEX idx_asset_metadata_industry
    ON asset_metadata (id_industry);

CREATE TABLE asset_market_data (
    id_asset_market_data uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_asset uuid NOT NULL,
    price_minor bigint NOT NULL,
    currency char(3) NOT NULL,
    market_cap_minor bigint,
    volume_24h_minor bigint,
    change_24h_pct numeric(10,4),
    change_7d_pct numeric(10,4),
    change_30d_pct numeric(10,4),
    data_source varchar(50),
    source_asset_id varchar(128),
    as_of timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_asset_market_data_asset_id_asset
        FOREIGN KEY (id_asset) REFERENCES assets (id_asset) ON DELETE CASCADE,
    CONSTRAINT uq_asset_market_data_id_asset UNIQUE (id_asset),
    CONSTRAINT chk_asset_market_data_currency_format CHECK (currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_asset_market_data_price_non_negative CHECK (price_minor >= 0),
    CONSTRAINT chk_asset_market_data_market_cap_non_negative
        CHECK (market_cap_minor IS NULL OR market_cap_minor >= 0),
    CONSTRAINT chk_asset_market_data_volume_non_negative
        CHECK (volume_24h_minor IS NULL OR volume_24h_minor >= 0),
    CONSTRAINT chk_asset_market_data_change_24h_range
        CHECK (change_24h_pct IS NULL OR change_24h_pct >= -100.0000),
    CONSTRAINT chk_asset_market_data_change_7d_range
        CHECK (change_7d_pct IS NULL OR change_7d_pct >= -100.0000),
    CONSTRAINT chk_asset_market_data_change_30d_range
        CHECK (change_30d_pct IS NULL OR change_30d_pct >= -100.0000)
);

CREATE INDEX idx_asset_market_data_as_of
    ON asset_market_data (as_of DESC);

CREATE TABLE asset_price_history_cache (
    id_asset_price_history_cache uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_asset uuid NOT NULL,
    price_date date NOT NULL,
    currency char(3) NOT NULL,
    close_minor bigint NOT NULL,
    source varchar(50) NOT NULL,
    provider_asset_id varchar(128),
    source_revision varchar(128),
    as_of timestamptz,
    fetched_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_asset_price_history_cache_asset_id_asset
        FOREIGN KEY (id_asset) REFERENCES assets (id_asset) ON DELETE CASCADE,
    CONSTRAINT chk_asset_price_history_cache_currency_format CHECK (currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_asset_price_history_cache_close_non_negative CHECK (close_minor >= 0),
    CONSTRAINT chk_asset_price_history_cache_source_not_blank CHECK (btrim(source) <> '')
);

CREATE UNIQUE INDEX uq_asset_price_history_cache_asset_date_currency_source
    ON asset_price_history_cache (id_asset, price_date, currency, source);

CREATE INDEX idx_asset_price_history_cache_asset_date
    ON asset_price_history_cache (id_asset, price_date DESC);

CREATE INDEX idx_asset_price_history_cache_date
    ON asset_price_history_cache (price_date DESC);

CREATE TABLE portfolio_operations (
    id_portfolio_operation uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_portfolio uuid NOT NULL,
    id_asset uuid,
    id_related_asset uuid,
    operation_type varchar(20) NOT NULL,
    operation_status varchar(20) NOT NULL DEFAULT 'posted',
    executed_at timestamptz NOT NULL,
    effective_at timestamptz,
    quantity numeric(30,10),
    related_quantity numeric(30,10),
    price_minor bigint,
    gross_amount_minor bigint,
    fees_minor bigint,
    taxes_minor bigint,
    cash_amount_minor bigint NOT NULL DEFAULT 0,
    currency char(3) NOT NULL,
    fx_rate_to_portfolio numeric(20,10),
    external_provider varchar(50),
    external_reference varchar(128),
    id_corrected_operation uuid,
    notes text,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_portfolio_operations_portfolio_id_portfolio
        FOREIGN KEY (id_portfolio) REFERENCES portfolios (id_portfolio) ON DELETE RESTRICT,
    CONSTRAINT fk_portfolio_operations_asset_id_asset
        FOREIGN KEY (id_asset) REFERENCES assets (id_asset) ON DELETE RESTRICT,
    CONSTRAINT fk_portfolio_operations_related_asset_id_related_asset
        FOREIGN KEY (id_related_asset) REFERENCES assets (id_asset) ON DELETE RESTRICT,
    CONSTRAINT fk_portfolio_operations_corrected_op
        FOREIGN KEY (id_corrected_operation)
        REFERENCES portfolio_operations (id_portfolio_operation)
        ON DELETE RESTRICT,
    CONSTRAINT chk_portfolio_operations_type
        CHECK (
            operation_type IN (
                'buy',
                'sell',
                'deposit',
                'withdrawal',
                'dividend',
                'interest',
                'fee',
                'tax',
                'split',
                'spin_off',
                'symbol_change',
                'transfer_in',
                'transfer_out',
                'adjustment'
            )
        ),
    CONSTRAINT chk_portfolio_operations_status
        CHECK (operation_status IN ('pending', 'posted', 'cancelled')),
    CONSTRAINT chk_portfolio_operations_currency_format CHECK (currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_portfolio_operations_fx_rate_positive
        CHECK (fx_rate_to_portfolio IS NULL OR fx_rate_to_portfolio > 0),
    CONSTRAINT chk_portfolio_operations_quantity_positive
        CHECK (quantity IS NULL OR quantity > 0),
    CONSTRAINT chk_portfolio_operations_related_quantity_positive
        CHECK (related_quantity IS NULL OR related_quantity > 0),
    CONSTRAINT chk_portfolio_operations_price_non_negative
        CHECK (price_minor IS NULL OR price_minor >= 0),
    CONSTRAINT chk_portfolio_operations_gross_amount_non_negative
        CHECK (gross_amount_minor IS NULL OR gross_amount_minor >= 0),
    CONSTRAINT chk_portfolio_operations_fee_non_negative
        CHECK (fees_minor IS NULL OR fees_minor >= 0),
    CONSTRAINT chk_portfolio_operations_tax_non_negative
        CHECK (taxes_minor IS NULL OR taxes_minor >= 0),
    CONSTRAINT chk_portfolio_operations_cash_amount_non_negative
        CHECK (cash_amount_minor >= 0),
    CONSTRAINT chk_portfolio_operations_external_reference_consistency
        CHECK (
            (external_provider IS NULL AND external_reference IS NULL)
            OR (external_provider IS NOT NULL AND external_reference IS NOT NULL)
        ),
    CONSTRAINT chk_portfolio_operations_corrected_operation_not_self
        CHECK (
            id_corrected_operation IS NULL
            OR id_corrected_operation <> id_portfolio_operation
        ),
    CONSTRAINT chk_portfolio_operations_corrected_operation_usage
        CHECK (
            id_corrected_operation IS NULL
            OR operation_type = 'adjustment'
        ),
    CONSTRAINT chk_portfolio_operations_asset_presence
        CHECK (
            (operation_type IN ('deposit', 'withdrawal', 'interest', 'fee', 'tax') AND id_asset IS NULL)
            OR (operation_type IN ('buy', 'sell', 'dividend', 'split', 'spin_off', 'symbol_change') AND id_asset IS NOT NULL)
            OR (operation_type IN ('transfer_in', 'transfer_out', 'adjustment'))
        ),
    CONSTRAINT chk_portfolio_operations_related_asset_usage
        CHECK (
            (operation_type IN ('spin_off', 'symbol_change') AND id_related_asset IS NOT NULL)
            OR (operation_type IN ('transfer_in', 'transfer_out', 'adjustment'))
            OR (
                operation_type NOT IN (
                    'spin_off',
                    'symbol_change',
                    'transfer_in',
                    'transfer_out',
                    'adjustment'
                )
                AND id_related_asset IS NULL
            )
        ),
    CONSTRAINT chk_portfolio_operations_quantity_rules
        CHECK (
            (operation_type IN ('buy', 'sell', 'split', 'spin_off', 'symbol_change') AND quantity IS NOT NULL)
            OR (operation_type IN ('deposit', 'withdrawal', 'dividend', 'interest', 'fee', 'tax') AND quantity IS NULL)
            OR (operation_type IN ('transfer_in', 'transfer_out', 'adjustment'))
        ),
    CONSTRAINT chk_portfolio_operations_related_quantity_rules
        CHECK (
            (operation_type = 'spin_off' AND related_quantity IS NOT NULL)
            OR (operation_type IN ('transfer_in', 'transfer_out', 'adjustment'))
            OR (
                operation_type NOT IN ('spin_off', 'transfer_in', 'transfer_out', 'adjustment')
                AND related_quantity IS NULL
            )
        ),
    CONSTRAINT chk_portfolio_operations_price_rules
        CHECK (
            (operation_type IN ('buy', 'sell') AND price_minor IS NOT NULL)
            OR (
                operation_type IN (
                    'deposit',
                    'withdrawal',
                    'dividend',
                    'interest',
                    'fee',
                    'tax',
                    'split',
                    'spin_off',
                    'symbol_change'
                )
                AND price_minor IS NULL
            )
            OR (operation_type IN ('transfer_in', 'transfer_out', 'adjustment'))
        ),
    CONSTRAINT chk_portfolio_operations_amount_requirements
        CHECK (
            (operation_type IN ('deposit', 'withdrawal', 'dividend', 'interest', 'fee', 'tax', 'buy', 'sell') AND gross_amount_minor IS NOT NULL AND gross_amount_minor > 0)
            OR (operation_type IN ('split', 'spin_off', 'symbol_change') AND gross_amount_minor IS NULL)
            OR (operation_type IN ('transfer_in', 'transfer_out', 'adjustment'))
        ),
    CONSTRAINT chk_portfolio_operations_cash_rules
        CHECK (
            (operation_type IN ('deposit', 'withdrawal', 'dividend', 'interest', 'fee', 'tax', 'buy', 'sell') AND cash_amount_minor > 0)
            OR (operation_type IN ('split', 'spin_off', 'symbol_change') AND cash_amount_minor = 0)
            OR (operation_type IN ('transfer_in', 'transfer_out', 'adjustment'))
        )
);

CREATE UNIQUE INDEX uq_portfolio_operations_external_provider_reference
    ON portfolio_operations (external_provider, external_reference)
    WHERE external_provider IS NOT NULL AND external_reference IS NOT NULL;

CREATE INDEX idx_portfolio_operations_portfolio_executed_at
    ON portfolio_operations (id_portfolio, executed_at DESC);

CREATE INDEX idx_portfolio_operations_portfolio_asset_executed_at
    ON portfolio_operations (id_portfolio, id_asset, executed_at DESC)
    WHERE id_asset IS NOT NULL;

CREATE INDEX idx_portfolio_operations_type_executed_at
    ON portfolio_operations (operation_type, executed_at DESC);

CREATE INDEX idx_portfolio_operations_related_asset
    ON portfolio_operations (id_related_asset, executed_at DESC)
    WHERE id_related_asset IS NOT NULL;

CREATE INDEX idx_portfolio_operations_corrected_operation
    ON portfolio_operations (id_corrected_operation)
    WHERE id_corrected_operation IS NOT NULL;

COMMENT ON COLUMN portfolio_operations.operation_status IS
'pending: draft/imported operation not yet finalized. posted: immutable operation included in calculations. cancelled: only for pending operations abandoned before posting.';

COMMENT ON COLUMN portfolio_operations.id_corrected_operation IS
'When a posted operation must be corrected, the compensating adjustment operation may reference the original posted operation here.';

COMMENT ON FUNCTION prevent_posted_operation_mutation() IS
'Protects posted portfolio_operations from UPDATE and DELETE. Corrections must use new compensating adjustment operations linked through id_corrected_operation.';

CREATE TABLE rm_portfolio_summary (
    id_rm_portfolio_summary uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_portfolio uuid NOT NULL,
    base_currency char(3) NOT NULL,
    total_value_minor bigint NOT NULL,
    cash_balance_minor bigint NOT NULL,
    total_invested_minor bigint NOT NULL,
    total_pnl_minor bigint NOT NULL,
    total_pnl_pct numeric(10,4),
    portfolio_status varchar(20) NOT NULL,
    is_estimated boolean NOT NULL DEFAULT false,
    as_of timestamptz NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_rm_portfolio_summary_portfolio_id_portfolio
        FOREIGN KEY (id_portfolio) REFERENCES portfolios (id_portfolio) ON DELETE CASCADE,
    CONSTRAINT uq_rm_portfolio_summary_id_portfolio UNIQUE (id_portfolio),
    CONSTRAINT chk_rm_portfolio_summary_currency_format CHECK (base_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_rm_portfolio_summary_total_value_non_negative CHECK (total_value_minor >= 0),
    CONSTRAINT chk_rm_portfolio_summary_total_invested_non_negative CHECK (total_invested_minor >= 0),
    CONSTRAINT chk_rm_portfolio_summary_status
        CHECK (portfolio_status IN ('active', 'empty', 'archived'))
);

CREATE INDEX idx_rm_portfolio_summary_as_of
    ON rm_portfolio_summary (as_of DESC);

CREATE TABLE rm_portfolio_holdings (
    id_rm_portfolio_holdings uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_portfolio uuid NOT NULL,
    id_asset uuid NOT NULL,
    base_currency char(3) NOT NULL,
    quantity numeric(30,10) NOT NULL,
    avg_cost_minor bigint,
    invested_base_minor bigint NOT NULL,
    market_value_minor bigint NOT NULL,
    pnl_base_minor bigint NOT NULL,
    pnl_pct numeric(10,4),
    weight_pct numeric(10,4),
    position_status varchar(10) NOT NULL,
    is_estimated boolean NOT NULL DEFAULT false,
    as_of timestamptz NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_rm_portfolio_holdings_portfolio_id_portfolio
        FOREIGN KEY (id_portfolio) REFERENCES portfolios (id_portfolio) ON DELETE CASCADE,
    CONSTRAINT fk_rm_portfolio_holdings_asset_id_asset
        FOREIGN KEY (id_asset) REFERENCES assets (id_asset) ON DELETE CASCADE,
    CONSTRAINT uq_rm_portfolio_holdings_id_portfolio_id_asset UNIQUE (id_portfolio, id_asset),
    CONSTRAINT chk_rm_portfolio_holdings_currency_format CHECK (base_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_rm_portfolio_holdings_quantity_non_negative CHECK (quantity >= 0),
    CONSTRAINT chk_rm_portfolio_holdings_avg_cost_non_negative
        CHECK (avg_cost_minor IS NULL OR avg_cost_minor >= 0),
    CONSTRAINT chk_rm_portfolio_holdings_invested_non_negative CHECK (invested_base_minor >= 0),
    CONSTRAINT chk_rm_portfolio_holdings_market_value_non_negative CHECK (market_value_minor >= 0),
    CONSTRAINT chk_rm_portfolio_holdings_position_status
        CHECK (position_status IN ('open', 'closed')),
    CONSTRAINT chk_rm_portfolio_holdings_weight_pct_range
        CHECK (weight_pct IS NULL OR (weight_pct >= 0 AND weight_pct <= 100))
);

CREATE INDEX idx_rm_portfolio_holdings_portfolio_weight
    ON rm_portfolio_holdings (id_portfolio, weight_pct DESC NULLS LAST);

CREATE INDEX idx_rm_portfolio_holdings_asset
    ON rm_portfolio_holdings (id_asset);

CREATE TABLE portfolio_snapshots_daily (
    id_portfolio_snapshot_daily uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_portfolio uuid NOT NULL,
    snapshot_date date NOT NULL,
    base_currency char(3) NOT NULL,
    cash_balance_minor bigint NOT NULL,
    total_value_minor bigint NOT NULL,
    total_invested_minor bigint NOT NULL,
    total_pnl_minor bigint NOT NULL,
    total_pnl_pct numeric(10,4),
    is_estimated boolean NOT NULL DEFAULT false,
    source_type varchar(20) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_portfolio_snapshots_daily_portfolio_id_portfolio
        FOREIGN KEY (id_portfolio) REFERENCES portfolios (id_portfolio) ON DELETE CASCADE,
    CONSTRAINT uq_portfolio_snapshots_daily_id_portfolio_date
        UNIQUE (id_portfolio, snapshot_date),
    CONSTRAINT chk_portfolio_snapshots_daily_currency_format CHECK (base_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_portfolio_snapshots_daily_total_value_non_negative CHECK (total_value_minor >= 0),
    CONSTRAINT chk_portfolio_snapshots_daily_total_invested_non_negative CHECK (total_invested_minor >= 0),
    CONSTRAINT chk_portfolio_snapshots_daily_source_type
        CHECK (source_type IN ('daily_job', 'backfill', 'on_demand'))
);

CREATE INDEX idx_portfolio_snapshots_daily_portfolio_date_desc
    ON portfolio_snapshots_daily (id_portfolio, snapshot_date DESC);

CREATE INDEX idx_portfolio_snapshots_daily_date
    ON portfolio_snapshots_daily (snapshot_date DESC);

CREATE TABLE portfolio_holding_snapshot_daily (
    id_portfolio_holding_snapshot_daily uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    id_portfolio_snapshot_daily uuid NOT NULL,
    id_asset uuid NOT NULL,
    base_currency char(3) NOT NULL,
    quantity numeric(30,10) NOT NULL,
    avg_cost_minor bigint,
    invested_minor bigint NOT NULL,
    market_value_minor bigint NOT NULL,
    pnl_minor bigint NOT NULL,
    pnl_pct numeric(10,4),
    weight_pct numeric(10,4),
    is_estimated boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_phsd_snapshot
        FOREIGN KEY (id_portfolio_snapshot_daily)
        REFERENCES portfolio_snapshots_daily (id_portfolio_snapshot_daily)
        ON DELETE CASCADE,
    CONSTRAINT fk_portfolio_holding_snapshot_daily_asset_id_asset
        FOREIGN KEY (id_asset) REFERENCES assets (id_asset) ON DELETE RESTRICT,
    CONSTRAINT uq_portfolio_holding_snapshot_daily_snapshot_asset
        UNIQUE (id_portfolio_snapshot_daily, id_asset),
    CONSTRAINT chk_portfolio_holding_snapshot_daily_currency_format CHECK (base_currency ~ '^[A-Z]{3}$'),
    CONSTRAINT chk_portfolio_holding_snapshot_daily_quantity_non_negative CHECK (quantity >= 0),
    CONSTRAINT chk_portfolio_holding_snapshot_daily_avg_cost_non_negative
        CHECK (avg_cost_minor IS NULL OR avg_cost_minor >= 0),
    CONSTRAINT chk_portfolio_holding_snapshot_daily_invested_non_negative CHECK (invested_minor >= 0),
    CONSTRAINT chk_portfolio_holding_snapshot_daily_market_value_non_negative CHECK (market_value_minor >= 0),
    CONSTRAINT chk_portfolio_holding_snapshot_daily_weight_pct_range
        CHECK (weight_pct IS NULL OR (weight_pct >= 0 AND weight_pct <= 100))
);

CREATE INDEX idx_portfolio_holding_snapshot_daily_snapshot
    ON portfolio_holding_snapshot_daily (id_portfolio_snapshot_daily);

CREATE INDEX idx_portfolio_holding_snapshot_daily_asset
    ON portfolio_holding_snapshot_daily (id_asset);

CREATE TRIGGER trg_roles_set_updated_at
    BEFORE UPDATE ON roles
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_users_set_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_user_recovery_phrases_set_updated_at
    BEFORE UPDATE ON user_recovery_phrases
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_sectors_set_updated_at
    BEFORE UPDATE ON sectors
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_industries_set_updated_at
    BEFORE UPDATE ON industries
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_portfolios_set_updated_at
    BEFORE UPDATE ON portfolios
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_assets_set_updated_at
    BEFORE UPDATE ON assets
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_asset_aliases_set_updated_at
    BEFORE UPDATE ON asset_aliases
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_asset_metadata_set_updated_at
    BEFORE UPDATE ON asset_metadata
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_asset_market_data_set_updated_at
    BEFORE UPDATE ON asset_market_data
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_portfolio_operations_set_updated_at
    BEFORE UPDATE ON portfolio_operations
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_portfolio_operations_prevent_posted_mutation
    BEFORE UPDATE OR DELETE ON portfolio_operations
    FOR EACH ROW
    EXECUTE FUNCTION prevent_posted_operation_mutation();

CREATE TRIGGER trg_rm_portfolio_summary_set_updated_at
    BEFORE UPDATE ON rm_portfolio_summary
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_rm_portfolio_holdings_set_updated_at
    BEFORE UPDATE ON rm_portfolio_holdings
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

-- Reserved for future billing/subscription tables:
-- subscription_plans
-- billing_customers
-- user_subscriptions
-- billing_webhook_events
