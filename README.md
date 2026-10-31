# Atol

*A dynamic dock and panel for GNOME 51+*

Dock e painel modulares para o GNOME. A dock começa vazia, e o usuário monta o que
ela mostra acoplando widgets a ela. Os widgets são desenvolvidos e distribuídos pelo
próprio projeto Atol, no lugar de extensões de terceiros. O nome vem de um atol, um
anel de ilhas: a dock e o painel são ilhas flutuantes de um mesmo conjunto.

> Em desenvolvimento. Ainda não há versão utilizável.

## Arquitetura
O Atol é um aplicativo separado, nativo de Wayland (Rust + GTK4/libadwaita).
O X11 não é suportado.

O compositor do GNOME (Mutter) não oferece a apps comuns os protocolos que uma
dock precisa (`wlr-layer-shell` e `foreign-toplevel`). Por isso, o Atol usa uma
extensão mínima do GNOME Shell, a `atol-bridge`, que só faz o que o Wayland
não deixa um app fazer:

- fixar a janela da dock na borda da tela e reservar o espaço dela;
- informar janelas, apps abertos e foco por D-Bus;
- ativar, minimizar e fechar janelas e abrir a grade de aplicativos.

Todo o visual, os widgets e as configurações ficam no aplicativo. A ponte é o
único ponto que depende de partes internas do GNOME, para que uma versão nova
do GNOME exija ajuste num lugar só.

| Componente | Função |
|------------|--------|
| `atol-dock` | A dock e os widgets |
| `atol-panel` | O painel superior |
| `atol-bridge` | Extensão do GNOME Shell que liga o aplicativo ao compositor |

## Compatibilidade
O Atol mira o GNOME 51 e as versões seguintes. O GNOME lança uma versão a cada
seis meses, e cada uma pode exigir ajustes na ponte; a tabela mostra o estado
em cada versão.

| GNOME | Atol |
|-------|------|
| 51 | em desenvolvimento |

## Branches
A `main` contém apenas documentação e licença. Todo o código vive em branches
de feature, que não compartilham arquivos com a `main`. Uma feature que
complementa outra nasce a partir dela.

| Branch | Conteúdo | Estado |
|--------|----------|--------|
| `main` | Documentação e licença | — |
| `feat/dock` | A dock (Rust + GTK4/libadwaita) | em desenvolvimento |
| `feat/bridge` | A `feat/dock` mais a `atol-bridge` e o teste num GNOME aninhado (`dev/nested.sh`) | em desenvolvimento |
| `feat/dash` | A `feat/bridge` mais o conteúdo da dock: favoritos, apps abertos, arrastar para reordenar, fixar e desafixar | em desenvolvimento |
| `feat/shell` | A `feat/dash` sem o conteúdo do Dash: a dock vazia, só o vidro onde os widgets serão acoplados | em desenvolvimento |
| `feat/launcher` | A `feat/shell` mais o widget lançador de aplicativos | em desenvolvimento |

## Diário
O registro do que foi feito em cada sessão fica em [`docs/diario/`](docs/diario/).

## Licença
[GPL-3.0-or-later](LICENSE).
