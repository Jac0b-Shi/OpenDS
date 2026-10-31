# OpenDS

|          |                      |
| -------- | -------------------- |
| **简体中文** | [English](README.md) |

把通过蓝牙连接的 DualSense 手柄变成一只本机虚拟 USB DualSense，让游戏按有线手柄对待，并把灯光、自适应扳机、震动、扬声器音频和触觉原样送回真实手柄。

## 快速开始

1. 解压 `OpenDS-…-windows-x64.zip` 到本地磁盘，保留全部文件。需要固定安装时，用 PowerShell 运行 `scripts/Install-OpenDS.ps1`。
2. 在 Windows 蓝牙设置里配对 DualSense，退出其他占用该手柄的工具。手柄之前连过别的设备时，先进入配对模式：
   > 确认玩家指示灯已熄灭。如果亮着，长按 PS 键直到熄灭，并断开 USB 线。按住**创建键**，同时按住 **PS 键**，直到光条开始闪烁。然后在 Windows 蓝牙设置里选择手柄。
   > 步骤摘自 [PlayStation 官方配对教程](https://www.playstation.com/zh-hans-cn/support/hardware/pair-dualsense-controller-bluetooth/)。[Apple 支持文档](https://support.apple.com/zh-cn/HT211813) 对创建键的位置有更清楚的示意，配对按键相同。
3. 双击 `opends.exe`。缺少驱动时按提示安装 USB/IP Win2 和 HidHide，并重启系统。
4. 在「连接与反馈」选中手柄并保存，回到概览点击「启动桥接」。
5. 进入游戏。桥接启动后再开游戏，手柄才会被识别为完整设备。

关闭设置窗口会留在托盘。停止桥接或从托盘退出会释放按键、移除虚拟设备并恢复手柄可见。

## 指令表与悬浮窗

在「实时监视」开始采集后，手柄图形下方会记录按键、左摇杆方向与力度变化。右摇杆默认不记录，可在「指令表设置」内开启；来源标识可选文字、图标或隐藏，阈值与颜色均可调整。触摸板只记录机械按压。

当前输入状态固定在最上方并持续计时，历史行显示该状态持续到下一次变化的时长，超过 99 秒显示 `99+ s`。断开和重连会显示为插头事件。

点击「悬浮窗」可独立显示指令表。悬浮窗支持置顶、拖动缩放、背景与内容分别调透明度，以及跟随软件或固定深浅主题。锁定后鼠标穿透，在主面板或托盘菜单解锁，也可以用「恢复悬浮窗」找回窗口。关闭自动配色后，深浅主题可以分别设置颜色。

开启「记录文件」会另存一份同名 `.commands.csv`，逐条保存指令、连接事件、分类设置和原始时间。

## 命令

日常使用走图形界面。命令适合脚本化、批量操作，以及让 AI agent 代你检查状态或改配置——`doctor`、`list` 和 `startup status` 都输出纯文本，可以直接喂给模型。

```powershell
.\opends.exe              # 打开设置界面
.\opends.exe tray         # 后台运行，只显示托盘图标
.\opends.exe bridge       # 直接桥接，Ctrl+C 停止
.\opends.exe bridge 30    # 桥接 30 秒
.\opends.exe list         # 列出已配对的蓝牙手柄
.\opends.exe doctor       # 检查依赖
.\opends.exe restore      # 恢复被隐藏的手柄
```

## 依赖

从 0.3.2 起，可在「诊断与日志」中分别为 USB/IP Win2 和 HidHide 选择安装目录；选择后立即保存并重新检查，也可点击「自动查找」清除手动设置。USB/IP 目录须包含 `usbip.exe`；HidHide 目录用于定位可选的配置工具，不能代替安装驱动。修改目录前需停止桥接。

| 组件          | 用途                |
| ----------- | ----------------- |
| USB/IP Win2 | 创建虚拟 USB 设备       |
| HidHide     | 隐藏物理手柄，避免游戏看到两个设备 |

`scripts/Install-Drivers.ps1` 从两个上游仓库的 Release 下载安装包，校验 SHA-256 和发布者签名后运行：

| 驱动          | 上游仓库                                                          |
| ----------- | ------------------------------------------------------------- |
| USB/IP Win2 | [vadimgrn/usbip-win2](https://github.com/vadimgrn/usbip-win2) |
| HidHide     | [nefarius/HidHide](https://github.com/nefarius/HidHide)       |

手动下载的文件名、地址和摘要见 `scripts/drivers.json`。

```powershell
.\scripts\Install-Drivers.ps1                          # 下载
.\scripts\Install-Drivers.ps1 -Install                 # 下载并安装
.\scripts\Install-Drivers.ps1 -Proxy http://127.0.0.1:7890
.\scripts\Install-Drivers.ps1 -ManualDirectory D:\Downloads -Install
```

发布包已包含 libVIIPER 和 Opus。

## 配置

配置默认读取 `%LOCALAPPDATA%\OpenDS\opends.ini`。如果在 EXE 同目录放一份 `opends.ini`，会优先采用它，变成便携模式。

```ini
[ds5]
port=3240
buffer=64
audio=1
haptics_gain=10
```

| 键                      | 说明                                                                           |
| ---------------------- | ---------------------------------------------------------------------------- |
| `port`                 | 虚拟设备端口                                                                       |
| `buffer`               | 音频缓冲块数，16–128。每块约 10.7 ms，队列满时 64 块约 683 ms、128 块约 1.37 s。调大更抗卡顿，代价是游戏声音落后画面 |
| `audio`                | 扬声器音频开关                                                                      |
| `haptics_gain`         | 触觉强度，10–20                                                                   |
| `microphone_gain_db`   | 麦克风增益，0–50 dB，带峰值限幅                                                          |
| `microphone_buffer_ms` | 麦克风抖动缓冲，16–512 毫秒                                                            |
| `monitor_sample_hz`    | 监视采样率，20 / 50 / 100 / 200                                                    |
| `monitor_fps`          | 监视刷新率，15 / 30 / 60                                                           |
| `monitor_seconds`      | 监视时间跨度，30–900 秒                                                              |
| `language`             | 界面语言，`auto` 跟随系统，或语言代码如 `zh-CN`                                              |
| `theme`                | 外观，`system` / `light` / `dark`                                               |
| `close_action`         | 关闭行为，`ask` / `tray` / `exit`                                                 |

修改后重启桥接生效。

## 登录自启

打开「常规设置」，勾选「登录 Windows 时在托盘运行」即可。安装时也可以一起配置：

```powershell
.\scripts\Install-OpenDS.ps1 -Startup
```

需要脚本化时用命令：

```powershell
.\opends.exe startup enable
.\opends.exe startup status
.\opends.exe startup disable
```

自启走当前用户的 Run 项，不需要管理员权限。

## 常见问题

**设备管理器里找不到手柄**

物理手柄被 HidHide 隐藏了。退出 OpenDS 会自动恢复；异常退出后运行 `opends.exe restore`。

**游戏里没有震动**

先启动 OpenDS 再开游戏。游戏通常在启动时探测手柄类型和支持的能力。中途接入的手柄会被识别成输入设备，但能力探测不会重跑，震动、自适应扳机和灯效可能都不生效。

**有震动但没声音**

手柄扬声器的音量独立于系统音量，由游戏通过输出报告设置；游戏没有提供相关选项时保持上一次的值。确认配置里 `audio=1`。

**音频断续**

把 `buffer` 调大，例如 96 或 128。

**Steam 游戏识别异常**

关闭 Steam Input。它会接管手柄，绕过本工具的虚拟设备。

**自动弹出放大镜或屏幕键盘**

Steam 的导航键组合键会把 PS 键加组合键翻译成系统快捷键（放大镜、屏幕键盘、录屏等）。关掉它：Steam → 设置 → 控制器 → 高级设置 → 取消勾选「启用控制器的导航键组合键」。

如果只是偶发，也可以直接在 Windows 里关掉对应的辅助功能快捷键。

**怎么排查连接问题**

日志在 `%LOCALAPPDATA%\OpenDS\logs\opends.log`，自启和手动启动共用同一个文件，记录连接、断线、桥接错误和音频统计。单文件上限 1 MiB，保留 `opends.log.1`、`opends.log.2` 两份历史。每行开头是 Unix 秒级时间戳。

## 兼容性

支持普通 DualSense 和 DualSense Edge。Edge 默认使用 Edge 的 PID，可在配置里设 `edge_as_ds=true` 改为呈现为普通 DualSense。同时只支持一只手柄。

麦克风是实验功能，默认关闭，支持增益和抖动缓冲调整，详见[麦克风](docs/guides/microphone.md)。

「实时监视」页面可观察手柄输入与扬声器、触觉、麦克风三组波形，详见[实时监视](docs/guides/monitor.md)。

## 构建

需要 Rust MSVC 工具链、Windows SDK 和 Git。

```powershell
cargo fetch --locked
cargo test --locked --offline
.\scripts\Build-Release.ps1
```

可选 `bundled-opus` 特性用 CMake 编译静态 Opus，CMake 4 需设置 `$env:CMAKE_POLICY_VERSION_MINIMUM='3.5'`。

修改 `vendor/viiper-source` 后重新编译 DLL，需要 Go 1.27.3 和 Windows C 编译器：

```powershell
.\scripts\Build-VIIPER.ps1 -Compiler <gcc.exe路径>
```

## 发布包

每个版本提供四个附件：

| 文件                            | 内容                                          |
| ----------------------------- | ------------------------------------------- |
| `OpenDS-<版本>-windows-x64.zip` | 可执行文件、DLL 和脚本                               |
| `OpenDS-<版本>-source.zip`      | 完整源码：Rust 源码、`vendor/viiper-source`、依赖许可证清单 |
| `SHA256SUMS.txt`              | 全部附件的校验和                                    |
| `TAG-SIGNATURE.json`          | 签名标签的回执                                     |

GitHub 页面上的 "Source code (zip)" 和 "Source code (tar.gz)" 是平台自动生成的，只含 Git 跟踪的文件，缺少依赖许可证清单和 VIIPER 源码归档。**需要源码请下载 `source.zip`**。

发布流程会校验 `source.zip` 里的 VIIPER 源码与二进制包中的 DLL 是否对应。

## 许可证

GPL-3.0-or-later。完整文本见 [LICENSE](LICENSE)，第三方组件见 [THIRD-PARTY.md](THIRD-PARTY.md)。

分发二进制时需一并提供 `vendor/viiper-source`，发布脚本会自动打包。

## 文档

- [使用指南](docs/guides/user-guide.md)
- [实时监视](docs/guides/monitor.md)
- [常规设置](docs/guides/settings.md)
- [麦克风](docs/guides/microphone.md)
- [开发者指南](docs/guides/developer-guide.md)
- [协议说明](docs/guides/protocol.md)
- [常驻运行](docs/guides/resident.md)
- [更新日志](CHANGELOG.md)

## 致谢

**运行时依赖**

| 项目                                                                  | 作用                                    |
| ------------------------------------------------------------------- | ------------------------------------- |
| [VIIPER](https://github.com/DualSenseClient/VIIPER)                 | 虚拟 USB DualSense 设备（GPL-3.0-or-later） |
| [USB/IP Win2](https://github.com/vadimgrn/usbip-win2)               | 创建虚拟 USB 设备                           |
| [HidHide](https://github.com/nefarius/HidHide)                      | 隐藏物理手柄                                |
| [Opus](https://opus-codec.org/)                                     | 音频编解码（BSD-3-Clause）                   |
| [native-windows-gui](https://github.com/gabdube/native-windows-gui) | 设置界面（MIT）                             |

**设计参考**

- [input-overlay](https://github.com/univrsal/input-overlay)：DualSense 图形布局参考，预设素材为 CC0
- [3d-controller-overlay](https://github.com/larfingshnew/3d-controller-overlay)：手柄三维呈现的视觉参考（MIT）

**思路启发**

- B 站 UP 主 [zhonghuihong](https://space.bilibili.com/37635277)：他的文档帮助厘清了 DualSense 在 PC 上的工作方式和调试思路。该软件为闭源，本项目未使用其代码。

## 法律声明

DualSense、PlayStation 是索尼互动娱乐（Sony Interactive Entertainment Inc.，SIE）的注册商标。

本项目是非官方第三方工具，与 SIE 无任何关联，未获其授权、赞助或认可。文中出现的其他商标归各自所有者所有。
