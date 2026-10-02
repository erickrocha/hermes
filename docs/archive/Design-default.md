> **Archived (EPIC-BO-01-S04, HRMS-410, U-002).** This document was inherited under the title
> "Design System — Hermes (Web)" but its body describes a different, earlier product: a pt-BR
> nutrition-clinic SaaS ("clínicas de nutrição", patient macro chips, a `frontend/src/styles/`
> tree that does not exist in this repository). Project Truth's own note is that retitling it to
> "Hermes" without updating the body made the confusion *more* urgent, not less — someone building
> a screen from this file would be building the wrong product's screen under the right name.
>
> It is kept here, unedited below this notice, only as a record of what the console's *current*
> default palette happened to be copied from before the Transmega tenant theme existed
> (`backoffice/src/theme.ts`, `backoffice/src/styles/main.scss`) — the actual, current source of
> truth for both the vendor's own theme and any tenant's. Do not build a new screen from the
> component/layout guidance below; it was never accepted for Hermes.
>
> ---

# Design System — Hermes (Web)

## 1. Conceito Visual
SaaS de gestão para clínicas de nutrição: visual limpo, confiável e "tech" sem ser
frio — fundo predominantemente branco/azulado, cards com bordas suaves e sombras
discretas, acentos em azul (ação/confiança) com toques pontuais de amarelo
(destaque/energia). Aplica-se tanto à landing page institucional quanto ao
dashboard interno (admin, sysadmin e área do paciente).

## 2. Paleta de Cores

Tokens definidos em `frontend/src/styles/_variables.scss` e expostos como CSS
custom properties em `_theme.scss` (`--bg-primary`, `--accent-primary`, etc.).

| Uso | Cor | Variável |
|---|---|---|
| Fundo principal | `#F7FAFF` (branco azulado) | `--bg-primary` |
| Fundo alternativo/cards | `#FFFFFF` | `--bg-alt` |
| Fundo seção de destaque escura | `#12467F` (azul profundo) | `--bg-dark` |
| Texto heading | `#0F2A47` (azul quase preto) | `--text-heading` |
| Texto body/secundário | `#5A6B80` (cinza azulado) | `--text-body` |
| Acento primário (CTA, links, ícones) | `#1D6FD1` | `--accent-primary` |
| Acento primário hover | `#155BAE` | `--accent-hover` |
| Acento secundário (destaque/badge) | `#FFC629` (amarelo) | `--accent-secondary` |
| Acento secundário hover | `#F2B300` | `--accent-secondary-hover` |
| Borda | `#DCE6F2` | `--border-color` |
| Fundo de badge/superfície sutil | `#EDF3FC` | `--badge-bg` |
| Fundo de badge com destaque | `#FFF4D1` | `--badge-accent-bg` |

**Regra do amarelo**: usar só como acento — fundo de badge, ícone de destaque ou
detalhe gráfico. Nunca como cor de texto sobre branco (reprova em contraste AA).
Texto e CTA são sempre azuis.

### Cores de status (usadas ad-hoc em alerts/banners, fora dos tokens de tema)
| Status | Cor | Uso |
|---|---|---|
| Sucesso | `#16A34A` / `#047857` (fundo `#ECFDF5`) | Banners e ícones de confirmação |
| Erro | `#DC2626` / `#B91C1C` (fundo `#FEF2F2`) | Validação, exclusão, estados de erro |
| Aviso | `#D97706` | Banners de atenção (ex: pagamento pendente) |
| Macros (nutrição) | proteína `#2563EB`, carbo `#B45309`, gordura `#B91C1C` | Chips de macro na área do paciente |

## 3. Tipografia
- **Heading**: `Jost` (peso leve/regular/medium/bold conforme contexto).
- **Corpo**: `Jost`, com fallback `Poppins`, sans-serif.
- Pesos: light `300`, regular `400`, medium `500`, bold `600`.
- Landing: `h1` grande e centralizado (`clamp(2.1rem, 5.2vw, 3.4rem)`), palavra de
  destaque em `--accent-primary` dentro de `<span>`. `h2` de seção também
  centralizado (`clamp(1.6rem, 3.4vw, 2.3rem)`, peso medium).
- Dashboard/admin: títulos de seção (`.section-title`) menores e à esquerda
  (`1.65rem`, peso medium), sem uppercase.
- Labels, badges e botões de ação em admin usam uppercase com letter-spacing
  amplo (`0.08–0.11em`); textos de corpo e nav não usam uppercase.
- Line-height generoso no body (`1.6`) para leitura confortável.

## 4. Componentes de UI

### Botões
- **Pill (landing)** — `.lp-btn`: rounded total (`border-radius-pill` = `9999px`),
  padding `11px 22px`, peso medium.
  - `.lp-btn-primary`: fundo `--accent-primary`, texto branco, hover
    `--accent-hover`.
  - `.lp-btn-outline`: transparente, borda `--border-color`, hover fica azul.
- **Gradiente (admin/sysadmin)** — `.btn-action-primary`: pill com gradiente
  `135deg, --accent-primary → --accent-hover`, texto branco uppercase,
  letter-spacing, sombra azul suave, leve `translateY(-1px)` no hover.
- **Retangular compacto (área do paciente)** — `.customer-btn-primary` /
  `-outline` / `-secondary`: `border-radius: 8px` (não pill), padding
  `0.6rem 1.4rem`, peso bold, sem uppercase.
- Todos os botões usam `transition: all 0.3s cubic-bezier(0.4,0,0.2,1)`
  (`$transition-smooth`) e `:focus-visible` com outline em amarelo (landing) ou
  azul translúcido (admin).

### Badges / Pills
- `.lp-badge`: pill pequeno, fundo `--badge-accent-bg` (amarelo claro), texto
  uppercase com tracking largo (`0.11em`), cor de texto = heading (nunca amarelo
  puro no texto). Variante `.lp-badge-onDark` para fundos escuros.
- `.customer-badge-pill`: pill com fundo `--badge-bg` (azul claro), texto azul,
  peso bold, sem uppercase — usado como tag informativa na área do paciente.

### Cards
- `.lp-card` (landing): borda `1px solid --border-color`, `border-radius-card`
  (`20px`), fundo branco, hover troca a cor da borda para azul (sem sombra
  pesada).
- `.customer-surface-card`: mesma borda/raio, mas com sombra suave
  (`0 4px 14px rgba(15,42,71,0.06)`) em vez de mudança de borda no hover.
- `.form-card` (admin): raio menor (`12px`), título com ícone azul à esquerda.
- Ícones dentro de cards ficam em contêiner circular/quadrado arredondado com
  fundo `--badge-accent-bg` (`.lp-icon`, 42×42, `border-radius: 12px`).

### Formulários
- Inputs/selects: fundo `--bg-primary`, borda `--border-color`,
  `border-radius: 6px`, padding `0.6rem 0.85rem`; desabilitado usa fundo
  `--badge-bg`.
- Labels: `--text-heading`, `0.82rem`, peso medium, acima do campo.
- Estado de erro: borda vermelha clara (`#FCA5A5`) + texto `#B91C1C`.

### Navegação / Chrome
- **Landing header** (`.lp-header`): sticky, fundo branco 90% opaco com
  `backdrop-filter: blur(10px)`, borda inferior sutil.
- **Topbar do dashboard** (`.main-topbar`): fundo branco sólido, sombra bem leve
  (`0 2px 10px rgba(0,0,0,0.02)`), logo à esquerda (símbolo em `--accent-primary`
  + wordmark caixa baixa + subtítulo uppercase pequeno com tracking), menu de
  usuário à direita em pill com avatar circular azul.
- Sidebar (`.main-sidebar`) e drawer mobile off-canvas com backdrop escuro
  (`rgba(0,0,0,0.4)`), toggle hamburger visível só abaixo de 901px.

### Ícones
- Biblioteca: `lucide-react` (line icons). Cor herdada via `currentColor`,
  tipicamente `--accent-primary` dentro de contêineres de destaque.

## 5. Layout & Espaçamento
- Container central com `max-width: 1200px` (`$container-max-width`).
- Seções da landing (`.lp-section`) com `padding: 72px 0`, alternando fundo
  branco / `--bg-primary` (`.lp-section-alt`) / azul profundo (`.lp-section-dark`,
  texto branco) para criar ritmo visual — mesmo princípio do site institucional
  da Lapidation, mas em tons azuis em vez de terrosos.
- Grids responsivos via `repeat(auto-fit, minmax(Npx, 1fr))`, travados em
  colunas fixas acima de breakpoints específicos quando o número de itens exige
  (ex.: 3 colunas fixas ≥1100px para evitar item órfão).
- Hero da landing: centralizado, fundo com gradiente radial sutil em amarelo
  claro sobre `--bg-primary`.
- Raios de borda padronizados: `pill` (9999px) para botões/badges/avatares,
  `card` (20px) para cards de destaque, `12px`/`8px`/`6px` para elementos
  aninhados menores (ícone, botão de app, input).
- Sombras sempre suaves e tingidas de azul-marinho (`rgba(15,42,71, x)`), nunca
  cinza puro — reforça a identidade "azul" mesmo em elevação neutra.

## 6. Tema & Modo
- Um único tema claro (não há dark mode implementado); `--bg-dark` é usado
  apenas como fundo de *seção*, não como tema alternativo global.
- Todas as cores de marca vêm de custom properties em `:root`, permitindo trocar
  o tema por tenant/clínica no futuro sem reescrever componentes.

## 7. Logo / Marca
- Símbolo abstrato em `--accent-primary` (currentColor) ao lado do wordmark.
- Wordmark "Hermes" em caixa baixa, sem tracking, cor de heading.
- Subtítulo institucional (nome da clínica ativa) em caixa alta, tamanho
  pequeno (`0.65rem`), tracking bem amplo (`0.2em`), cor de texto secundário —
  usado no topbar para indicar o tenant ativo.

## 8. Tom de Voz (refletido no visual)
Direto e orientado a benefício, em português (pt-BR): foco em resolver dor
operacional da clínica (ficha clínica, plano alimentar, cobrança) com uma
estética confiável de "software sério", evitando o tom lifestyle/premium da
marca irmã Lapidation — aqui o azul comunica solidez e o amarelo pontua
conquista/energia (ex.: badges de destaque, ícones de sucesso).
