meta-language-code = zh-CN
meta-language-name = 简体中文
meta-schema-version = 1
shortcut-desktop = 创建桌面快捷方式
battery-label = 手柄电量
battery-preview-title = OpenDS · 电量预览（模拟数据）
battery-preview-hint = 外观预览 · 模拟数据
battery-disconnected = 手柄未连接
battery-awaiting = 等待电量数据
battery-charging = 正在充电
battery-full = 已充满
battery-fault = 充电异常
battery-unknown = 电量不可用
battery-expired = 电量数据已过期
battery-discharging = 使用电池
battery-tooltip-level = 估算电量：{ $percent }%。手柄按约 10% 档位上报电量。
battery-tooltip-charging = 正在充电 · 估算电量：{ $percent }%。手柄按约 10% 档位上报电量。
battery-tooltip-fault = 手柄报告充电异常（代码 { $code }）。
gui-diagnostics-failed = 无法启动后台诊断：{ $error }
gui-diagnostics-worker-failed = 后台诊断失败，请刷新后重试。
shortcut-start-menu = 创建开始菜单快捷方式
shortcut-created = 已创建快捷方式：{ $path }
shortcut-exists = 此位置已有文件，未覆盖：{ $path }
shortcut-failed = 无法创建快捷方式：{ $error }
bridge-connection-diagnostic = 连接诊断
bridge-connection-resource-busy = 本次读取真实手柄特性时，Windows 返回过 170（请求的资源正在使用），随后连接失败。
    最终错误：{ $error }
    可能有其他程序同时访问物理手柄或改变其协议状态；这还不能确定具体是哪一个进程。
    请退出游戏和其他手柄工具，按 PS 键重连，先建立稳定的 OpenDS 桥接，再重新打开游戏。
bridge-connection-lost-immediately = 桥接连接后 { $milliseconds } 毫秒内，蓝牙手柄断开。
    原始错误：{ $error }
    可能原因包括其他程序访问物理手柄、蓝牙链路或供电异常、手柄输出冲突；仅凭此错误无法确定是哪一种。
    请先退出游戏和其他手柄工具，按 PS 键重连手柄，先启动 OpenDS 并等待连接稳定，再打开游戏。部分游戏只在启动时识别完整反馈能力。
bridge-connection-lost = 蓝牙连接在持续 { $seconds } 秒后中断。
    原始错误：{ $error }
    请检查手柄电量、蓝牙信号和其他手柄工具，按 PS 键重新连接。如果只在启动游戏时发生，请先退出游戏，建立桥接后再启动游戏。
bridge-connection-setup-failed = 手柄桥接初始化失败。
    原始错误：{ $error }
    请检查蓝牙连接、驱动诊断和 HidHide 访问权限，退出游戏及其他手柄工具后重试。上面的原始错误是当前可用依据，程序占用仅是一种可能原因。
gui-connection-interrupted = ● 蓝牙连接中断 · 等待重新连接
gui-connection-stopped = ● 桥接已停止 · 请查看断线原因
gui-connection-retrying = ● 桥接初始化失败 · 等待重试
gui-connection-confirming = ● 已重新连接 · 正在确认连接稳定
gui-connection-details = 查看断线原因
gui-connection-issue-title = 连接中断
gui-connection-recovery-heading = 连接异常 · 处理建议
language-file-dialog-error = 无法打开语言文件选择窗口（{ $code }）。
language-restart-dialog-error = 无法打开重启提示窗口（{ $code }）。

language-auto = 跟随系统语言
language-builtin = 内置
language-local = 本地文件
language-settings-title = 常规设置
language-settings-description = 语言、启动选项与窗口行为。
language-option = { $name } · { $source }（{ $coverage }%）
language-unavailable = 不可用，当前使用英文
language-label = 语言
language-import = 导入语言文件…
language-open-folder = 打开语言文件夹
language-pending = 语言设置将在重启 OpenDS 后生效。
language-restart-title = 应用语言
language-restart-message = 重启 OpenDS 后将使用{ $language }。
language-restart-bridge = 重启会短暂断开虚拟手柄，并停止正在进行的录音或文件记录。
language-restart-later = 稍后(&L)
language-restart-now = 现在重启(&R)
language-restart-error = 无法重启 OpenDS：{ $error }
language-import-error = 无法导入语言文件：{ $error }
language-import-title = 导入语言文件
language-import-filter = Fluent 语言文件 (*.ftl)
language-replace-title = 替换语言文件？
language-replace-message = { $language }已有翻译，是否使用这个本地文件替换？
language-import-done = 已导入{ $language }。选择该语言并重启 OpenDS 后生效。
language-check-result = { $language }：已翻译 { $translated } / { $total } 条（{ $percent }%）。
language-check-missing = 缺少条目：{ $id }
language-check-error = 无效的语言文件：{ $error }
language-check-usage = 使用 opends language check FILE.ftl
language-error-io = 无法读写语言文件：{ $error }
language-error-utf8 = 语言文件必须采用 UTF-8 编码。
language-error-code = 请使用规范的 BCP 47 语言代码（例如 de-DE）：{ $code }
language-error-file = 语言文件超过 2 MiB，或含有空字符。
language-error-depth = 语言文件嵌套超过支持的上限。
language-error-syntax = Fluent 语法无效：{ $detail }
language-error-cycle = 引用循环或嵌套过深：{ $detail }
language-error-reference = 引用的条目不存在：{ $detail }
language-error-duplicate = 消息 ID 重复：{ $detail }
language-error-missing-meta = 缺少必需的语言信息：{ $detail }
language-error-literal-meta = 语言信息必须使用纯文本：{ $detail }
language-error-schema = 不支持此语言文件的格式版本。
language-error-parameters = 消息参数与英文基准不一致：{ $detail }
language-error-restart = 请先翻译重启提示：{ $detail }
language-error-name = 语言名称须为单行，且不超过 128 个字符。

## attach

attach-cannot-find-usb-ip-win2-usbip-exe = 未找到 usbip.exe，请在“诊断与日志”中指定 USB/IP Win2 安装目录，或恢复自动查找。
# Parameters: $error
attach-could-not-start-usb-ip = 启动 USB/IP 操作失败：{ $error }
# Parameters: $output, $error
attach-usb-ip-failed = USB/IP 操作失败：{ $output } { $error }
attach-usb-ip-operation-cancelled-or-timed-out = USB/IP 操作已取消或超时
attach-attachment-succeeded-but-returned-no-valid-ude = 挂接成功但未返回有效 UDE 端口
attach-ude-port-is-out-of-range = UDE 端口超出范围
attach-refusing-to-detach-an-invalid-ude-port = 拒绝卸载无效 UDE 端口

## audio test

audio-test-click-record-to-record-5-seconds-after = 点击录音：倒数 3 秒后录制 5 秒。
# Parameters: $second
audio-test-starting-in = 准备开始 · { $second }
audio-test-working = 处理中…
audio-test-cancelled-ready-to-record-again = 已取消 / 已清除录音
audio-test-cancel-the-current-test-and-wait-for = 请先取消或等待当前测试结束
audio-test-recording-cleared = 录音不可用
audio-test-record-some-audio-first = 请先录制一段声音
audio-test-starting-in-3 = 准备开始 · 3
audio-test-playing-bluetooth-source-through-the-default-playback = 正在回听蓝牙端 · 默认播放设备
audio-test-playing-usb-capture-through-the-default-playback = 正在回听 USB 端 · 默认播放设备
audio-test-cancelled = 已取消
# Parameters: $frames, $silent, $longest, $bursts
audio-test-microphone-test-captured-frames-stereo-zero-frames = 麦克风测试：采集 { $frames } 帧；双声道全零 { $silent } 帧，最长 { $longest } 帧，内部至少 1ms 的零段 { $bursts } 个
# Parameters: $frames, $gap, $gaps, $device
audio-test-microphone-source-decoded-frames-longest-packet-gap = 麦克风对照：蓝牙端 { $frames } 帧，最长包间隔 { $gap }ms，诊断队列丢包 { $gaps }；USB 端 { $device } 帧
# Parameters: $seconds, $peak, $hint
audio-test-recorded-s-peak = 录制 { $seconds } 秒 · 峰值 { $peak }%{ $hint }
audio-test-no-audible-signal-recorded-check-mute-and = { " " }· 未录到声音，请检查静音与麦克风设置
audio-test-ready-to-play-back = { " " }· 请分别回听两端
audio-test-playback-finished = 回听结束
audio-test-could-not-start-the-audio-test = 无法启动音频测试
# Parameters: $operation, $error, $code
audio-test-failed = { $operation }失败：{ $error }（{ $code }）
audio-test-no-dualsense-microphone-found-connect-the-controller = 未找到 DualSense 麦克风，请先连接桥接
audio-test-multiple-dualsense-microphones-found-disconnect-the-other = 检测到多个 DualSense 麦克风，请仅保留待测设备
audio-test-open-dualsense-microphone = 打开 DualSense 麦克风
audio-test-prepare-recording-buffer = 准备录音缓冲
audio-test-submit-recording-buffer = 提交录音缓冲
audio-test-start-recording = 启动录音
audio-test-recording-timed-out-no-complete-usb-microphone = 录音超时：未收到完整的 USB 麦克风数据
# Parameters: $seconds
audio-test-recording-seconds-remaining = 正在录音 · 剩余 { $seconds } 秒
audio-test-finish-recording = 结束录音
# Parameters: $seconds
audio-test-microphone-recording-duration-s = 麦克风录音耗时：{ $seconds }s
audio-test-open-default-playback-device = 打开默认播放设备
audio-test-prepare-playback-buffer = 准备回听缓冲
audio-test-start-playback = 开始回听
audio-test-playback-timed-out = 回听超时
audio-test-stop-recording = 停止录音
audio-test-release-recording-buffer = 释放录音缓冲
audio-test-close-microphone = 关闭麦克风
audio-test-stop-playback = 停止回听
audio-test-release-playback-buffer = 释放回听缓冲
audio-test-close-playback-device = 关闭播放设备
audio-test-the-audio-driver-did-not-return-the = 音频驱动未返回测试缓冲；已保留缓冲避免释放仍被驱动使用的内存

## audio test view

audio-test-view-opends-microphone-test = OpenDS · 麦克风测试
audio-test-view-could-not-create-microphone-test-window = 无法打开麦克风测试窗口
audio-test-view-record-5-seconds-after-a-3-second = 倒数 3 秒后录音 5 秒，可分别回听蓝牙端和 USB 端。
    录音仅暂存在内存，关闭窗口即清除。
audio-test-view-record-5-seconds = 录音 5 秒
audio-test-view-cancel = 取消
audio-test-view-play-usb-capture = 回听 USB 端
audio-test-view-clear-recording = 清除录音
audio-test-view-playback-uses-the-default-system-playback-device = 回听使用系统默认播放设备。关闭此窗口会清除录音。
audio-test-view-play-bluetooth-source = 回听蓝牙端
# Parameters: $status, $packets, $samples, $frames, $underruns, $dropped, $buffered, $target
audio-test-view-bluetooth-opus-packets-decoded-samples-usb-frames = { $status }
    蓝牙 Opus { $packets } 包 · 解码 { $samples } 样本 · USB 接收 { $frames } 帧
    USB 欠载 { $underruns } 次 · 丢弃 { $dropped } ms · 当前缓冲 { $buffered }/{ $target } ms
audio-test-view-connect-the-bluetooth-controller-and-start-bridging = 请先连接蓝牙手柄并启动桥接。
    回听使用系统默认播放设备。关闭此窗口会清除录音。
audio-test-view-enable-microphone-forwarding-in-connection-feedback-save = 请先在连接与反馈中启用麦克风转发，保存并重启桥接。
    回听使用系统默认播放设备。关闭此窗口会清除录音。

## bridge

bridge-bridging-is-already-running-or-stopping = 桥接已运行或正在停止
bridge-matching-libviiper-dll-not-found-check-dependencies = 未找到配套 libVIIPER.dll，请先检查依赖。
bridge-the-runtime-does-not-support-microphone-forwarding = 配套运行库不支持麦克风转发；请更新 DLL 或关闭麦克风设置。
bridge-opus-encoder-unavailable-install-the-matching-runtime = Opus 编码器不可用；请安装配套运行库，或关闭音频和原生触觉转发。
bridge-bridge-thread-failed-unexpectedly-resources-have-been = 桥接线程异常；已执行资源清理，请检查日志。
bridge-bridge-stopped = 桥接已停止
# Parameters: $error
bridge-could-not-start-bridge-thread = 启动桥接线程失败：{ $error }
bridge-some-callback-removals-failed-callbacks-have-been = 部分回调注销失败；回调入口已隔离，随后移除虚拟设备
bridge-output-thread-failed-unexpectedly-stopping-the-session = 输出线程异常，已停止会话
bridge-could-not-remove-virtual-device-check-the = 移除虚拟设备失败；请检查 UDE 驱动状态
bridge-the-selected-bluetooth-dualsense-was-not-found = 未找到指定的蓝牙 DualSense；请检查连接和 HidHide 白名单。
bridge-waiting-for-bluetooth-dualsense = 等待蓝牙 DualSense 连接…
bridge-input-released-waiting-to-reconnect = 已释放输入，等待重新连接
bridge-input-probe-timeout = 蓝牙设备可打开，但未收到有效按键输入；暂不隐藏或创建虚拟手柄。请确认蓝牙连接，并检查其他手柄软件是否正在接管。
# Parameters: $seconds
bridge-retry-backoff = 连续建链失败；将在 { $seconds } 秒后重试，减少设备反复插拔。若 Steam 正在后台运行，请检查其桌面布局和指南键组合设置。
# Parameters: $pid
bridge-physical-pid-creating-a-virtual-usb-device = 连接 PID={ $pid }，创建虚拟 USB 设备…
# Parameters: $report
bridge-feature-0x-has-an-invalid-bluetooth-crc = 特性 0x{ $report } 蓝牙 CRC 无效，重试…
# Parameters: $report, $error
bridge-could-not-read-feature-0x-retrying = 读取特性 0x{ $report } 失败：{ $error }，重试…
# Parameters: $report
bridge-could-not-obtain-physical-feature-0x-no = 无法取得真实特性 0x{ $report }；未创建不完整的虚拟身份
bridge-could-not-start-the-usb-server-check = USB 服务启动失败，请检查 UDE 驱动和端口占用
bridge-could-not-create-virtual-dualsense = 创建虚拟 DualSense 失败
bridge-viiper-does-not-support-the-usb-audio = 配套 VIIPER 缺少真机 USB 描述符配置，请使用当前发布包中的 DLL
# Parameters: $firmware, $protocol
bridge-physical-controller-firmware-audio-protocol = 真实手柄固件特性={ $firmware }，震动协议={ $protocol }
bridge-read-failed-using-compatibility-mode = 读取失败，按兼容模式处理
# Parameters: $report
bridge-viiper-could-not-set-physical-feature-0x = 配套 VIIPER 无法设置真实特性 0x{ $report }
bridge-bluetooth-crc-verified-calibration-pairing-and-firmware = 已校验蓝牙 CRC 并转换为 USB 特性，预载真实校准、配对与固件；随后挂接 USB
bridge-could-not-identify-the-physical-device-instance = 无法识别物理设备实例 ID
# Parameters: $error
bridge-could-not-enable-full-input-mode = 激活完整模式失败：{ $error }
bridge-callback-registration-incomplete = 回调注册锁损坏
bridge-libviiper-does-not-support-output-callbacks-use = libVIIPER 缺少输出回调；请使用配套版本
bridge-libviiper-does-not-support-audio-haptics-or = libVIIPER 缺少音频/触觉/流重置回调；请使用配套版本
# Parameters: $error
bridge-could-not-start-output-thread = 输出线程启动失败：{ $error }
bridge-bridge-connected-input-output-and-audio-are = 桥接已连接；输入、输出、音频使用独立调度
bridge-could-not-configure-microphone-buffering-use-the = 麦克风缓冲配置失败，请使用配套 libVIIPER.dll
bridge-could-not-create-microphone-decoder = 麦克风解码器不可用
bridge-could-not-update-audio-monitor-subscription = 更新音频监视订阅失败
bridge-repeated-writes-to-the-physical-controller-failed = 连续写入物理手柄失败
bridge-no-valid-controller-input-for-3-seconds = 手柄连续 3 秒没有有效输入，已判定断线
# Parameters: $milliseconds
bridge-microphone-safety-buffer-changed-to-ms = 麦克风安全缓冲已调整为 { $milliseconds } ms
bridge-could-not-submit-virtual-device-input = 虚拟设备输入提交失败
# Parameters: $error
bridge-could-not-read-bluetooth-controller = 蓝牙手柄读取失败：{ $error }
# Parameters: $flags, $header, $packets, $samples, $frames
bridge-microphone-audio-flags-header-opus-packets-decoded = 麦克风：音频标记={ $flags } 头={ $header } Opus包={ $packets } 解码样本={ $samples } USB接收帧={ $frames }
# Parameters: $full, $simple, $gyro, $callbacks, $rumble, $haptics, $audio, $flags, $motor_left, $motor_right, $outputs, $failures
bridge-diagnostics-full-input-simple-input-gyro-game = 诊断：完整输入={ $full } 兼容输入={ $simple } 陀螺仪={ $gyro }；游戏反馈={ $callbacks } 非零震动={ $rumble } 原生触觉={ $haptics } 音频回调={ $audio }；最近标志={ $flags } 马达=({ $motor_left }, { $motor_right }) 蓝牙发送={ $outputs } 写失败={ $failures }
# Parameters: $bytes, $rejected, $nonzero, $failures
bridge-audio-batches-max-b-rejected-nonzero-encoding = 音频批次：最大={ $bytes }B 拒绝={ $rejected } 非零={ $nonzero } 编码失败={ $failures }

## config

# Parameters: $error
config-could-not-read-configuration = 读取配置失败：{ $error }
# Parameters: $line
config-configuration-line-is-missing = 配置第 { $line } 行缺少 =
# Parameters: $key, $value
config-invalid-configuration = 配置 { $key } 无效：{ $value }
# Parameters: $key
config-configuration-must-be-0-1-or-true = 配置 { $key } 必须为 0/1 或 true/false
config-configuration-values-must-not-contain-line-breaks = 配置值不能包含换行或空字符。
# Parameters: $error
config-could-not-save-configuration = 保存配置失败：{ $error }
config-dsx-udp-automatic-dsx-launch-and-xbox = DSX UDP/自动启动 DSX/Xbox/DS4 外观尚未支持，请关闭相应配置。
config-local-bridging-requires-auto-attach-true = 本机桥接需要 auto_attach=true。

## dependencies

# Parameters: $usbip, $hidhide, $viiper, $opus, $path
dependencies-usb-ip-win2-ude-hidhide-libviiper-opus = USB/IP Win2 UDE：{ $usbip }
    HidHide：{ $hidhide }
    libVIIPER：{ $viiper }
    Opus：{ $opus }
    配置：{ $path }
dependencies-service-and-attachment-tool-available = 服务与挂接工具可用
dependencies-service-running-usbip-exe-is-missing = 驱动已运行；未找到 usbip.exe，请指定安装目录
dependencies-not-running-or-not-installed-install-reboot = 驱动未运行；若未安装请先安装，按提示重启后检查
dependencies-enabled-hiding-the-physical-controller = 已启用，正在隐藏物理手柄
dependencies-accessible = 可访问
dependencies-not-accessible-check-driver-and-permissions = 驱动不可访问，请检查安装、重启及权限
dependencies-available = 可用
dependencies-matching-dll-missing-place-it-beside-opends = 缺少配套 DLL，请放到 opends.exe 同目录
dependencies-libopus-0-dll-missing-audio-and-native = 缺少 libopus-0.dll；音频和原生触觉不可用
# Parameters: $error
dependencies-could-not-read-installation-result = 无法读取安装结果：{ $error }
dependencies-unsupported-driver-component = 不支持的驱动组件
dependencies-could-not-locate-the-application-directory = 无法获取程序目录
dependencies-scripts-install-drivers-ps1-is-missing-extract = 缺少 scripts/Install-Drivers.ps1，请重新解压完整发布包。
dependencies-could-not-locate-the-installation-log-directory = 无法取得安装日志目录
# Parameters: $error
dependencies-could-not-start-installer = 无法启动安装向导：{ $error }

## gui

# Parameters: $milliseconds, $samples
gui-ms-samples = { $milliseconds } ms · { $samples } 样本
gui-start-bridge = 启动桥接
gui-stop = 停止
gui-save-settings = 保存设置
gui-refresh-devices = 刷新设备
gui-microphone-test = 麦克风测试…
gui-restore-controller-visibility = 恢复手柄可见性
gui-minimize-to-tray = 最小化到托盘
gui-exit-opends = 退出 OpenDS
gui-start-bridging-when-opends-opens = 打开 OpenDS 时自动开始桥接
gui-run-in-the-tray-at-windows-sign = 登录 Windows 时在托盘运行
gui-reconnect-automatically = 断线后自动重连
gui-hide-the-physical-controller-to-avoid-duplicate = 隐藏真实手柄，避免重复输入
gui-forward-game-audio = 转发游戏扬声器音频
gui-forward-native-stereo-haptics = 转发左右独立的原生触觉
gui-forward-controller-microphone = 转发手柄麦克风
gui-ask = 询问
gui-exit-application = 直接退出
gui-auto-select-bluetooth-dualsense = 自动选择蓝牙 DualSense
gui-automatic-game-setting-headphone-connection = 自动选择（游戏设置 / 耳机插入状态）
gui-controller-speaker = 手柄扬声器
gui-headphones = 耳机
gui-enable-hidhide-device-hiding = 启用 HidHide 设备隐藏
gui-add-application = 添加程序
gui-remove-selected = 移除选中
gui-allow-opends-access = 允许 OpenDS 访问
gui-check-again = 重新检查
gui-install-usb-ip-win2 = 安装 USB/IP Win2
gui-install-hidhide = 安装 HidHide
gui-overview = 概览
gui-live-monitor = 实时监视
gui-connection-feedback = 连接与反馈
gui-device-hiding = 设备隐藏
gui-diagnostics-logs = 诊断与日志
gui-stop-bridge = 停止桥接
gui-open-settings = 打开设置
gui-stop-bridging-to-download-and-open-the = 停止桥接后可下载并打开 USB/IP Win2 官方安装向导
gui-stop-bridging-to-download-and-open-the-2 = 停止桥接后可下载并打开 HidHide 官方安装向导
gui-installation-requires-administrator-approval-dependencies-will-be = 目录选择后立即保存。HidHide 直接使用已安装驱动，配置工具为可选项。安装驱动需管理员确认。
gui-first-install-the-missing-drivers-on-this = 部分依赖尚未就绪，请按上方状态处理。已安装的组件可手动指定安装目录，未安装的驱动请先安装。
gui-dualsense-control-center = DualSense 控制中心
gui-bluetooth-connection-usb-experience = 蓝牙连接 · USB 体验
gui-connection-status-and-feedback-at-a-glance = 连接状态与反馈数据，尽在此处。
gui-view-controller-input-and-audio-start-capture = 按键与音频实时呈现，采集由你手动开启。
gui-choose-your-controller-and-adjust-audio-haptics = 选择手柄，调整声音、触觉与连接行为。
gui-let-games-see-only-the-virtual-controller = 让游戏只接收虚拟手柄的输入。
gui-check-dependencies-and-review-connection-and-recovery = 检查运行依赖，查看连接与恢复记录。
gui-native-feedback-wireless-freedom = 原生反馈，无线连接。
gui-dualsense-bluetooth-local-virtual-usb-controller = DualSense 蓝牙连接 → 本机虚拟 USB 手柄
gui-input-reports = 输入报告
gui-output-feedback = 输出反馈
gui-audio-blocks = 音频数据块
gui-recent-activity = 最近活动
gui-ready-start-bridging-to-see-connection-and = 准备就绪。启动桥接后，这里会显示连接与恢复记录。
gui-on-close = 关闭时行为
gui-physical-controller = 物理手柄
gui-audio-output = 声音输出
gui-speaker-volume-0-255 = 扬声器音量  ·  0–255
gui-haptics-gain-10-20 = 触觉增益  ·  10–20
# Parameters: $gain
gui-microphone-db-0-50 = 麦克风 +{ $gain } dB · 0–50
gui-local-service-port = 本机服务端口
gui-speaker-haptics-buffer-blocks = 扬声器/触觉缓存 · 块
gui-microphone-safety-buffer-ms = 麦克风安全缓冲 · ms
gui-application-whitelist = 应用白名单
gui-listed-applications-can-still-see-the-physical = 列表中的程序仍能看见真实手柄；游戏通常无需添加。
gui-stopping-the-bridge-restores-the-hiding-settings = 退出桥接会恢复由 OpenDS 修改的隐藏状态。
gui-application-log = 运行日志
gui-port-must-be-1-65535 = 端口必须为 1–65535
gui-buffer-must-be-1-128-blocks = 缓冲块必须为 1–128
gui-speaker-volume-must-be-0-255 = 扬声器音量必须为 0–255
gui-haptics-gain-must-be-10-20 = 触觉增益必须为 10–20
gui-port-buffer-or-haptics-gain-is-out = 端口、缓冲块或触觉增益超出范围
gui-driver-installation-is-in-progress-wait-for = 正在安装驱动，请等待安装结束后再启动桥接。
gui-saved-controller-currently-disconnected = 已保存的手柄（当前未连接）
gui-another-driver-installation-is-in-progress-wait = 已有驱动安装正在进行，请等待完成。
gui-stop-bridging-before-installing-drivers = 请先停止桥接，再安装驱动。
# Parameters: $component
gui-preparing-complete-the-installation-in-the-official = 正在准备 { $component }，请在出现的官方向导中完成安装。
# Parameters: $message, $path
gui-installation-log = { $message } 安装日志：{ $path }
# Parameters: $component
gui-installed-dependencies-are-ready = { $component } 安装已完成，运行依赖已就绪。
# Parameters: $component
gui-wizard-finished-but-dependencies-are-not-ready = { $component } 向导已完成；依赖尚未就绪，请按安装提示重启后重新检查。
# Parameters: $component
gui-installation-failed-or-was-cancelled-see-the = { $component } 安装未完成或已取消，请查看日志。
gui-stopping-and-restoring-devices = 正在停止并恢复设备…
gui-connected-forwarding = ● 已连接 · 正在转发
gui-bridge-started-waiting-for-controller = ● 桥接已启动 · 等待手柄连接…
# Parameters: $error
gui-bridge-stopped = ○ 桥接已停止 · { $error }
gui-bridge-not-started = ○ 桥接尚未启动
gui-bridging-is-running-stop-it-before-installing = 桥接运行中，请先停止桥接，再修改依赖目录或安装驱动。
gui-ready-start-bridging-to-see-connection-feedback = 准备就绪。
    启动桥接后，这里会显示连接、反馈和恢复记录。
gui-some-dependencies-are-missing-or-inaccessible-open = 缺少或无法访问运行依赖，请打开诊断检查
gui-dependencies-are-ready = 运行依赖正常
gui-settings-saved-applied-the-next-time-bridging = 设置已保存，下次启动桥接生效
gui-select-a-whitelist-entry-first = 请先选择一个白名单条目
gui-could-not-change-hidhide-state = 写入 HidHide 开关失败
gui-opends-ui-test-simulated-signals = OpenDS · 界面测试（模拟信号）
gui-waiting-for-controller = 等待手柄连接
gui-tray-startup-did-not-display-the-waiting = 托盘启动时未显示等待手柄状态
gui-state-unavailable = 状态不可用
gui-ui-smoke-error = 界面测试错误
gui-overview-did-not-display-the-bridge-error = 概览未显示桥接错误
gui-opening-from-tray-did-not-refresh-the = 从托盘打开时状态未立即刷新
gui-main-window-is-missing-its-minimize-button = 主窗口缺少任务栏最小化按钮
gui-minimizing-did-not-preserve-the-taskbar-window = 最小化没有保留任务栏窗口
gui-minimizing-to-tray-did-not-hide-the = 最小化到托盘没有隐藏窗口或错误触发了退出
gui-restoring-from-tray-did-not-restore-the = 托盘恢复没有还原最小化窗口
gui-cancelling-close-changed-the-window-or-exit = 取消关闭改变了窗口或退出状态
gui-close-preference-was-not-persisted-or-synchronized = 关闭偏好未持久化或未同步设置页
gui-remembered-tray-action-did-not-take-effect = 已记住的托盘选项未生效
gui-could-not-restore-the-close-confirmation = 无法恢复关闭时询问
gui-window-or-taskbar-icon-was-not-loaded = 窗口或任务栏图标未加载
gui-could-not-create-the-ui-status-timer = 无法创建界面状态定时器

## gui close

gui-close-close-the-opends-window = 关闭 OpenDS 窗口？
gui-close-minimizing-to-the-tray-keeps-bridging-and = 最小化到托盘会保持桥接与采集运行。
    退出将停止桥接、结束文件记录并恢复设备可见性。
gui-close-remember-my-choice = 记住我的选项
# Parameters: $code
gui-close-could-not-open-close-options-0x-use = 无法打开关闭选项（0x{ $code }），请使用托盘菜单或侧栏按钮。
gui-close-close-action-or-remember-choice-test-failed = 关闭选项或记忆选项检查失败
gui-close-close-cancellation-test-failed = 取消关闭检查失败

## hid

hid-hid-closed = HID 读锁损坏
hid-hid-disconnected = HID 已断开
hid-hid-not-opened-for-writing = HID 写锁损坏
hid-invalid-hid-output-report-length = HID 输出长度无效
hid-hid-write-timed-out = HID 写入超时
hid-incomplete-hid-write = HID 写入不完整
hid-invalid-feature-report-length = 特性报告长度无效
hid-feature-report-read-timed-out = 特性报告读取超时
hid-invalid-feature-report-response-length = 特性报告内容无效

## hidhide

hidhide-duplicate-active-key-in-hidhide-recovery-journal = HidHide 恢复日志有重复 active 项
hidhide-empty-hidhide-recovery-journal = HidHide 恢复日志损坏
hidhide-invalid-hidhide-recovery-journal-version = HidHide 恢复日志版本无效
hidhide-corrupt-or-unsupported-hidhide-recovery-journal-file = HidHide 恢复日志不完整或版本不支持；已保留文件
hidhide-localappdata-is-not-set-cannot-locate-the = LOCALAPPDATA 未设置，无法定位 HidHide 恢复日志
# Parameters: $error
hidhide-could-not-read-hidhide-recovery-journal = 读取 HidHide 恢复日志失败：{ $error }
hidhide-could-not-read-hidhide-device-blacklist-recovery = 读取 HidHide 黑名单失败；恢复日志已保留
hidhide-could-not-read-hidhide-application-whitelist-recovery = 读取 HidHide 白名单失败；恢复日志已保留
hidhide-could-not-read-hidhide-state-recovery-journal = 读取 HidHide 开关失败；恢复日志已保留
hidhide-could-not-restore-hidhide-state-recovery-journal = HidHide 恢复写入失败；恢复日志已保留
hidhide-hidhide-recovery-verification-failed-recovery-journal-retained = HidHide 恢复回读不一致；恢复日志已保留
hidhide-hidhide-unavailable-cannot-restore-yet-recovery-journal = HidHide 不可用，尚不能恢复；恢复日志已保留
hidhide-could-not-read-the-recovery-path = 恢复路径不可用
# Parameters: $error
hidhide-hidhide-restored-but-its-recovery-journal-could = HidHide 已恢复，但删除恢复日志失败：{ $error }
hidhide-localappdata-is-not-set-refusing-to-hide = LOCALAPPDATA 未设置，拒绝在无恢复日志时隐藏设备
hidhide-unfinished-hidhide-recovery-found-run-restore-first = 还有未完成的 HidHide 恢复，请先执行 restore
hidhide-hidhide-driver-unavailable = HidHide 驱动不可用
hidhide-could-not-read-hidhide-state = 读取 HidHide 开关失败
hidhide-could-not-read-hidhide-device-blacklist = 读取 HidHide 黑名单失败
hidhide-could-not-read-hidhide-application-whitelist = 读取 HidHide 白名单失败
# Parameters: $error
hidhide-could-not-save-recovery-journal-driver-unchanged = 保存恢复日志失败，未改动驱动：{ $error }
# Parameters: $result
hidhide-could-not-hide-the-physical-controller = 隐藏物理手柄失败；{ $result }
hidhide-rolled-back = 已回滚
hidhide-invalid-characters-in-application-path = 程序路径含无效字符
# Parameters: $error
hidhide-application-path-unavailable = 程序路径不可用：{ $error }
hidhide-select-an-application-on-a-local-disk = 请选择本机磁盘上的程序，不支持 UNC 路径
# Parameters: $error
hidhide-could-not-convert-application-path = 转换磁盘路径失败：{ $error }
hidhide-invalid-application-path = 磁盘路径无效
hidhide-could-not-read-application-whitelist = 读取白名单失败
hidhide-could-not-write-or-verify-application-whitelist = 写入或验证白名单失败

## main

main-use-startup-enable-disable-status = 使用 startup enable / disable / status
# Parameters: $state
main-windows-sign-in-startup = Windows 登录自启：{ $state }
main-enabled = 开启
main-disabled = 关闭
main-opends-opends-gui-settings-and-tray-default = OpenDS
    { "  " }opends gui                 设置与托盘（默认）
    { "  " }opends bridge [秒数]        前台桥接；省略或 0 表示持续运行，Ctrl+C 停止
    { "  " }opends restore             恢复未完成的 HidHide 会话
    { "  " }opends doctor              依赖诊断（只读）
    { "  " }opends list                枚举物理蓝牙 DualSense
    { "  " }opends probe               读取一帧输入（不写设备）
main-gui-creation-icons-event-loop-and-clean = GUI 创建、图标加载、事件循环和正常退出检查通过
main-no-input-received-within-500-ms = 500ms 内无输入
# Parameters: $command
main-unknown-command-use-opends-help = 未知命令：{ $command }；使用 opends --help
main-hidhide-recovery-succeeded-and-was-verified = HidHide 恢复成功且回读一致
main-no-hidhide-session-needs-recovery = 没有待恢复的 HidHide 会话
main-seconds-must-be-a-non-negative-integer = 秒数必须为非负整数
main-console-state-lock-is-poisoned = 控制台状态锁损坏

## monitor

monitor-monitor-cache-would-exceed-32-mib-reduce = 监视缓存将超出 32 MiB 上限，请降低采样频率
monitor-monitor-cache-unavailable = 监视缓存不可用
monitor-could-not-start-monitor-worker = 监视线程不可用
monitor-start-capture-before-recording-to-a-file = 请先开始采集，再开启文件记录
monitor-recording-would-exceed-the-monitor-cache-limit = 记录队列将超出监视缓存上限，请降低采样频率

## monitor recording

monitor-recording-invalid-drive-directory = 保存目录无效
# Parameters: $error
monitor-recording-could-not-check-free-space-on-the = 无法检查目标磁盘可用空间：{ $error }
# Parameters: $megabytes
monitor-recording-only-mib-remain-on-the-target-disk = 目标磁盘剩余 { $megabytes } MiB，已停止记录以预留 512 MiB；桥接和采集继续运行
monitor-recording-recording-state-unavailable = 记录状态不可用
monitor-recording-recording-is-active-or-still-saving-try = 记录仍在运行或正在保存，请稍后重试
# Parameters: $error
monitor-recording-csv-recording-failed = CSV 记录失败：{ $error }
monitor-recording-recording-queue-full-or-capture-delayed-too = 记录缓冲已满或采集延迟过长，已停止记录；已有数据正在保存，采集和桥接继续运行
# Parameters: $path, $error
monitor-recording-could-not-write-to-check-write-permissions = 无法写入所选文件 { $path }：{ $error }。请检查保存位置的写入权限。

## monitor view

monitor-view-csv-files-csv-u0000-csv-u0000 = CSV 文件 (*.csv)
monitor-view-choose-where-to-save-the-csv-recording = 选择 CSV 记录文件保存位置
# Parameters: $code
monitor-view-could-not-choose-save-location-0x = 无法选择保存位置：0x{ $code }
# Parameters: $code
monitor-view-drawing-failed-0x = 绘制失败 0x{ $code }
monitor-view-muted = 静音
monitor-view-touch-l = 背键 L
monitor-view-touch-r = 背键 R
monitor-view-could-not-create-monitor-window = 创建监视窗口失败
monitor-view-start-capture = 开始采集
monitor-view-clear = 清空
monitor-view-source = 源信号
monitor-view-forwarded = 转发信号
monitor-view-separate-window = 独立窗口
monitor-view-always-on-top = 置顶
monitor-view-sample = 采样
monitor-view-refresh = 刷新
monitor-view-history = 跨度
monitor-view-waiting-for-manual-start = 等待手动开启
monitor-view-start-file-recording = 开始记录文件
# Parameters: $seconds
monitor-view-seconds = { $seconds } 秒
monitor-view-could-not-create-drawing-canvas = 创建绘制区域失败
# Parameters: $error
monitor-view-could-not-save-monitor-preferences = 监视设置保存失败：{ $error }
monitor-view-opends-live-monitor = OpenDS · 实时监视
monitor-view-could-not-create-separate-window = 独立窗口创建失败
monitor-view-dock-window = 收回窗口
monitor-view-stop-capture = 停止采集
monitor-view-checking = 正在检查…
monitor-view-stop-file-recording = 停止记录文件
monitor-view-saving = 正在保存…
monitor-view-csv-recording-failed = { " " }· CSV 记录失败
monitor-view-checking-save-location = { " " }· 正在检查保存位置…
monitor-view-recording = 记录中
monitor-view-saving-2 = 保存中
monitor-view-saved = 已保存
# Parameters: $count
monitor-view-dropped = { " " }/ 丢弃 { $count }
# Parameters: $capture, $connection, $recording, $drops, $errors, $resets, $cache
monitor-view-dropped-decode-errors-stream-resets-cache-mib = { $capture } · { $connection }{ $recording } · 丢弃 { $drops } · 解码失败 { $errors } · 流重置 { $resets } · 缓存 { $cache } MiB
monitor-view-capturing = 采集中
monitor-view-stopped-history-retained = 已停止 / 历史冻结
monitor-view-connected = 已连接
monitor-view-waiting-for-controller = 等待手柄
monitor-view-capture-stopped-last-input-retained = 采集已停止 · 输入保持最后一帧
monitor-view-disconnected-waiting-to-reconnect = 未连接 · 等待输入
monitor-view-waiting-for-new-input-reports = 等待新的输入报告
monitor-view-live-input-buttons-sticks-and-touchpad = 实时输入 · 按键、摇杆与触摸板
# Parameters: $lx, $ly, $rx, $ry
monitor-view-left-stick-x-y-right-stick-x = 左摇杆  X { $lx }   Y { $ly }
    右摇杆  X { $rx }   Y { $ry }
monitor-view-blue-source-orange-dots-forwarded = 蓝色源信号  /  橙色虚线转发信号
monitor-view-speaker-front-l-r = 扬声器 · 前置 L / R
monitor-view-haptics-rear-l-r = 触觉 · 后置 L / R
monitor-view-microphone-mono = 麦克风 · 单声道
monitor-view-microphone-disabled = 麦克风未启用
monitor-view-no-audio-signal-selected = 未选择音频信号
monitor-view-no-data-received-yet = 尚未收到数据
monitor-view-data-interrupted-500-ms = 数据中断 > 500 ms
monitor-view-silent-valid-zero-amplitude-pcm-received = 静音 · 已收到有效的零振幅 PCM
# Parameters: $route, $volume
monitor-view-volume = { $route } · 音量 { $volume }
monitor-view-headphone-route = 耳机路由
# Parameters: $gain, $limiter
monitor-view-source-original-level-forwarded-db = 源：原始电平 · 转发：+{ $gain } dB{ $limiter }
monitor-view-limited = { " " }/ 限幅
monitor-view-blue-source-orange-forwarded-fixed-100 = 蓝色源信号 / 橙色转发 · 固定 ±100%
monitor-view-opends-file-recording-stopped = OpenDS · 文件记录已停止

## persistence

persistence-opends-is-already-running-use-its-existing = OpenDS 已在运行；请从现有托盘实例操作。
# Parameters: $error
persistence-could-not-create-process-lock = 无法创建进程锁：{ $error }

## startup

# Parameters: $error
startup-windows-sign-in-startup-setup-failed = Windows 登录自启设置失败：{ $error }
startup-the-executable-path-is-not-valid-unicode = 程序路径不是有效 Unicode
startup-the-executable-path-contains-invalid-characters = 程序路径包含无效字符
startup-the-sign-in-startup-command-exceeds-the = 登录启动命令超过 Windows 的 260 字符限制，请使用更短安装路径

# Appearance and shared dialogs
appearance-label = 外观
appearance-system = 随系统
appearance-light = 浅色
appearance-dark = 深色
dialog-ok = 确定(&O)
dialog-yes = 是(&Y)
dialog-no = 否(&N)

dependency-directory-invalid = 所选目录中未找到 { $executable }：{ $path }。请选择程序的安装目录。
dependency-usbip-custom-invalid = 驱动已运行；指定目录中未找到 usbip.exe，请重新选择
dependency-choose-directory = 选择目录…
dependency-use-auto = 自动查找
dependency-open-hidhide = 打开 HidHide 配置
dependency-location = { $component } 位置（{ $mode }）
dependency-mode-auto = 自动查找
dependency-mode-manual = 手动指定
dependency-tool-not-found = 未找到工具；已安装时请选择安装目录
dependency-selected-missing = 未找到 { $executable }：{ $path }
dependency-stop-before-changing = 请先停止桥接，并等待安装完成后再修改目录。
dependency-directory-title = 选择包含 { $executable } 的安装目录
dependency-directory-encoding = 该目录无法保存为 UTF-8 路径。
dependency-hidhide-not-found = 未找到 HidHideClient.exe，请指定 HidHide 安装目录。桥接本身不需要此配置工具。
dependency-installer-finished = { $component } 安装向导已结束。请查看上方依赖状态；已安装时可指定目录，按安装提示重启。

# Input history
command-title = 指令表
command-overlay = 悬浮窗
command-lock = 锁定
command-settings = 指令表设置
command-clear = 清空
command-setting-value = 设置值
command-setting-color-value = 设置值（颜色：#RRGGBB）
command-apply = 应用
command-color = 选择颜色
command-defaults = 恢复默认
command-settings-hint = 选择设置项后修改并应用。来源支持文字／矢量图标／隐藏；自动配色适配纯黑白，关闭后分别设置深浅配色。
command-preview = 预览
command-invalid = 设置值无效
command-saved = 已保存
command-connected = 已连接
command-disconnected = 已断开
command-gap = 输入记录缺口
command-boundary = 分类设置已变更
command-source-text = 文字
command-source-icon = 图标
command-source-hidden = 隐藏
command-follow = 跟随软件
command-light = 浅色
command-dark = 深色
command-no = 关闭
command-yes = 开启
command-enabled = 记录指令
command-right = 记录右摇杆
command-source = 来源标识
command-rows = 主面板可见行数
command-overlay-rows = 悬浮窗可见行数
command-auto-colors = 自动适配主题配色
command-theme = 悬浮窗主题
command-bg-alpha = 背景不透明度（%）
command-fg-alpha = 内容不透明度（%）
command-topmost = 悬浮窗置顶
command-x = 窗口横向位置
command-y = 窗口纵向位置
command-width = 窗口宽度
command-height = 窗口高度
command-neutral = 中立颜色
command-dpad = 方向键颜色
command-button = 按键颜色
command-connection = 连接事件颜色
command-deadzone = 死区
command-medium = 中等力度
command-full = 推满
command-gentle = 轻推
command-background = 背景
command-text = 文字
command-dpad-prefix = 方向键
command-empty = 等待指令记录
command-reset = 恢复悬浮窗
command-menu-show = 显示／隐藏指令悬浮窗
command-menu-lock = 锁定／解锁指令悬浮窗
command-short-settings = 设置
command-short-reset = 恢复
command-file-failed = 指令文件记录失败
command-unlock = 解锁

command-color-preview = 颜色预览

command-content-size = 内容大小（50–500%）

command-display-min-ms = 小于此时长不显示（ms，0=关闭）
command-record-min-ms = 小于此时长不写指令文件（ms，0=关闭）

command-overlay-auto-rows = 悬浮窗行数自适应
command-separators = 显示记录分隔线
command-separator-alpha = 分隔线不透明度（%）
command-separator-style = 分隔线两端样式
command-separator-color = 分隔线颜色
command-separator-flat = 平直
command-separator-fade = 渐隐
command-separator-taper = 渐尖

command-ignore-strength = 不区分力度
command-unified-color = 统一颜色

command-group-record = 记录与显示
command-group-window = 窗口与布局
command-group-strength = 力度与死区
command-group-separator = 分隔线
command-group-colors = 颜色与主题配色

command-distinguish-strength = 区分力度
