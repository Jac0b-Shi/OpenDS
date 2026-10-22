# Client Libraries

Besides embedding the full stack with [libVIIPER](../libviiper/overview.md), VIIPER ships a
standalone **server** and generated **TCP client libraries** for several languages.
This is the right choice when the device logic lives in a different process than
the USBIP server, when you want multiple feeders on one machine, or when your
language does not have convenient C FFI bindings.

## Architecture

```text
Feeder application (C#, C++, Rust, TS, Go, ...)
        |
        | TCP / JSON + binary device streams (port 3242)
        v
viiper server (USB-IP on port 3241)
        |
        | USBIP
        v
usbip-win2 / vhci-hcd virtual host controller
        |
        v
OS, games, audio services
```

The `viiper` binary acts as a TCP server. Management commands (list/create/remove
buses and devices) are plain-text JSON requests; device input/output streams are
binary. All traffic is **authenticated and encrypted** by default (PBKDF2-SHA256
key derivation + ChaCha20-Poly1305 session encryption), so the client libraries
handle the handshake for you when you pass a password.

## Running the server

Start the server with the `server` subcommand:

```bash
viiper server
```

On first run the server generates a random API password and stores it in a key
file. On Windows this is `%APPDATA%\VIIPER\viiper.key.txt`; on Linux it is
`~/.config/github.com/DualSenseClient/viiper/viiper.key.txt` (or `/etc/viiper/`
for root). The password is also printed to the console on first start.

Key configuration options (see `viiper server --help` for all of them):

| Flag / env | Default | Description |
| --- | --- | --- |
| `--usb.addr` / `VIIPER_USB_ADDR` | `:3241` | USB-IP listen address |
| `--api.addr` / `VIIPER_API_ADDR` | `:3242` | TCP API listen address |
| `--api.auto-attach-local-client` / `VIIPER_API_AUTO_ATTACH_LOCAL_CLIENT` | `true` | Auto-attach created devices to the local USBIP client |
| `--api.require-localhost-auth` / `VIIPER_API_REQUIRE_LOCALHOST_AUTH` | `false` | Require the password even for localhost connections |

!!! info "USBIP required"
    The server still needs a USBIP client/driver on the machine that should see
    the virtual devices, exactly like [libVIIPER](../libviiper/overview.md). See
    [Installation › Requirements](../getting-started/installation.md#requirements).

### API overview

| Endpoint | Description |
| --- | --- |
| `ping` | Returns server + version |
| `bus/list` | List all buses |
| `bus/create [id]` | Create a bus (auto-assign ID if omitted) |
| `bus/remove <id>` | Remove a bus and all its devices |
| `bus/{id}/list` | List devices on a bus |
| `bus/{id}/add <json>` | Add a device (`{"type":"xbox360","deviceSpecific":{...}}`) |
| `bus/{id}/remove <devId>` | Remove a device |
| `bus/{id}/meta <devId> <meta-json>` | Merge-update a device's metadata (returns updated entry) |
| `bus/{busId}/{deviceid}` | Open the binary device stream |
| `bus/{busId}/{deviceid}/audio` | Open the speaker PCM stream (u16 LE length + PCM; `0xFFFF` = reset barrier). DualSense: 4ch S16LE @48kHz up to 392B; DualShock 4: 2ch S16LE @32kHz up to 132B. Fails for devices without audio. Streams end on disconnect; removing the device does not close open feeder streams |
| `bus/{busId}/{deviceid}/audio/haptics` | Open the DualSense rear-haptics PCM stream (2ch S16LE @48kHz, same framing, barriers included) |
| `bus/{busId}/{deviceid}/audio/mic` | Write raw mic frames: 192B DualSense frames (2ch S16LE @48kHz) or 32B DualShock 4 frames (mono S16LE @16kHz) |

## C# — `Viiper.Client`

The generated C# client library is published as a NuGet package (`Viiper.Client`)
and its source lives in [`clients/csharp/`](https://github.com/DualSenseClient/VIIPER/tree/main/clients/csharp).

```csharp
using System.Linq;
using Viiper.Client;
using Viiper.Client.Devices.Xbox360;
using Viiper.Client.Types;

var client = new ViiperClient("localhost", 3242, "your-password");

// Find or create a bus
var list = await client.BusListAsync();
uint busId;
bool createdBus = false;
if (list.Buses.Length == 0)
{
    busId = (await client.BusCreateAsync(null)).BusID;
    createdBus = true;
}
else
{
    busId = list.Buses.Min();
}

// Add a device and open its stream
var devInfo = await client.BusDeviceAddAsync(busId, new DeviceCreateRequest { Type = "xbox360" });
var device = await client.ConnectDeviceAsync(devInfo.BusID, devInfo.DevID);

// React to host output (rumble)
device.OnOutput = async stream =>
{
    var buf = new byte[Xbox360.OutputSize];
    await stream.ReadAsync(buf, 0, buf.Length);
    Console.WriteLine($"<- Rumble: Left={buf[0]}, Right={buf[1]}");
};

// Send input at ~60 Hz
var timer = new PeriodicTimer(TimeSpan.FromMilliseconds(16));
while (await timer.WaitForNextTickAsync())
{
    var state = new Xbox360Input
    {
        Buttons = (uint)Button.A,
        Lt = 128,
        Rt = 0,
        Lx = 20000,
        Ly = 0,
        Rx = 0,
        Ry = 0,
    };
    await device.SendAsync(state);
}

// Cleanup
await client.BusDeviceRemoveAsync(devInfo.BusID, devInfo.DevID);
if (createdBus) await client.BusRemoveAsync(busId);
```

Device-specific input types, output sizes and constants are under
`Viiper.Client.Devices.*` (e.g. `Xbox360Input`, `Dualshock4Input`, `DualsenseInput`,
`KeyboardInput`, `MouseInput`, `Ns2proInput` and their output helpers).

See [`examples/csharp/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/csharp)
for complete programs (virtual keyboard, mouse, Xbox 360 pad and DualShock 4 pad).

## C++

The C++ client is a header-only library requiring C++20, `nlohmann/json` and
OpenSSL. Source: [`clients/cpp/`](https://github.com/DualSenseClient/VIIPER/tree/main/clients/cpp).

```cpp
#include <viiper/viiper.hpp>
#include <thread>

int main(int argc, char** argv) {
    viiper::ViiperClient client("localhost", 3242, "your-password");

    auto buses_result = client.buslist();
    uint32_t bus_id = buses_result.value().buses[0];

    auto device_result = client.busdeviceadd(bus_id, {.type = "keyboard"});
    auto stream = client.connectDevice(device_result.value().busid,
                                       device_result.value().devid).value();

    stream->on_output(viiper::keyboard::OUTPUT_SIZE, [](const uint8_t* data, std::size_t len) {
        printf("<- LEDs: 0x%02X\n", data[0]);
    });

    viiper::keyboard::Input down = { .modifiers = 0, .keys = {viiper::keyboard::KeyEnter} };
    stream->send(down);
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    stream->send(viiper::keyboard::Input{.modifiers = 0, .keys = {}});
}
```

See [`examples/cpp/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/cpp)
for the CMake setup and full examples.

## Rust

The Rust client crate is `viiper-client` and supports both sync and async (Tokio)
usage. Source: [`clients/rust/`](https://github.com/DualSenseClient/VIIPER/tree/main/clients/rust).

```rust
use viiper_client::AsyncViiperClient;
use viiper_client::devices::keyboard::*;

#[tokio::main]
async fn main() {
    let client = AsyncViiperClient::new("localhost:3242".parse().unwrap());

    let resp = client.bus_list().await.unwrap();
    let bus_id = resp.buses[0];

    let device_info = client
        .bus_device_add(bus_id, &viiper_client::types::DeviceCreateRequest {
            r#type: Some("keyboard".to_string()),
            id_vendor: None,
            id_product: None,
            device_specific: None,
        })
        .await
        .unwrap();

    let mut stream = client
        .connect_device(device_info.bus_id, &device_info.dev_id)
        .await
        .unwrap();

    stream.send(&KeyboardInput {
        modifiers: 0,
        count: 1,
        keys: vec![KEY_ENTER],
    }).await.unwrap();
}
```

See [`examples/rust/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/rust)
for the complete sync and async examples.

## TypeScript

The TypeScript client is published as the `viiperclient` npm package. Source:
[`clients/typescript/`](https://github.com/DualSenseClient/VIIPER/tree/main/clients/typescript).

```typescript
import { ViiperClient, Keyboard } from "viiperclient";

const client = new ViiperClient("localhost", 3242, "your-password");

const buses = await client.buslist();
const busID = buses.buses[0];

const { device, response } = await client.addDeviceAndConnect(busID, { type: "keyboard" });

device.on("output", (buf: Buffer) => {
    console.log("<- LEDs: 0x" + buf.readUInt8(0).toString(16));
});

const down = new Keyboard.KeyboardInput({ Modifiers: 0, Count: 1, Keys: [Keyboard.Key.Enter] });
await device.send(down);
```

See [`examples/typescript/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/typescript).

## Go

The Go client library lives in the repository itself (`viiperclient` package) and
reuses the device packages directly. Examples:
[`examples/go/`](https://github.com/DualSenseClient/VIIPER/tree/main/examples/go).

```go
import (
    "context"
    "github.com/DualSenseClient/VIIPER/viiperclient"
    "github.com/DualSenseClient/VIIPER/device/xbox360"
)

api := viiperclient.NewWithPassword("localhost:3242", "your-password")

stream, _, err := api.AddDeviceAndConnect(context.Background(), busID, "xbox360", nil)
if err != nil { /* ... */ }
defer stream.Close()

_ = stream.WriteBinary(&xbox360.InputState{
    Buttons: xbox360.ButtonA,
    LT:      128,
    LX:      20000,
})
```

## Choosing between libVIIPER and the TCP clients

| | libVIIPER | Client libraries |
| --- | --- | --- |
| Process model | In-process (embedded) | Separate `viiper server` process |
| API style | Pure C (P/Invoke / FFI) | Native, generated per language |
| Latency | Lowest possible | Very low (loopback TCP) |
| Multiple feeders | One per process | Many clients, one server |
| Remote usage | No | Yes (over network, encrypted) |
| License | GPL-3.0 | MIT (client libraries) |
