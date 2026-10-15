# Runbook da virada — onboarding da Transmega no hermes

> **EPIC-XF-10-S07 · `HRMS-043` · `PD-018` · `D-02`**
>
> Este é o procedimento para colocar o hermes em produção e fazer o onboarding do seu
> primeiro tenant. Ele existe porque o `D-02` assumiu o compromisso de *"tudo rodando
> em produção até 2026-10-17"* e, até 2026-09-21, nenhum documento dizia como.
>
> **Leia tudo antes de começar.** O objetivo de um runbook é que
> ninguém esteja decidindo nada às 22:00 na noite da virada.

---

## 0. Quem

| Papel | Responsabilidade na noite |
|---|---|
| **Líder da virada** | Executa todos os comandos do §3. A única pessoa que digita. |
| **Product owner** | Detém o go/no-go em cada portão e o gatilho de rollback do §6. Confirma que os dados da Transmega estão corretos. |
| **Contato da Transmega** | Disponível por telefone. Confirma que consegue entrar e que a identidade visual é a deles. |

Uma pessoa executa os comandos. Todos os demais observam. Se o líder tiver dúvida em
qualquer portão, a resposta é **parar e chamar o responsável**, nunca improvisar.

---

## 1. Pré-requisitos — dias antes, não na noite

Nada do §3 começa até que cada linha aqui seja verdadeira.

| # | Item | Como é confirmado | Responsável |
|---|---|---|---|
| 1 | **`D-18` está resolvido**: existe uma VPS, dimensionada e paga, e alguém está nomeado como seu operador | O operador consegue entrar por SSH | Product owner + líder técnico |
| 2 | O DNS de `$HERMES_DOMAIN` aponta para a VPS, com TTL reduzido para 300s pelo menos 24h antes | `dig +short $HERMES_DOMAIN` | Líder da virada |
| 3 | Docker Engine + Compose v2 instalados na VPS | `docker compose version` | Líder da virada |
| 4 | Repositório clonado no host em `$PRODUCTION_PATH` | `ls infra/prod/docker-compose.yml` | Líder da virada |
| 5 | `infra/prod/.env` criado a partir de `.env.example`, **todos** os valores preenchidos | `./deploy.sh` se recusa a rodar caso contrário | Líder da virada |
| 6 | Segredos de token gerados **no host** com `openssl rand -hex 32`, e os dois diferentes | `deploy.sh` verifica ambos | Líder da virada |
| 7 | Existe uma tag de release e suas imagens estão publicadas | `docker manifest inspect $API_IMAGE:$IMAGE_TAG` | Líder técnico |
| 8 | **A paleta da Transmega está aprovada** (`EPIC-XF-10-S09`, `HRMS-409`, `D-12`) | Confirmação por escrito da Transmega | Product owner |
| 9 | Portas 80 e 443 abertas para o mundo; **3306 não** | `ss -tlnp` no host | Líder da virada |
| 10 | Ensaiado: §3 executado de ponta a ponta contra um host ou domínio descartável | O smoke check do ensaio passou | Líder da virada |

> **O item 8 é o que vai atrasar.** É o único item aqui que depende do
> departamento de marketing de outra empresa, e não da engenharia, e
> o `EPIC-BO-01` já está aceito com uma paleta *rascunho* (commit `8acb161`),
> o que facilita acreditar que está pronto. Não está pronto. Comece por ele.

> **O item 10 é o que será pulado.** Não pule. Um ensaio é a única
> coisa que transforma este documento de um plano em um procedimento.

---

## 2. Congelamento

Do início do §3 até o §5 passar:

- sem merges em `develop`,
- sem novas tags de release,
- sem alterações de schema.

---

## 3. A virada

Cada passo abaixo é executado a partir de `infra/prod` no host de produção.

### 3.1 Implantação

```bash
cd $PRODUCTION_PATH/infra/prod
./deploy.sh
```

O `deploy.sh` faz o seguinte, nesta ordem, e para na primeira falha:

| Passo | O que faz | Pode danificar um sistema em execução? |
|---|---|---|
| 1 | Se recusa a rodar com um `.env` incompleto | Não |
| 2 | Baixa o `IMAGE_TAG` exato | Não |
| 3 | **Faz o dump do banco de dados** no volume `mariadb-backups` | Não |
| 4 | Executa as migrações como etapa de implantação (`PD-033`, `HRMS-045`) | **Sim — primeiro passo destrutivo** |
| 5 | Inicia o Traefik, o MariaDB, a API e o console | Sim |
| 6 | Executa o `smoke.sh` (`HRMS-042`) | Não |

Os passos 1–3 são todos reversíveis sem fazer nada. O primeiro passo que altera
alguma coisa é o 4, e ele não roda até que exista um dump restaurável.

> **PORTÃO A — o script de implantação terminou com código 0.** Se não terminou, pare. Vá para o §6.
> Não o execute de novo "para ver se funciona desta vez".

### 3.2 TLS

O primeiro `docker compose up` dispara um desafio TLS-ALPN do Let's Encrypt.
Ele falha se o DNS estiver errado ou a porta 443 estiver bloqueada — por isso esses
são itens do §1.

```bash
curl -sSI https://$HERMES_DOMAIN/ | head -1        # expect HTTP/2 200
curl -sS  https://$HERMES_DOMAIN/api/health        # expect {"status":"ok","database":"up"}
```

> **PORTÃO B — o TLS é válido e `/api/health` informa o banco de dados no ar.**
> O Let's Encrypt aplica limite de taxa a falhas. Se o certificado não foi emitido, corrija o DNS
> ou o firewall e aguarde; não rode o `deploy.sh` em loop.

### 3.3 Primeiro login

O SysAdmin é criado a partir do `.env` a cada inicialização (`HRMS-024`).

```bash
./smoke.sh
```

Em seguida, entre pelo navegador em `https://$HERMES_DOMAIN` como
`$SYSADMIN_EMAIL`.

> **PORTÃO C — uma pessoa entrou no console em um navegador.** O `smoke.sh` comprova
> que a API responde; somente um navegador comprova que o console, o armazenamento de sessão e
> o roteamento `/api` de mesma origem funcionam juntos.

> **`D-17` (resolvido em 2026-09-18): não existe mecanismo de rotação de senha e
> nenhum será construído.** Trocar a senha do SysAdmin significa editar o `.env` e
> reiniciar a API. Quem puder ler o `.env` é o administrador da plataforma —
> trate o acesso ao host de acordo.

### 3.4 Criar a Transmega

No console, como SysAdmin:

1. Crie o tenant **Transmega** com seu identificador fiscal real (`EPIC-TP-01`).
2. Associe-o ao seu plano (`EPIC-TP-02`, exatamente um plano ativo).
3. Aplique a paleta **aprovada** da Transmega (`HRMS-409`, item 8 do §1).
4. Crie o primeiro proprietário do tenant da Transmega.

> **PORTÃO D — o contato da Transmega entra, nas suas próprias cores, e confirma
> que a identidade visual é a deles.** Este é o alvo de aceitação do `PD-018`, e é
> decisão do responsável, não do líder da virada.

### 3.5 Convites

Se `SMTP_*` estiver configurado, convide os usuários da Transmega e confirme que um e-mail
chega. Se não estiver configurado, as contas ainda são criadas e os tokens de convite
ainda são emitidos — o link precisa ser entregue de outra forma, e alguém deve ser
avisado de que esse é o plano (`EPIC-IA-07`).

---

## 4. Acompanhamento pós-virada

Por **24 horas** após o Portão D:

```bash
docker compose -f infra/prod/docker-compose.yml logs -f --tail=200 api
```

Fique atento às condições descritas em [`OPERATIONS.md`](OPERATIONS.md). Só aumente o TTL do DNS
de volta depois que o acompanhamento terminar.

---

## 5. Concluído

A virada está completa quando os quatro portões passaram, o acompanhamento de 24 horas não encontrou
nada e o responsável confirma. Registre o `IMAGE_TAG` implantado e a data no
registro de mudanças.

---

## 6. Rollback

**Gatilho — qualquer um destes, sem necessidade de debate:**

- o `deploy.sh` terminou com código diferente de zero e a causa não é compreendida em **15 minutos**;
- o `smoke.sh` falha após uma implantação bem-sucedida;
- o Portão C ou o Portão D falha;
- os dados do tenant parecem errados de qualquer forma;
- o responsável manda fazer rollback.

**Perder uma noite é barato. O primeiro dia da Transmega em uma plataforma quebrada não é.**

### 6.1 Rollback somente da aplicação (sem alteração de schema nesta release)

Reimplante a tag anterior. É por isso que `latest` nunca é publicada.

```bash
sed -i "s|^IMAGE_TAG=.*|IMAGE_TAG=<previous-tag>|" .env
./deploy.sh
```

### 6.2 Rollback incluindo uma alteração de schema

As migrações aqui são **somente para frente**: não existe passo `down`, de propósito.
Uma down-migration não testada executada contra dados em produção não é uma posição mais segura do que
uma restauração de dez minutos atrás. A posição de rollback é o dump que o `deploy.sh`
fez no passo 3.

```bash
# 1. Stop serving. Do this first: nothing should write during a restore.
docker compose -f docker-compose.yml stop api console

# 2. Find the dump taken by this deploy.
docker compose -f docker-compose.yml exec mariadb ls -lt /backups

# 3. Restore it.
docker compose -f docker-compose.yml exec -T mariadb sh -c \
  'mariadb --user=root --password="$MARIADB_ROOT_PASSWORD" "$MARIADB_DATABASE" \
   < /backups/<the-dump>.sql'

# 4. Bring the PREVIOUS image back.
sed -i "s|^IMAGE_TAG=.*|IMAGE_TAG=<previous-tag>|" .env
./deploy.sh

# 5. Verify.
./smoke.sh
```

> **Por que o passo 4 funciona.** A API se recusa a iniciar contra um schema que está
> *atrasado* em relação ao binário. Após uma restauração, o schema corresponde à tag *anterior*;
> então a imagem anterior inicia e a nova não iniciaria — a proteção que faz uma
> implantação ruim falhar de forma explícita é a mesma que torna este rollback seguro.

### 6.3 Se a restauração também falhar

Pare. Deixe o sistema fora do ar e chame o responsável e o líder técnico. Um banco de dados
parcialmente restaurado atendendo tráfego real é pior do que uma indisponibilidade, e o dump em
`/backups` continua lá.

---

## 7. O que este runbook não cobre

Declarado para que ninguém descubra isso às 02:00:

- **Backups após a virada.** O `deploy.sh` faz dump *antes de cada implantação*. Isso é uma
  posição de rollback, **não** uma agenda de backup. Um backup agendado fora do host é
  um item do `OPERATIONS.md` e **não está construído** — os dados do primeiro tenant estão
  protegidos apenas contra uma implantação ruim, não contra a perda da VPS.
- **Implantações sem indisponibilidade.** Host único, container de API único: uma implantação é uma
  indisponibilidade curta. Aceito para um tenant; reavaliar antes do segundo.
- **Escala horizontal.** O advisory lock do `HRMS-022` significa que várias instâncias
  *podem* ser migradas com segurança, mas nada aqui executa mais de uma.
- **Recuperação de desastre do próprio host.** Reconstruir a partir deste repositório
  restaura o *ambiente* (`HRMS-039`); não restaura os *dados* sem
  um backup fora do host.
