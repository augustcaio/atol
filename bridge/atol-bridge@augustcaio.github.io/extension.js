// SPDX-License-Identifier: GPL-3.0-or-later
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Mtk from 'gi://Mtk';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const DOCK_APP_ID = 'io.github.augustcaio.Atol.Dock';
// Padrão do Mutter 51: raio 24 px e saturação 1.25.
const BLUR_RADIUS = 16;
const BLUR_SATURATION = 1.8;

// Centraliza a dock na borda inferior do monitor primário, no tamanho que o app pediu.
const BottomCenter = GObject.registerClass({
    Implements: [Meta.ExternalConstraint],
}, class BottomCenter extends GObject.Object {
    vfunc_constrain(_window, info) {
        const {width, height} = info.new_rect;
        const m = global.display.get_monitor_geometry(global.display.get_primary_monitor());
        info.set_rect(new Mtk.Rectangle({
            x: m.x + Math.floor((m.width - width) / 2),
            y: m.y + m.height - height,
            width,
            height,
        }));
        // true: nenhuma outra regra mexe na posição; sem isso o Mutter empurraria
        // a dock para fora do espaço que ela mesma reserva.
        return true;
    }
});

export default class AtolBridge extends Extension {
    enable() {
        this._constraint = new BottomCenter();
        this._docks = new Map();

        // O vidro da dock é blur do Mutter (ext-background-effect), e o protocolo não leva
        // força nem cor. Como no Liquid Glass: blur leve, para o papel de parede continuar
        // reconhecível (blur forte mistura tudo num tom leitoso), e saturação alta, para as
        // cores atrás da dock ficarem vivas. Vale para a sessão toda; o disable() devolve.
        this._blurDefaults = global.compositor.get_background_blur_params();
        global.compositor.set_background_blur_params(BLUR_RADIUS, BLUR_SATURATION, this._blurDefaults[2]);

        global.display.connectObject('window-created', (_, w) => this._watch(w), this);
        Main.layoutManager.connectObject('monitors-changed', () => this._replaceAll(), this);
        for (const actor of global.get_window_actors())
            this._track(actor.meta_window, true);
    }

    disable() {
        global.display.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        for (const window of [...this._docks.keys()])
            this._untrack(window, true);
        this._docks = null;
        this._constraint = null;
        global.compositor.set_background_blur_params(...this._blurDefaults);
        this._blurDefaults = null;
    }

    // No Wayland a janela nasce antes de o app mandar o app_id (wm_class ainda é null).
    _watch(window) {
        if (window.get_wm_class() !== null) {
            this._track(window);
            return;
        }
        window.connectObject('notify::wm-class', () => {
            window.disconnectObject(this);
            this._track(window);
        }, this);
    }

    _track(window, existing = false) {
        // _docks é null se a extensão foi desligada antes de o app_id chegar
        if (!this._docks || window.get_wm_class() !== DOCK_APP_ID || this._docks.has(window))
            return;

        // DOCK: o Mutter tira a janela do Alt+Tab, da visão geral e da lista de janelas.
        window.set_type(Meta.WindowType.DOCK);
        window.stick();
        window.make_above();
        window.add_external_constraint(this._constraint);

        // Ator invisível que reserva o espaço da dock (strut) sem capturar cliques.
        const strut = new St.Widget({reactive: false, opacity: 0});
        Main.layoutManager.addChrome(strut, {affectsStruts: true, trackFullscreen: true});
        this._docks.set(window, strut);

        window.connectObject(
            'position-changed', () => this._syncStrut(window),
            'size-changed', () => this._syncStrut(window),
            'unmanaged', () => this._untrack(window, false),
            this);
        // Janela nova recebe a restrição no posicionamento inicial; mover antes disso
        // (janela ainda não pronta) gera avisos do Mutter.
        if (existing)
            this._place(window);
    }

    _untrack(window, restore) {
        window.disconnectObject(this);
        this._docks.get(window).destroy();
        this._docks.delete(window);
        if (!restore)
            return;
        window.remove_external_constraint(this._constraint);
        window.unmake_above();
        window.unstick();
        window.set_type(Meta.WindowType.NORMAL);
    }

    // Mover a janela força o Mutter a rodar as restrições, e a nossa decide a posição.
    _place(window) {
        const r = window.get_frame_rect();
        window.move_resize_frame(false, r.x, r.y, r.width, r.height);
        this._syncStrut(window);
    }

    _replaceAll() {
        for (const window of this._docks.keys())
            this._place(window);
    }

    _syncStrut(window) {
        const r = window.get_frame_rect();
        const strut = this._docks.get(window);
        strut.set_position(r.x, r.y);
        strut.set_size(r.width, r.height);
        console.log(`atol-bridge: dock em ${r.x},${r.y} ${r.width}x${r.height}`);
    }
}
