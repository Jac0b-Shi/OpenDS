# Installation

VIIPER comes in two flavors sharing the same USB/USBIP stack:

- **VIIPER TCP Server** — a portable standalone executable (`viiper.exe` /
  `viiper`) exposing a TCP management API plus binary device streams.
- **libVIIPER** — a shared library (`libVIIPER.dll` / `libVIIPER.so`) that
  embeds the full stack in-process via a pure C API.

Both flavors rely on USBIP, so install a USBIP client first.

!!! warning "License"
    Linking against libVIIPER requires your application to be licensed under the **GPL-3.0**
    (or a compatible license). Prefer MIT-licensed TCP client libraries in
    [`clients/`](https://github.com/DualSenseClient/VIIPER/tree/main/clients)
    with the standalone server to avoid GPL obligations.

## Requirements

### USBIP

=== "Windows"

    VIIPER requires the signed
    [usbip-win2 0.9.8.1 x64 release](https://github.com/vadimgrn/usbip-win2/releases/tag/v.0.9.8.1).
    Install that release before starting `viiper server` or an application that
    embeds libVIIPER.

    !!! danger "Do not substitute another USBIP build"
        VIIPER targets **0.9.8.1**. The installer verifies the version before
        allowing VIIPER to start.

    !!! warning "USBIP-Win2 signing certificate"
        The upstream installer may add the publicly available USBIP
        test-signing CA to **Trusted Root Certification Authorities**. After
        installation, you may remove the certificate named **USBIP** with
        `certlm.msc` (run as administrator). Keep the driver and userspace
        files from the same release.

=== "Linux"

    #### Ubuntu/Debian

    ```bash
    sudo apt install linux-tools-generic
    ```

    [Ubuntu USBIP Manual](https://manpages.ubuntu.com/manpages/noble/man8/usbip.8.html)

    #### Arch Linux

    ```bash
    sudo pacman -S usbip
    ```

    [Arch Wiki: USBIP](https://wiki.archlinux.org/title/USB/IP)

    ### Linux Kernel Module Setup

    !!! info "USBIP Client Requirement"
        USBIP requires the `vhci-hcd` (Virtual Host Controller Interface) kernel module on Linux for client operations. This includes auto-attach and manual device attachment for both flavors.

    Most Linux distributions include this module but do not load it automatically.

    See the [USBIP Setup guide](usbip.md) for detailed instructions.

## VIIPER TCP Server

A self-contained portable executable. It hosts the USBIP server on `:3241`
and the TCP management API on `:3242` (`--usb.addr`, `--api.addr`).

### Pre-built binaries

Download `viiper-windows-<arch>.zip` / `viiper-linux-<arch>.tar.gz` from the
[DualSenseClient/VIIPER Releases](https://github.com/DualSenseClient/VIIPER/releases) page.

### Automated install scripts

=== "Windows"

    ```powershell
    irm https://dualsenseclient.github.io/VIIPER/stable/install.ps1 | iex
    ```

    Installs to `%LOCALAPPDATA%\VIIPER\viiper.exe`, installs usbip-win2
    `0.9.8.1` (admin prompt, reboot may be required), registers autorun via
    `viiper install`, then starts `viiper server`.

=== "Linux"

    ```bash
    curl -fsSL https://dualsenseclient.github.io/VIIPER/stable/install.sh | sh
    ```

    Installs to `/usr/local/bin/viiper`, installs USBIP via the distro package
    manager when available, loads `vhci_hcd`, and configures a systemd service.

### Running

```bash
viiper server
viiper server --usb.addr :3241 --api.addr :3242 --api.auto-attach-local-client=true
```

On first run the server generates an API password at
`%APPDATA%\VIIPER\viiper.key.txt` (Windows) or
`~/.config/github.com/DualSenseClient/viiper/viiper.key.txt` (Linux user,
`/etc/viiper/` for root/systemd) and prints it. Localhost clients skip auth
by default (`--api.require-localhost-auth=false`).

Other commands: `viiper proxy` (USBIP traffic proxy, `--proxy-addr`,
`--proxy-upstream`), `viiper config init server|proxy`, `viiper codegen`.

See [Client Libraries](../server/client-libraries.md) and the Go examples in
[`examples/go/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/go)
for driving the server over TCP.

## Getting libVIIPER

### Pre-built Binaries

Download the latest `libVIIPER` release artifact from the [DualSenseClient/VIIPER Releases](https://github.com/DualSenseClient/VIIPER/releases) page.
The archive contains:

- `libVIIPER.dll` / `libVIIPER.so`: the shared library
- `libVIIPER.h`: the C header
- `libVIIPER.def`: Windows import definition (for generating `.lib`/`.dll.a` import libraries)

### Building from Source

```bash
git clone https://github.com/DualSenseClient/VIIPER.git
cd VIIPER
just build-libVIIPER
```

The output will be in `dist/libVIIPER/`.

!!! info "CGO Required"
    Building libVIIPER requires CGO (`CGO_ENABLED=1`) and a C compiler (GCC / MSVC / Clang) in `PATH`.
    On Windows, [mingw-w64](https://www.mingw-w64.org/) or MSVC is required.

See [libVIIPER documentation](../libviiper/overview.md) for integration guides and examples.
