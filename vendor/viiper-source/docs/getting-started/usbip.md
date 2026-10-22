# USBIP

=== "Windows"

    VIIPER requires signed
    [usbip-win2 0.9.8.1](https://github.com/vadimgrn/usbip-win2/releases/tag/v.0.9.8.1).
    Do not substitute a different userspace/driver build.

    !!! warning "USBIP-Win2 signing certificate"
        The installer may add the publicly available USBIP test-signing
        CA as a trusted root. After installation, you may remove the certificate
        named **USBIP** with `certlm.msc` (run as administrator). Keep the
        driver and userspace files from the same release installed together.

=== "Linux"

    ### Arch Linux

    ```bash
    sudo pacman -S usbip
    ```

    [Arch Wiki: USBIP](https://wiki.archlinux.org/title/USB/IP)

    ??? tip "Steam OS users"
        If you are installing VIIPER on Steam OS, switch to desktop mode and
        enable write access to the root filesystem first:

        ```bash
        sudo steamos-readonly disable
        ```

    ### Ubuntu/Debian

    ```bash
    sudo apt install linux-tools-generic
    ```

    [Ubuntu USBIP Manual](https://manpages.ubuntu.com/manpages/noble/man8/usbip.8.html)

    ### Linux kernel module setup

    !!! info "USBIP client requirement"
        USBIP requires the `vhci-hcd` (Virtual Host Controller Interface)
        kernel module on Linux. Most distributions include it but do not load
        it automatically.

    #### One-time setup

    ```bash
    echo "vhci-hcd" | sudo tee /etc/modules-load.d/vhci-hcd.conf
    sudo modprobe vhci-hcd
    ```

    #### Manual loading

    ```bash
    sudo modprobe vhci-hcd
    ```

    #### Verification

    ```bash
    lsmod | grep vhci_hcd
    ```
