#!/usr/bin/env bash
# One-time system setup. The app itself runs as the desktop user.
set -euo pipefail
if [[ $EUID -ne 0 ]]; then
    exec pkexec /bin/bash "$(realpath -- "${BASH_SOURCE[0]}")" "$@"
fi
packages=()
/usr/bin/python3 -c 'import gi; gi.require_version("Gtk", "3.0"); from gi.repository import Gtk; import cairo; import gi._gi_cairo' 2>/dev/null || packages+=(python3-gi python3-gi-cairo gir1.2-gtk-3.0)
/usr/bin/python3 -c 'import gi; gi.require_version("AyatanaAppIndicator3", "0.1")' 2>/dev/null || packages+=(gir1.2-ayatanaappindicator3-0.1)
command -v parec >/dev/null && command -v paplay >/dev/null && command -v pactl >/dev/null || packages+=(pulseaudio-utils)
ldconfig -p | awk '/libopus.so.0/ && /x86-64/ {found=1} END {exit !found}' || packages+=(libopus0)
if ! command -v usbip >/dev/null || ! usbip version >/dev/null 2>&1; then
    packages+=(linux-tools-generic "linux-tools-$(uname -r)")
fi
if ! modinfo vhci_hcd >/dev/null 2>&1; then
    packages+=("linux-modules-extra-$(uname -r)")
fi
if ((${#packages[@]})); then
    apt-get update
    apt-get install -y "${packages[@]}"
fi
modprobe vhci_hcd
install -d /etc/modules-load.d /etc/udev/rules.d
printf 'vhci_hcd\n' > /etc/modules-load.d/opends.conf
cat > /etc/udev/rules.d/70-opends.rules <<'RULES'
# Physical Bluetooth DualSense and Edge only. USB virtual devices are excluded.
SUBSYSTEM=="hidraw", KERNELS=="0005:054C:0CE6.*", TAG+="uaccess"
SUBSYSTEM=="hidraw", KERNELS=="0005:054C:0DF2.*", TAG+="uaccess"
SUBSYSTEM=="input", KERNEL=="event*", KERNELS=="0005:054C:0CE6.*", TAG+="uaccess"
SUBSYSTEM=="input", KERNEL=="event*", KERNELS=="0005:054C:0DF2.*", TAG+="uaccess"
RULES
udevadm control --reload-rules
udevadm trigger --subsystem-match=hidraw
udevadm trigger --subsystem-match=input
udevadm settle
echo 'OpenDS Ubuntu setup complete. Reconnect your Bluetooth DualSense, then run ./opends doctor.'
