# Hermes — execução com Docker Compose (infra/prod)

Este guia mostra como subir o Hermes na sua máquina para **testes humanos**, usando os arquivos desta pasta.
Todos os comandos são executados **dentro de `infra/prod/`**:

```bash
cd 04-codebase/hermes/infra/prod
```

## Qual variante usar?

| # | Variante | Arquivos | TLS | Imagens | Para que serve |
|---|----------|----------|-----|---------|----------------|
| A | **Interna** (recomendada para teste humano) | `docker-compose.internal.yml` + `.env.internal.example` | não (HTTP) | construídas do código local | Console + API + MariaDB em uma só porta (`http://localhost:8080`) |
| B | Interna + acesso ao banco | A + `docker-compose.db-access.yml` | não | idem | Abrir o MariaDB no DBeaver (somente `127.0.0.1`) |
| C | **Ensaio de produção** (Traefik, HTTP) | `docker-compose.yml` + `docker-compose.rehearsal.yml` | não (sem Let's Encrypt) | construídas e marcadas à mão | Validar roteamento Traefik / strip de `/api` na sua máquina |
| D | Produção real | `docker-compose.yml` + `deploy.sh` | Let's Encrypt | puxadas de um registro (ghcr.io) | **Não roda localmente** (exige domínio público e DNS) |

Para testar telas e regras de negócio, use **A** (e **B** se quiser olhar o banco). **C** só existe para ensaiar a infraestrutura de produção. **D** está descrita no fim apenas para referência.

## Pré-requisitos

- Docker Engine com o plugin Compose v2 (`docker compose version`).
- `curl` e `openssl` (o `deploy-internal.sh` e o `smoke.sh` usam `curl`).
- Porta **8080** livre (variante A/B) ou **80** e **443** livres (variante C). Para outra porta, altere `CONSOLE_PORT` no `.env` e ajuste `HERMES_URL` junto.
- Primeira construção: o build do backend (Rust) e do console leva alguns minutos.

---

## Variante A — Interna (HTTP, uma porta)

Sobe `mariadb` + `api` + `console`. Só o console é publicado (`CONSOLE_PORT`, padrão 8080); ele serve a tela e repassa `/api` para o backend via nginx ([nginx.internal.conf](nginx.internal.conf)). Banco e API **não** ficam expostos no host.

### 1. Criar o `.env`

```bash
cp .env.internal.example .env
```

Preencha (o `.env` é ignorado pelo git; **nunca** o versione):

```bash
# gere cada segredo — os dois tokens precisam ser DIFERENTES e ter >= 32 bytes
openssl rand -hex 32   # ACCESS_TOKEN_SECRET
openssl rand -hex 32   # REFRESH_TOKEN_SECRET
openssl rand -hex 16   # MARIADB_ROOT_PASSWORD / MARIADB_PASSWORD
```

Campos obrigatórios:

| Variável | Valor para teste local |
|----------|------------------------|
| `HERMES_URL` | `http://localhost:8080` (sem `/` no final; é a origem liberada no CORS e usada nos links de convite) |
| `CONSOLE_PORT` | `8080` |
| `IMAGE_TAG` | `local` (ou uma tag fixa por atualização, para poder voltar atrás) |
| `MARIADB_ROOT_PASSWORD`, `MARIADB_PASSWORD` | senhas geradas acima |
| `DATABASE_URL` | `mysql://hermes:<MARIADB_PASSWORD>@mariadb:3306/hermes` — **a senha deve ser igual à de `MARIADB_PASSWORD`** (se a senha tiver caracteres especiais, use hex) |
| `ACCESS_TOKEN_SECRET`, `REFRESH_TOKEN_SECRET` | segredos gerados acima |
| `SYSADMIN_EMAIL`, `SYSADMIN_PASSWORD` | credenciais do administrador da plataforma (recriadas a cada subida da API) |

Opcionais (podem ficar em branco no teste):

- `SMTP_*` — sem eles o usuário é criado, mas o convite **não é enviado**; o servidor avisa no log e você repassa o link manualmente.
- Rastreamento (PinME/Traccar): a conta é **do tenant**, não do ambiente. O dono do tenant a cadastra no console (Tenants > Rastreamento: URL, e-mail e senha); sem ela o tenant fica sem mapa e sem leitura de presença.
- `FUEL_API_*` — token vazio = sincronização de combustível indisponível (503).
- `GARAGE_PRESENCE_POLL_INTERVAL_SECONDS` (60; `0` desliga), `NON_EXECUTION_EVALUATION_INTERVAL_SECONDS` (300; `0` desliga).

### 2. Subir (caminho recomendado)

```bash
./deploy-internal.sh
```

O script valida o `.env`, constrói as imagens, sobe o MariaDB, **grava um dump** em `/backups` (volume `hermes_mariadb-backups`), roda as migrações, sobe `mariadb api console`, espera a API e executa o `smoke.sh` (health, login do SysAdmin e console servido). Termina com `Implantado <tag> em <URL>`.

> Na primeira vez o banco está vazio: o dump é de um banco sem tabelas e isso é esperado.

### 3. Usar

- Console: <http://localhost:8080>
- Login: `SYSADMIN_EMAIL` / `SYSADMIN_PASSWORD` do `.env`.
- Health: <http://localhost:8080/api/health>
- Swagger/OpenAPI **ficam desligados** (a API roda com `APP_ENV=production`).

### 4. Operação do dia a dia

```bash
C="docker compose --env-file .env -f docker-compose.internal.yml"

$C ps                      # estado dos serviços
$C logs -f api             # logs da API (convites sem SMTP aparecem aqui)
$C logs -f console         # nginx
$C restart api             # após editar SYSADMIN_PASSWORD, SMTP etc. no .env (use `up -d api` para reler o .env)
$C stop                    # para sem apagar nada (dados preservados)
$C up -d mariadb api console   # volta (sem migrar)
./smoke.sh                 # reexecuta a verificação (SMOKE_BASE_URL=http://localhost:8080 ./smoke.sh)
```

> Alterou variáveis do `.env`? `restart` **não** relê o `.env`; use `$C up -d api` para recriar o contêiner.

### 5. Atualizar após mudança de código

Reexecute `./deploy-internal.sh` (de preferência com uma `IMAGE_TAG` nova no `.env`). Ele refaz build, dump e migração antes de reiniciar. **Voltar atrás:** reexecute com a `IMAGE_TAG` anterior; se a falha foi de esquema, restaure o dump (seção "Backup e restauração").

### 6. Desligar / zerar

```bash
$C down              # remove os contêineres, MANTÉM os dados (volumes)
$C down -v           # remove também os volumes: APAGA banco e backups
```

Para recomeçar do zero (banco limpo): `$C down -v && ./deploy-internal.sh`.

### Subida manual (sem o script)

Só se você souber o que está fazendo — o script existe para não improvisar a ordem backup → migração → início:

```bash
C="docker compose --env-file .env -f docker-compose.internal.yml"
$C --profile deploy build
$C up -d mariadb
$C --profile deploy run --rm migrate     # precisa terminar com código 0
$C up -d mariadb api console
SMOKE_BASE_URL=http://localhost:8080 ./smoke.sh
```

A API **se recusa a subir** com o esquema atrasado: se esquecer a migração, ela falha de forma explícita.

---

## Variante B — Interna + acesso ao banco (DBeaver)

Acrescenta o arquivo `docker-compose.db-access.yml`, que publica o MariaDB **somente em `127.0.0.1:3307`** (nunca em `0.0.0.0`).

```bash
C="docker compose --env-file .env -f docker-compose.internal.yml -f docker-compose.db-access.yml"
$C up -d mariadb
```

Conexão no DBeaver / cliente SQL:

| Campo | Valor |
|-------|-------|
| Host | `127.0.0.1` |
| Porta | `3307` (ou `DB_ACCESS_PORT` do `.env`) |
| Banco | `hermes` (`MARIADB_DATABASE`) |
| Usuário / senha | `MARIADB_USER` / `MARIADB_PASSWORD` (ou `root` / `MARIADB_ROOT_PASSWORD`) |

Pode combinar com os demais serviços: use o mesmo `$C` acima em `up -d mariadb api console`. **Importante:** o `deploy-internal.sh` usa só o arquivo internal; se rodá-lo depois, o `up` do script recria o `mariadb` sem a porta. Para fechar o acesso, suba de novo sem o `-f docker-compose.db-access.yml`.

---

## Variante C — Ensaio de produção local (Traefik, HTTP)

Roda o `docker-compose.yml` de produção (Traefik na frente, console e API na mesma origem, `/api` com o prefixo removido) com o overlay `docker-compose.rehearsal.yml`, que **adiciona roteadores HTTP puros** e suprime o redirect `:80 → :443`.

**O que valida:** roteamento, strip de `/api`, console servido, CORS coerente, ordem migração → início.
**O que NÃO valida:** emissão de certificado Let's Encrypt, redirect 80→443, HSTS, renovação de certificado.

> Este caminho **usa a porta 80 e monta `/var/run/docker.sock` (somente leitura)**. Não o use ao mesmo tempo que a variante A só por causa da porta — as portas são diferentes (80/443 vs 8080), então podem coexistir, mas o `name: hermes` é o mesmo projeto Compose: **derrube uma antes de subir a outra** para não misturar contêineres e volumes.

### 1. Construir as imagens localmente

O `docker-compose.yml` espera imagens `${API_IMAGE}:${IMAGE_TAG}` de um registro. Localmente, construa e marque você mesmo (o `deploy.sh` faria `pull` e falharia):

```bash
docker build -t hermes-api:rehearsal     ../../backend
docker build -t hermes-console:rehearsal ../../backoffice
```

### 2. Criar um `.env` próprio para o ensaio

Use um arquivo separado para não sobrescrever o `.env` da variante A:

```bash
cp .env.example .env.rehearsal
```

Ajuste em `.env.rehearsal`:

```dotenv
HERMES_DOMAIN=localhost
ACME_EMAIL=ops@example.com          # exigido pelo compose, mas não é usado de fato sem TLS
API_IMAGE=hermes-api
CONSOLE_IMAGE=hermes-console
IMAGE_TAG=rehearsal
# + senhas, DATABASE_URL (com a mesma senha de MARIADB_PASSWORD), tokens, SYSADMIN_* como na variante A
FUEL_API_BASE_URL=
```

### 3. Subir

```bash
C="docker compose --env-file .env.rehearsal -f docker-compose.yml -f docker-compose.rehearsal.yml"

$C up -d mariadb
$C --profile deploy run --rm migrate          # precisa sair com código 0
$C up -d traefik mariadb api console
```

### 4. Verificar e usar

```bash
set -a; . ./.env.rehearsal; set +a
SMOKE_BASE_URL=http://localhost ./smoke.sh
```

Atenção: o `smoke.sh` lê `./.env` (não `.env.rehearsal`), então exporte as variáveis como acima (ou passe `SYSADMIN_EMAIL`/`SYSADMIN_PASSWORD` na linha de comando).

- Console: <http://localhost> (porta 80)
- Painel/logs do Traefik: `$C logs -f traefik` (o `accesslog` está ligado; 404 do próprio Traefik = roteador não casou).

### 5. Desligar

```bash
$C down        # mantém dados
$C down -v     # apaga banco, backups e o volume do letsencrypt
```

---

## Variante D — Produção real (referência, não roda localmente)

`docker-compose.yml` + `./deploy.sh` num VPS com domínio apontando para ele:

- `.env` a partir de `.env.example` com `HERMES_DOMAIN`, `ACME_EMAIL`, `API_IMAGE`, `CONSOLE_IMAGE` e `IMAGE_TAG` **imutável** (o script recusa `latest`).
- O script faz: validar `.env` → `pull` → dump → migração → `up -d traefik mariadb api console` → `smoke.sh` contra `https://$HERMES_DOMAIN`.
- Procedimento completo e rollback: [docs/RUNBOOK-CUTOVER.md](../../docs/RUNBOOK-CUTOVER.md); operação e logs: [docs/OPERATIONS.md](../../docs/OPERATIONS.md).

---

## Backup e restauração (variantes A/B)

O dump antes de cada deploy fica no volume `hermes_mariadb-backups`, em `/backups`.

```bash
C="docker compose --env-file .env -f docker-compose.internal.yml"

# listar
$C exec mariadb ls -lh /backups

# backup manual
$C exec -T mariadb sh -c \
  'mariadb-dump --user=root --password="$MARIADB_ROOT_PASSWORD" --single-transaction --routines --triggers "$MARIADB_DATABASE" > /backups/manual-$(date -u +%Y%m%dT%H%M%SZ).sql'

# copiar para fora do volume
docker cp "$($C ps -q mariadb)":/backups/<arquivo>.sql ./

# restaurar (pare a API antes)
$C stop api console
$C exec -T mariadb sh -c \
  'mariadb --user=root --password="$MARIADB_ROOT_PASSWORD" "$MARIADB_DATABASE" < /backups/<arquivo>.sql'
$C up -d api console
```

## Problemas comuns

| Sintoma | Causa provável |
|---------|----------------|
| `deploy-internal.sh`: `X está vazio no .env` | variável obrigatória sem valor |
| `DATABASE_URL ainda tem a senha de exemplo` | trocar `TROQUE-PELA-SENHA-DO-BANCO` pela `MARIADB_PASSWORD` |
| API cai no boot reclamando de segredos | tokens com < 32 bytes ou iguais entre si |
| API cai no boot reclamando de esquema | migração não rodou (`run --rm migrate`) |
| Login no navegador falha por CORS | `HERMES_URL` diferente do endereço que você digita (ex.: `localhost` vs `127.0.0.1` vs IP da LAN) |
| `Access denied` do banco após trocar senha | o volume `mariadb-data` guarda a senha da **primeira** subida; as variáveis `MARIADB_*` só valem num banco novo. Use `down -v` (apaga os dados) ou altere a senha dentro do MariaDB |
| Porta em uso | outro processo em 8080 (A/B) ou 80/443 (C): mude `CONSOLE_PORT` + `HERMES_URL`, ou pare o outro |
| Convite não chega | `SMTP_*` em branco: pegue o link nos logs da API |
| Traefik devolve 404 (variante C) | overlay `rehearsal` não aplicado, ou `HERMES_DOMAIN` diferente do host usado no navegador |
