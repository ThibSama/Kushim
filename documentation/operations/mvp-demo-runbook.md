# Runbook de démo MVP Kushim

_Date : 2026-06-11_

---

## 1. Objectif de la démo

Montrer que Kushim est un MVP fonctionnel de suivi de portefeuille d'investissement :

- authentification réelle ;
- création de portefeuille et enregistrement d'opérations ;
- données dérivées (KPIs, positions, évolution historique) calculées par le worker à partir de données réelles ;
- navigation dans le catalogue d'actifs et les positions du portefeuille ;
- le tout démontrable localement avec un mock provider de données de marché, ou des données seedées/validées explicitement.

**Ce que cette démo n'est pas :**

- une démo de production ;
- une démo de données de marché réelles généralisées ;
- une démo avec conversion FX ;
- une démo avec un broker ou une plateforme de trading.

---

## 2. Services requis

| Service | Compose name | Port | Rôle |
|---|---|---|---|
| PostgreSQL | `database` | 5432 | Base de données |
| Redis | `redis` | 6379 | Rate limiting, état worker |
| `kushim-auth/api` | `kushim-auth-api` | 3002 | Authentification (signup, login, JWT, handoff) |
| `kushim-api` | `kushim-api` | 8080 | API métier synchrone |
| `kushim-worker` | `kushim-worker` | 8081 | Read models, snapshots, backfill |
| `kushim-market-data` | `kushim-market-data` | 8082 | Données de marché (mock provider + Finnhub courant actions en validation dev) |
| `kushim-auth/front` | `kushim-auth-front` | 3001 | Frontend auth (login, signup) |
| `kushim-app` | — (local dev server) | 5173 | Frontend authentifié |

---

## 3. Checklist pré-démo

- [ ] Docker Desktop en cours d'exécution
- [ ] Repository cloné et à jour
- [ ] Tous les services backend buildés et démarrés
- [ ] Health checks OK sur les 4 services backend
- [ ] Au moins un asset AAPL actif en base (symbol = 'AAPL', status = 'active', native_currency = 'USD')
- [ ] Un portefeuille USD démo créé avec des opérations (dépôt + achat AAPL)
- [ ] Market-data refresh exécuté (mock provider recommandé pour la démo)
- [ ] Worker rebuild + snapshots exécutés
- [ ] Frontend dev server lancé (`npm run dev`)
- [ ] Navigateur ouvert sur `http://localhost:5173`

---

## 4. Démarrage des services

### 4.1 Services Docker (backend + infra)

```powershell
cd E:\Kushim
docker compose build
docker compose up -d database redis kushim-auth-api kushim-api kushim-worker kushim-market-data
```

### 4.2 Vérification des health checks

```powershell
# Auth API
(Invoke-WebRequest -Uri "http://localhost:3002/health" -UseBasicParsing).Content

# Business API
(Invoke-WebRequest -Uri "http://localhost:8080/health" -UseBasicParsing).Content

# Worker
(Invoke-WebRequest -Uri "http://localhost:8081/health" -UseBasicParsing).Content

# Market-data
(Invoke-WebRequest -Uri "http://localhost:8082/health" -UseBasicParsing).Content
```

Tous doivent retourner `{"status":"ok",...}`.

### 4.3 Frontend auth (optionnel — nécessaire pour login via navigateur)

```powershell
cd E:\Kushim\kushim-auth\front
npm install
npm run dev
# → http://localhost:3001
```

### 4.4 Frontend app

```powershell
cd E:\Kushim\kushim-app
npm install
npm run dev
# → http://localhost:5173
```

---

## 5. Refresh des données de marché

Le chemin recommandé pour une démo MVP supervisée reste le provider `mock`, car il alimente aussi les prix historiques de façon déterministe. Finnhub peut être utilisé séparément pour valider des prix courants d'actions avec une allowlist courte.

Finnhub est live-validé pour les prix courants AAPL/MSFT/NVDA. BTC/crypto n'est pas live-validé avec le plan actuel : la tentative mappée `BTC=BINANCE:BTCUSDT` retourne `403 Forbidden`. Les candles historiques Finnhub `/stock/candle` retournent aussi `403 Forbidden` avec le plan actuel.

### 5.0 Option dev : prix courants Finnhub actions uniquement

Cette commande suppose que `FINNHUB_API_KEY` est présent dans `kushim-market-data/.env`, ignoré par Git. Ne jamais afficher ni committer la clé.

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=refresh_current_market_data `
  -e MARKET_DATA_PROVIDER=finnhub `
  -e MARKET_DATA_SYMBOL_ALLOWLIST=AAPL,MSFT,NVDA `
  kushim-market-data
```

Ne pas utiliser cette commande pour BTC ni pour les backfills historiques tant que l'accès provider n'est pas confirmé.

### 5.1 Chemin recommandé démo : mock

Ces commandes alimentent la base avec des prix mock USD pour les 7 symboles supportés (AAPL, MSFT, NVDA, BTC, ETH, SPY, VTI).

#### Prix courants

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=refresh_current_market_data `
  -e MARKET_DATA_PROVIDER=mock `
  kushim-market-data
```

#### Prix historiques

```powershell
docker compose run --rm `
  -e MARKET_DATA_MODE=once `
  -e MARKET_DATA_JOB=fill_missing_price_history_cache `
  -e MARKET_DATA_PROVIDER=mock `
  -e MARKET_DATA_HISTORY_DATE_FROM=2026-05-01 `
  -e MARKET_DATA_HISTORY_DATE_TO=2026-06-11 `
  kushim-market-data
```

Ajuster les dates selon la période de la démo. Le mock provider génère des prix USD déterministes (pas aléatoires).

---

## 6. Worker rebuild et snapshots

Ces commandes supposent qu'un portefeuille existe avec des opérations postées. Remplacer `$portfolioId` par l'UUID réel.

### 6.1 Rebuild read models courants

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=rebuild_current_read_models `
  -e WORKER_TARGET_PORTFOLIO_ID=$portfolioId `
  kushim-worker
```

### 6.2 Snapshot journalier courant

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=generate_daily_snapshots `
  -e WORKER_TARGET_PORTFOLIO_ID=$portfolioId `
  -e WORKER_SNAPSHOT_DATE=2026-06-11 `
  kushim-worker
```

### 6.3 Backfill historique (optionnel — pour le graphe évolution)

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=backfill_daily_snapshots `
  -e WORKER_TARGET_PORTFOLIO_ID=$portfolioId `
  -e WORKER_BACKFILL_DATE_FROM=2026-05-10 `
  -e WORKER_BACKFILL_DATE_TO=2026-06-10 `
  kushim-worker
```

Ajuster les dates. Maximum : 366 jours. Les dates antérieures à la création du portefeuille sont ignorées automatiquement.

---

## 7. Setup du portefeuille démo recommandé

Pour une démo complète, créer :

1. Un utilisateur démo (via signup dans `kushim-auth/front` ou via le smoke test)
2. Un portefeuille **USD** (obligatoire pour que le mock provider produise des valuations non-estimées)
3. Des opérations :
   - 1 dépôt de 10 000 USD (daté ~1 mois avant la démo pour l'historique)
   - 1 achat de 5 AAPL (daté quelques jours après le dépôt)
   - 1 achat de 3 AAPL supplémentaires (daté plus tard)
   - 1 dividende AAPL (optionnel)
4. Exécuter le market-data refresh (section 5)
5. Exécuter le worker rebuild + backfill (section 6)

**Alternative rapide** : exécuter le smoke test backend qui fait tout automatiquement :

```powershell
.\scripts\demo\backend-e2e.ps1
# → 18/18 assertions, portefeuille démo créé avec données
```

Puis récupérer le `portfolioId` affiché dans la sortie pour les commandes worker.

---

## 8. Parcours démo frontend (pas à pas)

### Étape 1 — Authentification

1. Ouvrir `http://localhost:3001` (auth frontend)
2. Se connecter (login) ou créer un compte (signup)
3. Après login → redirection automatique vers `http://localhost:5173` avec `?handoff_code=...`
4. L'app échange le code et établit la session

**Si le handoff ne fonctionne pas** : l'auth frontend peut ne pas être câblé nativement. Dans ce cas, utiliser le handoff manuel (voir la section Troubleshooting).

### Étape 2 — Sélection du portefeuille

1. Si aucun portefeuille → l'app affiche un CTA "Créer un portefeuille"
2. Créer un portefeuille avec nom + devise **USD**
3. Le portefeuille est automatiquement sélectionné

### Étape 3 — Dashboard

Points à montrer :
- **KPIs** : valeur nette, montant investi, gain/perte (données réelles du read model `/summary`)
- **Graphe évolution** : historique multi-jours (données réelles des snapshots `/snapshots/daily`)
- **Allocation** : répartition par classe d'actif en camembert (données réelles des `/holdings`)
- **Top 5 actifs** : positions principales (données réelles des `/holdings`)
- **Transactions récentes** : dernières opérations (données réelles)
- **Bandeau** : indique que le benchmark reste en démonstration — c'est correct

### Étape 4 — Transactions

Points à montrer :
- Liste des opérations avec types, dates, montants
- Filtres par type, statut, période
- Métriques (achats, ventes, dépôts, dividendes)
- Bouton "Ajouter une opération" → créer un dépôt ou un achat

### Étape 5 — Refresh après nouvelle opération (optionnel)

Après avoir ajouté une opération dans la démo :

```powershell
docker compose run --rm `
  -e WORKER_MODE=once `
  -e WORKER_JOB=rebuild_current_read_models `
  -e WORKER_TARGET_PORTFOLIO_ID=$portfolioId `
  kushim-worker
```

Puis rafraîchir le navigateur → les KPIs et positions se mettent à jour.

### Étape 6 — Catalogue d'actifs

Points à montrer :
- `/assets` : liste du catalogue (données réelles de `GET /v1/assets`)
- Recherche : taper "AAPL" → 2 résultats
- Filtres : par classe d'actif, par statut
- Pagination : "Charger plus d'actifs"
- Clic sur un actif → page détail

### Étape 7 — Détail d'un actif

Points à montrer :
- `/assets/:id` : identité (nom, ticker, ISIN, classe, bourse, devise)
- Données de marché : prix courant, source, timestamp
- Métadonnées : secteur, industrie, pays (si présents)
- Aliases (si présents)

### Étape 8 — Positions du portefeuille

Points à montrer :
- `/positions` : positions réelles du portefeuille actif
- Cartes résumé : nombre de positions, valeur totale, P&L
- Tableau : nom, ticker, quantité, coût moyen, valeur marché, P&L, poids
- Recherche par nom/ticker, filtre par classe
- Badge "Estimé" si applicable
- Clic sur une position → détail de l'actif (`/assets/:id`)

### Étape 9 — Paramètres

Points à montrer :
- Profil utilisateur réel (nom, handle, rôle, date de création)
- **Ne pas cliquer** les boutons d'action (voir section 9)

### Étape 10 — Logout

- Cliquer "Se déconnecter"
- Token révoqué côté serveur, session nettoyée, redirection vers le login

---

## 9. Zones à éviter pendant la démo

| Élément | Raison | Conséquence si cliqué |
|---|---|---|
| Dashboard → "Ajouter un actif" (en bas de la section top actifs) | Modal placeholder non fonctionnel | Formulaire s'ouvre mais ne fait rien |
| Dashboard → section Benchmark | Données de démonstration (pas réelles) | Affiché mais trompeur si présenté comme réel |
| Paramètres → "Enregistrer les préférences" | Pas de handler backend | Pas de feedback |
| Paramètres → "Mettre à jour le mot de passe" | Pas de handler backend | Pas de feedback |
| Paramètres → "Supprimer mon compte" | Pas de handler backend | Pas de feedback |
| Création de portefeuille EUR | Mock provider = prix USD uniquement | Holdings `is_estimated=true`, valeurs à 0 |
| Données de marché présentées comme réelles | Mock provider avec prix déterministes | Prix ne reflètent pas le marché réel |

---

## 10. Limitations connues

| Limitation | Impact | Contournement |
|---|---|---|
| Mock market-data provider | Prix USD déterministes, pas de données réelles | Accepter pour la démo, ne pas présenter comme données de marché réelles |
| Pas de FX | Portefeuille EUR → valuations estimées | Utiliser un portefeuille USD |
| Pas de scheduler production | Worker/market-data doivent être lancés manuellement | Exécuter les jobs en mode `once` avant la démo |
| Auth handoff manuel possible | `kushim-auth/front` peut ne pas rediriger automatiquement | Préparer le handoff ou se reconnecter manuellement |
| Benchmark = données démo | Section benchmark du dashboard est un mock | Le bandeau l'indique clairement |
| KPI "Meilleur actif" = "—" | Donnée non branchée sur le KPI card | Cosmétique |
| Quantité brute en Transactions | Affiche `3.0000000000` au lieu de `3` | Cosmétique (Positions est corrigé) |
| Token access TTL = 15 min | Si la démo dure plus de 15 min, le refresh peut être nécessaire | Automatique via le store auth |
| Boutons Paramètres non fonctionnels | UI only, pas de backend | Ne pas cliquer en démo |

---

## 11. Troubleshooting

### Service ne démarre pas

```powershell
docker compose logs <service-name> --tail 50
```

Vérifier : base de données accessible, port libre, env vars correctes.

### Health check échoue

```powershell
docker compose ps
# Vérifier que le service est "running" et "healthy"
```

Si `unhealthy` : attendre 30 secondes et réessayer. Si persistant, redémarrer :

```powershell
docker compose up -d --force-recreate <service-name>
```

### Token expiré (401 Unauthorized)

Le refresh est automatique dans `kushim-app`. Si le refresh échoue aussi → logout automatique → se reconnecter.

Pour la démo backend (PowerShell), se re-authentifier :

```powershell
$loginBody = '{"username":"<username>","password":"<password>"}'
$loginResponse = Invoke-WebRequest `
  -Uri "http://localhost:3002/auth/login" `
  -Method POST -ContentType "application/json" `
  -Body $loginBody -UseBasicParsing
$token = ($loginResponse.Content | ConvertFrom-Json).access_token
$headers = @{ Authorization = "Bearer $token" }
```

### `data_available = false` sur le dashboard

Cause : le worker n'a pas été exécuté pour ce portefeuille.

Fix : exécuter le rebuild read models (section 6.1).

### Positions / KPIs affichent des valeurs à 0

Causes possibles :
1. Market-data refresh non exécuté → exécuter section 5
2. Portefeuille EUR avec mock USD → recréer en USD
3. Asset sans `native_currency = "USD"` → vérifier l'asset en base

### Duplicate user (409 Conflict)

Le `username` existe déjà. Utiliser un nom d'utilisateur différent.

### Handoff ne fonctionne pas

Si `kushim-auth/front` ne redirige pas vers `kushim-app` :
1. Vérifier que l'auth frontend tourne (`http://localhost:3001`)
2. Vérifier les env vars (`NEXT_PUBLIC_APP_URL`, etc.)
3. Alternative : se connecter via l'API (PowerShell), récupérer le token, et le stocker manuellement dans localStorage :

```javascript
// Dans la console navigateur de kushim-app :
localStorage.setItem('kushim_access_token', '<access_token>');
localStorage.setItem('kushim_refresh_token', '<refresh_token>');
location.reload();
```

### Frontend ne charge pas

```powershell
cd E:\Kushim\kushim-app
npm install
npm run dev
```

Vérifier les env vars dans `.env` (copier `.env.example` si nécessaire).

---

## 12. Checklist d'acceptation démo

Après avoir exécuté le parcours complet :

- [ ] Login/signup fonctionne
- [ ] Portefeuille USD créé et sélectionné
- [ ] Opérations visibles dans Transactions
- [ ] Dashboard affiche des KPIs non-nuls
- [ ] Graphe évolution affiche un historique
- [ ] Allocation affiche une répartition
- [ ] Top actifs affiche au moins 1 position
- [ ] Catalogue actifs affiche des résultats
- [ ] Recherche "AAPL" retourne des résultats
- [ ] Détail actif affiche les informations
- [ ] Positions affiche les holdings avec valeur et P&L
- [ ] Clic position → détail actif fonctionne
- [ ] Paramètres affiche le profil réel
- [ ] Logout fonctionne
- [ ] Aucune erreur console bloquante
- [ ] Bandeau benchmark visible et correct

---

## Références

- [Backend E2E smoke test runbook](backend-demo-e2e.md)
- [Backend E2E smoke test script](../../scripts/demo/backend-e2e.ps1)
- [Audit MVP readiness](../reports/audit-mvp-readiness-2026-06-11.fr.md)
- [MVP progress report (FR)](../reports/kushim-mvp-progress-report.fr.md)
- [Docker local dev](docker-local-dev.md)
- [Validation commands](validation-commands.md)
