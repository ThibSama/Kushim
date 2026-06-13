# Rapport de progression MVP Kushim

_Date de mise à jour : 2026-06-11_

## 1. Résumé exécutif

Kushim a désormais un socle backend MVP sérieux autour d'une architecture claire :

- `portfolio_operations` comme source de vérité
- `kushim-api` pour les écritures et lectures synchrones user-facing
- `kushim-worker` pour les read models, snapshots et backfills contrôlés
- `asset_price_history_cache` comme cache historique déterministe

L'état global du projet :

- le backend principal est avancé et validé sur plusieurs axes ;
- le backend E2E est désormais démontrable localement via un smoke test automatisé (`scripts/demo/backend-e2e.ps1`, 18/18 assertions passées) ;
- `kushim-market-data` dispose d'un mock provider (défaut sûr) et d'un provider Finnhub gardé par allowlist ; les quotes courantes Finnhub sont live-validées pour AAPL, MSFT et NVDA uniquement ;
- `kushim-app` n'affiche plus aucune donnée financière simulée côté utilisateur : benchmark démo, swap simulé et formulaires Settings non fonctionnels ont été retirés ; toutes les valeurs visibles proviennent de l'API, des read models, des opérations persistées ou d'états explicitement indisponibles ;
- le catalogue d'actifs MVP dispose d'un seed canonique stable (`infra/postgres/init/002_seed_canonical_assets.sql`) pour AAPL, MSFT et NVDA ; les démos backend, les validations Finnhub contrôlées et les tests d'intégration market-data réutilisent ces lignes au lieu d'en créer à chaque exécution ; un job CI dédié `canonical-seed` valide l'idempotence et l'identité du seed ;
- `kushim-auth/front` est câblé à `kushim-auth/api` pour login, signup, recovery et handoff Redis ;
- la production n'est pas le niveau visé aujourd'hui.

En une phrase :

> Kushim est prêt pour une démo MVP interne supervisée : backend E2E validé, frontend sans donnée utilisateur simulée (benchmark démo, swap simulé et formulaires Settings non fonctionnels retirés), market-data avec mock provider (défaut sûr) et Finnhub gardé (quotes courantes actions validées), pas de mise en production. Les prix de marché peuvent toujours provenir du mock provider ou du provider Finnhub gardé ; la source reste explicite côté UI.

## 2. Objectif MVP produit

Le MVP Kushim vise à démontrer qu'un utilisateur peut, à terme :

- s'authentifier ;
- créer des portefeuilles ;
- enregistrer des `portfolio_operations` ;
- consulter des actifs ;
- voir des summaries et holdings courants ;
- consulter des snapshots historiques lorsqu'ils existent ;
- auditer les corrections et l'historique d'opérations ;
- préparer des vues de performance et d'évolution historique.

Kushim ne vise pas, à ce stade, à être :

- un broker ;
- une plateforme d'exécution ;
- une banque ;
- un PSP ;
- un vendeur de market data.

## 3. Architecture générale

### Services

- `kushim-auth/api` : service d'authentification
- `kushim-api` : API métier synchrone user-facing
- `kushim-worker` : jobs de fond, read models, snapshots, backfills contrôlés
- `kushim-market-data` : service de market data avec mock provider et Finnhub gardé
- `kushim-app` : frontend authentifié
- `kushim-website` : site vitrine
- `kushim-auth/front` : frontend auth
- `infra/postgres` : PostgreSQL
- `infra/redis` : Redis
- `infra/nginx` : reverse proxy local

### Séparation clé

- `kushim-api` écrit la vérité fonctionnelle user-facing
- `kushim-worker` calcule et persiste la donnée dérivée
- `kushim-market-data` fournit les données de marché (mock par défaut, Finnhub gardé pour validation dev)

### Flux de données actuel

```text
portfolio_operations
  -> kushim-worker rebuild_current_read_models
  -> rm_portfolio_summary / rm_portfolio_holdings
  -> kushim-worker generate_daily_snapshots
  -> portfolio_snapshots_daily / portfolio_holding_snapshot_daily
  -> kushim-api read-only endpoints
```

## 4. Matrice de statut par service

| Service | Statut | Commentaire |
|---|---|---|
| `kushim-auth/api` | Implémenté et validé | Auth backend réel et durci |
| `kushim-api` | Implémenté et validé | API MVP business avancée |
| `kushim-worker` | Implémenté et validé | Pipeline courant + snapshots + backfill V1 |
| `kushim-market-data` | Implémenté avec mock + Finnhub gardé | Deux jobs validés, Finnhub quotes courantes live-validées AAPL/MSFT/NVDA |
| `kushim-auth/front` | Implémenté pour la démo MVP | UI auth câblée à l'API ; handoff Redis opérationnel |
| `kushim-app` | Largement implémenté | Zéro donnée frontend simulée (benchmark démo, swap simulé, formulaires Settings retirés) ; toutes les valeurs visibles proviennent de l'API ou d'états indisponibles explicites |
| `kushim-website` | Implémenté | Site marketing présent |
| `infra/postgres` | Implémenté et validé | DDL V3 riche et cohérent |
| `infra/redis` | Implémenté minimalement | Utile aujourd'hui pour auth et check worker |
| `infra/nginx` | Implémenté pour dev | Reverse proxy local minimal |

## 5. Statut base de données

## 5.1 DDL

Le DDL `infra/postgres/init/001_init.sql` est riche et cohérent avec l'architecture cible.

Il couvre :

- auth
- actifs
- portefeuilles
- ledger `portfolio_operations`
- read models
- snapshots
- cache de prix historique

## 5.2 Points validés

- `portfolio_operations` est la source de vérité
- corrections via `adjustment + id_corrected_operation`
- soft delete sur users et portfolios
- triggers `updated_at`
- trigger d'immuabilité pour les opérations `posted`
- read models rebuildables
- snapshots dérivés
- `asset_price_history_cache` déterministe

## 5.3 État MVP

Base de données :

- **Implémentée et validée**

## 6. Statut `kushim-auth/api`

## 6.1 Fonctionnalités

Implémenté :

- signup
- login
- refresh
- logout
- `/auth/me`
- recovery setup
- reset password

## 6.2 Sécurité

Implémenté :

- Argon2id
- JWT access/refresh
- rotation refresh
- `revoked_tokens`
- Redis rate limiting
- headers `no-store`
- JSON strict
- logs sécurité redacted

## 6.3 Ownership DB

Écrit :

- `users`
- `user_recovery_phrases`
- `revoked_tokens`

Lit :

- `roles`

## 6.4 Statut MVP

`kushim-auth/api` :

- **Implémenté et validé**

## 6.5 Limitation connue

- pas encore de token family/session table

## 7. Statut `kushim-api`

## 7.1 Fonctionnalités métier

Implémenté :

- portfolios
- lifecycle `portfolio_operations`
- corrections
- audit
- assets read-only
- summary read-only
- holdings read-only
- snapshots daily read-only
- historical holdings by snapshot read-only

## 7.2 Garanties d'architecture

Confirmé :

- pas de logique worker dans `kushim-api`
- pas de génération de read models
- pas de génération de snapshots
- pas de reconstruction historique

## 7.3 Sécurité / robustesse HTTP

Implémenté :

- JWT access validation
- refresh token rejection
- erreurs JSON normalisées
- body JSON strict
- cross-user -> `404`
- soft-delete masqué

## 7.4 Ownership DB

Écrit :

- `portfolios`
- `portfolio_operations`

Lit :

- assets
- market data courante
- read models
- snapshots
- cache de prix historique

## 7.5 Statut MVP

`kushim-api` :

- **Implémenté et validé**

## 7.6 Limitation connue

- `kushim-api` dépend du worker pour toute donnée dérivée

## 8. Statut `kushim-worker`

## 8.1 Foundation

Implémenté :

- config loading
- PostgreSQL PgPool
- optional Redis check
- `/health` et `/ready`
- modes `idle | once | loop`
- graceful shutdown

## 8.2 Jobs actuels

Implémenté :

- `noop`
- `rebuild_current_read_models`
- `generate_daily_snapshots`
- `refresh_current_portfolio_state`
- `backfill_daily_snapshots`

## 8.3 Current-state pipeline

Implémenté :

- rebuild des read models courants
- génération des snapshots journaliers courants
- composite refresh

## 8.4 Backfill historique V1

Implémenté :

- mono-portefeuille explicite
- plage de dates explicite
- range max 366 jours
- valorisation via `asset_price_history_cache` uniquement
- pas de fetch externe
- pas de FX
- idempotence

## 8.5 Ownership DB

Écrit :

- `rm_portfolio_summary`
- `rm_portfolio_holdings`
- `portfolio_snapshots_daily`
- `portfolio_holding_snapshot_daily`

N'écrit pas :

- `portfolio_operations`
- `portfolios`
- `asset_market_data`
- `asset_price_history_cache`

## 8.6 Statut MVP

`kushim-worker` :

- **Implémenté et validé**

## 8.7 Limites connues

- corporate actions encore V1 conservatrices
- pas de multi-portfolio backfill
- pas de queue Redis
- pas de locks
- pas de scheduler avancé

## 9. Statut `kushim-market-data`

## 9.1 État réel

Implémenté avec deux providers :

- `refresh_current_market_data` : écrit `asset_market_data` pour les assets actifs supportés
- `fill_missing_price_history_cache` : écrit `asset_price_history_cache` pour les dates manquantes
- **mock provider** : prix USD déterministes pour 7 symboles (AAPL, MSFT, NVDA, BTC, ETH, SPY, VTI) — défaut sûr pour les démos MVP
- **Finnhub provider** : premier provider réel gardé par configuration et allowlist
  - quotes courantes live-validées pour AAPL, MSFT, NVDA
  - BTC a un chemin de mapping provider (`BTC=BINANCE:BTCUSDT`), mais le plan gratuit actuel retourne `403 Forbidden` — BTC n'est pas live-validé
  - candles historiques `/stock/candle` implémentées, mais l'accès dépend du plan/entitlement Finnhub — retourne `403 Forbidden` avec le plan actuel
  - gestion typée des erreurs provider (401, 403, 429) sans fallback silencieux vers mock
  - allowlist obligatoire avant tout appel Finnhub
- modes `once | loop | idle`
- endpoints `/health` et `/ready`
- 68 tests passants (unité + intégration)

## 9.2 Ce qui manque ou reste différé

- stratégie provider production (Finnhub MVP gardé ≠ stratégie production)
- couverture asset élargie au-delà de l'allowlist actuelle
- validation BTC/crypto live (dépend du plan provider)
- validation candles historiques Finnhub (dépend du plan provider)
- enrichissement asset
- support FX dans le pipeline market-data
- politique de fraîcheur et réconciliation
- scheduler production, queues, locks

## 9.3 Statut MVP

`kushim-market-data` :

- **Implémenté et validé localement (mock provider + Finnhub gardé pour quotes courantes actions)**
- le service n'est pas production-ready

## 10. Statut frontend

## 10.1 `kushim-auth/front`

Présent :

- pages auth
- UX auth

Manque :

- câblage réel à `kushim-auth/api`

Statut :

- **Partiellement implémenté**

## 10.2 `kushim-app`

Présent et câblé à l'API réelle :

- authentification (handoff, validation session, refresh, logout)
- liste/création/sélection de portefeuilles
- opérations liste/création (cash + liées à un actif : buy, sell, dividend)
- dashboard KPIs, évolution, allocation, top 5 actifs (read models réels)
- catalogue d'actifs (`/assets`) avec recherche, filtres, pagination (données réelles)
- détail actif (`/assets/:id`) avec identité, market data, métadonnées (données réelles)
- positions du portefeuille (`/positions`) avec recherche, filtres, tri, pagination (holdings réels)
- page transactions avec recherche, filtres, métriques (opérations réelles)
- états `data_available=false` / `read_model_missing` / `snapshot_missing`

Mocks restants côté utilisateur :

- aucun. La section benchmark démo, l'action "Échanger des actifs" simulée et les formulaires Paramètres non fonctionnels (préférences, changement de mot de passe, suppression de compte) ont été retirés de l'application authentifiée. Le fichier `kushim-app/src/mocks/demoPortfolio.ts` a été supprimé et le dossier `src/mocks/` n'existe plus.

Statut :

- **Zéro donnée frontend simulée — toutes les valeurs visibles proviennent de l'API, des read models, des opérations persistées ou d'états indisponibles explicites**

## 10.3 `kushim-website`

Présent :

- landing marketing

Statut :

- **Implémenté**

## 10.4 Conséquence MVP

Le travail restant principal sur `kushim-app` concerne désormais le câblage natif de `kushim-auth/front` (préférences, changement de mot de passe, suppression de compte) et l'intégration d'un vrai benchmark une fois qu'un endpoint d'historique d'indice existera côté backend. Aucune donnée frontend simulée ne subsiste dans les chemins applicatifs normaux.

## 11. Statut Docker / infra

## 11.1 Docker Compose

Présent :

- services principaux
- Postgres
- Redis
- Nginx

## 11.2 Health checks

Présents pour :

- auth API
- main API
- worker
- database
- redis

## 11.3 Reverse proxy

Nginx route actuellement :

- website
- auth front
- app
- API

## 11.4 Statut MVP

Infra locale :

- **Suffisante pour le dev et la validation locale**
- **pas encore une stratégie de production**

## 12. Testing et validation

## 12.1 État connu

Services Rust documentés comme validés :

- `kushim-auth/api`
- `kushim-api`
- `kushim-worker`
- `kushim-market-data` (mock provider)

Compteurs observés dans le dépôt :

- auth : ~63 tests
- api : ~157 tests
- worker : ~60 tests
- market-data : ~68 tests

## 12.2 Backend E2E smoke test

**Validé localement.**

Un script automatisé exécute la chaîne complète backend :

- script : `scripts/demo/backend-e2e.ps1`
- runbook : `documentation/operations/backend-demo-e2e.md`
- résultat : **18/18 assertions passées**
- services couverts : `kushim-auth/api`, `kushim-api`, `kushim-market-data`, `kushim-worker`
- scénario : signup → portfolio → dépôt → achat → market-data refresh (mock) → worker rebuild/snapshots/backfill → vérification API

Limites du smoke test :

- utilise le mock provider (pas de données de marché réelles)
- ne valide pas les frontends
- ne valide pas le déploiement production
- ne valide pas les conversions FX
- le backfill multi-jours est limité par la date `created_at` du portfolio

## 12.3 Couverture

Bien couverts :

- auth
- API métier
- read models
- snapshots
- backfill V1
- market-data mock provider
- chaîne backend E2E (smoke test)

Peu ou pas couverts :

- frontends
- E2E full-stack (frontend + backend)
- provider Finnhub en conditions production (seules quotes courantes actions validées en dev)

## 13. Sécurité

## 13.1 Points forts

- séparation access / refresh
- refresh rejection dans `kushim-api`
- rate limiting Redis auth
- body JSON strict
- erreurs normalisées
- posted operations immuables
- ownership DB clair

## 13.2 Limites

- pas de token family
- pas de revoke-all-sessions sur reset password
- pas de revocation access-token check dans `kushim-api`
- observabilité production incomplète

## 13.3 Risque accepté

- `RUSTSEC-2023-0071` suivi comme advisory connu

## 14. Liste TODO / deferred

Principaux chantiers différés :

- token family / session table
- revoke all sessions on password reset
- matrix opération <-> asset class
- gestion plus nuancée inactive/delisted/merged
- FX history cache / FX policy
- multi-portfolio backfill orchestration
- optimized incremental backfill
- split / spin_off / symbol_change plus riches
- Redis queues
- distributed locks
- production scheduler
- stratégie provider market-data production et rollout élargi au-delà du MVP Finnhub gardé
- durcissement auth frontend/session (`kushim-auth/front` -> `kushim-auth/api` est câblé ; stockage token encore MVP)
- dashboard benchmark réel (actuellement démo)
- paramètres backend handlers (préférences, mot de passe, suppression compte)
- correction/audit UX
- CI/CD
- production secrets
- backups
- observability
- nginx hardening
- deployment strategy
- suivi `cargo audit` pour `RUSTSEC-2023-0071`

Complétés récemment (anciennement différés) :

- dashboard frontend wiring (Pass 5/5b — KPIs, évolution, allocation, top actifs)
- UI `data_available=false` / `read_model_missing` / `snapshot_missing` (Pass 5)
- assets page real data wiring (Pass 7)
- AssetDetail page real data wiring (Pass 7)
- positions page real data wiring (Pass 8)

Référence centrale :

- [documentation/mvp/deferred-todos.md](../mvp/deferred-todos.md)

## 15. Évaluation de readiness MVP

## 15.1 Backend MVP

Le backend principal est à un niveau MVP solide et **démontrable de bout en bout localement** :

- auth
- ledger
- API sync
- worker rebuild
- worker snapshots
- backfill historique V1
- market-data mock provider + Finnhub gardé (quotes courantes actions)
- **smoke test E2E automatisé : 18/18 assertions passées** (`scripts/demo/backend-e2e.ps1`)

Les quatre briques backend (`kushim-auth/api`, `kushim-api`, `kushim-market-data`, `kushim-worker`) sont intégrées dans le scénario de smoke test.

## 15.1b Dry-run 10 minutes — 2026-06-11

**Validé le 2026-06-11** — Scénario A (mock provider uniquement).

Flux validé de bout en bout :

- auth (signup, login, session)
- portfolio USD créé
- 2 opérations (dépôt $10 000 + achat 10 AAPL @ $195.23)
- market-data mock refresh (10 updated, 2 historical inserted, 0 erreur)
- worker rebuild (1 holding, $10 000), snapshot (2026-06-11), backfill (1 snapshot)
- toutes les pages navigateur validées : Dashboard, Positions, Transactions, Assets, AssetDetail, Settings, Logout
- zéro erreur console bloquante

Points non bloquants observés :

- graphe évolution : 1 seul point (portfolio créé le jour même)
- P&L = 0 (prix mock = prix d'achat — attendu)
- `created_at` affiche "Non disponible" (palliatif frontend en place, fix racine côté auth différé)

## 15.2 Demo MVP utilisateur — Pass 6 : historique multi-jours validé

**Le graphique "Évolution du portefeuille" du Dashboard affiche désormais un historique multi-jours réel.**

Validé le 2026-06-11 avec :

- un portefeuille USD contenant 4 opérations étalées du 10 mai au 1er juin 2026
- 32 prix historiques AAPL USD dans `asset_price_history_cache` (mock provider)
- 33 snapshots journaliers générés (32 backfill + 1 courant)
- API `data_available: true` avec tri, filtrage par date, pagination fonctionnels
- Dashboard : graphique visible, sélecteurs de période (1M, 3M, 6M, 1Y, MAX) fonctionnels, zéro erreur console

Limites de cette démo :

- mock provider uniquement (prix USD déterministes, pas de données réelles)
- portefeuille USD obligatoire (le mock ne génère que des prix USD)
- pas de conversion FX
- handoff auth Redis câblé ; injection manuelle de tokens réservée au troubleshooting

Ce qui reste nécessaire pour une démo utilisateur complète :

- durcissement auth frontend/session production (handoff câblé ; stockage localStorage encore limitation MVP)
- stabiliser et étendre l'accès provider au-delà du chemin MVP Finnhub gardé (le mock reste suffisant pour une démo supervisée)
- suppression des résidus mock restants (benchmark, boutons paramètres)

## 15.3 Demo MVP utilisateur — Pass 7 : catalogue d'actifs réel

**Les pages Assets et AssetDetail de `kushim-app` affichent désormais des données réelles.**

Validé le 2026-06-11 avec :

- `/assets` : catalogue réel via `GET /v1/assets`, recherche, filtres (classe, statut), pagination
- `/assets/:id` : détail réel via `GET /v1/assets/{id}`, identité, données de marché, métadonnées, aliases
- routes en anglais (`/assets`, `/assets/:id`), labels UI en français
- `/actifs` et `/actifs/:id` redirigent vers `/assets` et `/assets/:id`
- store Zustand dédié (`src/stores/assets.ts`)
- états loading, empty, erreur pour les deux pages
- 25 actifs affichés au chargement, recherche "AAPL" → 2 résultats, détail Apple Inc. complet
- zéro erreur console, lint propre, build OK

Décisions terminologiques :

- `/assets` = catalogue d'instruments disponibles dans Kushim (pas les positions utilisateur)
- `/positions` réservé pour la future page de positions du portefeuille
- `/holding` et `/holdings` ne sont pas utilisés comme routes user-facing

Limites :

- pas de graphique historique de prix sur la page détail
- pas de lien vers les opérations liées à cet actif
- données de marché dépendent du mock provider (pas de données réelles)

## 15.4 Demo MVP utilisateur — Pass 8 : page positions réelle

**La page `/positions` de `kushim-app` affiche désormais les positions réelles du portefeuille actif.**

Validé le 2026-06-11 avec :

- `/positions` : données réelles via `GET /v1/portfolios/{id}/holdings`
- Cartes résumé : nombre de positions, valeur de marché totale, P&L total
- Tableau : nom, ticker, bourse, classe, quantité, coût moyen, valeur, P&L (% + montant), poids
- Recherche par nom/ticker, filtre par classe, tri (poids, valeur, nom)
- Clic sur une position → `/assets/:id` (détail catalogue)
- Badge « Estimé » quand `is_estimated=true`
- États : loading, erreur, `data_available=false` / `read_model_missing`, positions vides, pas de portfolio
- Zéro erreur console, lint propre, build OK

Décisions terminologiques :

- Route user-facing : `/positions` (pas `/holdings`)
- Label UI : « Positions » (pas « Holdings »)
- API backend utilise `holdings` en interne — intentionnel
- `/holding` et `/holdings` ne sont pas des routes user-facing

Polish Pass 8b :

- Formatage des quantités : zéros terminaux supprimés (`8.0000000000` → `8`), locale française (`Intl.NumberFormat`)
- Pagination : chargement initial de 25 positions, bouton « Charger plus de positions » basé sur la pagination API (`has_more`, offset)
- Cohérence devise : les cartes résumé utilisent désormais la devise des holdings avec fallback portfolio, corrigeant un affichage EUR parasite lors du chargement concurrent

Limites :

- pas de tri côté client sur les colonnes (tri côté API uniquement)
- dépend de la génération des read models par `kushim-worker`
- données de marché dépendent du mock provider

## 15.5 Production readiness

Non.

Le projet n'est pas encore à présenter comme production-ready.

## 16. Prochaines étapes recommandées

### ~~Priorité 1~~ — Largement réalisé

~~Brancher les frontends~~ → `kushim-app` est désormais largement câblé à `kushim-api` (auth, portefeuilles, opérations, dashboard, actifs, positions). Résidus mock isolés.

### Priorité 1 (nouvelle)

Câbler `kushim-auth/front` → `kushim-auth/api` :

- durcir le modèle handoff/session pour un usage production
- améliorer le parcours login/signup natif

### Priorité 2

Stabiliser et étendre la stratégie market-data :

- décider la stratégie provider production au-delà du MVP Finnhub gardé
- valider l'entitlement candles historiques Finnhub ou maintenir les backfills historiques sur mock/données seedées
- étendre la couverture asset au-delà de l'allowlist actuelle (AAPL, MSFT, NVDA)
- planifier le support FX ultérieurement

### ~~Priorité 3~~ — Réalisé

~~Ajouter un parcours E2E de démo~~ → **Fait.**

Le smoke test backend E2E est implémenté et validé : `scripts/demo/backend-e2e.ps1` (18/18 assertions).

### Priorité 3 (nouvelle)

Intégrer le smoke test E2E dans un pipeline CI automatisé.

## 17. Risques et décisions à prendre

Décisions importantes à venir :

- stratégie provider market-data production (Finnhub MVP gardé est un premier pas, pas une stratégie finale)
- entitlement candles historiques et couverture crypto/BTC
- priorité frontend vs extension market-data
- niveau de sophistication corporate actions avant démo large
- intégration CI du smoke test E2E

## 18. Appendix

### Documents clés

- [README racine](../../README.md)
- [Architecture overview](../architecture/overview.md)
- [Service boundaries](../architecture/service-boundaries.md)
- [Data flow](../architecture/data-flow.md)
- [Database architecture](../database/database-architecture.md)
- [Portfolio reconstruction](../database/portfolio-reconstruction.md)
- [MVP scope](../mvp/mvp-scope.md)
- [Deferred TODOs](../mvp/deferred-todos.md)
- [Docker local dev](../operations/docker-local-dev.md)
- [Validation commands](../operations/validation-commands.md)
- [MVP demo runbook (frontend + backend)](../operations/mvp-demo-runbook.md)
- [Backend E2E demo runbook](../operations/backend-demo-e2e.md)
- [Backend E2E smoke test script](../../scripts/demo/backend-e2e.ps1)
