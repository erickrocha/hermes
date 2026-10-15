# Integração de rastreamento de veículos

Como o Hermes obtém a posição e o estado da ignição dos veículos e — tão importante quanto — o que
ele deliberadamente **não** faz com isso ainda.

Decisões por trás deste documento: `PD-025` (consumir um provedor, não construir um rastreador),
`D-15` (qual provedor, e seu contrato), `D-05`/`D-06` (escopo de tenant fora de uma requisição).
Requisitos: `HRMS-900`…`HRMS-905`.

## O formato: consumido, não reconstruído

O Hermes **consome a API de um provedor de GPS de terceiros já existente**. Ele não implementa um
rastreador próprio que faz polling de veículos (`HRMS-900`).

Isso é uma reversão deliberada de como funciona o sistema que o Hermes substitui. O `operacao-trm`
executa seu próprio `rastreador-cron`, que faz polling dos dispositivos e grava o estado de
produção. A `PD-025` descarta isso: a plataforma foi criada para aposentar esse job, e
reimplementá-lo aqui herdaria a manutenção junto com o comportamento.

## O provedor e seu contrato

O `HRMS-901` exige que o contrato do provedor seja registrado **antes** de o componente de ingestão
ser projetado, para que o componente seja projetado uma única vez em vez de reescrito quando o
formato se mostrar diferente. Ele é registrado aqui.

| Propriedade | Valor |
|---|---|
| Provedor | **Traccar auto-hospedado** (aditivo da `D-15` de 2026-10-09); os rastreadores são VITANA VIT-4100-R. Originalmente PinME — `https://api.pinme.io/api`, um serviço hospedado compatível com Traccar, que o `operacao-trm` ainda usa |
| Família | **Traccar** (API REST: `/devices`, `/positions`, `/reports/events`) |
| Autenticação | **HTTP Basic em toda chamada** — `Authorization: Basic base64(email:password)`. Não há `/session`, nem troca de token, nem token bearer |
| Direção | **Pull.** O Hermes pergunta; o provedor nunca chama o Hermes |
| Frequência de atualização | Guiada pelo dispositivo. As posições são lidas sob demanda; um fix com mais de **5 minutos** não é tratado como atual |

Este contrato não precisou ser obtido de forma especulativa. O `operacao-trm` integra exatamente
este provedor em produção hoje, e sua edge function `rastreador-status` é uma implementação de
referência funcional. A narrativa está em
`01-project_truth/operacao-trm/01-ai-native/rastreador-status-edge-doc.md`.

**Por que pull importa além do formato de transmissão.** Um provedor push tornaria a ingestão um
endpoint de entrada autenticado, colocando-a na superfície pública do crate `application` e sob a disciplina
de whitelist do `EPIC-XF-03`. Com pull, ela vira código agendado, fora de requisição — o que a
coloca diretamente na questão de escopo de tenant abaixo. Os dois formatos são componentes
diferentes, e não um mesmo componente com uma constante diferente, e é por isso que o `HRMS-901`
insistiu em saber isso primeiro.

### Endpoints usados

| Endpoint | Método | Finalidade |
|---|---|---|
| `/positions` | GET | Última posição conhecida de **todos** os dispositivos, em uma única chamada. Não é por dispositivo — o cliente indexa por `deviceId` |
| `/reports/events` | GET | Alternativa (fallback). `?deviceId=…&from=…&to=…&type=ignitionOn&type=ignitionOff`, com parâmetros `deviceId` e `type` repetidos e uma janela em ISO-8601 |

### Campos do payload dos quais se depende

`latitude`, `longitude`, `valid`, `serverTime` / `fixTime` / `deviceTime` (vale o primeiro
presente), `attributes.ignition` (booleano) e `attributes.totalDistance` / `odometer` / `distance`
(metros acumulados).

## Atualidade e degradação

Estas regras são **herdadas de uma integração em produção, não inventadas aqui**, e duas delas
foram pagas com incidentes reais registrados nas próprias notas do `operacao-trm`. Elas estão
escritas com o raciocínio anexado para que um leitor futuro não "simplifique" uma regra cujo custo
foi esquecido.

- Uma posição com mais de **5 minutos**, ou marcada como `valid: false`, ou sem o atributo
  `ignition`, não é considerada confiável como atual. A ingestão recorre ao relatório de eventos de
  ignição em uma janela de **24 horas**.
- Um timestamp no *futuro* é desvio de relógio, não atualidade. Um dispositivo com a hora errada não
  pode alegar estar permanentemente atual.
- Nenhum evento também ⇒ o status é **desconhecido**, explicitamente sinalizado como não confiável
  em vez de presumido.
- Um relatório de eventos com falha **degrada apenas os dispositivos que precisavam dele**. Nunca
  faz o lote falhar: os veículos que responderam por `/positions` continuam sendo leituras boas. Uma
  chamada a `/positions` com falha *faz* o lote falhar, porque não há resposta parcial a que
  recorrer.
- **Uma leitura desatualizada nunca pode deixar um veículo marcado como estando na base.** Um
  *evento* de ignição é evidência confiável sobre a ignição — é um fato registrado — portanto tal
  leitura é confiável. O que ela não é é evidência da *posição* atual.
  `VehicleTrackingStatus::supports_presence_claim` concentra essa distinção em um só lugar, em vez
  de em cada ponto de chamada que possa esquecê-la.
- A última coordenada conhecida é anexada **mesmo quando desatualizada**: um veículo estacionado
  para de emitir, mas não se moveu, e uma visão da garagem ainda precisa de um lugar onde
  desenhá-lo.

## Onde fica a fronteira

O `HRMS-902` exige que o contrato do provedor fique confinado a uma única fronteira de ingestão,
para que trocar de provedor seja uma mudança em um módulo e não no domínio.

| Camada | Arquivo | Conhece o PinME? |
|---|---|---|
| Fronteira | `backend/business/src/gateway/tracking_provider.rs` | **Sim — e nenhuma outra conhece** |
| Vocabulário | `backend/business/src/domain/vehicle_tracking.rs` | Não |
| Regras | `backend/business/src/use_cases/vehicle_tracking_use_case.rs` | Não |

A separação não é cosmética. A regra de atualidade, a decisão de fallback e a regra de presença são
política *do Hermes*; os nomes dos campos JSON são do provedor. Se vivessem juntos, trocar de
provedor significaria redecidir a política, e a suíte de aceitação afirma a separação (`FT-003`)
justamente porque fronteiras se erodem silenciosamente.

A configuração é nomeada pelo papel e não pelo fornecedor — `TRACKING_API_BASE_URL`,
`TRACKING_API_EMAIL`, `TRACKING_API_PASSWORD` — para que uma troca de provedor não seja uma mudança
na configuração de implantação. As credenciais são codificadas em Base64 uma única vez no
construtor e nunca aparecem em uma mensagem de erro nem em uma linha de log.

## Escopo de tenant

A ingestão roda **fora de qualquer requisição autenticada**, que é exatamente o risco sobre o qual
o `HRMS-008` e a `U-016` foram escritos.

A `D-05` tornou `Denied` o padrão fora de requisição. Antes desta integração, a única saída desse
padrão era `run_as_platform`, que concede a plataforma inteira — de modo que qualquer coisa agindo
em nome de um único tenant precisava assumir um alcance em toda a plataforma de que não necessitava.
Isso é escopo adquirido por omissão, o acidente que a `U-016` nomeia.

`entity::audit::run_for_tenant(tenant_id, actor, …)` foi adicionada para isso: um escopo de tenant
**explícito e nomeado** (`HRMS-903`). A ingestão usa essa função e não usa `run_as_platform`; o
`FT-004` e o `FT-005` afirmam as duas metades.

## O que deliberadamente não é construído

**Nada é persistido.** Não há tabela de rastreamento, nem entidade, nem migração.

O `HRMS-905` exige que o sistema declare o que consome os dados de rastreamento ingeridos *antes* de
esses dados serem persistidos. Os consumidores candidatos — a fila de serviço da garagem e o ciclo
de combustível — são a decisão **`D-01`** do `operacao-trm`, que está em aberto e que **não é uma
decisão do Hermes tomar**. Construir um repositório de telemetria agora seria inventar um esquema
para um leitor que ninguém especificou.

Isso é o requisito sendo cumprido, e não uma lacuna no trabalho.

O que a fatia *efetivamente* define é o formato que um repositório guardaria, porque o
`VehicleTrackingStatus` carrega as quatro propriedades que a nota da `PD-025` argumenta que a `D-01`
precisa: um **valor**, sua **proveniência** (qual fonte respondeu), sua **qualidade** (confiável ou
não) e um **instante de referência** (as-of). Quando a `D-01` nomear um consumidor, a entrada já
será explícita e com proveniência, em vez de algo a ser adaptado depois.

O Hermes também não possui nenhuma entidade de veículo, garagem, viagem ou combustível, e esta
fatia não introduz nenhuma. O rastreamento chega como uma *integração*, não como um modelo de
domínio; o domínio de operações de frota chega de `01-project_truth/operacao-trm/`, não daqui.
