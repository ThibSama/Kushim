# kushim-auth/api

Service d'authentification Rust/Axum pour Kushim.

Il gere :
- l'inscription
- la connexion
- la rotation des refresh tokens
- la revocation de refresh tokens
- l'identite de l'utilisateur courant
- la phrase de recuperation
- le reset de mot de passe via `public_handle + recovery_phrase`
- le rate limiting Redis des flux sensibles

Il ne gere pas :
- les portefeuilles
- les `portfolio_operations`
- les actifs de marche
- les snapshots
- les read models portfolio

## Perimetre metier

Tables possedees :
- `users`
- `user_recovery_phrases`
- `revoked_tokens`

Tables lues en lecture seule :
- `roles`

Tables non possedees :
- `portfolios`
- `portfolio_operations`
- `assets`
- snapshots
- read models portfolio

## Variables d'environnement

Variables supportees :
- `DATABASE_URL`
- `REDIS_URL`
- `RATE_LIMIT_ENABLED`
- `AUTH_SERVICE_HOST`
- `AUTH_SERVICE_PORT`
- `APP_ENV`
- `RUST_LOG`
- `AUTH_JWT_SECRET`
- `JWT_ISSUER`
- `ACCESS_TOKEN_TTL_SECONDS`
- `REFRESH_TOKEN_TTL_SECONDS`

Exemple hote :

```dotenv
DATABASE_URL=postgresql://kushim:kushim_secret_dev@localhost:5432/kushim
REDIS_URL=redis://127.0.0.1:6379/0
RATE_LIMIT_ENABLED=false
AUTH_SERVICE_HOST=0.0.0.0
AUTH_SERVICE_PORT=3002
APP_ENV=development
RUST_LOG=info
AUTH_JWT_SECRET=dev_only_change_me_minimum_32_chars
JWT_ISSUER=kushim-auth
ACCESS_TOKEN_TTL_SECONDS=900
REFRESH_TOKEN_TTL_SECONDS=2592000
```

Exemple Docker :

```dotenv
DATABASE_URL=postgresql://kushim:kushim_secret_dev@database:5432/kushim
REDIS_URL=redis://redis:6379/0
RATE_LIMIT_ENABLED=true
AUTH_SERVICE_HOST=0.0.0.0
AUTH_SERVICE_PORT=3002
APP_ENV=docker
RUST_LOG=info
AUTH_JWT_SECRET=dev_only_change_me_minimum_32_chars
JWT_ISSUER=kushim-auth
ACCESS_TOKEN_TTL_SECONDS=900
REFRESH_TOKEN_TTL_SECONDS=2592000
```

Regles importantes :
- depuis l'hote, PostgreSQL est atteint via `localhost:5432`
- depuis Docker, PostgreSQL est atteint via `database:5432`
- depuis l'hote, Redis est atteint via `127.0.0.1:6379`
- depuis Docker, Redis est atteint via `redis:6379`
- si `RATE_LIMIT_ENABLED=true`, Redis devient une dependance obligatoire au demarrage et dans `/ready`
- les secrets de production ne doivent jamais etre commites
- `AUTH_JWT_SECRET` doit faire au moins 32 caracteres
- en `APP_ENV=production`, le secret JWT ne peut pas reutiliser le secret de dev ni contenir des placeholders evidents (`dev_only`, `change_me`, `changeme`, `secret`, `example`)

## Endpoints

### `GET /health`

But :
- verifier que le process HTTP repond
- exposer un marqueur du binaire charge

Auth :
- aucune

Reponse type :

```json
{
  "status": "ok",
  "service": "kushim-auth",
  "version": "0.1.0",
  "environment": "docker",
  "routes_version": "auth-routes-v1"
}
```

### `GET /ready`

But :
- verifier PostgreSQL
- verifier Redis si `RATE_LIMIT_ENABLED=true`

Auth :
- aucune

Succes :
- `200`

Erreurs communes :
- `503 service_unavailable`

### `POST /auth/signup`

But :
- creer un utilisateur actif
- emettre un couple `access_token + refresh_token`

Auth :
- aucune

Requete :

```json
{
  "username": "Alice",
  "public_handle": "alice_handle",
  "password": "correct horse battery"
}
```

Reponse :

```json
{
  "user": {
    "id_user": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
    "username": "Alice",
    "public_handle": "alice_handle",
    "role": "user",
    "recovery_setup_completed": false,
    "created_at": "2026-06-05T10:00:00Z"
  },
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "access_token_expires_at": "2026-06-05T10:15:00Z",
  "refresh_token_expires_at": "2026-07-05T10:00:00Z"
}
```

Erreurs communes :
- `400` validation
- `409 public_handle_conflict`
- `429 rate_limited`

### `POST /auth/login`

But :
- authentifier un utilisateur actif
- emettre un nouveau couple de tokens

Auth :
- aucune

Requete :

```json
{
  "public_handle": "alice_handle",
  "password": "correct horse battery"
}
```

Reponse :
- meme structure que `POST /auth/signup`

Erreurs communes :
- `400` validation
- `401 invalid_credentials`
- `429 rate_limited`

### `POST /auth/refresh`

But :
- valider un refresh token
- revoquer l'ancien `jti`
- emettre un nouveau couple `access + refresh`

Auth :
- aucune, mais un refresh token valide est requis dans le body

Requete :

```json
{
  "refresh_token": "eyJ..."
}
```

Reponse :

```json
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "access_token_expires_at": "2026-06-05T10:15:00Z",
  "refresh_token_expires_at": "2026-07-05T10:00:00Z"
}
```

Erreurs communes :
- `400` validation
- `401 invalid_token`
- `401 invalid_token_type`
- `401 token_expired`
- `401 refresh_token_revoked`
- `429 rate_limited`

### `POST /auth/logout`

But :
- revoquer un refresh token

Auth :
- aucune, mais un refresh token est fourni dans le body

Requete :

```json
{
  "refresh_token": "eyJ..."
}
```

Reponse :

```json
{
  "success": true
}
```

Comportement :
- refresh token valide -> `200`
- refresh token deja revoque mais encore structurellement valide -> `200`
- access token passe a la place -> `401`
- token malforme -> `401`
- token expire -> `401`

### `GET /auth/me`

But :
- renvoyer l'utilisateur courant associe a un access token valide

Auth :
- `Authorization: Bearer <access_token>`

Reponse :

```json
{
  "user": {
    "id_user": "2ff2d2ae-9f7c-4d7b-bf35-3a7d7c2d6f4f",
    "username": "Alice",
    "public_handle": "alice_handle",
    "role": "user",
    "recovery_setup_completed": true,
    "created_at": "2026-06-05T10:00:00Z"
  }
}
```

Erreurs communes :
- `401 missing_bearer_token`
- `401 invalid_bearer_token`
- `401 token_expired`

### `POST /auth/recovery/setup`

But :
- definir ou remplacer la phrase de recuperation d'un utilisateur authentifie
- marquer `users.recovery_setup_completed = true`

Auth :
- `Authorization: Bearer <access_token>`

Requete :

```json
{
  "current_password": "correct horse battery",
  "recovery_phrase": "this is a long recovery phrase"
}
```

Reponse :

```json
{
  "success": true
}
```

Erreurs communes :
- `400` validation
- `401 invalid_bearer_token`
- `401 invalid_credentials`
- `429 rate_limited`

### `POST /auth/recovery/reset-password`

But :
- reinitialiser un mot de passe a partir de `public_handle + recovery_phrase`

Auth :
- aucune

Requete :

```json
{
  "public_handle": "alice_handle",
  "recovery_phrase": "this is a long recovery phrase",
  "new_password": "a brand new secure password"
}
```

Reponse :

```json
{
  "success": true
}
```

Erreurs communes :
- `400` validation
- `401 invalid_recovery_phrase`
- `429 rate_limited`

## Politique mot de passe

Le service applique :
- hachage `Argon2id`
- salt aleatoire securise
- stockage en format PHC encode

Regles :
- minimum `12` caracteres
- maximum `128` caracteres
- mot de passe vide ou blanc rejete

Garanties :
- le mot de passe en clair n'est jamais stocke
- `password_hash` n'est jamais renvoye par l'API
- aucun mot de passe en clair n'est logge volontairement

## Modele de tokens

Le service emet :
- un access token court
- un refresh token plus long

Claims JWT :
- `sub`
- `public_handle`
- `role`
- `token_type`
- `jti`
- `iat`
- `exp`
- `iss`

Regles :
- chaque token possede un `jti` unique
- les access tokens servent aux endpoints proteges
- les refresh tokens servent a la rotation de session
- `revoked_tokens` ne stocke que des `jti`, jamais les tokens bruts
- apres `refresh`, l'ancien refresh token ne peut plus etre reutilise
- apres `logout`, le refresh token revoque ne peut plus etre reutilise

## Modele de phrase de recuperation

Regles :
- une seule phrase de recuperation par utilisateur
- la phrase est hachee avant stockage
- la phrase en clair n'est jamais stockee
- la phrase hachee n'est jamais renvoyee par l'API
- la table de recovery codes n'existe pas et ne doit pas etre recreee

Flux :
- `POST /auth/recovery/setup`
  - utilisateur authentifie
  - mot de passe courant requis
  - phrase de recuperation hachee et upsertee
- `POST /auth/recovery/reset-password`
  - `public_handle`
  - `recovery_phrase`
  - `new_password`

## Rate limiting

Si `RATE_LIMIT_ENABLED=true`, les endpoints sensibles sont limites via Redis avec une fenetre fixe.

Cles Redis :
- `rate_limit:{scope}:{identifier}:{window}`

Scopes utilises :
- `auth`
- `login:ip`
- `login:handle`
- `signup:ip`
- `refresh:ip`
- `recovery_reset:ip`
- `recovery_reset:handle`
- `recovery_setup:ip`
- `recovery_setup:user`

Limites actuelles :
- login : `20 / 10 min` par IP
- login : `10 / 10 min` par `public_handle`
- signup : `10 / 1 h` par IP
- refresh : `60 / 10 min` par IP
- recovery reset : `10 / 1 h` par IP
- recovery reset : `5 / 1 h` par `public_handle`
- recovery setup : `20 / 1 h` par IP
- recovery setup : `5 / 1 h` par utilisateur
- fallback global auth : `120 / min` par IP

Reponse quand la limite est depassee :

```json
{
  "error": {
    "code": "rate_limited",
    "message": "too many attempts, please try again later"
  }
}
```

Le header `Retry-After` est ajoute.

Important :
- l'extraction IP actuelle s'appuie sur `X-Forwarded-For`, puis `X-Real-IP`, puis `unknown`
- si le service est derriere Nginx plus tard, la confiance proxy devra etre configuree explicitement

## Headers de securite HTTP

Toutes les routes `/auth/*` ajoutent :
- `Cache-Control: no-store`
- `Pragma: no-cache`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: no-referrer`

But :
- eviter la mise en cache accidentelle des reponses contenant des tokens
- durcir le comportement navigateur/client

## JSON request hardening

Les DTOs de requete utilisent `serde(deny_unknown_fields)`.

Consequence :
- les champs JSON inattendus sont rejetes
- un payload avec `extra_field` ne sera plus accepte silencieusement

Comportement actuel :
- ces erreurs de deserialisation remontent via l'extractor Axum
- le statut observe reste `422`

## Politique de logs securite

Le service loggue des evenements securite utiles sans secrets :
- `login_failed`
- `login_success`
- `refresh_failed`
- `refresh_success`
- `logout`
- `recovery_setup_failed`
- `recovery_setup_success`
- `reset_password_failed`
- `reset_password_success`
- `rate_limited`

Regles :
- ne jamais logguer mot de passe, phrase de recuperation, JWT, `password_hash` ou `phrase_hash`
- ne pas logguer de body brut
- les `public_handle` sont redacted dans les logs quand utilises

## Limitation connue

Sans table de session ou de token family :
- un reset de mot de passe ne peut pas revoquer automatiquement tous les refresh tokens deja emis et inconnus du serveur

Amelioration future recommandee :
- introduire plus tard une vraie couche de session/token family
- ne pas inventer cette table dans ce service tant que l'architecture globale n'est pas validee

## Suivi supply-chain

`cargo audit` signale actuellement :
- `RUSTSEC-2023-0071` sur `rsa`

Analyse actuelle :
- le service utilise `HS256` pour ses JWT, pas RSA
- le risque reel direct pour ce service est donc limite
- la dependance reste presente via `jsonwebtoken`

Action recommandee :
- garder `cargo audit` dans les checks
- ajouter `cargo deny` en CI si possible
- reevaluer a chaque mise a jour de `jsonwebtoken` et `sqlx`

## Developpement local

Commandes utiles :

```powershell
cd E:\Kushim\kushim-auth\api
cargo fmt
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
```

Avec PostgreSQL explicite depuis l'hote :

```powershell
$env:DATABASE_URL='postgresql://kushim:kushim_secret_dev@localhost:5432/kushim'
cargo test
```

Execution locale :

```powershell
cd E:\Kushim\kushim-auth\api
copy .env.example .env
cargo run
```

## Docker

```powershell
cd E:\Kushim
docker compose build kushim-auth-api
docker compose up -d database redis kushim-auth-api
docker compose logs -f kushim-auth-api
```

Checks simples :

```powershell
curl http://127.0.0.1:3002/health
curl http://127.0.0.1:3002/ready
```

## Exemples de smoke tests

### Signup

```powershell
curl -X POST http://127.0.0.1:3002/auth/signup `
  -H "Content-Type: application/json" `
  -d "{\"username\":\"Alice\",\"public_handle\":\"alice_handle\",\"password\":\"correct horse battery\"}"
```

### Login

```powershell
curl -X POST http://127.0.0.1:3002/auth/login `
  -H "Content-Type: application/json" `
  -d "{\"public_handle\":\"alice_handle\",\"password\":\"correct horse battery\"}"
```

### Refresh

```powershell
curl -X POST http://127.0.0.1:3002/auth/refresh `
  -H "Content-Type: application/json" `
  -d "{\"refresh_token\":\"<refresh_token>\"}"
```

### Me

```powershell
curl http://127.0.0.1:3002/auth/me `
  -H "Authorization: Bearer <access_token>"
```

### Logout

```powershell
curl -X POST http://127.0.0.1:3002/auth/logout `
  -H "Content-Type: application/json" `
  -d "{\"refresh_token\":\"<refresh_token>\"}"
```

### Recovery setup

```powershell
curl -X POST http://127.0.0.1:3002/auth/recovery/setup `
  -H "Authorization: Bearer <access_token>" `
  -H "Content-Type: application/json" `
  -d "{\"current_password\":\"correct horse battery\",\"recovery_phrase\":\"this is a long recovery phrase\"}"
```

### Reset password

```powershell
curl -X POST http://127.0.0.1:3002/auth/recovery/reset-password `
  -H "Content-Type: application/json" `
  -d "{\"public_handle\":\"alice_handle\",\"recovery_phrase\":\"this is a long recovery phrase\",\"new_password\":\"a brand new secure password\"}"
```

## Notes de test

- les tests repository requierent PostgreSQL
- les tests auth/integration requierent PostgreSQL
- cote hote, `DATABASE_URL` doit viser `localhost:5432`
- cote conteneur, `DATABASE_URL` doit viser `database:5432`
- si `RATE_LIMIT_ENABLED=true`, Redis doit etre disponible
- le conteneur expose `routes_version` dans `/health` et `/ready` pour confirmer le bon binaire
