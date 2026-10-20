# Operação do hermes em produção

> **EPIC-XF-10-S08 · `HRMS-044` · `D-18`**
>
> Para onde vão os logs, o que é monitorado e quem é acionado. O requisito é
> direto sobre o motivo de existir: *"para que uma falha após a virada seja
> observada, e não reportada por um usuário."*
>
> O procedimento de implantação está em [`RUNBOOK-CUTOVER.md`](RUNBOOK-CUTOVER.md). Este arquivo
> trata dos dias seguintes.

---

## 1. O que está rodando

Conforme `D-18`: **uma única VPS rodando Docker Compose atrás do Traefik**, definida por
[`infra/prod/docker-compose.yml`](../infra/prod/docker-compose.yml).

| Container | O que é | Exposição |
|---|---|---|
| `traefik` | Terminação TLS, Let's Encrypt, roteamento | **:80, :443 — as únicas portas públicas** |
| `api` | `hermes`, a API em Rust | via Traefik, sob `/api` |
| `console` | o backoffice em React sobre nginx | via Traefik, todo o resto |
| `mariadb` | o banco de dados | **não publicado** — somente na rede do compose |
| `migrate` | etapa pontual de implantação, `profiles: [deploy]` | nunca em execução |

O console e a API respondem no mesmo hostname. `/api/*` vai para a API
com o prefixo removido; todo o resto é o console.

---

## 2. Para onde vão os logs

Todo container usa o driver `json-file` com **10 MB × 5 arquivos** por
container. Essa é a resposta atual e honesta, e ela tem duas consequências
que vale declarar com clareza:

- os logs ficam **somente na VPS**. Se o host for perdido, eles se perdem junto;
- eles **rotacionam**. Sob carga, "a semana passada" pode não existir mais.

```bash
cd $PRODUCTION_PATH/infra/prod

docker compose logs -f --tail=200 api        # follow the API
docker compose logs --since 1h api           # the last hour
docker compose logs --since 30m traefik      # HTTP-level view: status codes, TLS
docker compose logs mariadb | tail -100
```

`RUST_LOG` controla a verbosidade da API e é `info` em produção. **Não rode
`debug` em produção** além de uma janela curta e deliberada: ele é barulhento o
bastante para rotacionar para fora as entradas interessantes, e registra detalhes
de requisições pertencentes a um tenant.

> **Não construído, e uma lacuna real.** Não há *envio* de logs — nenhum
> coletor central, nenhuma retenção além da rotação, nenhuma busca. Para um tenant
> em um host isso é um ponto de partida defensável, mas não deve sobreviver ao
> segundo tenant. Quem assumir isso é responsável por levantar o tema como uma nova
> decisão; não trate o §2 como uma resposta final.

---

## 3. O que é monitorado e quem é acionado

`HRMS-044` exige *"um conjunto declarado de condições monitoradas e um destinatário
nomeado para cada uma"*. Preencha os nomes antes da virada — uma tabela de papéis
sem o nome de ninguém é o modo de falha que este requisito existe para evitar.

| # | Condição | Como é detectada | Severidade | Destinatário |
|---|---|---|---|---|
| 1 | **O site está fora do ar** — `GET /` não retorna 200 | Verificação externa de disponibilidade a cada 60s | **Acionar imediatamente** | _líder da virada_ |
| 2 | **A API está com problemas** — `GET /api/health` não retorna 200, ou informa `"database":"down"` | Verificação externa de disponibilidade a cada 60s | **Acionar imediatamente** | _líder da virada_ |
| 3 | **Certificado TLS expirando** em até 14 dias | E-mail do Let's Encrypt para `ACME_EMAIL`, mais uma verificação externa | Alta | _líder técnico_ |
| 4 | **Disco acima de 80%** na VPS | Verificação no host | Alta — o MariaDB corrompe com o disco cheio | _líder técnico_ |
| 5 | **Um container está reiniciando repetidamente** | `docker compose ps` mostra reinícios crescendo | Alta | _líder técnico_ |
| 6 | **A API se recusou a iniciar** — `Refusing to start:` ou `Database schema is N migration(s) behind` no log | Inspeção do log após cada implantação | Alta | _líder da virada_ |
| 7 | **Pico de falhas de login** | Log de acesso do Traefik, taxa de 401 | Média — credential stuffing | _líder técnico_ |
| 8 | **Falha na ingestão de Traccar/GPS** (`EPIC-FT-01`) | Log da API: erros do gateway de rastreamento | Média — telemetria desatualizada, não é uma indisponibilidade | _líder técnico_ |
| 9 | **Falha no e-mail de convite** (`EPIC-IA-07`) | Log da API: o aviso SMTP em destaque | Baixa — as contas continuam funcionando, os links precisam ser entregues manualmente | _product owner_ |

**As condições 1 e 2 precisam de um verificador externo** — algo que não esteja nesta VPS,
porque um host fora do ar não consegue informar que está fora do ar. Qualquer serviço
hospedado de monitoramento de disponibilidade apontado para `https://$HERMES_DOMAIN/api/health` atende às duas; o
endpoint já distingue "processo no ar" de "processo no ar, mas banco de dados
inacessível" (`HRMS-042`), que é a distinção que importa.

> **Nada acima está automatizado ainda.** Os endpoints e as linhas de log existem; o
> verificador e o roteamento de alertas são configuração em um serviço de monitoramento que
> o `D-18` não decidiu. Até que sejam configurados, as condições 1–9 são uma lista de
> verificação manual, e o §4 é como elas são checadas.

---

## 4. Verificação diária

Dois minutos, uma vez por dia, até que o monitoramento seja automatizado.

```bash
cd $PRODUCTION_PATH/infra/prod

docker compose ps                                  # all Up, restarts not climbing
curl -sS https://$HERMES_DOMAIN/api/health          # {"status":"ok","database":"up"}
df -h /                                            # under 80%
docker compose logs --since 24h api | grep -iE 'error|refusing|panic' | tail -20
```

---

## 5. Operações de rotina

### Implantar ou fazer rollback

Ambos são a mesma ação — veja [`RUNBOOK-CUTOVER.md`](RUNBOOK-CUTOVER.md) §3
e §6. Fazer rollback é reimplantar o `IMAGE_TAG` anterior.

### Reiniciar a API

```bash
docker compose restart api
```

Seguro: a API não aplica migrações na inicialização (`PD-033`) e se recusa a iniciar se
o schema estiver atrasado, em vez de alterá-lo.

### Trocar um segredo

Edite `infra/prod/.env` e depois rode `docker compose up -d api`. Note os dois que
se comportam de forma diferente:

- **`SYSADMIN_PASSWORD`** é reaplicada a cada inicialização (`HRMS-024`). Conforme `D-17`,
  não existe mecanismo de rotação e nenhum será construído — editar o `.env` e
  reiniciar *é* o procedimento.
- **`ACCESS_TOKEN_SECRET` / `REFRESH_TOKEN_SECRET`** invalidam todos os tokens emitidos quando
  alterados. Todos os usuários são desconectados. Faça isso de forma deliberada.

### Fazer backup do banco de dados

O `deploy.sh` faz um dump antes de cada implantação, no volume `mariadb-backups`. **Isso
é uma posição de rollback, não um backup.** Ele fica no mesmo host dos dados que
protege; portanto sobrevive a uma implantação ruim, mas não sobrevive à perda da VPS.

Um backup agendado **fora do host** **não está construído**. Até que esteja, esta é
a versão manual, e ela deve ser executada em uma agenda da qual alguém seja responsável:

```bash
docker compose exec -T mariadb sh -c \
  'mariadb-dump --user=root --password="$MARIADB_ROOT_PASSWORD" \
   --single-transaction --routines --triggers "$MARIADB_DATABASE"' \
  | gzip > hermes-$(date -u +%Y%m%dT%H%M%SZ).sql.gz
# then copy it OFF this host.
```

> A restauração nunca foi ensaiada com dados no formato de produção. Um backup
> que ninguém restaurou é uma hipótese.

---

## 6. Lacunas conhecidas

Listadas para que sejam decididas em vez de descobertas:

| Lacuna | Consequência | Onde se encaixa |
|---|---|---|
| Sem agenda de backup fora do host | Perder a VPS faz perder os dados da Transmega | Próxima decisão após o `D-18` |
| Sem envio nem retenção de logs | Sem forense além da rotação | Idem |
| Sem alertas automatizados | O §3 depende de alguém executar o §4 | Idem |
| Host único, container de API único | Toda implantação é uma indisponibilidade curta; sem failover | Aceito para um tenant |
| Restauração nunca ensaiada | A posição de rollback não está comprovada | Ensaiar antes da virada |
