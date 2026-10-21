# Plano — importação do snapshot Operação TRM no Hermes (DEV)

Estado em 2026-10-20. Documento de passagem para continuar em outra máquina.

## Objetivo

Importar no banco **de desenvolvimento** os dados reais (defasados) da Transmega exportados do Operação TRM, criando o tenant **Transmega**. Duas fases. Importar não é portar regras do TRM: só dados.

- Fonte (somente leitura, nunca editar nem commitar): `04-codebase/operacao-trm/operacao-trm-completo-2026-08-10T19-01-22-645Z.json` (62 MB, gerado em 10/08/2026; dados operacionais de ~26/06 a 10/08/2026). O arquivo está na árvore do submódulo `operacao-trm`; na outra máquina será preciso copiá-lo (não está versionado de forma portátil).
- O arquivo **não tem leituras de GPS** (sem lat/long/velocidade). Só o `rastreador_device_id` por veículo, odômetro do rastreador e o histórico de tags `tag_rastreador`. Posições reais virão da API do rastreador (ver `docs/TRACKING.md`).

## Estado atual (já feito)

- MariaDB de dev ativo e saudável (projeto `dev`, container `dev-mariadb-1`, porta 3307, volume `dev_mariadb-data`).
- Migrations aplicadas (`cd backend && cargo run -p application -- migrate`): 90 migrations. Todas as tabelas de importação continuam vazias; **tenant ainda não criado**.
- Fase 1 importada em 2026-10-20 para o tenant DEV `id=1`: 33 veículos, 16 clientes, 5 tipos de serviço, 8 modelos de garagem, 123 peças e 123 ajustes de estoque. O device `393333` ficou apenas no veículo ativo `FJZ1D61`.
- Mapeamentos aplicados: `clientes.ativo=null` → `Unknown`; `tiposServico.ativo=null` → inativo; `DIESEL` → `External`; `LIMP INTERNA` → `Internal`; `obrigatorio_saida` ausente → `false`. Fase 2 continua pendente.
- `tools/import_trm_snapshot.py` usa UUIDs determinísticos e aceita reexecução. Não usar `--remove-orphans` ao iniciar o compose.

## Passo 0 — Tenant Transmega

Tabela `tenant`: `business_name`, `tax_id` (NOT NULL, único), `country_code` (NOT NULL, `BR`), `business_plan_id` (opcional). `business_plan` está vazio; plano não é necessário para importar.
**Pendente do dono:** CNPJ real da Transmega. Usar um valor marcado como provisório se não houver, e registrar.
Criar via API de SysAdmin (`POST /tenant`, `SYSADMIN_EMAIL` do `backend/.env`) ou por SQL com `uuid` binário; preferir a API para passar pelas regras de domínio.

## Fase 1 — dados mestres

| TRM (`dados.*`) | Hermes | Regras |
|---|---|---|
| `veiculos` (33) | `vehicle` | `plate`: maiúsculas, sem espaços (`fjz1D52`, `FJZ0i41`, `EKH 8243`). `prefix`←`prefixo`; `model`←`modelo`; `vehicle_type`←`tipo` (texto livre); `odometer_km`←`km_atual`; `tank_capacity_liters`←`tanque_litros`; `tracker_device_id`←`rastreador_device_id`; `status`: `Active` se `ativo`, senão `Inactive`. **Não migrar** `tag_garagem`, `fretado_desde`, `ign_off_desde`, `viagem_verificar_*`, `fuel_*`, `tracker_pending`, `odo_*_m` (estado transitório do TRM). |
| `clientes` (16) | `customer` | `name`, `notes`←`observacao`, `status`←`ativo` (conferir o vocabulário aceito em `business/src/use_cases/customer_use_case.rs`; o status é obrigatório e não vazio). |
| `tiposServico` (5) | `service_type` | `code` não existe no TRM: gerar a partir do nome (único por tenant, `uq_service_type_tenant_code`); `category`, `active`. |
| `servicosGaragemModelos` (8) | `garage_service_model` | `name_key` = `normalize_name(name)` (ver `garage_service_model_use_case.rs`), único por tenant. `service_group` é `Internal` ou `External`; `required_for_departure`, `display_order`←`ordem`, `applicability` padrão `{"scope":"all"}`. Verificar se o tenant novo já recebe modelos padrão antes de inserir, para não duplicar. |
| `pecas` (123) | `part` + `stock_movement` | `name`, `category` (43 vazias), `application`←`aplicacao`, `minimum_stock`←`estoque_minimo`, `unit` obrigatório (TRM não tem: usar `UN`), `unit_value_cents`←`valor_unitario`×100, `default_supplier`←`fornecedor_padrao`, `location`←`localizacao`. O estoque atual (`estoque_atual`) entra como **uma** movimentação `Adjustment` (`cost_source` `None`, sem custo unitário/total), sem copiar as 281 movimentações do TRM; `RegisteredUnitValue` fica no custo cadastrado da peça. O saldo do Hermes é calculado pela soma das movimentações (`current_stock`). |
| `fornecedores` (21) | (a definir) | Não há tabela `supplier`; fornecedor é texto livre (`default_supplier`, `supplier` em OS/movimentação). Não importar agora. |
| `modelosPreventiva` (15) | `preventive_plan` (fase 2) | Os nomes embutem prefixos de veículos ("… 1647 2501 …"); reestruturar na fase 2. |

Decisões já tomadas na análise:
- `tracker_device_id` é **único globalmente** (`uq_vehicle_tracker_device`). O device `393333` aparece em dois veículos do TRM (id 15 FJZ0A31, inativo; id 29 FJZ1D61): manter só no ativo (29); o 15 fica sem device.
- Veículo 24 (NTQ8D39, prefixo 1039) não tem device: importar sem.
- Veículo 15 é inativo (`Inactive`).
- Motoristas **ficam fora**: no Hermes motorista é `user` com e-mail/senha/CPF (`role` `Driver`), o TRM não tem e-mail. Fica para a fase do app do motorista (C-032). Quando entrarem, CPF/telefone são dado pessoal: pseudonimizar fora de produção.
- Descartar sempre: `usuarios`, `pushSubscriptions`, `sugestoesIa`, `aprendizadosIa`, `avisosMotorista`, `notificacoesManutencao`, `manutencoesNaoExecutadas`, `historico` (exceto `tag_rastreador`, ver fase 2).

## Fase 2 — histórico e massa de teste

| TRM | Hermes | Regras |
|---|---|---|
| `abastecimentos` (451 vivos) | `fuel_entry` | `recorded_at`←`data_hora`, `volume_liters`←`volume_litros`, `value_cents`←`valor`×100, `odometer_km`←`odometro_km`, `station`←`posto`, `full_tank`←`tanque_cheio`, `origin` (valores atuais do TRM: `cta_sync`, `motorista_foto`, `manual_retroativo`; conferir o vocabulário do Hermes), `provider_transaction_id`←`externo_id` (único: torna a carga idempotente), `deleted_at`. |
| `evolucaoKm` (76.941 linhas) | `km_evolution` | **Deduplicar antes**: só ~762 leituras distintas (veículo, data, hora, km). Descartar `cadastro_inicial` redundantes. Há 26 regressões de odômetro por digitação (ex.: veículo 23: 1340015→340015; veículo 20: 620056→62092): rejeitar ou importar sinalizadas em `notes`. |
| `preventivas` (72) | `preventive_plan` | `plan_name`←`tipo_manutencao`, `control_type`, `interval_km`←`regra_km`, `interval_days`←`regra_dias`, `last_service_km`←`ultimo_km`, `last_service_date`←`ultima_data`. |
| `ordemServicos` (129 vivas) | `work_order` | `origin`, `opened_at`←`data_abertura`, `odometer_km`←`km_atual`, `service_type`, `description`←`descricao_geral`, `status`, `supplier`, nota fiscal. Itens/lançamentos/pedidos de compra: depois. |
| `necessidades` (67), `linhasEscala` (85), `viagensExtra` (260), `escalas` (102) | `transport_demand`, `transport_demand_allocation`, `extra_trip`, `daily_schedule` | Só como massa de teste (datas jun–ago/2026). `daily_schedule` exige `driver_id`: sem motoristas importados, **pular escalas** ou aguardar. `viagensExtra.destino` tem mojibake (`SÃƒO PAULO`): corrigir a codificação ao importar. |
| `historico` ação `tag_rastreador` (8.027) | `vehicle_presence_event` (avaliar) | Linha do tempo GARAGEM / DISPONIVEL / FRETADO / EM VIAGEM; útil para testar a máquina de estados. Confirmar o mapeamento antes. |
| `ciclosGaragem` (22.953) | — | 98% têm saída = retorno no mesmo instante (artefato). Só ~679 são viagens reais. Não importar agora. |

Qualidade a lembrar: `km_atual` (painel) e `odo_base_m` (rastreador) divergem em centenas de km por veículo; decidir a fonte da verdade antes de usar o odômetro do rastreador. Os dados têm ~2,5 meses de defasagem: servem de semente, não de estado atual.

## Como implementar

Script único em `tools/` na raiz do repositório SDD (fora do código do Hermes, sem crates novas, conforme `project-patterns.md`):

1. Lê o JSON (caminho por argumento), sem alterá-lo.
2. Gera/executa `INSERT` com `uuid` binário (16 bytes), `created_by='import-trm'`, `tenant_id` do Transmega e `created_at`/`updated_at` preenchidos. Se `created_at` não existir na fonte, usar `updated_at`; se ambos estiverem ausentes, usar o instante da importação. Embora as colunas aceitem `NULL` no banco, os modelos ORM exigem timestamps presentes para ler as entidades.
3. `--dry-run` primeiro: imprime contagens, rejeições e duplicidades.
4. Idempotente: UUID determinístico por tabela + ID original do TRM; chaves naturais também são validadas quando o domínio já as define. Não usar `name` como identidade de peça: há dois pares de peças com nomes repetidos.
5. Uma transação por fase; falha aborta tudo.
6. Validação ao final: contagens por tabela, nenhum `vehicle` sem tenant, e `cargo test -p business --test schema_matches_entities -- --ignored` com `DATABASE_URL=mysql://hermes:brutal@localhost:3307/hermes`.

Fase 1: `python3 tools/import_trm_snapshot.py` executa somente a prévia. Para aplicar, usar `--apply --tenant-id ID` depois de criar/confirmar o tenant e resolver todos os bloqueios da prévia. Valores nulos de `clientes.ativo`, `tiposServico.ativo` e `servicosGaragemModelos.grupo` exigem opções explícitas; nenhuma linha é gravada enquanto houver bloqueios. Fase 2 ainda não está implementada no script.

Cuidado ao escrever fora de requisição: o escopo de tenant é negado por padrão (`entity/src/audit.rs`); escrever por SQL direto ignora isso, então o script deve ser explícito com `tenant_id`.

## Retomada na outra máquina

```
cd 04-codebase/hermes
docker compose -f infra/dev/docker-compose.yml up -d --wait
cd backend && cargo run -p application -- migrate     # confirma 90 migrations
```
Depois: criar o tenant (passo 0) → script em dry-run → fase 1 → validar → fase 2.

## Pendências para o dono
1. CNPJ real da Transmega.
2. Confirmar os vocabulários (`customer.status`, `fuel_entry.origin`, `unit`) lendo o código antes de gravar.
3. Decidir se motoristas entram agora (exige e-mail) ou só na fase do app.
