# kushim-worker

`kushim-worker` est le service Rust charge des traitements de fond de Kushim.

Ce qu'il possede ou possedera :
- generation de `rm_portfolio_summary`
- generation de `rm_portfolio_holdings`
- generation de `portfolio_snapshots_daily`
- generation de `portfolio_holding_snapshot_daily`
- reconstruction/replay
- backfills et traitements planifies

Ce qui reste **hors perimetre V1** :
- aucune reconstruction historique plus avancee que le backfill quotidien controle
- aucune generation de snapshots pour des dates passees arbitraires
- aucune integration market data externe
- aucun appel API externe
- aucune queue Redis
- aucun verrou distribue
- aucune conversion FX de snapshot

## Jobs et modes

`WORKER_JOB` supporte :
- `noop`
- `rebuild_current_read_models`
- `generate_daily_snapshots`
- `refresh_current_portfolio_state`
- `process_portfolio_refresh_requests`
- `backfill_daily_snapshots`

Défaut Docker Compose : `WORKER_MODE=loop` + `WORKER_JOB=process_portfolio_refresh_requests`
(consommateur de rafraîchissement automatique, intervalle de poll court).

`process_portfolio_refresh_requests` (P0 — rafraîchissement automatique) :
- file durable PostgreSQL `portfolio_refresh_requests` (pas de Redis/queue externe)
- réclame les requêtes éligibles via `FOR UPDATE SKIP LOCKED` dans une courte
  transaction (marque `processing`, enregistre le worker + horodatage, incrémente
  `attempts`), puis relâche la transaction avant le rebuild lourd
- exécute `refresh_current_portfolio_state` pour le portefeuille ciblé uniquement
  (rebuild read models courants + snapshot quotidien courant)
- marque `completed`, ou planifie un retry borné / `failed` terminal après
  `WORKER_REFRESH_MAX_ATTEMPTS`
- récupère les requêtes `processing` abandonnées (worker mort) après
  `WORKER_REFRESH_LOCK_TIMEOUT_SECONDS`
- tunables : `WORKER_REFRESH_BATCH_SIZE`, `WORKER_REFRESH_MAX_ATTEMPTS`,
  `WORKER_REFRESH_RETRY_DELAY_SECONDS`, `WORKER_REFRESH_LOCK_TIMEOUT_SECONDS`

`noop` :
- log start/end
- aucune ecriture base

`rebuild_current_read_models` :
- lit `portfolios`
- lit `portfolio_operations` avec `operation_status = 'posted'`
- lit `asset_market_data` pour la valorisation courante
- ecrit `rm_portfolio_summary`
- ecrit `rm_portfolio_holdings`
- n'ecrit pas de snapshots
- ne fetch pas de prix externes
- ne reconstruit pas d'historique

`generate_daily_snapshots` :
- lit `portfolios`
- lit `rm_portfolio_summary`
- lit `rm_portfolio_holdings`
- ecrit `portfolio_snapshots_daily`
- ecrit `portfolio_holding_snapshot_daily`
- ne relit pas `portfolio_operations`
- ne fetch pas de prix externes
- ne reconstruit pas de snapshot historique

`refresh_current_portfolio_state` :
1. lance `rebuild_current_read_models`
2. puis lance `generate_daily_snapshots`
- lit `portfolios`
- lit `portfolio_operations` avec `operation_status = 'posted'`
- lit `asset_market_data`
- lit `rm_portfolio_summary`
- lit `rm_portfolio_holdings`
- ecrit `rm_portfolio_summary`
- ecrit `rm_portfolio_holdings`
- ecrit `portfolio_snapshots_daily`
- ecrit `portfolio_holding_snapshot_daily`
- fail-fast : si l'etape rebuild echoue, l'etape snapshot n'est pas lancee
- rerun meme entree = idempotent

`backfill_daily_snapshots` :
- exige `WORKER_TARGET_PORTFOLIO_ID`
- exige `WORKER_BACKFILL_DATE_FROM` et `WORKER_BACKFILL_DATE_TO`
- lit `portfolios`
- lit `portfolio_operations` avec `operation_status = 'posted'`
- lit `asset_price_history_cache`
- ecrit `portfolio_snapshots_daily`
- ecrit `portfolio_holding_snapshot_daily`
- n'ecrit pas `rm_portfolio_summary`
- n'ecrit pas `rm_portfolio_holdings`
- n'utilise pas `asset_market_data` pour l'historique
- n'appelle aucune API externe
- rerun meme plage = idempotent

Comportement `generate_daily_snapshots` :
- utilise `WORKER_SNAPSHOT_DATE` si defini
- sinon utilise la date UTC courante
- si `WORKER_TARGET_PORTFOLIO_ID` est defini, ne traite qu'un portefeuille
- si `rm_portfolio_summary` manque pour un portefeuille, il est skippe et aucun snapshot n'est cree
- si le summary existe mais sans holdings, un snapshot portefeuille est cree avec zero holding snapshot
- rerun meme date = idempotent
- les holdings snapshot existants pour cette meme date sont remplaces, sans doublons

Comportement `refresh_current_portfolio_state` :
- reutilise `WORKER_TARGET_PORTFOLIO_ID`
- reutilise `WORKER_SNAPSHOT_DATE`
- applique le meme ciblage portefeuille aux deux etapes
- applique la meme date de snapshot a l'etape snapshot
- n'essaie pas de backfill historique

Comportement `backfill_daily_snapshots` :
- V1 cible un seul portefeuille explicitement
- V1 exige une plage de dates explicite
- V1 accepte seulement `WORKER_MODE=once` ou `idle`
- `WORKER_MODE=loop` est rejete pour le backfill
- genere les snapshots date par date dans la plage
- si le portefeuille n'existait pas encore pour une date, cette date est skippee
- si le portefeuille existait deja mais sans operation, un snapshot zero est cree
- si un prix historique manque ou si la devise du prix ne correspond pas a `base_currency`, la valorisation vaut `0` et le snapshot est marque `is_estimated`
- n'utilise aucun fallback silencieux vers `asset_market_data`

Limitations V1 du rebuild current read models :
- ignore `pending` et `cancelled`
- pas de generation de snapshots
- pas de FX de valorisation holdings a partir de `asset_market_data`
- pas de cout de revient complexe pour `spin_off`
- `split` est traite de facon conservative avec les champs actuellement disponibles
- `symbol_change` deplace la quantite et le cout moyen si les champs permettent une interpretation simple
- si le prix courant manque ou si la devise du prix ne correspond pas a `base_currency`, la valorisation est marquee `is_estimated`

Limitations V1 de `generate_daily_snapshots` :
- depend de read models deja presents
- ne cree pas de snapshot si le summary courant manque
- pas de backfill
- pas de reconstruction depuis les operations
- pas de conversion FX
- pas de fetch de prix

Limitations V1 de `refresh_current_portfolio_state` :
- pas de backfill historique
- pas de reconstruction pour des dates passees arbitraires
- pas de fetch market data
- pas de queue Redis ni lock distribue
- depend des memes limites de calcul que `rebuild_current_read_models`
- depend des memes limites de snapshot courant que `generate_daily_snapshots`

Limitations V1 de `backfill_daily_snapshots` :
- portefeuille cible obligatoire
- plage maximum de `366` jours
- pas de backfill multi-portefeuilles
- pas de FX
- pas de fetch de prix
- pas de fallback vers `asset_market_data`
- pas de reconstruction historique plus avancee que le replay V1 actuel
- `split`, `spin_off`, `symbol_change` gardent les limites conservatives du replay V1

Travaux differes qui demandent une decision produit/architecture :
- orchestration multi-portefeuille, queue Redis, verrous distribues et scheduler production
- politique FX et restatement historique quand les devises ou prix changent
- regles metier detaillees pour delisting, merges, splits complexes, spin-offs et symbol changes

Petites passes sures possibles :
- garder cette README alignee avec `documentation/mvp/deferred-todos.md`
- documenter les limites V1 quand un nouveau job worker est ajoute
- ajouter ou mettre a jour des validations ciblees sans changer les contrats API ni le schema

`WORKER_MODE` supporte :
- `idle` : verifie les dependances puis attend un shutdown propre
- `once` : execute le job selectionne une seule fois puis sort avec succes
- `loop` : execute le job selectionne periodiquement

## Variables d'environnement

- `DATABASE_URL` : requis
- `APP_ENV` : `development`, `docker`, etc.
- `RUST_LOG` : niveau de logs
- `WORKER_NAME` : nom logique du worker
- `WORKER_MODE` : `idle`, `once`, `loop`
- `WORKER_JOB` : `noop`, `rebuild_current_read_models`, `generate_daily_snapshots`, `refresh_current_portfolio_state`, `process_portfolio_refresh_requests`, `backfill_daily_snapshots`
- `WORKER_REFRESH_BATCH_SIZE` : taille de lot du consommateur (défaut 10)
- `WORKER_REFRESH_MAX_ATTEMPTS` : tentatives avant `failed` terminal (défaut 5)
- `WORKER_REFRESH_RETRY_DELAY_SECONDS` : délai de retry (défaut 30)
- `WORKER_REFRESH_LOCK_TIMEOUT_SECONDS` : délai de récupération d'un verrou périmé (défaut 300)
- `WORKER_POLL_INTERVAL_SECONDS` : intervalle positif pour `loop`
- `WORKER_TARGET_PORTFOLIO_ID` : optionnel, limite le job a un portefeuille
- `WORKER_SNAPSHOT_DATE` : optionnel, date ISO `YYYY-MM-DD` pour `generate_daily_snapshots` et `refresh_current_portfolio_state`
- `WORKER_BACKFILL_DATE_FROM` : requis pour `backfill_daily_snapshots`, date ISO `YYYY-MM-DD`
- `WORKER_BACKFILL_DATE_TO` : requis pour `backfill_daily_snapshots`, date ISO `YYYY-MM-DD`
- `REDIS_URL` : optionnel, seulement si vous voulez verifier Redis au startup
- `WORKER_HEALTH_HOST` : optionnel, a definir avec `WORKER_HEALTH_PORT`
- `WORKER_HEALTH_PORT` : optionnel, a definir avec `WORKER_HEALTH_HOST`

## Sante / readiness

Si `WORKER_HEALTH_HOST` et `WORKER_HEALTH_PORT` sont definis, le worker expose :
- `GET /health`
- `GET /ready`

`/ready` verifie PostgreSQL avec `SELECT 1`.

Ces routes sont internes/dev uniquement et n'exposent aucune API metier.

## Lancer localement

```powershell
Copy-Item .env.example .env
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
$env:WORKER_MODE='once'
$env:WORKER_JOB='rebuild_current_read_models'
cargo run
```

Run no-op explicite :

```powershell
$env:WORKER_MODE='once'
$env:WORKER_JOB='noop'
cargo run
```

Run snapshots journaliers depuis les read models courants :

```powershell
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
$env:WORKER_MODE='once'
$env:WORKER_JOB='generate_daily_snapshots'
$env:WORKER_TARGET_PORTFOLIO_ID=''
$env:WORKER_SNAPSHOT_DATE='2026-06-06'
cargo run
```

Run refresh courant end-to-end :

```powershell
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
$env:WORKER_MODE='once'
$env:WORKER_JOB='refresh_current_portfolio_state'
$env:WORKER_TARGET_PORTFOLIO_ID=''
$env:WORKER_SNAPSHOT_DATE='2026-06-06'
cargo run
```

Run backfill historique controle :

```powershell
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
$env:WORKER_MODE='once'
$env:WORKER_JOB='backfill_daily_snapshots'
$env:WORKER_TARGET_PORTFOLIO_ID='<uuid>'
$env:WORKER_BACKFILL_DATE_FROM='2026-06-01'
$env:WORKER_BACKFILL_DATE_TO='2026-06-03'
cargo run
```

## Validation Rust

```powershell
cargo fmt --check
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
```

## Docker

Build :

```powershell
docker compose build kushim-worker
```

Run :

```powershell
docker compose up -d --force-recreate database kushim-worker
docker compose logs --tail=100 kushim-worker
```

Run once avec rebuild current read models :

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=rebuild_current_read_models `
  kushim-worker
```

Run once avec snapshot journalier courant :

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=generate_daily_snapshots `
  -e WORKER_TARGET_PORTFOLIO_ID=<uuid> `
  -e WORKER_SNAPSHOT_DATE=2026-06-06 `
  kushim-worker
```

Run once avec refresh courant complet :

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=refresh_current_portfolio_state `
  -e WORKER_TARGET_PORTFOLIO_ID=<uuid> `
  -e WORKER_SNAPSHOT_DATE=2026-06-06 `
  kushim-worker
```

Run once avec backfill historique controle :

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=backfill_daily_snapshots `
  -e WORKER_TARGET_PORTFOLIO_ID=<uuid> `
  -e WORKER_BACKFILL_DATE_FROM=2026-06-01 `
  -e WORKER_BACKFILL_DATE_TO=2026-06-03 `
  kushim-worker
```

Si les endpoints health sont actifs :

```powershell
curl http://127.0.0.1:8081/health
curl http://127.0.0.1:8081/ready
```

## Frontiere d'ecriture actuelle

Cette pass:
- etablit PostgreSQL
- verifie Redis seulement si configure
- peut ecrire :
  - `rm_portfolio_summary`
  - `rm_portfolio_holdings`
  - `portfolio_snapshots_daily`
  - `portfolio_holding_snapshot_daily`
- n'ecrit pas :
  - `portfolio_operations`
  - `portfolios`
  - `assets`
  - `asset_market_data`
  - `asset_price_history_cache`

Prochaine pass recommandee :
- durcir ensuite la reconstruction historique explicite a partir des snapshots + deltas
- ou etendre prudemment le backfill a davantage de cas selon la priorite produit

## Demo historical backfill (Pass 6)

Procedure validee pour generer un historique multi-jours visible dans le Dashboard :

### Pre-requis

1. Un portefeuille USD avec des operations etalees sur plusieurs jours
2. Des prix historiques USD dans `asset_price_history_cache` couvrant la plage
3. Les services `database`, `kushim-worker`, `kushim-market-data` demarres

### Etape 1 : Remplir le cache de prix historiques

```powershell
docker exec kushim-kushim-market-data-1 /usr/local/bin/kushim-market-data
# ou via docker compose run :
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=fill_missing_price_history_cache `
  -e MARKET_DATA_PROVIDER=mock `
  -e MARKET_DATA_HISTORY_DATE_FROM=2026-05-10 `
  -e MARKET_DATA_HISTORY_DATE_TO=2026-06-10 `
  kushim-market-data
```

### Etape 2 : Lancer le backfill worker

```powershell
docker exec -e WORKER_MODE=once `
  -e WORKER_JOB=backfill_daily_snapshots `
  -e WORKER_TARGET_PORTFOLIO_ID=<uuid> `
  -e WORKER_BACKFILL_DATE_FROM=2026-05-10 `
  -e WORKER_BACKFILL_DATE_TO=2026-06-10 `
  -e WORKER_HEALTH_HOST=0.0.0.0 `
  -e WORKER_HEALTH_PORT=8091 `
  kushim-kushim-worker-1 /usr/local/bin/kushim-worker
```

### Etape 3 : Generer le snapshot courant

```powershell
docker exec -e WORKER_MODE=once `
  -e WORKER_JOB=refresh_current_portfolio_state `
  -e WORKER_TARGET_PORTFOLIO_ID=<uuid> `
  -e WORKER_HEALTH_HOST=0.0.0.0 `
  -e WORKER_HEALTH_PORT=8092 `
  kushim-kushim-worker-1 /usr/local/bin/kushim-worker
```

### Verification

- API : `GET /v1/portfolios/<uuid>/snapshots/daily?sort=asc` doit retourner `data_available: true` avec les snapshots
- Dashboard : le graphique "Evolution du portefeuille" affiche l'historique multi-jours
- Les selecteurs de periode (1M, 3M, 6M, 1Y, MAX) filtrent correctement

### Points cles

- Le mock provider genere des prix USD uniquement
- Le portefeuille doit avoir `base_currency = 'USD'` pour que les prix correspondent
- Le backfill filtre `asset_price_history_cache` par `currency = base_currency`
- Les snapshots sont idempotents (rerun = meme resultat)
- `WORKER_HEALTH_PORT` doit etre different du port 8081 deja utilise par le worker en service
