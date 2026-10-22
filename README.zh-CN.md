# OpenDS

**简体中文** | [English](README.md)

Windows 上的轻量 DualSense 蓝牙桥接工具。将真实蓝牙手柄映射为本机虚拟 USB DualSense，并回传游戏输出、声音和原生触觉。设置界面采用 NWG 原生控件与 GDI 绘制，不使用 WebView。

## 开始使用

1. 将 `OpenDS-…-windows-x64.zip` **完整解压到本机磁盘**，保留 `opends.exe`、两个 DLL 和 `scripts` 目录。需要固定安装时，右键使用 PowerShell 运行 `scripts/Install-OpenDS.ps1`，它会安装到当前用户目录并创建开始菜单快捷方式。
2. 在 Windows 蓝牙设置中配对 DualSense，关闭其他占用同一手柄的桥接工具。
3. 双击 `opends.exe`。缺少依赖会进入“诊断与日志”，按需打开官方 USB/IP Win2 和 HidHide 安装向导，遵循安装器提示重启 Windows。驱动安装需要管理员权限；日常运行使用普通权限。
4. 在“连接与反馈”选择手柄并保存，回到概览点击“启动桥接”。修改设置在下一次启动桥接时生效。
5. 在游戏中启用 DualSense 原生支持；如果 Steam Input 接管手柄，原生音频/触觉可能不会送到虚拟设备。程序转发游戏实际产生的反馈，不能给不支持的游戏自动生成原生触觉。

关闭设置窗口会留在托盘运行；停止桥接或从托盘退出会释放按键、关闭反馈、移除虚拟设备并恢复本次新增的隐藏状态。

## 依赖下载与离线安装

安装按钮使用 `scripts/Install-Drivers.ps1`：只从固定官方发布地址下载，先校验 SHA-256 和 Authenticode 发布者，再运行安装向导。USB/IP 选择 compact 安装，避免额外安装 GUI、SDK 和调试符号。

下载优先使用 `HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY` 或 Windows 系统代理；**没有检测到代理就停止并显示手动下载地址**。支持显式指定代理：

```powershell
.\scripts\Install-Drivers.ps1 -Proxy http://127.0.0.1:7890
.\scripts\Install-Drivers.ps1 -ManualDirectory D:\Downloads -Install
```

第一条只下载和校验，不安装。手动下载的文件名、URL 和摘要见 `scripts/drivers.json`。安装失败或取消不会继续启动桥接；完成后重新打开 OpenDS 再检查。发布包已带 libVIIPER 和 Opus，不需要另外搜索 DLL。

## 设置与恢复

默认配置在 `%LOCALAPPDATA%\OpenDS\opends.ini`。如需便携配置，在 EXE 同目录放置 `opends.ini`。路径不依赖终端当前目录。“打开 OpenDS 时自动开始桥接”和“登录 Windows 时在托盘运行”可分别设置。登录自启使用当前用户 Run 项，无需管理员权限；后台模式直接等待蓝牙手柄，不弹主窗口。先安装到固定目录，再开启自启，避免启动项指向临时解压目录。

安装时可使用 `scripts/Install-OpenDS.ps1 -Startup`；也可在固定安装目录运行 `opends.exe startup enable`、`startup disable` 或 `startup status`。直接运行 `opends.exe tray` 可手动进入相同后台模式。

HidHide 页面支持全局开关及程序白名单，程序路径自动转换为驱动要求的 NT 卷路径。桥接运行时禁止修改本工具管理的白名单。仅隐藏本次连接的物理手柄；正常结束只撤销本次新增的条目，保留用户其他配置。

异常退出后，下次打开会自动恢复。需要手动恢复时先退出所有 OpenDS，再运行：

```powershell
.\opends.exe restore
```

或运行 `scripts/Restore-Visibility.ps1`。恢复日志位于 `%LOCALAPPDATA%\OpenDS\hidhide-state.txt`；驱动失败或回读不一致会保留日志供重试，**请勿手动删除它**。强制结束进程/断电无法执行即时恢复，需重新运行程序。

## 反馈与音频

- 兼容输入与完整输入分别映射；完整报告验证 CRC；断线释放按键，支持停止和重连。
- 转发灯光、左右扳机、传统震动；原生触觉保留左右独立通道，音频暂停后仍可更新扳机和灯光。
- 扬声器音频与原生触觉可分别关闭；路由可自动跟随游戏/耳机插入状态，也可指定扬声器或耳机。不会修改 Windows 默认音频设备。
- 麦克风是**实验功能，默认关闭**。开启后解码蓝牙 Opus、转为虚拟 USB 麦克风帧。报告先分流，避免音频包污染摇杆输入。该蓝牙采集状态可能持续到连接结束；关闭设置后如要完全停止物理采集，请停止桥接并重新连接手柄。
- 普通 DualSense 保留原身份；Edge 默认使用 Edge PID，可在配置中设置 `edge_as_ds=true` 以呈现普通 DualSense。当前仅支持一个手柄。
- DSX UDP、自动启动 DSX、Xbox/DS4 外观未实现；启用相应旧配置会明确报错。

## 验证范围

当前软件测试覆盖报告映射/CRC、左右触觉、队列边界/重置、输出线程预热与暂停、真实 Windows 异步 I/O 取消、恢复日志保留、原子配置写入和多实例互斥。程序嵌入 Per-Monitor V2 清单，按显示器 DPI 重建字体及布局，计算测试覆盖 100% / 150% / 200% / 250%。

真机已验证虚拟 USB 的陀螺仪数据、灯光、自适应扳机、传统震动、原生触觉左右通道，以及触觉 PCM 回放与手柄扬声器出声。麦克风录音、耳机路由、自适应扳机全模式、跨显示器缩放仍需真机验收。设备身份字段已按真实 USB 形态对齐，并经 Windows HID API 读回校验。

Windows 会按不同设备实例保留音频端点及声道设置。请在启动原神前连接桥接，并确认该端点为四声道、全部全音域；不要在每次运行时更改 USB 身份。`scripts/Check-AudioIdentity.ps1 -OutputPath baseline.json` 可保存只读快照，重连后用 `-CompareTo baseline.json` 检查端点和设置是否保持。该工具不修改默认音频输出。旧设备实例的清理应在结束游戏后进行；仅改显示名不能合并设备实例。

## 命令与构建

```powershell
.\opends.exe doctor          # 只读依赖诊断
.\opends.exe list            # 枚举蓝牙手柄
.\opends.exe bridge          # 持续运行，Ctrl+C 停止
.\opends.exe bridge 30       # 最多 30 秒
cargo test --locked --offline
.\scripts\Build-Release.ps1
```

构建需要 Rust MSVC 工具链、Windows SDK（含资源编译器）、Git。首次获取 Rust 依赖须先设置可用代理，再运行 `cargo fetch --locked`。默认构建使用随包的动态 Opus。可选 `bundled-opus` 后端使用 CMake 编译静态 Opus，CMake 4 需设置 `$env:CMAKE_POLICY_VERSION_MINIMUM='3.5'`；两种后端都应用复杂度设置。

配套 VIIPER 的修改源码在 `vendor/viiper-source`。重编 DLL 另需 Go 1.27.1 和 Windows C 编译器，使用 `scripts/Build-VIIPER.ps1 -Compiler <gcc.exe路径> -Proxy <代理地址>`；依赖已经缓存时可改用 `-Offline`。随后运行发布脚本，确保新 DLL 与 OpenDS 一同打包。构建工具仅用于开发，不是运行时依赖。

此版本为本地测试发布，OpenDS EXE 未做 Authenticode 签名。驱动安装器使用上游官方签名。许可证为 GPL-3.0-or-later；完整许可证、第三方声明与配套源码归档随发布产物提供。没有自动更新或遥测。

发布 EXE 静态链接 VC 运行时，不额外要求安装 VC++ Redistributable。更新版本前请从托盘退出旧 OpenDS，再启动新版。

## 许可证

本项目采用 **GNU General Public License 第 3 版或任何更新版本**
（SPDX：`GPL-3.0-or-later`）。完整条款见 [LICENSE](LICENSE)。

本项目采用 `GPL-3.0-or-later`，与随包分发的 VIIPER 保持一致：
下游可以按 GPL 第 3 版或任何更新版本使用本项目。

第三方组件与各自许可证见 [THIRD-PARTY.md](THIRD-PARTY.md) 与 `licenses/`。
分发二进制时须一并提供 `vendor/viiper-source`（GPL-3.0 对应源码）；发布脚本已自动打包。
