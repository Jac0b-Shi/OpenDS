# OpenDS 开发者指南

## 架构

```
┌─────────────┐   HID 输入    ┌──────────────┐
│ 蓝牙手柄     │ ───────────→ │              │
│ (物理)       │ ←─────────── │   bridge     │
└─────────────┘   反馈报告     │              │
                               └──────┬───────┘
                                      │ 输入帧
                                      ▼
                               ┌──────────────┐
                               │ 虚拟 USB 设备 │ ←── 游戏
                               │ (libVIIPER)  │ ──→ 输出报告 / 音频 PCM
                               └──────────────┘
```

三条独立通路：

| 通路 | 方向 | 载体 |
| --- | --- | --- |
| 输入 | 手柄 → 虚拟设备 | HID 输入报告 64 字节 |
| 输出反馈 | 虚拟设备 → 手柄 | 输出报告（灯光/扳机/震动） |
| 音频 | 虚拟设备 → 手柄 | 4ch PCM → Opus → 蓝牙 0x36 |

---

## 模块

| 文件 | 职责 |
| --- | --- |
| `main.rs` | 命令行入口、桥接主循环 |
| `bridge.rs` | 会话编排：创建虚拟设备、启动各线程、处理设备切换 |
| `hid.rs` | HID 设备枚举与读写（`CreateFile` / `ReadFile` / `WriteFile`） |
| `viiper.rs` | libVIIPER 的 FFI 绑定（设备创建、输出回调、音频回调） |
| `hidhide.rs` | HidHide 驱动 IOCTL（黑名单、白名单、总开关、崩溃自愈） |
| `report.rs` | 报告构造与 CRC32 |
| `vendor/viiper-source/device/dualsense/opends_usb_profile.go` | 普通 DualSense 的真机 USB 描述符配置 |
| `output.rs` | 输出状态适配、扳机模式、振动模式 |
| `audio.rs` | PCM 处理：下混、重采样、触觉抽取、报告组装 |
| `audio_thread.rs` | 音频编码线程与环形缓冲 |
| `opus.rs` | libopus 运行时绑定 |
| `config.rs` | `opends.ini` 解析 |
| `logging.rs` | 诊断日志 |
| `startup.rs` | 当前用户的 Windows 登录自启设置 |

---

## 线程模型

```
界面主线程
  └─ 处理窗口和托盘事件，定时读取桥接统计

桥接线程
  └─ 异步读取手柄输入（20ms 超时），收到报告后立即写虚拟设备

输出回调（libVIIPER goroutine）
  └─ 只入队，不做重活

输出线程（512 帧 / 10.67ms 节拍）
  └─ 取 512 帧 → 下混扬声器与独立左右触觉 → 扬声器重采样至 480 帧并编码 Opus → 组装 0x36 → 写手柄
```

**约束**：回调运行在 USB 通路上，**绝不能阻塞**。所有回调只做拷贝或入队，抢不到锁就丢弃该帧。

缓冲上限默认 4 块（每块 512 帧，约 43ms），可通过 `opends.ini` 的 `buffer` 调整。溢出时丢最老的块，保证延迟有界。

---

## 构建

```powershell
cargo build --release
```

依赖：

| 依赖 | 形式 | 许可证 |
| --- | --- | --- |
| libopus | `vendor/opus/libopus-0.dll` | BSD |
| libVIIPER | `vendor/viiper/libVIIPER.dll` | GPL-3.0 |
| VIIPER 源码 | `vendor/viiper-source/` | GPL-3.0 |

**注意**：`vendor/viiper-source/` 是 `libVIIPER.dll` 的对应源码，GPLv3 要求分发二进制时一并提供。构建脚本 `scripts/Build-Release.ps1` 会自动打包。

无网络环境可用 `cargo build --offline`。

---

## 测试

```powershell
cargo test --offline
```

单元测试覆盖：报告布局、CRC、重采样、触觉抽取、下混、配置钳制。

真机验证脚本（需连接手柄）：

| 脚本 | 用途 |
| --- | --- |
| `scripts/ds5_probe.py` | 枚举 HID 设备，检查报告长度 |
| `scripts/ds5_output_test.py` | 发送灯光/振动/扳机测试报告 |
| `scripts/verify_feedback.py` | 端到端验证反馈通路（加速度计 RMS 变化） |
| `scripts/verify_audio.py` | 验证音频通路 |
| `scripts/analyze_usb_identity.py` | 对比设备身份字段 |

---

## 调试

**看桥接统计**

```
已读 N 帧，转发 N 帧，输出 N 帧，音频回调 N 帧 → 编码 N 块，失败 N，队列 N 块
```

- `输出 N 帧` 不涨：游戏没写输出报告，或回调未注册
- `音频回调 N 帧` 不涨：游戏没打开音频接口
- `队列 N 块` 持续增长：消费速度跟不上，需要减小缓冲或检查编码耗时

**恢复手柄可见**

```powershell
opends.exe restore
```

**日志**

`logging.rs` 会在启动时做自检，异常写入 `%LOCALAPPDATA%\OpenDS\`。

---

## 扩展点

| 需求 | 位置 |
| --- | --- |
| DualSense Edge 适配 | `bridge.rs` 的 PID 选择、`vendor/viiper-source/device/dualsense/` 的 Edge 配置；仍需 Edge 真机验收 |
| 支持 DS4 | 新增 `ds4.rs`，复用 `hid.rs` 与 `hidhide.rs` |
| 自定义报告 | `report.rs` 的构造器 |
| 音频质量调整 | `audio.rs` 的 `OpusEncoder` 实现与码率常量 |

---

## 协议要点

参见 [`protocol.md`](protocol.md)：USB 描述符、HID 报告格式、蓝牙音频报告与 CRC。
