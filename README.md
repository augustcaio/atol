# Atol

*A dynamic dock and panel for GNOME 50+*

Dock e painel para o GNOME, com widgets próprios embutidos, sem depender de
extensões externas. O nome vem de um atol, um anel de ilhas: a dock e o painel
são ilhas flutuantes de um mesmo conjunto.

> Em desenvolvimento. Ainda não há versão utilizável.

## Arquitetura
O Atol é um **aplicativo separado**, nativo de Wayland (Rust + GTK4/libadwaita).
O X11 não é suportado.

O compositor do GNOME (Mutter) não oferece a apps comuns os protocolos que uma
dock precisa (`wlr-layer-shell` e `foreign-toplevel`). Por isso, o Atol usa uma
extensão mínima do GNOME Shell, a **atol-bridge**, que só faz o que o Wayland
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
O GNOME lança uma versão a cada seis meses, e cada uma pode exigir ajustes na
ponte.

| GNOME | Atol |
|-------|------|
| 51 | em desenvolvimento |
| 50 | não testado |

## Branches
A `main` contém apenas documentação e licença. Todo o código vive em branches
de feature, que não compartilham arquivos com a `main`.

| Branch | Conteúdo | Estado |
|--------|----------|--------|
| `main` | Documentação e licença | — |

## Licença
[GPL-3.0-or-later](LICENSE).
