# Hermes

Um plano de controle SaaS multi-tenant: uma API HTTP em Rust mais um console administrativo em React (o
"backoffice") que gerenciam *tenants*, *planos comerciais* (produtos de assinatura com preço) e
*usuários* com três papéis (administrador da plataforma, proprietário do tenant, usuário do tenant). O Hermes é uma
camada de administração de plataforma e cobrança, não uma aplicação vertical por si só — ele existe para
substituir o [operacao-trm](https://github.com/amzontasp/operacao-trm) (um sistema legado, de um único tenant, de
operação de frotas) por uma arquitetura multi-tenant adequada. A Transmega, operadora atual do operacao-trm,
é o primeiro tenant do Hermes.

## Estrutura do repositório

| Caminho | O que é |
|---|---|
| `backend/` | Workspace Cargo em Rust — veja abaixo |
| `backoffice/` | Console administrativo em Vite + React + TypeScript — veja [`backoffice/README.md`](backoffice/README.md) |
| `client-apps/` | Lógica compartilhada em Kotlin Multiplatform e o app Android TV nativo da garagem |
| `infra/dev/docker-compose.yml` | MariaDB local para desenvolvimento |

### O workspace do backend

Quatro crates com um grafo de dependência estritamente em um sentido, `web → business → entity`, `web → migration`:

| Crate | Responsabilidade |
|---|---|
| `entity` | Apenas as definições de tabela do SeaORM. Nenhuma lógica de negócio. |
| `migration` | Migrações do SeaORM e os dados iniciais em `migration/data/`. |
| `business` | Estruturas de domínio, casos de uso, gateways. Nenhum tipo HTTP — testável sem servidor e, pela feature `mock` do cargo, sem banco de dados. |
| `web` | A API HTTP em Axum: rotas, endpoints, contratos JSON, autenticação, OpenAPI. |

Toda tabela, coluna, entidade e valor de enum é nomeado em inglês, sem exceção — uma reação
deliberada ao esquema em português do operacao-trm, e uma regra que vale para toda tabela futura,
não só para as que um dia substituirão parte do domínio do operacao-trm.

## Executando localmente

1. Suba o banco de dados: `docker compose -f infra/dev/docker-compose.yml up -d`.
2. Copie `backend/.env.example` para `backend/.env` e preencha com valores reais. Os dois segredos
   de token **precisam** ser trocados: gere cada um com `openssl rand -hex 32` (os valores de exemplo
   são propositalmente curtos demais e o servidor os recusa).
3. Aplique as migrações — um passo separado e deliberado (PD-033):
   `cargo run --manifest-path backend/Cargo.toml -- migrate`.
   O servidor **não** migra na inicialização; ele se recusa a iniciar enquanto houver alguma migração
   pendente e informa quais faltam. Rode isto após cada pull que adicione uma migração.
4. Execute a API: `cargo run --manifest-path backend/Cargo.toml`. Em desenvolvimento, o Swagger UI é
   servido em `/swagger-ui` e o documento OpenAPI em `/api-docs/openapi.json`.
5. Execute o console: veja [`backoffice/README.md`](backoffice/README.md).
6. Compile o cliente Android TV: a partir de `client-apps/`, execute
   `ANDROID_HOME="$HOME/Android/Sdk" ANDROID_SDK_ROOT="$HOME/Android/Sdk" ./gradlew :androidApp:assembleDebug`.
   Os builds de debug usam por padrão a API do emulador em `http://10.0.2.2:8080`; defina `HERMES_API_BASE_URL` ou
   `-PhermesApiBaseUrl=...` para apontar para outra API. O HTTP sem criptografia fica habilitado apenas em builds de debug;
   configure uma URL HTTPS para os builds de release.
   Para um APK de release distribuído manualmente, configure `HERMES_RELEASE_KEYSTORE`,
   `HERMES_RELEASE_STORE_PASSWORD`, `HERMES_RELEASE_KEY_ALIAS` e
   `HERMES_RELEASE_KEY_PASSWORD` no ambiente de build e então execute
   `./gradlew :androidApp:assembleRelease`. Mantenha o keystore e as senhas fora do controle de versão.
   Envie `androidApp/build/outputs/apk/release/androidApp-release.apk` ao usuário para instalação local.

## Implantação

As migrações são uma etapa da implantação, nunca um efeito colateral de iniciar o serviço:

1. Execute `hermes migrate` uma vez contra o banco de destino. Ele toma um lock no banco,
   então dois jobs de implantação rodando-o ao mesmo tempo são seguros, e ele não faz nada quando não há
   migração pendente.
2. Inicie (ou faça o rollout do) serviço com `hermes` e nenhum argumento.

Nunca edite uma migração que já foi aplicada em qualquer lugar; corrija avançando, com uma nova (HRMS-026).

## Configuração

Toda configuração é uma variável de ambiente (um `backend/.env` local também é lido; um
`.env` malformado aborta a inicialização em vez de ser carregado parcialmente). Veja `backend/.env.example` para
a lista completa e os valores de desenvolvimento local que combinam com `infra/dev/docker-compose.yml`.

| Variável | Obrigatória | Observações |
|---|---|---|
| `DATABASE_URL`, `HOST`, `PORT` | sim | |
| `ACCESS_TOKEN_SECRET`, `REFRESH_TOKEN_SECRET` | sim | ≥ 32 bytes cada e diferentes entre si, ou a inicialização é recusada |
| `SYSADMIN_EMAIL`, `SYSADMIN_PASSWORD` | sim | O administrador da plataforma, provisionado na primeira inicialização |
| `APP_ENV` | não | Ausente ou `development` = máquina de desenvolvedor. Qualquer outro valor (`production`, `staging`, …) desliga o Swagger UI **e** o documento OpenAPI, e torna `CORS_ALLOWED_ORIGINS` obrigatória |
| `CORS_ALLOWED_ORIGINS` | fora do desenvolvimento | Origens separadas por vírgula. Em desenvolvimento, se ausente, qualquer origem é permitida; nos demais ambientes a inicialização é recusada sem ela |
| `ACCESS_TOKEN_HOURS`, `REFRESH_TOKEN_DAYS` | não | Validade dos tokens; padrão de 3 horas / 7 dias. Valores inválidos voltam ao padrão |
| `SMTP_*`, `BACKOFFICE_BASE_URL` | não | E-mail de convite — veja abaixo |

A conta do administrador da plataforma (`SYSADMIN_EMAIL`/`SYSADMIN_PASSWORD`) é provisionada na primeira
inicialização e reapontada se o e-mail configurado mudar. As senhas são armazenadas com Argon2id; qualquer hash
bcrypt legado é atualizado no próximo login bem-sucedido daquele usuário.

**O e-mail de convite de conta é a única exceção que não falha rápido.** `SMTP_HOST`/`SMTP_PORT`/`SMTP_USER`/
`SMTP_PASSWORD`/`SMTP_FROM`/`BACKOFFICE_BASE_URL` são lidos sob demanda por `POST /user`, não na subida.
Sem eles, criar um usuário continua funcionando — a conta e o token de convite de 7 dias são criados de qualquer
forma — só que o servidor não consegue enviar o link por e-mail, e avisa de forma bem visível nos logs em vez de
descartá-lo em silêncio. Veja `business/src/commons/email_sender.rs` para o raciocínio.

## Verificação

A partir de `backend/`:

- `cargo test --workspace` — testes unitários nos quatro crates.
- `cargo test -p business --features mock --test mock` — os testes de integração sem banco de dados
  que provam que o isolamento entre tenants realmente chega à camada de gateway, e não só ao query builder.
- `cargo clippy --workspace --all-targets -- -D warnings` — o CI falha com qualquer aviso.
- Com o banco local migrado:
  `DATABASE_URL=mysql://hermes:brutal@localhost:3307/hermes cargo test -p business --test schema_matches_entities -- --ignored`
  — prova que toda coluna de entidade existe no esquema migrado.

Os testes de aceitação (Gate 3) ficam em [`acceptance/`](acceptance/README.md).

A partir de `backoffice/`, veja [`backoffice/README.md`](backoffice/README.md).

## Para saber mais

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — por que Rust, por que esta divisão de crates, por que um
  console implantado separadamente.
