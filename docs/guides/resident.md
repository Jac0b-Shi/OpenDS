# 登录自启与常驻运行

## 开启自启

先安装到固定目录，再开启登录自启：

```powershell
.\scripts\Install-OpenDS.ps1 -Startup
```

默认安装到 `%LOCALAPPDATA%\Programs\OpenDS`，已有配置会保留。登录 Windows 后进入托盘模式并自动等待蓝牙手柄。系统可能延迟启动项，进游戏前确认桥接已连接。

设置界面的「登录 Windows 时在托盘运行」可单独开关，与「打开 OpenDS 时自动开始桥接」互不影响。也可以在安装目录直接操作：

```powershell
.\opends.exe startup enable
.\opends.exe startup status
.\opends.exe startup disable
.\opends.exe tray
```

`tray` 不显示主窗口，重复启动会复用已有进程。点击托盘图标打开设置，从托盘退出会停止桥接并恢复手柄可见。

## 日志

诊断日志在 `%LOCALAPPDATA%\OpenDS\logs\opends.log`。

登录自启和手动启动写同一个文件，没有单独的自启日志。

记录内容包括：连接与断线、桥接错误、麦克风与音频统计、诊断计数（完整输入、游戏反馈、马达状态、写失败次数）。

- 单文件上限 1 MiB，写满后轮转
- 保留 `opends.log.1` 和 `opends.log.2` 两份历史
- 每行开头的数字是 Unix 秒级时间戳

转换时间戳：

```powershell
[DateTimeOffset]::FromUnixTimeSeconds(1792723950).LocalDateTime
```

## 长期运行观测

在固定安装目录运行：

```powershell
.\scripts\Measure-Resident.ps1 -Hours 168 -IntervalSeconds 60
```

采样期间不影响桥接状态。报告默认写入 `%LOCALAPPDATA%\OpenDS\measurements`，每分钟记录 CPU、私有内存、工作集、句柄和线程数，并保留实际观察时长。

判断内存是否泄漏，要比较相同连接和游戏状态下的趋势，单次工作集波动说明不了问题。

## 更新

先从托盘退出旧版本，再运行安装脚本。配置保存在 `%LOCALAPPDATA%\OpenDS\opends.ini`，升级不会覆盖。

Windows 会按设备实例分别保存音频端点设置，升级后这些设置仍然保留。
