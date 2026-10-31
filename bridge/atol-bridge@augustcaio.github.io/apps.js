// SPDX-License-Identifier: GPL-3.0-or-later
// Serviço D-Bus que mostra à dock os apps abertos e executa no Shell o que ela pede.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const BUS_NAME = 'io.github.augustcaio.Atol.Bridge';
const OBJECT_PATH = '/io/github/augustcaio/Atol/Bridge';
const IFACE = `<node>
  <interface name="io.github.augustcaio.Atol.Bridge">
    <method name="GetApps"><arg name="apps" type="a(sbb)" direction="out"/></method>
    <method name="Activate"><arg name="app_id" type="s" direction="in"/></method>
    <method name="NewWindow"><arg name="app_id" type="s" direction="in"/></method>
    <method name="LaunchAction">
      <arg name="app_id" type="s" direction="in"/>
      <arg name="action" type="s" direction="in"/>
    </method>
    <method name="Quit"><arg name="app_id" type="s" direction="in"/></method>
    <method name="ToggleAppGrid"/>
    <signal name="AppsChanged"/>
  </interface>
</node>`;

export class AppsService {
    constructor() {
        this._appSystem = Shell.AppSystem.get_default();
        this._tracker = Shell.WindowTracker.get_default();
        this._changedId = 0;

        this._dbus = Gio.DBusExportedObject.wrapJSObject(IFACE, this);
        this._dbus.export(Gio.DBus.session, OBJECT_PATH);
        this._nameId = Gio.bus_own_name_on_connection(Gio.DBus.session, BUS_NAME,
            Gio.BusNameOwnerFlags.NONE, null, null);

        const changed = () => this._queueChanged();
        this._appSystem.connectObject('app-state-changed', changed, this);
        this._tracker.connectObject(
            'notify::focus-app', changed,
            'tracked-windows-changed', changed,
            this);
        global.display.connectObject(
            'window-demands-attention', changed,
            'window-marked-urgent', changed,
            this);
    }

    destroy() {
        this._appSystem.disconnectObject(this);
        this._tracker.disconnectObject(this);
        global.display.disconnectObject(this);
        if (this._changedId)
            GLib.source_remove(this._changedId);
        Gio.bus_unown_name(this._nameId);
        this._dbus.unexport();
    }

    // Abrir um app muda estado, janelas e foco de uma vez; um sinal só basta.
    _queueChanged() {
        if (this._changedId)
            return;
        this._changedId = GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            this._changedId = 0;
            this._dbus.emit_signal('AppsChanged', null);
            return GLib.SOURCE_REMOVE;
        });
    }

    // A entrada vem de qualquer processo da sessão: só aceita apps que o Shell conhece.
    _app(appId) {
        const app = this._appSystem.lookup_app(appId);
        if (!app)
            throw new Error(`app desconhecido: ${appId}`);
        return app;
    }

    GetApps() {
        const focused = this._tracker.focus_app;
        return this._appSystem.get_running().flatMap(app => {
            // janelas fora da barra de tarefas (a própria dock, por exemplo) não contam
            const windows = app.get_windows().filter(w => !w.skip_taskbar);
            if (windows.length === 0 && app.state !== Shell.AppState.STARTING)
                return [];
            const urgent = windows.some(w => w.urgent || w.demands_attention);
            return [[app.get_id(), app === focused, urgent]];
        });
    }

    // Mesmo comportamento do clique no Dash do GNOME.
    Activate(appId) {
        this._app(appId).activate();
        Main.overview.hide();
    }

    NewWindow(appId) {
        const app = this._app(appId);
        if (app.state === Shell.AppState.RUNNING && app.can_open_new_window())
            app.open_new_window(-1);
        else
            app.activate();
        Main.overview.hide();
    }

    LaunchAction(appId, action) {
        const app = this._app(appId);
        if (!app.app_info?.list_actions().includes(action))
            throw new Error(`ação desconhecida: ${action}`);
        app.launch_action(action, global.get_current_time(), -1);
        Main.overview.hide();
    }

    Quit(appId) {
        this._app(appId).request_quit();
    }

    ToggleAppGrid() {
        if (Main.overview.visible)
            Main.overview.hide();
        else
            Main.overview.showApps();
    }
}
