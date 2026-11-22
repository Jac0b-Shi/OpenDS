"""One app palette for GTK surfaces, controls, dialogs and custom canvases."""
from gi.repository import Gdk, Gio, Gtk

PALETTES = {
    False: dict(bg='#f4f6fa', card='#ffffff', fg='#202b3d', muted='#6b778c',
                border='#dce3ee', hover='#edf2fa', selected='#e8f0ff',
                selected_fg='#2363ce', accent='#2868d8', error='#a92d28'),
    True: dict(bg='#10151e', card='#1a2230', fg='#e6ecf5', muted='#a3b1c7',
               border='#3f4e64', hover='#253349', selected='#243d62',
               selected_fg='#a9ccff', accent='#3976df', error='#ffac9f'),
}


def stylesheet(dark):
    p = PALETTES[dark]
    return '''
    @define-color theme_bg_color %(bg)s;
    @define-color theme_base_color %(card)s;
    @define-color theme_fg_color %(fg)s;
    @define-color theme_text_color %(fg)s;
    @define-color theme_selected_bg_color %(accent)s;
    @define-color theme_selected_fg_color #ffffff;
    window, .background, dialog { background-color:%(bg)s; color:%(fg)s; }
    stack, viewport, scrolledwindow { background-color:%(bg)s; color:%(fg)s; }
    label, checkbutton, radiobutton { color:%(fg)s; }
    .sidebar { background:%(card)s; padding:24px 14px; border-right:1px solid %(border)s; }
    .brand, .page-title { font-size:25px; font-weight:700; }
    .section-title { font-size:13px; font-weight:700; }
    .muted { color:%(muted)s; font-size:11px; }
    .card { background:%(card)s; border:1px solid %(border)s; border-radius:10px; padding:18px; }
    button { border-radius:6px; padding:7px 11px; background-image:none;
             background-color:%(card)s; color:%(fg)s; border:1px solid %(border)s; box-shadow:none; }
    button:hover { background-color:%(hover)s; }
    button:checked, button:active { background-color:%(selected)s; color:%(selected_fg)s; }
    button:disabled { color:%(muted)s; background-color:%(bg)s; }
    button label { color:inherit; }
    .navigation { border:0; box-shadow:none; background:transparent; min-height:36px; padding:4px 12px; }
    .navigation:hover { background:%(hover)s; }
    .navigation.selected { background:%(selected)s; color:%(selected_fg)s; font-weight:700; }
    entry, spinbutton, spinbutton entry, combobox button {
        background-image:none; background-color:%(card)s; color:%(fg)s; border-color:%(border)s; }
    checkbutton check, radiobutton radio {
        background-image:none; background-color:%(card)s; color:%(fg)s;
        border:1px solid %(border)s; box-shadow:none; min-width:14px; min-height:14px; }
    checkbutton check { border-radius:3px; }
    checkbutton:hover check, radiobutton:hover radio { border-color:%(accent)s; }
    checkbutton check:checked, checkbutton check:indeterminate,
    radiobutton radio:checked, radiobutton radio:indeterminate {
        background-color:%(accent)s; border-color:%(accent)s; color:white; }
    checkbutton:disabled, radiobutton:disabled { color:%(muted)s; }
    checkbutton:disabled check, radiobutton:disabled radio { opacity:0.55; }
    switch { background:%(border)s; color:%(fg)s; border-color:%(border)s; }
    switch:checked { background:%(accent)s; }
    switch slider { background:%(card)s; border-color:%(border)s; }
    button.primary { background:%(accent)s; color:white; border-color:%(accent)s; }
    button.primary:disabled { background:%(selected)s; color:%(muted)s; border-color:%(border)s; }
    popover, menu, menuitem { background-color:%(card)s; color:%(fg)s; }
    popover, menu { border-color:%(border)s; }
    menuitem:hover, menuitem:selected { background-color:%(selected)s; color:%(selected_fg)s; }
    menuitem label { color:inherit; }
    separator { background-color:%(border)s; }
    scrollbar, scrollbar trough { background-color:%(bg)s; }
    scrollbar slider { background-color:%(border)s; }
    textview, textview text { background:%(card)s; color:%(fg)s; }
    entry selection, textview text selection { background:%(accent)s; color:white; }
    .phase { font-size:17px; font-weight:600; }
    .counter { font-size:30px; font-weight:700; }
    .battery-number { font-size:29px; font-weight:700; color:#38a96b; }
    .error { color:%(error)s; }
    .three-badge { background:%(selected)s; color:%(selected_fg)s; padding:5px 9px;
                   border-radius:5px; font-size:10px; font-weight:bold; }
    .three-problem { background:%(card)s; color:%(error)s; padding:20px; border-radius:8px; }
    window.input-overlay { background-color:transparent; background-image:none; }
    ''' % p


class Theme:
    def __init__(self, owner):
        self.owner = owner
        self.native = Gtk.Settings.get_default()
        self.provider = None
        self.connections = []
        self.system = None
        source = Gio.SettingsSchemaSource.get_default()
        schema = source.lookup('org.gnome.desktop.interface', True) if source else None
        if schema:
            self.system = Gio.Settings.new_full(schema, None, None)
            for key in ('color-scheme', 'gtk-theme'):
                if schema.has_key(key):
                    self.connections.append((self.system, self.system.connect('changed::'+key, self.system_changed)))
        self.connections.append((self.native, self.native.connect('notify::gtk-theme-name', self.system_changed)))
        owner.connect('destroy', self.close)

    def system_dark(self):
        if self.system:
            schema = self.system.props.settings_schema
            if schema.has_key('color-scheme'):
                scheme = self.system.get_string('color-scheme')
                if scheme in ('prefer-dark', 'prefer-light'):return scheme == 'prefer-dark'
            if schema.has_key('gtk-theme') and self.system.get_string('gtk-theme').lower().endswith('-dark'):
                return True
        return self.native.get_property('gtk-theme-name').lower().endswith('-dark')

    def system_changed(self, *_):
        if self.owner.get('theme', 'system') == 'system':self.apply()

    def apply(self):
        requested = self.owner.get('theme', 'system')
        self.owner.dark = requested == 'dark' or requested == 'system' and self.system_dark()
        self.native.set_property('gtk-application-prefer-dark-theme', self.owner.dark)
        provider = Gtk.CssProvider()
        provider.load_from_data(stylesheet(self.owner.dark).encode())
        screen = self.owner.get_screen() or Gdk.Screen.get_default()
        if self.provider:Gtk.StyleContext.remove_provider_for_screen(screen, self.provider)
        Gtk.StyleContext.add_provider_for_screen(screen, provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self.provider = provider
        if self.owner.built:
            self.owner.monitor_canvas.queue_draw();self.owner.history_canvas.queue_draw()
            if self.owner.overlay:self.owner.overlay_canvas.queue_draw()
        self.owner.queue_draw()

    def close(self, *_):
        for obj, signal_id in self.connections:obj.disconnect(signal_id)
        self.connections.clear()
        if self.provider:
            Gtk.StyleContext.remove_provider_for_screen(Gdk.Screen.get_default(), self.provider)
            self.provider = None
