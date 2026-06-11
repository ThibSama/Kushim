# Audit MVP Readiness — Kushim

_Date : 2026-06-11_

---

## A. Résumé exécutif

Kushim est prêt pour une démo MVP interne. Le backend est solide et démontrable de bout en bout (smoke test 18/18). Le frontend (`kushim-app`) est largement câblé aux données réelles : authentification, portefeuilles, opérations, dashboard (KPIs, évolution, allocation, top actifs), catalogue d'actifs, détail actif, positions du portefeuille. Les résidus mock sont isolés et correctement signalés (benchmark dashboard, boutons paramètres, modal "Ajouter un actif"). Le market-data fonctionne avec un mock provider déterministe (7 symboles USD). Aucun blocage critique n'empêche une démo supervisée.

**Verdict : GO pour démo interne supervisée.** Pas de mise en production.

---

## B. Fichiers / zones audités

### Documentation

| Fichier | Lu | Constat |
|---|---|---|
| `README.md` (racine) | Oui | Sous-estime l'avancement de `kushim-app` — "progressively wired" |
| `kushim-api/README.md` | Oui | Complet et à jour ; dernière ligne contient un texte périmé sur GitHub CLI |
| `kushim-worker/README.md` | Oui | À jour, précis |
| `kushim-market-data/README.md` | Oui | À jour ; note MVP légèrement datée (le service fait partie du E2E validé) |
| `kushim-app/README.md` | Oui | Détaillé et à jour après Pass 8b |
| `AGENTS.md` | Oui | À jour |
| `kushim-api/AGENTS.md` | Oui | À jour |
| `documentation/mvp/deferred-todos.md` | Oui | **Corrigé** : 3 entrées "deferred" déplacées vers "completed" |
| `documentation/reports/kushim-mvp-progress-report.en.md` | Oui | **Corrigé** : section 10.2 mise à jour |
| `documentation/reports/kushim-mvp-progress-report.fr.md` | Oui | **Corrigé** : section 10.2 mise à jour |

### Routes et terminologie

| Route | État | Label UI |
|---|---|---|
| `/dashboard` | Fonctionnel | Tableau de bord |
| `/assets` | Fonctionnel | Actifs |
| `/assets/:id` | Fonctionnel | Détail de l'actif |
| `/positions` | Fonctionnel | Positions |
| `/transactions` | Fonctionnel | Transactions |
| `/parametres` | Fonctionnel | Paramètres |
| `/actifs` → `/assets` | Redirect OK | — |
| `/actifs/:id` → `/assets/:id` | Redirect OK | — |
| `/settings` → `/parametres` | Redirect OK | — |
| `/holding`, `/holdings` | N'existent pas | Correct |

### Frontend — pages et stores audités

- `src/stores/auth.ts` — tokens localStorage, refresh sur 401, logout propre
- `src/stores/portfolio.ts` — liste/création/sélection, persistence active portfolio
- `src/stores/operations.ts` — CRUD opérations, types de référence
- `src/stores/portfolioReadModels.ts` — summary, holdings, snapshots, pagination
- `src/stores/assets.ts` — catalogue, recherche, détail
- `src/app/pages/Dashboard.tsx` — KPIs, évolution, allocation, top 5, benchmark (mock)
- `src/app/pages/Transactions.tsx` — opérations réelles
- `src/app/pages/Positions.tsx` — holdings réels
- `src/app/pages/Settings.tsx` — profil réel, actions mock
- `src/app/components/Navbar.tsx` — liens corrects, profil réel
- `src/app/routes.tsx` — toutes les routes vérifiées

### Backend / Worker / Market-data

- Architecture respectée : `kushim-api` lecture seule pour les read models
- `kushim-worker` écrit les read models et snapshots
- `kushim-market-data` mock provider, 7 symboles USD
- Smoke test E2E backend : 18/18 assertions (`scripts/demo/backend-e2e.ps1`)

---

## C. Vision MVP confirmée

Le MVP Kushim démontre qu'un utilisateur peut :

1. S'authentifier (signup → login → handoff → session)
2. Créer et sélectionner un portefeuille
3. Enregistrer des opérations (dépôt, achat, vente, dividende…)
4. Consulter le tableau de bord avec des KPIs, une évolution historique, une allocation et un top actifs **basés sur des données réelles**
5. Parcourir le catalogue d'actifs avec recherche et filtres
6. Consulter le détail d'un actif (identité, market data, métadonnées)
7. Voir les positions du portefeuille avec valeur de marché et P&L
8. Consulter l'historique des transactions avec filtres et métriques

Kushim n'est **pas** un broker, une plateforme d'exécution, un fournisseur de données de marché ni un service de production.

---

## D. Parcours démo MVP recommandé

### Backend uniquement (automatisé)

```powershell
# Pré-requis : services Docker up
scripts/demo/backend-e2e.ps1
# Résultat attendu : 18/18 assertions ✓
```

### Démo frontend supervisée

1. Lancer les services : `docker compose up -d`
2. Lancer le frontend : `cd kushim-app && npm run dev`
3. Se connecter via `kushim-auth/front` → handoff vers `kushim-app`
4. Dashboard : vérifier KPIs, graphe évolution, allocation, top 5 actifs
5. Transactions : voir les opérations, ajouter un dépôt et un achat
6. Worker : `kushim-worker --job rebuild_current_read_models --mode once`
7. Rafraîchir le dashboard → données mises à jour
8. Assets : parcourir le catalogue, rechercher "AAPL", consulter le détail
9. Positions : voir les holdings, filtrer, vérifier les montants
10. Paramètres : vérifier le profil utilisateur

**Attention démo** : ne pas cliquer "Ajouter un actif" depuis le dashboard (modal placeholder). Ne pas cliquer les boutons d'action dans Paramètres (UI only).

---

## E. État frontend réel

### Câblé aux données réelles

| Page / composant | Source de données | État |
|---|---|---|
| Auth (handoff, session, refresh, logout) | `kushim-auth/api` | ✅ Fonctionnel |
| Portefeuilles (liste, création, sélection) | `GET/POST /v1/portfolios` | ✅ Fonctionnel |
| Dashboard — KPIs | `GET /v1/portfolios/{id}/summary` | ✅ Read model réel |
| Dashboard — Évolution | `GET /v1/portfolios/{id}/snapshots/daily` | ✅ Snapshots réels |
| Dashboard — Allocation | Dérivée de `/holdings` | ✅ Holdings réels |
| Dashboard — Top 5 actifs | `GET /v1/portfolios/{id}/holdings?limit=5` | ✅ Holdings réels |
| Dashboard — Transactions récentes | `GET /v1/portfolios/{id}/operations` | ✅ Opérations réelles |
| Transactions (liste, création, filtres) | `GET/POST /v1/portfolios/{id}/operations` | ✅ Fonctionnel |
| Catalogue actifs (`/assets`) | `GET /v1/assets` | ✅ Données réelles |
| Détail actif (`/assets/:id`) | `GET /v1/assets/{id}` | ✅ Données réelles |
| Positions (`/positions`) | `GET /v1/portfolios/{id}/holdings` | ✅ Holdings réels |
| Paramètres — Profil | `GET /auth/me` | ✅ Données réelles |
| États indisponibles | `data_available=false` | ✅ Gérés proprement |

### Résidus mock (isolés)

| Élément | Nature | Impact démo |
|---|---|---|
| Dashboard — section Benchmark | Import `mocks/demoPortfolio` | Faible — signalé par le bandeau |
| Dashboard — KPI "Meilleur actif" | Toujours "—" (`kpiPlaceholder`) | Cosmétique |
| Dashboard — modal "Ajouter un actif" | Formulaire non fonctionnel | Éviter en démo |
| Paramètres — "Enregistrer les préférences" | Pas de handler | Éviter en démo |
| Paramètres — "Mettre à jour le mot de passe" | Pas de handler | Éviter en démo |
| Paramètres — "Supprimer mon compte" | Pas de handler | Éviter en démo |
| Transactions — quantité brute | `3.0000000000` au lieu de `3` | Cosmétique (positions page corrigée, transactions non) |

---

## F. État backend / worker / market-data

### `kushim-auth/api`

- **Implémenté et validé** — signup, login, refresh, logout, recovery, rate limiting, Argon2id
- ~63 tests

### `kushim-api`

- **Implémenté et validé** — portefeuilles, opérations, corrections, audit, read models (lecture), snapshots (lecture), actifs (lecture)
- ~157 tests
- Pas de logique worker, pas de génération de read models

### `kushim-worker`

- **Implémenté et validé** — rebuild read models, snapshots, backfill V1
- ~60 tests
- Modes : `idle | once | loop`
- Limites : mono-portefeuille, 366 jours max, pas de scheduler production

### `kushim-market-data`

- **Implémenté et validé localement (mock provider + Finnhub gardé)**
- ~68 tests
- Mock provider : prix USD déterministes pour 7 symboles (AAPL, MSFT, NVDA, BTC, ETH, SPY, VTI)
- Finnhub provider gardé : quotes courantes live-validées pour AAPL, MSFT, NVDA ; BTC non validé (403) ; candles historiques non validées (403)
- Pas de FX

### Smoke test E2E

- Script : `scripts/demo/backend-e2e.ps1`
- Résultat : **18/18 assertions passées**
- Scénario : signup → portfolio → deposit → buy → market-data refresh → worker rebuild/snapshots/backfill → API verification
- Limitation : mock provider uniquement, pas de frontend, pas de FX

---

## G. Documentation : écarts détectés

| Document | Écart | Sévérité | Action |
|---|---|---|---|
| `deferred-todos.md` | 3 items "deferred" déjà complétés | Moyen | **Corrigé** dans cet audit |
| Progress reports (FR + EN) section 10.2 | `kushim-app` décrit comme "Partiellement implémenté" avec "Missing: real integration" | Moyen | **Corrigé** dans cet audit |
| `README.md` racine | "progressively wired" sous-estime l'avancement actuel | Faible | Signalé — pas de réécriture dans ce pass |
| `kushim-api/README.md` dernière ligne | Texte périmé sur GitHub CLI auth | Faible | Signalé — suppression triviale mais hors scope strict |
| `kushim-market-data/README.md` | Note MVP dit "not part of validated core" alors que le service est dans le E2E | Faible | Signalé |
| Progress reports section 14 | Liste de deferred TODOs contient "dashboard frontend wiring" et "data_available=false UI states" — déjà implémentés | Faible | Signalé — correction nécessite réécriture de section |

---

## H. Corrections appliquées

| Fichier | Correction | Type |
|---|---|---|
| `documentation/mvp/deferred-todos.md` | 3 items déplacés de "Deferred" vers "Completed" (Assets page, AssetDetail page, allocation stats) + ajout Positions page | Documentation |
| `documentation/reports/kushim-mvp-progress-report.en.md` | Section 10.2 réécrite pour refléter l'état réel de `kushim-app` | Documentation |
| `documentation/reports/kushim-mvp-progress-report.fr.md` | Section 10.2 réécrite (version française) | Documentation |
| `kushim-app/src/app/pages/Dashboard.tsx` ligne 1026 | Lien `/actifs` → `/assets` (élimine le redirect inutile) | Code — typo route |

Aucune nouvelle fonctionnalité. Aucun changement d'API. Aucun changement de DDL. Aucun changement d'architecture.

---

## I. Validation lint / build / diff

| Vérification | Résultat |
|---|---|
| `npm run lint` | ✅ Clean (0 erreur, 0 warning) |
| `npm run build` | ✅ OK (739ms) |
| `git diff --check` | ⚠️ Warnings CRLF uniquement (normal sur Windows, pas de whitespace errors) |
| Console navigateur (toutes pages) | ✅ 0 erreur (vérifié avant cet audit) |

---

## J. Risques avant démo

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| Worker pas lancé → `data_available=false` partout | Haute si oublié | Bloquant pour dashboard/positions | Lancer le worker avant la démo (`--job rebuild_current_read_models --mode once`) |
| Market-data pas lancé → pas de prix → valuations à 0 | Haute si oublié | KPIs et positions sans valeur marché | Lancer market-data avant worker (`refresh_current_market_data` + `fill_missing_price_history_cache`) |
| Portefeuille EUR avec mock USD-only → prix manquants | Moyenne | Valuations incorrectes | Créer un portefeuille USD pour la démo |
| Démonstrateur clique "Ajouter un actif" (Dashboard) | Faible | Modal non fonctionnel visible | Consigne démo : éviter ce bouton |
| Démonstrateur clique actions Paramètres | Faible | Pas de feedback | Consigne démo : éviter ces boutons |
| Auth handoff manuel requis | Certaine | Friction UX | Préparer le handoff avant la démo |
| Quantité brute affichée en Transactions (`3.0000000000`) | Certaine | Cosmétique | Accepter pour la démo (Positions page corrigée) |

---

## K. Mocks / limites explicitement acceptés

| Élément | Raison | Statut |
|---|---|---|
| Mock market-data provider (défaut) | Finnhub gardé existe pour validation dev ; provider production = chantier séparé | Accepté pour MVP |
| Benchmark dashboard | Données de benchmark réelles = hors scope MVP | Accepté, signalé par bandeau |
| Pas de FX conversion | Complexité FX = chantier séparé | Accepté — démo en USD |
| Pas de scheduler production | Architecture déploiement = chantier séparé | Accepté — mode `once` suffisant |
| `kushim-auth/front` pas câblé | Handoff manuel fonctionne | Accepté — friction UX seulement |
| Boutons Paramètres non fonctionnels | Backend handlers = travail futur | Accepté — ne pas cliquer en démo |
| KPI "Meilleur actif" toujours "—" | Donnée disponible mais non branchée sur le KPI card | Accepté — cosmétique |
| Quantité brute en Transactions | Formatage appliqué sur Positions mais pas Transactions | Accepté — cosmétique |

---

## L. Points à ne pas ouvrir maintenant

Ces sujets sont identifiés mais **explicitement hors scope** de cet audit et de la prochaine itération :

1. Stratégie provider market-data production (au-delà du MVP Finnhub gardé)
2. Conversion FX et politique de restatement
3. Scheduler production, queues Redis, locks distribués
4. Câblage `kushim-auth/front` → `kushim-auth/api`
5. Refactoring architecture frontend ou backend
6. Modification du DDL PostgreSQL
7. CI/CD et déploiement production
8. Token family / session table
9. Réécriture du Dashboard ou des pages existantes
10. Introduction de routes `/holding` ou `/holdings`

---

## M. Verdict MVP readiness

### Backend

**GO** — Solide, démontrable, testé (18/18 E2E), architecture respectée.

### Frontend

**GO avec réserves** — Largement câblé aux données réelles. Résidus mock isolés et gérables par consignes démo. Pas de blocage critique.

### Global

**GO pour démo interne supervisée.**

Conditions :
- Préparer l'environnement (services Docker, market-data refresh, worker rebuild)
- Créer un portefeuille USD avec quelques opérations
- Suivre le parcours démo recommandé (section D)
- Éviter les zones mock identifiées (section K)

**Pas de mise en production.**

---

## N. Prochaine action recommandée

1. **Immédiat** : valider ce rapport avec le porteur de projet
2. **Court terme** : effectuer une démo interne en suivant le parcours section D
3. **Après démo** : prioriser entre :
   - Câblage `kushim-auth/front` (supprime le handoff manuel)
   - Stratégie provider market-data production (au-delà du Finnhub MVP gardé)
   - Formatage quantité dans Transactions (alignement avec Positions)
   - Branchement du KPI "Meilleur actif" sur les données holdings existantes
4. **Ne pas ouvrir** : les sujets listés en section L

---

## O. Addendum post-Finnhub

_Ajouté le 2026-06-11 après l'audit initial._

Après l'audit initial, `kushim-market-data` a été étendu avec un provider Finnhub gardé par configuration et allowlist. Le mock provider reste le défaut sûr pour les démos MVP. Les quotes courantes Finnhub ont été validées localement pour AAPL, MSFT et NVDA. BTC n'est pas considéré comme validé avec le plan gratuit actuel, le mapping crypto ayant retourné `403 Forbidden`. Les candles historiques Finnhub sont implémentées mais dépendent des droits du compte (retournent `403 Forbidden` avec le plan actuel). Le compteur de tests market-data passe de ~8 à 68.

Un dry-run 10 minutes complet (Scénario A — mock provider) a été exécuté et validé le 2026-06-11 : auth, portfolio, opérations, market-data refresh, worker rebuild/snapshots/backfill, et toutes les pages navigateur validées avec zéro erreur console bloquante.

Le verdict MVP reste inchangé : **GO pour démo interne supervisée**, pas production-ready.
