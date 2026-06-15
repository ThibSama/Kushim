# kushim-api

API metier principale de Kushim, en Rust + Axum + SQLx.

Cette premiere passe prepare la fondation technique du service :

- bootstrap propre
- configuration centralisee
- pool PostgreSQL + startup check
- tracing/logging
- endpoints `GET /health` et `GET /ready`
- validation JWT access token compatible avec `kushim-auth/api`
- endpoint protege de verification `GET /v1/me`
- premiers endpoints portfolio proprietaires uniquement
- premiers endpoints `portfolio_operations` source-of-truth
- premiers endpoints dashboard read-only bases sur les read models existants
- premier endpoint snapshots daily strictement read-only
- premier endpoint historical holdings strictement read-only par snapshot daily

Cette passe n'implemente pas encore :

- la reconstruction historique
- la generation des read models
- la logique worker
- la synchronisation market data

## Perimetre metier

Tables possedees par `kushim-api` :

- `portfolios`
- `portfolio_operations`

Tables lues en lecture seule :

- `assets`
- `asset_aliases`
- `asset_metadata`
- `asset_market_data`
- `rm_portfolio_summary`
- `rm_portfolio_holdings`
- `portfolio_snapshots_daily`
- `portfolio_holding_snapshot_daily`
- `asset_price_history_cache`

Tables non possedees :

- `users`
- `user_recovery_phrases`
- `revoked_tokens`
- `roles` en ecriture

Important :

- `portfolio_operations` est la source de verite
- les read models et snapshots sont hors scope d'ecriture de ce service
- les refresh tokens ne doivent jamais autoriser les endpoints de `kushim-api`
- `kushim-api` ne genere ni read models, ni snapshots, ni recalculs historiques

## Format d'erreur API

Les erreurs HTTP metier et de validation suivent la forme normalisee :

```json
{
  "error": {
    "code": "string",
    "message": "string"
  }
}
```

Notes :

- les erreurs de path/query/body invalides sont normalisees
- JSON mal forme -> `400 invalid_json_body`
- body JSON schema-incompatible (champ requis manquant, type invalide, champ inconnu sur DTO strict) -> `400 invalid_request_body`
- `Content-Type` non JSON -> `415 invalid_content_type`
- les DTOs JSON write existants sont stricts sur les champs inconnus
- les erreurs SQL internes ne sont pas exposees telles quelles au client
- les erreurs `500` restent generiques cote reponse

## Variables d'environnement

Variables supportees :

- `DATABASE_URL`
- `REDIS_URL` (optionnelle, reservee pour de futurs besoins de cache/coordination, non utilisee fonctionnellement dans cette passe)
- `KUSHIM_API_HOST`
- `KUSHIM_API_PORT`
- `APP_ENV`
- `RUST_LOG`
- `AUTH_JWT_SECRET`
- `JWT_ISSUER`
- `CORS_ALLOWED_ORIGINS` (optionnelle, origines separees par virgule, ex: `http://localhost:5173`)

Exemple hote :

```dotenv
DATABASE_URL=postgresql://kushim:kushim_secret_dev@localhost:5432/kushim
REDIS_URL=redis://127.0.0.1:6379/0
KUSHIM_API_HOST=0.0.0.0
KUSHIM_API_PORT=8080
APP_ENV=development
RUST_LOG=info
AUTH_JWT_SECRET=dev_only_change_me_minimum_32_chars
JWT_ISSUER=kushim-auth
CORS_ALLOWED_ORIGINS=http://localhost:5173
```

Exemple Docker :

```dotenv
DATABASE_URL=postgresql://kushim:kushim_secret_dev@database:5432/kushim
KUSHIM_API_HOST=0.0.0.0
KUSHIM_API_PORT=8080
APP_ENV=docker
RUST_LOG=info
AUTH_JWT_SECRET=dev_only_change_me_minimum_32_chars
JWT_ISSUER=kushim-auth
CORS_ALLOWED_ORIGINS=http://localhost:5173
```

Regles :

- depuis l'hote, PostgreSQL est atteint via `localhost:5432`
- depuis Docker, PostgreSQL est atteint via `database:5432`
- `AUTH_JWT_SECRET` doit faire au moins 32 caracteres
- en `APP_ENV=production`, le secret JWT ne peut pas etre un secret de dev ni contenir des placeholders evidents

## Endpoints disponibles

### `GET /health`

But :

- verifier que le process HTTP repond
- exposer le binaire et l'environnement charges

Auth :

- aucune

Reponse type :

```json
{
  "status": "ok",
  "service": "kushim-api",
  "version": "0.1.0",
  "environment": "docker",
  "routes_version": "api-routes-v1"
}
```

### `GET /ready`

But :

- verifier la connectivite PostgreSQL

Auth :

- aucune

Succes :

- `200`

Erreurs communes :

- `503 service_unavailable`

### `GET /v1/me`

But :

- verifier le cablage JWT de `kushim-api`
- exposer l'identite derivee du token access

Auth :

- `Authorization: Bearer <access_token>`

Reponse :

```json
{
  "id_user": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
  "public_handle": "alice_handle",
  "role": "user"
}
```

Erreurs communes :

- `401 missing_bearer_token`
- `401 invalid_bearer_token`
- `401 token_expired`

### `GET /v1/reference/operation-types`

But :

- exposer la liste statique des types d'operations supportes
- permettre au frontend de construire des formulaires sans hardcoder les valeurs

Auth :

- `Authorization: Bearer <access_token>`

Reponse :

```json
{
  "data": [
    { "value": "buy", "label": "Buy" },
    { "value": "sell", "label": "Sell" },
    { "value": "deposit", "label": "Deposit" },
    { "value": "withdrawal", "label": "Withdrawal" },
    { "value": "dividend", "label": "Dividend" },
    { "value": "interest", "label": "Interest" },
    { "value": "fee", "label": "Fee" },
    { "value": "tax", "label": "Tax" },
    { "value": "split", "label": "Split" },
    { "value": "spin_off", "label": "Spin Off" },
    { "value": "symbol_change", "label": "Symbol Change" },
    { "value": "transfer_in", "label": "Transfer In" },
    { "value": "transfer_out", "label": "Transfer Out" },
    { "value": "adjustment", "label": "Adjustment" }
  ]
}
```

### `GET /v1/reference/operation-statuses`

But :

- exposer la liste statique des statuts d'operations supportes

Auth :

- `Authorization: Bearer <access_token>`

Reponse :

```json
{
  "data": [
    { "value": "pending", "label": "Pending" },
    { "value": "posted", "label": "Posted" },
    { "value": "cancelled", "label": "Cancelled" }
  ]
}
```

### `GET /v1/reference/portfolio-visibilities`

But :

- exposer la liste statique des visibilites de portefeuille supportees

Auth :

- `Authorization: Bearer <access_token>`

Reponse :

```json
{
  "data": [
    { "value": "private", "label": "Private" },
    { "value": "public", "label": "Public" },
    { "value": "unlisted", "label": "Unlisted" }
  ]
}
```

### `GET /v1/reference/currencies` (P1)

But :

- exposer le catalogue canonique des devises supportees pour la creation de
  portefeuilles et de `portfolio_operations`
- source unique de verite partagee avec la validation backend (le frontend
  ne maintient aucune liste cote client)

Auth :

- `Authorization: Bearer <access_token>` ; les refresh tokens sont rejetes
  comme sur les autres endpoints `/v1/reference/*`

Origine : ISO 4217 codes ordinaires actifs (snapshot 2026-06-15). Exclusions
documentees : unites metaux precieux (`XAU`/`XAG`/`XPD`/`XPT`), unites de
fonds/reglement (`XBA`-`XBD`, `XDR`, `XSU`, `XUA`), code de test (`XTS`),
code aucune-devise (`XXX`), cryptos (jamais ISO 4217 actif), codes retires
(p. ex. `HRK` remplace par `EUR` en 2023).

Garanties :

- ordre alphabetique deterministe par code ;
- pas de doublon ;
- codes en MAJUSCULES sur 3 lettres ASCII ;
- contient au minimum EUR, USD, GBP, JPY, CHF, CAD, AUD.

Reponse type :

```json
{
  "data": [
    { "value": "AED", "label": "UAE Dirham" },
    { "value": "EUR", "label": "Euro" },
    { "value": "USD", "label": "US Dollar" }
  ]
}
```

### `GET /v1/assets`

But :

- rechercher et lister les assets existants en lecture seule
- exposer des donnees stables pour la selection d'assets lors des `portfolio_operations`

Auth :

- `Authorization: Bearer <access_token>`

Important :

- endpoint strictement read-only
- aucun enrichissement, refresh fournisseur ou appel API externe
- `kushim-api` ne cree, ne modifie et ne supprime jamais d'asset

Query params :

- `search` optionnel
- `asset_class` optionnel
- `ticker` optionnel
- `isin` optionnel
- `exchange` optionnel
- `status` optionnel, defaut `active`
- `limit` optionnel, defaut `50`, max `100`
- `offset` optionnel, defaut `0`

Recherche :

- `search` correspond a `name`, `ticker`, `isin` et aux `asset_aliases`
- les filtres supplementaires utilisent uniquement des bind parameters SQLx
- les query params inconnus sont ignores

Tri :

- `name ASC`
- `ticker ASC`
- `exchange ASC`

Exemple de reponse :

```json
{
  "assets": [
    {
      "id_asset": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
      "name": "Apple Inc.",
      "ticker": "AAPL",
      "isin": "US0378331005",
      "exchange": "NASDAQ",
      "symbol": "AAPL",
      "network": null,
      "asset_class": "equity",
      "status": "active",
      "native_currency": "USD",
      "created_at": "2026-06-05T14:30:00Z",
      "updated_at": "2026-06-05T14:30:00Z",
      "metadata": {
        "country": "USA",
        "website_url": "https://example.com",
        "logo_url": "https://example.com/logo.png",
        "description": "Test asset",
        "provider": "fixture",
        "provider_asset_id": "provider-asset-id",
        "sector": "Technology",
        "industry": "Software",
        "last_synced_at": "2026-06-05T14:30:00Z"
      },
      "market_data": {
        "price_minor": 12345,
        "currency": "USD",
        "market_cap_minor": 999999,
        "volume_24h_minor": 4444,
        "change_24h_pct": "1.5000",
        "change_7d_pct": "2.2500",
        "change_30d_pct": "3.7500",
        "data_source": "fixture",
        "source_asset_id": "asset-source",
        "as_of": "2026-06-05T14:30:00Z"
      },
      "aliases": null
    }
  ],
  "pagination": {
    "limit": 50,
    "offset": 0,
    "returned": 1,
    "has_more": false
  }
}
```

Erreurs communes :

- `400 invalid_asset_class`
- `400 invalid_asset_status`
- `400 invalid_limit`
- `400 invalid_offset`
- `401 missing_bearer_token`
- `401 invalid_bearer_token`

### `GET /v1/assets/{id_asset}`

But :

- lire un asset existant par identifiant
- inclure ses metadonnees, sa market data courante et ses aliases quand disponibles

Auth :

- `Authorization: Bearer <access_token>`

Comportement :

- `200` si l'asset existe
- `400` si `id_asset` n'est pas un UUID valide
- `404` si l'asset n'existe pas

Exemple de reponse :

```json
{
  "asset": {
    "id_asset": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
    "name": "Apple Inc.",
    "ticker": "AAPL",
    "isin": "US0378331005",
    "exchange": "NASDAQ",
    "symbol": "AAPL",
    "network": null,
    "asset_class": "equity",
    "status": "active",
    "native_currency": "USD",
    "created_at": "2026-06-05T14:30:00Z",
    "updated_at": "2026-06-05T14:30:00Z",
    "metadata": {
      "country": "USA",
      "website_url": "https://example.com",
      "logo_url": "https://example.com/logo.png",
      "description": "Test asset",
      "provider": "fixture",
      "provider_asset_id": "provider-asset-id",
      "sector": "Technology",
      "industry": "Software",
      "last_synced_at": "2026-06-05T14:30:00Z"
    },
    "market_data": {
      "price_minor": 12345,
      "currency": "USD",
      "market_cap_minor": 999999,
      "volume_24h_minor": 4444,
      "change_24h_pct": "1.5000",
      "change_7d_pct": "2.2500",
      "change_30d_pct": "3.7500",
      "data_source": "fixture",
      "source_asset_id": "asset-source",
      "as_of": "2026-06-05T14:30:00Z"
    },
    "aliases": [
      {
        "alias": "AAPL.OQ",
        "alias_type": "ticker",
        "source": "fixture",
        "valid_from": "2026-01-01",
        "valid_to": "2026-12-31"
      }
    ]
  }
}
```

### `POST /v1/portfolios`

But :

- creer un portefeuille pour l'utilisateur authentifie

Auth :

- `Authorization: Bearer <access_token>`

Regles :

- `id_user` provient uniquement du token
- `base_currency` est trim/upcase et doit appartenir au catalogue canonique
  (`GET /v1/reference/currencies`). Codes mal formes -> `400`
  `invalid_base_currency`. Codes 3 lettres valides hors catalogue ->
  `422` `unsupported_currency` (P1)
- `visibility` accepte `private`, `public`, `unlisted`
- `visibility` par defaut : `private`

Reponse type :

```json
{
  "portfolio": {
    "id_portfolio": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
    "name": "My Portfolio",
    "base_currency": "EUR",
    "visibility": "private",
    "created_at": "2026-06-05T14:30:00Z",
    "updated_at": "2026-06-05T14:30:00Z"
  }
}
```

### `GET /v1/portfolios`

But :

- lister uniquement les portefeuilles non supprimes de l'utilisateur authentifie

Auth :

- `Authorization: Bearer <access_token>`

### `GET /v1/portfolios/{id_portfolio}`

But :

- lire un portefeuille uniquement s'il appartient a l'utilisateur authentifie

Auth :

- `Authorization: Bearer <access_token>`

Comportement :

- `200` si le portefeuille appartient a l'utilisateur
- `404` s'il n'existe pas, est soft-delete, ou appartient a un autre utilisateur

### `GET /v1/portfolios/{id_portfolio}/summary`

But :

- lire le read model courant `rm_portfolio_summary` d'un portefeuille
- exposer un resume dashboard sans recalculer l'etat du portefeuille

Auth :

- `Authorization: Bearer <access_token>`

Important :

- endpoint strictement read-only
- `kushim-api` lit seulement `rm_portfolio_summary`
- si le read model n'existe pas encore, l'API renvoie `data_available=false`
- aucun recalcul, aucun refresh, aucun appel worker

Comportement :

- `200` + `data_available=true` si le read model existe
- `200` + `data_available=false` si le portefeuille existe mais que le read model n'a pas encore ete genere
- `404` si le portefeuille n'existe pas, est soft-delete, ou appartient a un autre utilisateur

Exemple avec read model disponible :

```json
{
  "data_available": true,
  "summary": {
    "id_portfolio": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
    "base_currency": "EUR",
    "total_value_minor": 123456,
    "cash_balance_minor": 10000,
    "total_invested_minor": 100000,
    "total_pnl_minor": 23456,
    "total_pnl_pct": "23.4500",
    "portfolio_status": "active",
    "is_estimated": false,
    "as_of": "2026-06-05T14:30:00Z",
    "updated_at": "2026-06-05T14:30:00Z"
  },
  "reason": null
}
```

Exemple si le read model est absent :

```json
{
  "data_available": false,
  "summary": null,
  "reason": "read_model_missing"
}
```

### `GET /v1/portfolios/{id_portfolio}/holdings`

But :

- lire les lignes courantes de `rm_portfolio_holdings`
- exposer les holdings dashboard sans recalculer depuis `portfolio_operations`

Auth :

- `Authorization: Bearer <access_token>`

Important :

- endpoint strictement read-only
- `kushim-api` ne genere ni holdings ni resume
- si `rm_portfolio_summary` n'existe pas encore, l'API renvoie `data_available=false`
- si le summary existe mais qu'aucune ligne holding n'existe, l'API renvoie `data_available=true` avec `holdings=[]`

Query params :

- `limit` optionnel, defaut `50`, max `100`
- `offset` optionnel, defaut `0`
- `sort` optionnel : `weight_desc` par defaut, `value_desc`, `name_asc`
- `asset_class` optionnel
- `search` optionnel, correspond a `asset.name`, `asset.ticker`, `asset.isin`

Tri :

- `weight_desc` -> `weight_pct DESC, market_value_minor DESC, asset.name ASC`
- `value_desc` -> `market_value_minor DESC, weight_pct DESC, asset.name ASC`
- `name_asc` -> `asset.name ASC, asset.ticker ASC, asset.exchange ASC`

Exemple de reponse :

```json
{
  "data_available": true,
  "holdings": [
    {
      "id_asset": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
      "asset": {
        "id_asset": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
        "name": "Apple Inc.",
        "ticker": "AAPL",
        "isin": "US0378331005",
        "exchange": "NASDAQ",
        "asset_class": "equity",
        "status": "active",
        "native_currency": "USD"
      },
      "base_currency": "EUR",
      "quantity": "10.5000000000",
      "avg_cost_minor": 10000,
      "invested_base_minor": 100000,
      "market_value_minor": 123456,
      "pnl_base_minor": 23456,
      "pnl_pct": "23.4500",
      "weight_pct": "12.3400",
      "position_status": "open",
      "is_estimated": false,
      "as_of": "2026-06-05T14:30:00Z",
      "updated_at": "2026-06-05T14:30:00Z"
    }
  ],
  "pagination": {
    "limit": 50,
    "offset": 0,
    "returned": 1,
    "has_more": false
  },
  "reason": null
}
```

Erreurs communes :

- `400 invalid_limit`
- `400 invalid_offset`
- `400 invalid_sort`
- `400 invalid_asset_class`
- `401 missing_bearer_token`
- `401 invalid_bearer_token`
- `404 portfolio_not_found`

### `GET /v1/portfolios/{id_portfolio}/snapshots/daily`

But :

- lire les snapshots journaliers deja generes dans `portfolio_snapshots_daily`
- exposer une base simple pour les vues d'historique et de courbe

Auth :

- `Authorization: Bearer <access_token>`

Important :

- endpoint strictement read-only
- `kushim-api` ne genere aucun snapshot
- `kushim-api` ne reconstruit jamais l'historique depuis `portfolio_operations`
- aucun appel worker ou market-data

Query params :

- `date_from` optionnel, format ISO `YYYY-MM-DD`
- `date_to` optionnel, format ISO `YYYY-MM-DD`
- `limit` optionnel, defaut `100`, max `366`
- `offset` optionnel, defaut `0`
- `sort` optionnel : `asc` par defaut, `desc`

Comportement :

- snapshots presents -> `200`, `data_available=true`
- aucun snapshot pour le portefeuille / la plage demandee -> `200`, `data_available=false`, `snapshots=[]`
- `404` uniquement si le portefeuille n'existe pas, est soft-delete, ou appartient a un autre utilisateur

Tri :

- `sort=asc` -> `snapshot_date ASC, created_at ASC`
- `sort=desc` -> `snapshot_date DESC, created_at DESC`

Exemple de reponse :

```json
{
  "data_available": true,
  "snapshots": [
    {
      "id_portfolio_snapshot_daily": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
      "id_portfolio": "7ccbe650-6f9a-49a8-a5be-ec6bdbb94f43",
      "snapshot_date": "2026-06-05",
      "base_currency": "EUR",
      "cash_balance_minor": 10000,
      "total_value_minor": 123456,
      "total_invested_minor": 100000,
      "total_pnl_minor": 23456,
      "total_pnl_pct": "23.4500",
      "is_estimated": false,
      "source_type": "daily_job",
      "created_at": "2026-06-05T23:59:10Z"
    }
  ],
  "pagination": {
    "limit": 100,
    "offset": 0,
    "returned": 1,
    "has_more": false
  }
}
```

Erreurs communes :

- `400 invalid_date_from`
- `400 invalid_date_to`
- `400 invalid_date_range`
- `400 invalid_limit`
- `400 invalid_offset`
- `400 invalid_sort`
- `401 missing_bearer_token`
- `401 invalid_bearer_token`
- `404 portfolio_not_found`

### `GET /v1/portfolios/{id_portfolio}/snapshots/daily/{snapshot_date}/holdings`

But :

- lire les holdings historiques deja stockees pour un snapshot journalier donne
- exposer une base simple pour les vues holdings historiques sans reconstruction

Auth :

- `Authorization: Bearer <access_token>`

Important :

- endpoint strictement read-only
- `kushim-api` ne cree jamais de snapshot ni de snapshot holding
- `kushim-api` ne reconstruit jamais les holdings depuis `portfolio_operations`
- aucun appel worker, market-data ou fournisseur externe

Path params :

- `snapshot_date` au format ISO `YYYY-MM-DD`

Query params :

- `limit` optionnel, defaut `50`, max `100`
- `offset` optionnel, defaut `0`
- `sort` optionnel : `weight_desc` par defaut, `value_desc`, `name_asc`
- `asset_class` optionnel
- `search` optionnel sur `name`, `ticker`, `isin`

Comportement :

- snapshot present avec holdings -> `200`, `data_available=true`
- snapshot present sans holdings -> `200`, `data_available=true`, `holdings=[]`
- snapshot absent pour `snapshot_date` -> `200`, `data_available=false`, `snapshot=null`, `holdings=[]`, `reason="snapshot_missing"`
- `404` uniquement si le portefeuille n'existe pas, est soft-delete, ou appartient a un autre utilisateur

Tri :

- `weight_desc` -> `weight_pct DESC`, puis `market_value_minor DESC`
- `value_desc` -> `market_value_minor DESC`
- `name_asc` -> `asset.name ASC`

Exemple de reponse :

```json
{
  "data_available": true,
  "snapshot": {
    "id_portfolio_snapshot_daily": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
    "id_portfolio": "7ccbe650-6f9a-49a8-a5be-ec6bdbb94f43",
    "snapshot_date": "2026-06-05",
    "base_currency": "EUR",
    "cash_balance_minor": 10000,
    "total_value_minor": 123456,
    "total_invested_minor": 100000,
    "total_pnl_minor": 23456,
    "total_pnl_pct": "23.4500",
    "is_estimated": false,
    "source_type": "daily_job",
    "created_at": "2026-06-05T23:59:10Z"
  },
  "holdings": [
    {
      "id_portfolio_holding_snapshot_daily": "df33a53d-6f64-42ec-929d-5f122e8e060c",
      "id_portfolio_snapshot_daily": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
      "id_asset": "f5281663-b8c4-4e22-a97e-b304732faf7f",
      "asset": {
        "id_asset": "f5281663-b8c4-4e22-a97e-b304732faf7f",
        "name": "Apple Inc.",
        "ticker": "AAPL",
        "isin": "US0378331005",
        "exchange": "NASDAQ",
        "asset_class": "equity",
        "status": "active",
        "native_currency": "USD"
      },
      "base_currency": "EUR",
      "quantity": "10.5000000000",
      "avg_cost_minor": 10000,
      "invested_minor": 100000,
      "market_value_minor": 123456,
      "pnl_minor": 23456,
      "pnl_pct": "23.4500",
      "weight_pct": "12.3400",
      "is_estimated": false,
      "created_at": "2026-06-05T23:59:10Z"
    }
  ],
  "reason": null,
  "pagination": {
    "limit": 50,
    "offset": 0,
    "returned": 1,
    "has_more": false
  }
}
```

Erreurs communes :

- `400 invalid_snapshot_date`
- `400 invalid_limit`
- `400 invalid_offset`
- `400 invalid_sort`
- `400 invalid_asset_class`
- `401 missing_bearer_token`
- `401 invalid_bearer_token`
- `404 portfolio_not_found`

### `POST /v1/portfolios/{id_portfolio}/operations`

But :

- creer une `portfolio_operation`
- ecrire dans la source de verite sans recalcul synchrone

Auth :

- `Authorization: Bearer <access_token>`

Statut :

- si `operation_status` est omis, la valeur appliquee est `pending`
- une creation `posted` (ou la pose d'une operation pending, ou une correction
  posted) ecrit l'operation ET met en file une requete de rafraichissement
  durable dans `portfolio_refresh_requests` au sein de la **meme transaction
  PostgreSQL** (atomicite, pas de refresh perdu)

Reponse (enveloppe) :

- pour une ecriture qui produit une operation `posted` :
  `{ "operation": { ... }, "refresh_request": { "id_portfolio_refresh_request": "...", "status": "pending", "requested_at": "..." } }`
- pour une creation `pending` : `{ "operation": { ... }, "refresh_request": null }`

Important :

- `currency` est requise par le DDL pour toutes les operations
- pour `buy` et `sell`, le DDL impose `price_minor`, `gross_amount_minor` et `cash_amount_minor`
- `id_asset` et `id_related_asset` sont maintenant validates au niveau service avant ecriture
- si un asset reference n'existe pas, l'API renvoie une erreur `400` propre au lieu d'une erreur SQL
- en V1, tout asset reference doit etre actuellement `active`
- `spin_off` et `symbol_change` exigent `id_asset != id_related_asset`
- l'API ne fait qu'ecrire la source de verite + la requete de refresh ; elle
  n'ecrit jamais `rm_portfolio_summary`, `rm_portfolio_holdings` ou les snapshots

P1 — contrat devises et cross-currency :

- `currency` est trim/upcase et validee contre le catalogue canonique. Code
  mal forme -> `400` `invalid_currency`. Code valide hors catalogue ->
  `422` `unsupported_currency`.
- Si `operation.currency == portfolio.base_currency`, aucun
  `fx_rate_to_portfolio` n'est requis.
- Si `operation.currency != portfolio.base_currency`, **ET** que l'operation
  a une jambe monetaire convertie par le worker (`cash_amount_minor != 0`),
  ET que le statut transmis est `posted`, alors un `fx_rate_to_portfolio`
  positif est obligatoire AVANT toute insertion. Direction conventionnelle :
  `1 unite de operation.currency = fx_rate_to_portfolio unites de
  portfolio.base_currency`.
- En cas de violation, l'API renvoie `422` `unsupported_cross_currency` et
  NI l'operation NI la `portfolio_refresh_requests` ne sont creees (rejet
  atomique avant insert).
- Les operations zero-cash (split, spin_off, symbol_change avec
  `cash_amount_minor = 0`) ne requierent jamais de `fx_rate_to_portfolio`
  meme en cross-currency (le worker n'a rien a convertir).
- Le contrat s'applique aux quatre chemins de posting : creation directe
  posted, transition pending->posted via `/post`, creation d'une correction
  posted, post d'une correction pending. Aucun chemin ne contourne la
  garde.
- Les operations posted historiques anterieures a P1 et sans
  `fx_rate_to_portfolio` restent lisibles ; le fallback worker (contribution
  zero + `is_estimated = true`) est preserve pour compatibilite.
  (seul `kushim-worker` calcule la donnee derivee)

Exemple deposit :

```json
{
  "operation_type": "deposit",
  "executed_at": "2026-06-05T10:00:00Z",
  "gross_amount_minor": 100000,
  "cash_amount_minor": 100000,
  "currency": "EUR",
  "metadata": {}
}
```

Exemple buy :

```json
{
  "id_asset": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
  "operation_type": "buy",
  "executed_at": "2026-06-05T10:00:00Z",
  "quantity": "10.5000000000",
  "price_minor": 12345,
  "gross_amount_minor": 129622,
  "cash_amount_minor": 129622,
  "currency": "EUR",
  "metadata": {}
}
```

### `GET /v1/portfolios/{id_portfolio}/operations`

But :

- lister les operations d'un portefeuille possede par l'utilisateur

Filtres supportes :

- `operation_status`
- `operation_type`
- `id_asset`

Tri :

- `executed_at DESC, created_at DESC`

### `GET /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}`

But :

- lire une operation unique

Comportement :

- `404` si l'operation n'existe pas, n'appartient pas au portefeuille, ou si le portefeuille n'appartient pas a l'utilisateur

### `PATCH /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}`

But :

- modifier uniquement une operation `pending`

Regles :

- `posted` -> rejet `409`
- `cancelled` -> rejet `409`
- aucune correction de `posted` ici
- les references `id_asset` / `id_related_asset` sont revalidees avant update
- un asset inactif ou manquant est rejete en `400`

### `POST /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/cancel`

But :

- annuler une operation `pending`

Regles :

- `pending` -> `cancelled`
- `posted` -> rejet `409`
- `cancelled` -> succes idempotent

### `POST /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/corrections`

But :

- corriger explicitement une operation `posted` sans la modifier
- creer une nouvelle operation `adjustment` liee via `id_corrected_operation`

Regles :

- l'operation d'origine doit etre `posted`
- `pending` et `cancelled` sont rejetes
- `operation_type` est force a `adjustment`
- `id_corrected_operation` est force a l'id de l'operation d'origine
- `operation_status` par defaut : `pending`
- `cancelled` n'est pas accepte a la creation
- si `id_asset` ou `id_related_asset` sont fournis, ils doivent exister et etre `active`
- aucune mise a jour directe de l'operation `posted`
- aucune ecriture dans read models ou snapshots

Exemple de requete :

```json
{
  "executed_at": "2026-06-06T10:00:00Z",
  "cash_amount_minor": 5000,
  "currency": "EUR",
  "notes": "correction adjustment",
  "metadata": {
    "reason": "manual_correction"
  }
}
```

Exemple de reponse :

```json
{
  "operation": {
    "id_portfolio_operation": "3b4209a3-18e5-4ca6-b65f-5a89cd5ca6e5",
    "id_portfolio": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
    "operation_type": "adjustment",
    "operation_status": "pending",
    "id_corrected_operation": "1c7b76ea-4911-43f0-8540-484f8b3df4b7",
    "executed_at": "2026-06-06T10:00:00Z",
    "cash_amount_minor": 5000,
    "currency": "EUR",
    "metadata": {
      "reason": "manual_correction"
    },
    "created_at": "2026-06-05T16:30:00Z",
    "updated_at": "2026-06-05T16:30:00Z"
  }
}
```

### `POST /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/post`

But :

- rendre explicite la transition `pending -> posted`
- figer ensuite l'operation comme source de verite immuable

Regles :

- seule une operation `pending` peut etre postee
- une operation deja `posted` est rejetee en `409`
- une operation `cancelled` est rejetee en `409`
- avant la transition, `kushim-api` revalide les regles metier du payload existant
- cette revalidation inclut l'existence et le statut `active` des assets references
- en V1, une operation `pending` peut donc etre rejetee au posting si son asset est devenu `inactive`, `delisted` ou `merged`
- une fois `posted`, l'operation sera consommee plus tard par le worker pour les calculs et read models
- cet endpoint ne recalcule rien lui-meme

Exemple de reponse :

```json
{
  "operation": {
    "id_portfolio_operation": "3b4209a3-18e5-4ca6-b65f-5a89cd5ca6e5",
    "id_portfolio": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
    "operation_type": "deposit",
    "operation_status": "posted",
    "executed_at": "2026-06-05T10:00:00Z",
    "gross_amount_minor": 100000,
    "cash_amount_minor": 100000,
    "currency": "EUR",
    "created_at": "2026-06-05T16:30:00Z",
    "updated_at": "2026-06-05T16:35:00Z"
  }
}
```

### `GET /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/corrections`

But :

- lire toutes les operations `adjustment` qui corrigent une operation donnee
- exposer la relation `id_corrected_operation` sans recalcul

Regles :

- l'operation cible doit appartenir au portefeuille de l'utilisateur
- seules les corrections du meme portefeuille sont retournees
- tri : `executed_at ASC, created_at ASC`
- aucune ecriture read-model, snapshot ou worker

Exemple de reponse :

```json
{
  "operation": {
    "id_portfolio_operation": "1c7b76ea-4911-43f0-8540-484f8b3df4b7",
    "operation_type": "deposit",
    "operation_status": "posted",
    "executed_at": "2026-06-05T10:00:00Z",
    "created_at": "2026-06-05T16:30:00Z",
    "updated_at": "2026-06-05T16:35:00Z"
  },
  "corrections": [
    {
      "id_portfolio_operation": "3b4209a3-18e5-4ca6-b65f-5a89cd5ca6e5",
      "operation_type": "adjustment",
      "operation_status": "pending",
      "id_corrected_operation": "1c7b76ea-4911-43f0-8540-484f8b3df4b7",
      "executed_at": "2026-06-06T10:00:00Z",
      "created_at": "2026-06-05T16:40:00Z",
      "updated_at": "2026-06-05T16:40:00Z"
    }
  ]
}
```

### `GET /v1/portfolios/{id_portfolio}/refresh-requests/{id_refresh_request}`

But :

- suivre l'etat d'une requete de rafraichissement automatique enfilee lors de la
  pose d'une operation

Auth :

- `Authorization: Bearer <access_token>`
- verifie que le portefeuille appartient a l'utilisateur authentifie
- verifie que la requete appartient au portefeuille ; sinon `404`

Reponse :

```json
{
  "refresh_request": {
    "id_portfolio_refresh_request": "...",
    "id_portfolio": "...",
    "status": "pending | processing | completed | failed",
    "attempts": 0,
    "requested_at": "...",
    "processing_started_at": null,
    "completed_at": null,
    "updated_at": "...",
    "error_code": null
  }
}
```

Important :

- l'erreur interne brute (`last_error`) n'est jamais exposee ; seul un
  `error_code` public (`refresh_failed`) est renvoye quand `status = failed`
- la donnee derivee (summary/holdings/snapshots) reste lue via ses endpoints
  read-only dedies

### `GET /v1/portfolios/{id_portfolio}/operations/{id_portfolio_operation}/audit`

But :

- fournir une vue d'audit simple autour d'une operation
- exposer l'operation, son eventuelle operation corrigee, et ses corrections

Regles :

- pour une operation normale : renvoie l'operation et ses corrections
- pour une `adjustment` : renvoie aussi `corrected_operation`
- `correction_count` est derive de la liste renvoyee
- aucune reconstruction ni timeline globale

Exemple de reponse :

```json
{
  "operation": {
    "id_portfolio_operation": "3b4209a3-18e5-4ca6-b65f-5a89cd5ca6e5",
    "operation_type": "adjustment",
    "operation_status": "pending",
    "id_corrected_operation": "1c7b76ea-4911-43f0-8540-484f8b3df4b7",
    "executed_at": "2026-06-06T10:00:00Z",
    "created_at": "2026-06-05T16:40:00Z",
    "updated_at": "2026-06-05T16:40:00Z"
  },
  "corrected_operation": {
    "id_portfolio_operation": "1c7b76ea-4911-43f0-8540-484f8b3df4b7",
    "operation_type": "deposit",
    "operation_status": "posted",
    "executed_at": "2026-06-05T10:00:00Z",
    "created_at": "2026-06-05T16:30:00Z",
    "updated_at": "2026-06-05T16:35:00Z"
  },
  "corrections": [
    {
      "id_portfolio_operation": "3b4209a3-18e5-4ca6-b65f-5a89cd5ca6e5",
      "operation_type": "adjustment",
      "operation_status": "pending",
      "id_corrected_operation": "1c7b76ea-4911-43f0-8540-484f8b3df4b7",
      "executed_at": "2026-06-06T10:00:00Z",
      "created_at": "2026-06-05T16:40:00Z",
      "updated_at": "2026-06-05T16:40:00Z"
    }
  ],
  "correction_count": 1
}
```

### `GET /v1/portfolios/{id_portfolio}/operations/audit`

But :

- fournir une timeline d'audit paginee au niveau portefeuille
- exposer uniquement les operations primaires en top-level
- imbriquer les corrections `adjustment` sous l'operation corrigee

Regles :

- endpoint read-only, sans recalcul de portefeuille
- seules les operations avec `id_corrected_operation IS NULL` apparaissent en top-level
- les corrections sont retournees dans `corrections`
- pagination `offset` sur les operations top-level uniquement
- tri top-level : `executed_at DESC, created_at DESC`
- tri corrections : `executed_at ASC, created_at ASC`
- aucune ecriture read-model, snapshot ou worker

Query params :

- `limit` optionnel, defaut `50`, max `100`
- `offset` optionnel, defaut `0`
- `operation_status` optionnel : `pending`, `posted`, `cancelled`
- `operation_type` optionnel : type d'operation supporte (`deposit`, `buy`, `sell`, etc.)

Les filtres :

- s'appliquent uniquement aux operations top-level
- ne filtrent pas les corrections imbriquees d'une operation top-level retenue

Exemple de reponse :

```json
{
  "items": [
    {
      "operation": {
        "id_portfolio_operation": "1c7b76ea-4911-43f0-8540-484f8b3df4b7",
        "operation_type": "deposit",
        "operation_status": "posted",
        "executed_at": "2026-06-05T10:00:00Z",
        "created_at": "2026-06-05T16:30:00Z",
        "updated_at": "2026-06-05T16:35:00Z"
      },
      "corrections": [
        {
          "id_portfolio_operation": "3b4209a3-18e5-4ca6-b65f-5a89cd5ca6e5",
          "operation_type": "adjustment",
          "operation_status": "pending",
          "id_corrected_operation": "1c7b76ea-4911-43f0-8540-484f8b3df4b7",
          "executed_at": "2026-06-06T10:00:00Z",
          "created_at": "2026-06-05T16:40:00Z",
          "updated_at": "2026-06-05T16:40:00Z"
        }
      ],
      "correction_count": 1
    }
  ],
  "pagination": {
    "limit": 50,
    "offset": 0,
    "returned": 1,
    "has_more": false
  }
}
```

Format d'erreur normalise :

```json
{
  "error": {
    "code": "invalid_bearer_token",
    "message": "access token is invalid"
  }
}
```

## Contrat Auth/JWT

`kushim-api` valide les access tokens emis par `kushim-auth/api` avec :

- signature `HS256`
- `iss = JWT_ISSUER`
- `exp` valide
- `token_type = access`

Claims attendus :

- `sub`
- `public_handle`
- `role`
- `token_type`
- `jti`
- `iat`
- `exp`
- `iss`

Regles :

- un refresh token est explicitement rejete
- l'identite utilisateur provient du claim `sub`
- aucun `user_id` de requete ne doit primer sur le token
- les routes operations utilisent la meme validation access-token uniquement

## Structure actuelle

```text
src/
  main.rs
  lib.rs
  config.rs
  state.rs
  errors.rs
  db/
    mod.rs
  http/
    mod.rs
    assets.rs
    health.rs
    me.rs
    portfolio_operations.rs
    portfolio_read_models.rs
    portfolio_snapshots.rs
    portfolios.rs
    reference.rs
  auth/
    mod.rs
    claims.rs
    extractor.rs
  domain/
    mod.rs
    asset.rs
    portfolio.rs
    portfolio_operation.rs
    portfolio_read_model.rs
    portfolio_snapshot.rs
  repositories/
    mod.rs
    assets.rs
    portfolio_read_models.rs
    portfolio_snapshots.rs
    portfolios.rs
    portfolio_operations.rs
  services/
    mod.rs
    assets.rs
    portfolio_read_models.rs
    portfolio_snapshots.rs
    portfolios.rs
    portfolio_operations.rs
```

## Developpement local

```powershell
cd E:\Kushim\kushim-api
cargo fmt --check
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
$env:AUTH_JWT_SECRET='dev_only_change_me_minimum_32_chars'
$env:JWT_ISSUER='kushim-auth'
cargo check
cargo test
cargo run
```

Smoke tests :

```powershell
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/ready
curl http://127.0.0.1:8080/v1/me
curl http://127.0.0.1:8080/v1/me -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/reference/operation-types -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/reference/operation-statuses -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/reference/portfolio-visibilities -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/assets -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/assets/<id_asset> -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios/<id_portfolio>/summary -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios/<id_portfolio>/holdings -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios/<id_portfolio>/snapshots/daily -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios/<id_portfolio>/snapshots/daily/2026-06-05/holdings -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios/<id_portfolio>/operations -H "Authorization: Bearer <access_token>"
```

## Docker

```powershell
cd E:\Kushim
docker compose build kushim-api
docker compose up -d --force-recreate database kushim-api
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/ready
curl http://127.0.0.1:8080/v1/me
curl http://127.0.0.1:8080/v1/assets -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios/<id_portfolio>/summary -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios/<id_portfolio>/holdings -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios/<id_portfolio>/snapshots/daily -H "Authorization: Bearer <access_token>"
curl http://127.0.0.1:8080/v1/portfolios/<id_portfolio>/snapshots/daily/2026-06-05/holdings -H "Authorization: Bearer <access_token>"
```

Note Docker :

- apres un rebuild, utiliser `--force-recreate` evite de tester un ancien conteneur encore en cours d'execution

## Limitations actuelles

- seuls les endpoints portfolio de base sont implementes
- seuls les endpoints `portfolio_operations` source-of-truth de base sont implementes
- le module `assets` est strictement read-only
- la creation d'assets, leur enrichissement et le refresh market data appartiennent a `kushim-market-data` ou a un futur outillage admin
- les corrections de `posted` ne couvrent pas encore de workflow comptable plus riche que la creation d'un `adjustment`
- le posting explicite ne declenche encore aucun worker ni recalcul synchrone
- les endpoints `corrections`, `audit` et `operations/audit` sont purement consultatifs et ne recalculent rien
- les endpoints `summary` et `holdings` ne font que lire les read models existants ; ils ne les generent jamais
- l'endpoint `snapshots/daily` ne fait que lire les snapshots existants ; il ne les genere jamais
- l'endpoint `snapshots/daily/{snapshot_date}/holdings` ne fait que lire les snapshot holdings existants ; il ne les genere jamais
- aucune reconstruction `snapshot + delta operations + price cache`
- aucune verification de revocation de token cote `kushim-api`
- l'advisory transitive `RUSTSEC-2023-0071` via `jsonwebtoken`/`rsa` reste connue et suivie ; `kushim-api` utilise uniquement `HS256`
- `REDIS_URL` est seulement reservee pour les evolutions futures
- aucune logique worker, read-model, snapshot ou market-data n'est embarquee dans `kushim-api`
  Redis :
- la dependance et la variable `REDIS_URL` sont conservees intentionnellement
- `kushim-api` ne demarre pas Redis ni n'en depend au bootstrap aujourd'hui
- cette reserve technique est destinee a de futurs usages limites: cache court, coordination, rate limiting eventuel, ou interop avec worker/job systems

## CORS

`kushim-api` supporte CORS configurable via `CORS_ALLOWED_ORIGINS`.

Comportement :

- si non definie, aucun header CORS n'est ajoute (comportement par defaut)
- si definie, seules les origines listees sont autorisees (pas de wildcard `*`)
- supporte des origines multiples separees par virgule : `http://localhost:5173,https://app.kushim.io`
- fallback sur `CORS_ALLOWED_ORIGIN` (singulier) si `CORS_ALLOWED_ORIGINS` n'est pas definie
- methodes autorisees : `GET`, `POST`, `PATCH`, `OPTIONS`
- headers autorises : `Content-Type`, `Authorization`
- le frontend utilise `Authorization: Bearer <access_token>`, pas de cookies

Exemple dev local :

```dotenv
CORS_ALLOWED_ORIGINS=http://localhost:5173
```

L’authentification GitHub CLI a expiré. Exécutez gh auth login pour actualiser le statut de la pull request.
