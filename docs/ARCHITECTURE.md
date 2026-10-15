# Justificativa da arquitetura

EPIC-XF-08-S02 (HRMS-034). As três decisões abaixo — Rust, a divisão em crates e o console
implantado separadamente — foram tomadas pelo dono do projeto antes de este repositório ter
qualquer ADR ou nota de design, portanto nada no histórico de commits as registra.

O raciocínio do dono foi capturado diretamente em **2026-09-19, ao fechar a decisão D-13**. Cada
seção apresenta primeiro esse raciocínio e depois o que a estrutura construída entrega de forma
observável. Os dois são mantidos separados de propósito: o primeiro é o motivo pelo qual a escolha
foi feita, o segundo é o motivo pelo qual ela continua se sustentando. Um leitor futuro deve
conseguir distinguir um do outro.

## Por que Rust

**Raciocínio do dono (D-13).** Conhecimento prévio da linguagem e o fato de ela ser segura e
rápida. A escolha foi deliberada com base nesses motivos, e não o resultado de um levantamento de
alternativas.

O que ela entrega, de forma observável: segurança de memória e correção em tempo de compilação para
um serviço que guarda credenciais e dados de cobrança, um único binário autocontido para implantar
e um perfil de release ajustado para isso (`opt-level = 3`, LTO thin, em `backend/Cargo.toml`).

## Por que um workspace de quatro crates (`entity` / `business` / `migration` / `application`)

**Raciocínio do dono (D-13).** Organização, reuso e boas práticas. A divisão é um padrão estrutural
que o dono aplica deliberadamente, e não uma reação a um problema específico que esta base de
código tenha enfrentado.

O que ela entrega, de forma observável — o grafo de dependências é estritamente de mão única
(`application → business → entity`, `application → migration`) e o crate `application` gera o
único binário `hermes`:

- `business` não tem dependência de `axum` e não usa nenhum tipo HTTP — compila e é testável como
  uma biblioteca comum, independentemente de existir um servidor.
- A feature do cargo `mock` (`business/mock` → `sea-orm/mock`) existe justamente para que a lógica
  de domínio possa ser testada sem nenhum banco de dados — veja `business/tests/mock.rs`.
- `entity` carrega definições de tabelas e nada além disso, de modo que uma migração ou uma consulta
  nunca tenha lógica de negócio escondida dentro de um `Model`.

Duas regras ratificadas pelo dono agora se somam a esse layout como restrições permanentes:
identificadores de persistência somente em inglês, e exatamente esta divisão em quatro crates, sem
desvios (AD-019, AD-020). Elas vieram depois do fato — codificam a divisão em vez de explicar sua
origem.

## Por que um console implantado separadamente (`backoffice/`)

**Raciocínio do dono (D-13).** Espera-se que o backend atenda **muitos clientes** — um aplicativo
móvel, o console web e integrações de terceiros. Um console soldado ao servidor faria do primeiro
cliente adicional uma reescrita; por isso a API foi mantida como o produto e o console como um
consumidor dela.

Isso resolve uma questão que a estrutura sozinha não conseguia responder: a separação foi feita
para múltiplos clientes previstos, e não por independência de implantação nem por divisão de
equipes.

O que ela entrega, de forma observável: `backoffice/` é seu próprio pacote npm, com sua própria
cadeia de ferramentas, e acessa a API somente por HTTP em um `VITE_API_URL` configurável. Ele não
possui nenhum acesso privilegiado próprio — toda decisão de autorização que ele exibe é imposta
novamente pela API, nunca delegada a ele (veja `backend/business/src/domain/authorization.rs`). A
API é, portanto, o único ponto de integração: o console pode ser substituído, ou ganhar um segundo
cliente ao lado, sem que o backend mude.

Não há um segundo cliente neste repositório hoje; o design prevê um.

## O que este documento não é

Ele não repete as restrições de plataforma, de interface e de estrutura já registradas como
entradas `AD-###` no workspace de project-truth do SDD deste projeto (fora deste repositório) — esse
é o registro autoritativo e versionado, e duplicá-lo aqui apenas criaria uma segunda cópia sujeita
a divergir. Este arquivo existe para que um(a) engenheiro(a) novo(a) trabalhando a partir de um
checkout apenas *deste* repositório — sem acesso àquele workspace — não precise reconstruir o
"porquê" a partir das migrações.
