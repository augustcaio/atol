#!/bin/sh
# Abre um GNOME aninhado com configuração própria e descartável, com a atol-bridge
# ligada e a dock rodando dentro dele. Nada muda na sessão real.
# Mudou algo em src/ ou no Cargo.toml: a dock é recompilada e reiniciada sozinha.
# Mudou a ponte: feche e abra de novo (o Shell não recarrega o JS das extensões).
# Uso: dev/nested.sh [argumentos extras do gnome-shell, ex.: --headless --virtual-monitor 1600x900]
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
uuid=atol-bridge@augustcaio.github.io
display=atol-nested-$$  # nome próprio por execução: testes em paralelo não colidem
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/atol/target}"

cargo build --manifest-path "$repo/Cargo.toml"

tmp=$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/atol-nested.XXXXXX")
# O Shell cria este arquivo ao iniciar e só o apaga após 60 s; se um teste curto o
# deixar para trás, o próximo login da sessão real desliga todas as extensões.
guard="${XDG_RUNTIME_DIR:-/tmp}/gnome-shell-disable-extensions"
rm -f "$guard"
cleanup() {
    # Serviços da sessão aninhada (gvfsd-metadata, por exemplo) ainda gravam em $tmp
    # logo depois que o Shell fecha; apagar antes deixa a pasta para trás. Espera até 5 s.
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        grep -qsF "XDG_DATA_HOME=$tmp/data" /proc/[0-9]*/environ || break
        sleep 0.5
    done
    rm -rf "$tmp" "$guard"
}
trap cleanup EXIT
mkdir -p "$tmp/data/gnome-shell/extensions"
ln -s "$repo/bridge/$uuid" "$tmp/data/gnome-shell/extensions/$uuid"
touch "$tmp/stamp"

# Config e dados isolados: o dconf do aninhado não é o da sessão real.
# GTK_A11Y=none: sem o registro de acessibilidade no barramento aninhado, os apps GTK
# (a dock e o visualizador do devkit) não conseguem se registrar e não abrem.
export repo uuid display stamp="$tmp/stamp" extra="$*"
XDG_CONFIG_HOME="$tmp/config" XDG_DATA_HOME="$tmp/data" \
GTK_A11Y=none dbus-run-session -- sh -c '
    gsettings set org.gnome.shell enabled-extensions "[\"$uuid\"]"
    gnome-shell --wayland --no-x11 --wayland-display "$display" ${extra:---devkit} &
    shell=$!
    while [ ! -S "$XDG_RUNTIME_DIR/$display" ]; do sleep 0.2; done

    start_dock() {
        WAYLAND_DISPLAY="$display" GDK_BACKEND=wayland "$CARGO_TARGET_DIR/debug/atol-dock" &
        dock=$!
    }
    start_dock

    # Polling de 1 s em vez de inotify: sem dependência extra.
    while kill -0 $shell 2>/dev/null; do
        sleep 1
        [ -n "$(find "$repo/src" "$repo/Cargo.toml" -newer "$stamp" -print -quit)" ] || continue
        touch "$stamp"
        # build falhou: a dock antiga continua rodando e o erro aparece no terminal
        cargo build --manifest-path "$repo/Cargo.toml" || continue
        kill $dock 2>/dev/null && wait $dock
        echo "atol: dock recarregada"
        start_dock
    done
'
