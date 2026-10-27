# Análise — migração dos dados reais da Transmega (Supabase → Hermes)

Estado em 2026-10-27. Somente leitura: nada foi alterado no Supabase nem nos bancos do Hermes.
Este documento **não contém dado pessoal nem credencial**; os dados extraídos ficaram em arquivo local fora do repositório (ver §9).

## 1. Onde estão os dados

Projeto Supabase **Operação TRM** (`eopqabjcdqtzocnpaulo`, região us-east-2). O schema `public` tem 61 tabelas, mas os dados reais estão em **quatro camadas**:

| Camada | O que é | Uso |
|---|---|---|
| `app_state['operacao-trm']` (JSON, 4 MB) | Cadastros e operação: veículos, motoristas, clientes, fornecedores, OS e itens, atendimentos de garagem, viagens extras, necessidades, escalas, inspeções, preventivas, manutenções planejadas | **Fonte principal** |
| `app_state['abastecimentos-data']` (JSON, 2 MB) | 4.639 abastecimentos | Fonte |
| Tabelas `trm_*` (id texto + `data` JSON + `deleted_at`) | Coleções grandes: km, histórico, ciclos, serviços de garagem, checklists, peças, estoque, lançamentos, despesas | Fonte |
| Tabelas relacionais (`veiculos`, `ordens_servico`, `motoristas`…) | Só 6–8 linhas cada (semente de uma migração para relacional que não foi adiante) | **Ignorar** |

Há também fotos no **Supabase Storage** (URLs públicas, p.ex. bucket `checklist-fotos`).

**Os dados estão vivos**: o cron do TRM gravou hoje (27/10) e há OS abertas hoje. A migração precisa de um plano de corte (§7).

## 2. Inventário e destino no Hermes

"Vivos" = sem `deleted_at`. O Hermes tem entidade equivalente para quase tudo.

| Origem | Vivos | Destino Hermes | Observação |
|---|---:|---|---|
| veículos | 36 (35 ativos) | `vehicle` | Placas e devices únicos, sem duplicidade. 33 têm rastreador |
| clientes | 16 | `customer` | |
| tipos de serviço | 6 | `service_type` | `codigo` ausente em 4: gerar |
| modelos de serviço da garagem | 8 | `garage_service_model` | |
| peças / movimentações de estoque | 287 / 704 | `part` / `stock_movement` | `unidade` existe na origem |
| fornecedores | 63 | — | Hermes não tem tabela; vira texto (`supplier`) |
| abastecimentos | 4.591 | `fuel_entry` | `externo_id` único → carga idempotente. 48 excluídos/unificados |
| evolução de km | 90.205 linhas → **~4.466 distintas** | `km_evolution` | 89.303 vêm de abastecimento (redundantes). 49 regressões de odômetro |
| OS / itens / lançamentos | 449 / 677 / 236 | `work_order` / `work_order_item` / `work_order_posting` | 7 OS sem número e sem veículo válido |
| pedidos de compra | 23 | `purchase_order` | |
| despesas de veículo | 174 | `vehicle_expense` | |
| preventivas / modelos | 96 / 15 | `preventive_plan` (+ alertas/extensões) | Por veículo; `km_data` → `Both` |
| manutenções planejadas / não executadas | 228 / 90 | `maintenance_plan` / `non_execution_*` | |
| atendimentos de garagem | 1.460 (20 não encerrados) | `garage_attendance` | Nenhum veículo com mais de um aberto |
| serviços da garagem (por atendimento) | 10.356 (8.973 ligados a atendimento existente) | `garage_service` | 1.383 órfãos |
| logs de serviço da garagem | 7.493 | `garage_service_log` | |
| histórico de presença (`tag_rastreador`) | 26.270 | `vehicle_presence_event` | Avaliar; ver §4 |
| inspeções técnicas / itens | 115 / 1.448 | `technical_inspection` / `_item` | |
| checklists de motorista / itens / modelos | 652 / 9.771 / 4 | `checklist_run` / `checklist_answer` / `checklist_template` | Dependem de motorista (§3.1) |
| necessidades / linhas de escala / escalas | 69 / 110 / 115 | `transport_demand` / `daily_schedule` | `daily_schedule` exige motorista |
| viagens extras | 914 (de 1.202) | `extra_trip` | 45 `codigo_externo` repetidos; 194 com mojibake no destino |
| tanque interno, feriado, folgas, liberações | 7 / 1 / 1 / 1 | `internal_tank`, `holiday`, `customer_day_off`, `garage_release` | |
| usuários | 8 | `user` | Ver §3.2 |

**Descartar** (sem equivalente ou sem valor): sugestões/aprendizados de IA, assinaturas push, avisos ao motorista, notificações de manutenção, tabelas do assistente WhatsApp, `trm_tv_pairs`, diagnósticos e cron do `app_state`, `relatorio_servicos_veiculo` (derivado), `ciclos_garagem` (40.879; a doc anterior já apurou que ~98% são artefato — o Hermes recalcula `garage_cycle_state`).

## 3. Diferenças de modelo que exigem decisão

### 3.1 Motoristas × usuários (a maior)
- TRM: **55 motoristas** (52 ativos) sem e-mail; 48 têm CPF (1 CPF duplicado); só 4 têm telefone.
- Hermes: motorista é `user` com **e-mail e senha** (`role = Driver`), e `checklist_run`, `daily_schedule` e `extra_trip` referenciam esse usuário.
- Consequência: sem os motoristas, **não entram** checklists (652), escalas (115) nem os motoristas das viagens. Quem passa 6 meses de operação sem motorista perde a parte mais útil do histórico.
- Opções: (a) criar usuários `Driver` com e-mail provisório e conta **desabilitada** até o convite (preserva vínculos; CPF/telefone são dado pessoal); (b) importar sem os vínculos. Recomendo (a). Precisa da sua decisão e de como os e-mails reais serão obtidos.
- 3 escalas apontam para motorista que não existe no cadastro.

### 3.2 Usuários do console (8)
- Perfis: 6 Administrador, 2 Manutenção → `TenantOwner`/`TenantUser`/`Mechanic` (a definir por pessoa).
- **Não migrar senhas.** O campo `observacao` dos usuários contém textos que parecem senhas; ele não deve ser copiado. Os usuários entram por **convite** (fluxo existente `account_invite`).
- `modulos` (lista de telas) não tem equivalente direto: o Hermes usa perfis de acesso.

### 3.3 Identificadores
- TRM: `uuid`/`text`. Hermes: `id` numérico + `uuid` binário. Manter o padrão do importador atual: **UUID determinístico** por (tabela, id TRM), com as FKs resolvidas por busca. Isso torna a carga reexecutável.

### 3.4 Fotos e anexos
- 89 OS, 132 itens de OS, itens de checklist e 15 `anexos` têm foto/arquivo no Supabase Storage.
- O Hermes **não tem mecanismo de anexo** (o código documenta a lacuna: `foto`, anexo de nota fiscal ficaram de fora).
- Opções: não migrar fotos agora (mantendo o Storage do Supabase disponível), ou copiar os arquivos e guardar a URL quando houver anexo no Hermes. **Não desligar o Supabase antes disso.**

### 3.5 Vocabulários (mapeamento)
| Conceito | TRM | Hermes |
|---|---|---|
| Status da OS | concluida, aberta, cancelada, parcialmente_resolvida, aguardando_peca | Concluded, Open, Cancelled, PartiallyResolved, AwaitingParts |
| Status do item | resolvido, pendente, cancelado, aguardando_peca (15 nulos) | Resolved, Pending, Cancelled, AwaitingParts |
| Origem da OS | checklist_motorista, preventiva_automatica, manual, portal_inspecao, verificacao_consolidada (7 nulos) | Checklist, Preventive, Manual, Inspection (`verificacao_consolidada` e nulos: a decidir) |
| Origem do abastecimento | cta_sync, motorista_foto, manual, manual_retroativo | CtaSync, DriverPhoto, Manual |
| Status do atendimento | saiu_operacao, triagem, enviado_manutencao, manutencao_planejada | LeftForOperation, Open, ReleasedWithPendency/Open (a decidir) |
| Estado do serviço da garagem | pendente, feito, nao_precisou, nao_fez, ok | Pending, Performed, NotNeeded, NotDone (`ok` → Performed) |
| Controle preventivo | km, km_data | Kilometers, Both |
| Tipo de necessidade | recorrente, linha_recorrente, linha_extra | `demand_kind`: Line, ExtraLine, OneOffTrip (a mapear) |
| Status da manutenção planejada | concluida, nao_executada, cancelada, planejada | a mapear em `maintenance_plan.status` |

### 3.6 Horários e datas
TRM guarda `data` + `hora` locais (BRT). O Hermes usa UTC (`DateTimeUtc`). A conversão precisa de fuso explícito (America/Sao_Paulo), não "cortar a string".

### 3.7 Estado transitório do TRM
`tag_garagem`, `ign_off_desde`, `fretado_desde`, ciclos e campos `fuel_*` são estado derivado. O Hermes **calcula** isso (presença, ciclo, tanque). Migrar só a tag atual do veículo (`garage_tag`) e, se houver confiança, os eventos de presença.

## 4. Qualidade dos dados
- 7 OS sem número e sem veículo válido; 4 abastecimentos sem veículo.
- 49 regressões de odômetro (digitação) — rejeitar ou marcar em `notes`.
- 1.383 serviços de garagem apontam para atendimento que não existe.
- 45 códigos externos repetidos em viagens extras; 56 viagens sem veículo (aceitável: o veículo é opcional).
- 194 viagens com mojibake em origem/destino: corrigir a codificação (ex.: `SÃƒO` → `SÃO`).
- Nomes de checklist têm emoji; irrelevante para a migração, mas confirmar o limite de tamanho da coluna.

## 5. Ordem de carga (dependências)
1. Tenant Transmega (já existe no dev). 2. Cadastros: clientes, tipos de serviço, modelos de garagem, veículos, peças. 3. Usuários e motoristas (§3.1/3.2). 4. Estoque, abastecimentos, km, preventivas, inspeções. 5. OS e itens, lançamentos, pedidos de compra, despesas. 6. Atendimentos, serviços e logs da garagem, presença. 7. Necessidades, viagens, escalas, checklists.

## 6. Dev antes da OCI
O dev está desligado (nenhum contêiner `dev-*` ativo). A carga no dev usa o tenant Transmega que já existe; os demais dados do dev podem ser removidos (confirmado). Ensaio no dev → conferência de contagens contra este inventário → só então a OCI, com o dump prévio do `deploy.sh`.

## 7. Corte (por ser dado vivo)
O TRM continua gravando. Estratégia: (a) ensaios no dev quantas vezes forem necessárias (idempotente); (b) **janela de congelamento** curta no TRM para a carga final na OCI; (c) conferência de contagens; (d) só então virar o uso. O importador atual só insere: para uma segunda passada com dados alterados será preciso `upsert` (UUID determinístico já permite).

## 8. Decisões pendentes (do dono)
1. Motoristas: criar usuários `Driver` desabilitados com e-mail provisório (recomendado) ou importar sem vínculos?
2. Fotos: migrar agora (exige mecanismo de anexo) ou manter no Supabase por ora?
3. Histórico de presença (`tag_rastreador`, 26 mil linhas): importar ou só a tag atual?
4. Km: importar apenas as ~4.466 leituras distintas (recomendado).
5. Usuários do console: perfil de cada um e envio de convite.
6. CNPJ real da Transmega no tenant (hoje o `empresas.documento` do Supabase é nulo).
7. Data/hora do congelamento do TRM para a carga final.

## 9. Observações operacionais
- Os dados extraídos (91 MB, com dado pessoal) estão **fora do repositório**, em arquivo `chmod 600` no diretório temporário da sessão. Apagar ao terminar.
- A extração usou `supabase db dump --remote=<ref>`, que baixa a imagem Docker `supabase/postgres` (permanece no cache do Docker; nenhum contêiner ficou ativo).
- A pasta `04-codebase/operacao-trm` não foi tocada.

## 10. Decisões do dono e execução no dev (2026-10-27)

**Decisões:** (1) motoristas viram usuários `Driver` que entram com o **CPF** e uma senha padrão; (2) fotos ficam de fora por ora, a migrar para um storage depois; (3) usuários do console também com senha padrão; (4)–(6) importar tudo o que for possível.

**Mudanças de código:** login por e-mail **ou CPF** (11 dígitos, sem `@`; vale só se houver exatamente uma conta com o CPF) em `authentication_use_case.rs`, com teste; app do motorista com o campo "E-mail ou CPF". Ferramentas: `tools/extract_trm_supabase.py` (Supabase → snapshot, só leitura), `tools/import_trm_full.py` (carga completa, reexecutável) e `tools/hash_password.py` (hash da senha padrão, sem a senha em argumento).

**Resultado no dev (tenant Transmega, id 1):** 36 veículos, 16 clientes, 62 usuários (55 motoristas, 5 proprietários, 2 mecânicos), 287 peças, 704 movimentações (saldo idêntico ao do TRM), 4.587 abastecimentos, 872 leituras de km, 96 preventivas, 449 OS (676 itens, 229 lançamentos de peça), 1.460 atendimentos, 8.595 serviços e 6.738 logs de garagem, 6.469 eventos de presença, 652 checklists (9.724 respostas), 115 inspeções, 228 manutenções planejadas, 914 viagens, 69 demandas e 104 escalas diárias. Login de motorista por CPF verificado na API.

**O que não entrou, e por quê:**
- Fotos e anexos (decisão 2).
- 47 respostas "não se aplica" de checklist (o Hermes só tem conforme/não conforme).
- 22 de 23 pedidos de compra (texto livre de itens que não são peça cadastrada; o Hermes exige a peça).
- 6 lançamentos de OS de serviço (o Hermes só lança peça).
- 89.303 leituras de km de origem "abastecimento" (já estão em `fuel_entry`, D-29) e 30 repetidas.
- Alertas e prorrogações de preventiva, ocorrências de não execução, fornecedores (sem tabela), sugestões de IA, notificações push, avisos ao motorista.
- 1.761 serviços e 753 logs de garagem sem atendimento correspondente.
- 10 contas de motorista ficam **desabilitadas**: 3 inativos no TRM e 7 ativos sem CPF utilizável (um deles por CPF repetido). Sem CPF não há como entrar; os vínculos (escalas, checklists, viagens) foram preservados.

**Pendências antes da OCI:** definir a senha padrão (hoje o dev usa uma senha só de desenvolvimento), o CNPJ real da Transmega (o tenant do dev usa um provisório), o perfil de cada usuário do console (hoje: Administrador → TenantUser, Manutenção → mecânico) e a janela de congelamento do TRM. O Hermes não tem "trocar a senha no primeiro acesso": com senha padrão compartilhada, convém trocá-las logo após o início.

## 11. Execução na OCI (2026-10-27)

Carga completa no tenant 1 da OCI com `import_trm_full.py --replace-import`, numa única transação, depois de um dump (`/backups/pre-trm-import-20261027T141044Z.sql`, volume `hermes_mariadb-backups`). A OCI já tinha uma importação antiga (agosto, 33 veículos e 129 OS, todas com `created_by = import-trm`); só essas linhas foram removidas e substituídas pelas dos dados vivos. Contagens finais iguais às do dev: 36 veículos, 448 OS, 4.589 abastecimentos, 1.460 atendimentos, 652 checklists, 914 viagens etc.; 60 usuários importados (54 motoristas, 4 `TenantUser`, 2 mecânicos) além das 4 contas que já existiam.

**Incidente corrigido:** a primeira versão do upsert sobrescreveu duas contas existentes que coincidiam por e-mail ou CPF (o motorista de teste e `fezonturismo@gmail.com`). Foram restauradas a partir do dump, coluna por coluna, e o upsert agora nunca altera linha que não seja da própria importação (testado). Consequência: essas duas pessoas continuam com as contas antigas e não ganharam conta nova.

**Senha padrão** definida pelo dono. **Login por CPF** só funciona depois que a mudança de `authentication_use_case.rs` for mesclada e implantada na OCI (hoje o login de motorista por CPF responde 401; por e-mail com a senha padrão responde 200).
