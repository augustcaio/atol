#!/bin/sh
# Abre um GNOME aninhado com configuração própria e descartável, com a atol-bridge
# ligada e a dock rodando dentro dele. Nada muda na sessão real.
# Uso: dev/nested.sh [argumentos extras do gnome-shell, ex.: --headless --virtual-monitor 1600x900]
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
uuid=atol-bridge@augustcaio.github.io
display=atol-nested
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/atol/target}"

cargo build --manifest-path "$repo/Cargo.toml"

tmp=$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/atol-nested.XXXXXX")
# O Shell cria este arquivo ao iniciar e só o apaga após 60 s; se um teste curto o
# deixar para trás, o próximo login da sessão real desliga todas as extensões.
guard="${XDG_RUNTIME_DIR:-/tmp}/gnome-shell-disable-extensions"
rm -f "$guard"
trap 'rm -rf "$tmp" "$guard"' EXIT
mkdir -p "$tmp/data/gnome-shell/extensions"
ln -s "$repo/bridge/$uuid" "$tmp/data/gnome-shell/extensions/$uuid"

# Config e dados isolados: o dconf do aninhado não é o da sessão real.
# GTK_A11Y=none: sem o registro de acessibilidade no barramento aninhado, os apps GTK
# (a dock e o visualizador do devkit) não conseguem se registrar e não abrem.
XDG_CONFIG_HOME="$tmp/config" XDG_DATA_HOME="$tmp/data" \
GTK_A11Y=none dbus-run-session -- sh -c '
    gsettings set org.gnome.shell enabled-extensions "[\"$1\"]"
    gnome-shell --wayland --no-x11 --wayland-display "$2" ${4:---devkit} &
    shell=$!
    while [ ! -S "$XDG_RUNTIME_DIR/$2" ]; do sleep 0.2; done
    WAYLAND_DISPLAY="$2" GDK_BACKEND=wayland "$3" &
    wait $shell
' sh "$uuid" "$display" "$CARGO_TARGET_DIR/debug/atol-dock" "$*"
