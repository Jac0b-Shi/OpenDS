meta-language-code = en-US
meta-language-name = English
meta-schema-version = 1
shortcut-desktop = Create desktop shortcut
battery-label = Controller battery
battery-preview-title = OpenDS · Battery preview (simulated data)
battery-preview-hint = Appearance preview · simulated data
battery-disconnected = Controller disconnected
battery-awaiting = Waiting for battery data
battery-charging = Charging
battery-full = Fully charged
battery-fault = Charging fault
battery-unknown = Battery unavailable
battery-expired = Battery data expired
battery-discharging = On battery
battery-tooltip-level = Estimated battery: { $percent }%. The controller reports approximately 10% ranges.
battery-tooltip-charging = Charging · estimated battery: { $percent }%. The controller reports approximately 10% ranges.
battery-tooltip-fault = The controller reported a charging fault (code { $code }).
gui-diagnostics-failed = Could not start background diagnostics: { $error }
gui-diagnostics-worker-failed = Background diagnostics failed. Refresh to try again.
shortcut-start-menu = Create Start menu shortcut
shortcut-created = Shortcut created: { $path }
shortcut-exists = A file already exists at { $path }. It has not been replaced.
shortcut-failed = Could not create the shortcut: { $error }
bridge-connection-diagnostic = Connection diagnostic
bridge-connection-resource-busy = Windows returned error 170 (resource in use) while reading the physical controller's feature reports, and this connection attempt then failed.
    Final error: { $error }
    Another program may already be accessing the physical controller or changing its protocol state. This does not identify the responsible process.
    Exit the game and other controller tools, reconnect with PS, and establish a stable OpenDS bridge before restarting the game.
bridge-connection-lost-immediately = The Bluetooth controller disconnected { $milliseconds } ms after the bridge connected.
    Error: { $error }
    Possible causes include another application accessing the physical controller, a Bluetooth link or power problem, or conflicting controller output. This error alone does not identify the cause.
    Exit the game and other controller tools, reconnect the controller, and start OpenDS first. Wait for a stable connection before starting the game. Some games enable full feedback only when the controller is present at launch.
bridge-connection-lost = The Bluetooth connection was lost after { $seconds } seconds.
    Error: { $error }
    Check the controller battery, Bluetooth signal, and other controller tools. Press PS to reconnect. If the problem occurs when starting a game, exit it and establish the bridge before restarting the game.
bridge-connection-setup-failed = The controller bridge could not be initialized.
    Error: { $error }
    Check the Bluetooth connection, driver diagnostics and HidHide access. Close games and other controller tools before retrying. The original error above is the available evidence; application conflicts are only a possible cause.
gui-connection-interrupted = ● Bluetooth disconnected · Waiting to reconnect
gui-connection-stopped = ● Bridge stopped · See connection details
gui-connection-retrying = ● Bridge initialization failed · Waiting to retry
gui-connection-confirming = ● Reconnected · Checking connection stability
gui-connection-details = Connection details
gui-connection-issue-title = Connection interrupted
gui-connection-recovery-heading = Connection issue · Recovery advice
language-file-dialog-error = Unable to open the language file dialog ({ $code }).
language-restart-dialog-error = Unable to open the restart dialog ({ $code }).

language-auto = Follow system language
language-builtin = Built-in
language-local = Local file
language-settings-title = General settings
language-settings-description = Language, startup and window behavior.
language-option = { $name } · { $source } ({ $coverage }%)
language-unavailable = Unavailable; using English
language-label = Language
language-import = Import language file…
language-open-folder = Open language folder
language-pending = Language changes take effect after restarting OpenDS.
language-restart-title = Apply language
language-restart-message = Restart OpenDS to use { $language }.
language-restart-bridge = Restarting will briefly disconnect the virtual controller and stop any recording.
language-restart-later = &Later
language-restart-now = &Restart now
language-restart-error = Could not restart OpenDS: { $error }
language-import-error = Could not import the language file: { $error }
language-import-title = Import language file
language-import-filter = Fluent language files (*.ftl)
language-replace-title = Replace language file?
language-replace-message = A translation for { $language } already exists. Replace it with this local file?
language-import-done = Imported { $language }. Select it and restart OpenDS to apply it.
language-check-result = { $language }: { $translated } / { $total } messages translated ({ $percent }%).
language-check-missing = Missing message: { $id }
language-check-error = Invalid language file: { $error }
language-check-usage = Use opends language check FILE.ftl
language-error-io = Could not read or write the language file: { $error }
language-error-utf8 = The language file must use UTF-8 encoding.
language-error-code = Use a canonical BCP 47 language code, such as de-DE: { $code }
language-error-file = The language file exceeds 2 MiB or contains NUL characters.
language-error-depth = Language file nesting exceeds the supported limit.
language-error-syntax = Invalid Fluent syntax: { $detail }
language-error-cycle = Cyclic or excessively nested reference: { $detail }
language-error-reference = Unknown reference: { $detail }
language-error-duplicate = Duplicate message ID: { $detail }
language-error-missing-meta = Required metadata is missing: { $detail }
language-error-literal-meta = Metadata must be literal text: { $detail }
language-error-schema = This language file format version is not supported.
language-error-parameters = Message parameters do not match the English catalog: { $detail }
language-error-restart = The restart prompt must be translated: { $detail }
language-error-name = The language name must be a single line of at most 128 characters.

## attach

attach-cannot-find-usb-ip-win2-usbip-exe = Cannot find usbip.exe. Open Diagnostics and select the USB/IP Win2 installation directory, or restore automatic discovery.
# Parameters: $error
attach-could-not-start-usb-ip = Could not start USB/IP: { $error }
# Parameters: $output, $error
attach-usb-ip-failed = USB/IP failed: { $output } { $error }
attach-usb-ip-operation-cancelled-or-timed-out = USB/IP operation cancelled or timed out
attach-attachment-succeeded-but-returned-no-valid-ude = Attachment succeeded but returned no valid UDE port
attach-ude-port-is-out-of-range = UDE port is out of range
attach-refusing-to-detach-an-invalid-ude-port = Refusing to detach an invalid UDE port

## audio test

audio-test-click-record-to-record-5-seconds-after = Click Record to record 5 seconds after a 3-second countdown.
# Parameters: $second
audio-test-starting-in = Starting in { $second }
audio-test-working = Working…
audio-test-cancelled-ready-to-record-again = Cancelled / ready to record again
audio-test-cancel-the-current-test-and-wait-for = Cancel the current test and wait for it to finish
audio-test-recording-cleared = Recording cleared
audio-test-record-some-audio-first = Record some audio first
audio-test-starting-in-3 = Starting in 3
audio-test-playing-bluetooth-source-through-the-default-playback = Playing Bluetooth source through the default playback device
audio-test-playing-usb-capture-through-the-default-playback = Playing USB capture through the default playback device
audio-test-cancelled = Cancelled
# Parameters: $frames, $silent, $longest, $bursts
audio-test-microphone-test-captured-frames-stereo-zero-frames = Microphone test: captured { $frames } frames; { $silent } stereo-zero frames, longest run { $longest } frames; { $bursts } internal zero bursts longer than 1 ms
# Parameters: $frames, $gap, $gaps, $device
audio-test-microphone-source-decoded-frames-longest-packet-gap = Microphone source: decoded { $frames } frames; longest packet gap { $gap } ms; { $gaps } interruptions; USB endpoint { $device } frames
# Parameters: $seconds, $peak, $hint
audio-test-recorded-s-peak = Recorded { $seconds } s · peak { $peak }%{ $hint }
audio-test-no-audible-signal-recorded-check-mute-and = { " " }· No audible signal recorded. Check mute and microphone settings.
audio-test-ready-to-play-back = { " " }· Ready to play back
audio-test-playback-finished = Playback finished
audio-test-could-not-start-the-audio-test = Could not start the audio test
# Parameters: $operation, $error, $code
audio-test-failed = { $operation } failed: { $error } ({ $code })
audio-test-no-dualsense-microphone-found-connect-the-controller = No DualSense microphone found. Connect the controller and start bridging.
audio-test-multiple-dualsense-microphones-found-disconnect-the-other = Multiple DualSense microphones found. Disconnect the other devices.
audio-test-open-dualsense-microphone = Open DualSense microphone
audio-test-prepare-recording-buffer = Prepare recording buffer
audio-test-submit-recording-buffer = Submit recording buffer
audio-test-start-recording = Start recording
audio-test-recording-timed-out-no-complete-usb-microphone = Recording timed out: no complete USB microphone audio received
# Parameters: $seconds
audio-test-recording-seconds-remaining = Recording · { $seconds } seconds remaining
audio-test-finish-recording = Finish recording
# Parameters: $seconds
audio-test-microphone-recording-duration-s = Microphone recording duration: { $seconds } s
audio-test-open-default-playback-device = Open default playback device
audio-test-prepare-playback-buffer = Prepare playback buffer
audio-test-start-playback = Start playback
audio-test-playback-timed-out = Playback timed out
audio-test-stop-recording = Stop recording
audio-test-release-recording-buffer = Release recording buffer
audio-test-close-microphone = Close microphone
audio-test-stop-playback = Stop playback
audio-test-release-playback-buffer = Release playback buffer
audio-test-close-playback-device = Close playback device
audio-test-the-audio-driver-did-not-return-the = The audio driver did not return the test buffer; retaining it to avoid freeing memory still in use.

## audio test view

audio-test-view-opends-microphone-test = OpenDS · Microphone test
audio-test-view-could-not-create-microphone-test-window = Could not create microphone test window
audio-test-view-record-5-seconds-after-a-3-second = Record 5 seconds after a 3-second countdown, then compare Bluetooth source and USB capture.
    Recordings stay in memory and are cleared when this window closes.
audio-test-view-record-5-seconds = Record 5 seconds
audio-test-view-cancel = Cancel
audio-test-view-play-usb-capture = Play USB capture
audio-test-view-clear-recording = Clear recording
audio-test-view-playback-uses-the-default-system-playback-device = Playback uses the default system playback device. Closing this window clears the recording.
audio-test-view-play-bluetooth-source = Play Bluetooth source
# Parameters: $status, $packets, $samples, $frames, $underruns, $dropped, $buffered, $target
audio-test-view-bluetooth-opus-packets-decoded-samples-usb-frames = { $status }
    Bluetooth Opus: { $packets } packets · decoded: { $samples } samples · USB: { $frames } frames
    USB underruns: { $underruns } · dropped: { $dropped } ms · buffered: { $buffered }/{ $target } ms
audio-test-view-connect-the-bluetooth-controller-and-start-bridging = Connect the Bluetooth controller and start bridging first.
    Playback uses the default system playback device. Closing this window clears the recording.
audio-test-view-enable-microphone-forwarding-in-connection-feedback-save = Enable microphone forwarding in Connection & feedback, save, and restart bridging.
    Playback uses the default system playback device. Closing this window clears the recording.

## bridge

bridge-bridging-is-already-running-or-stopping = Bridging is already running or stopping
bridge-matching-libviiper-dll-not-found-check-dependencies = Matching libVIIPER.dll not found. Check dependencies first.
bridge-the-runtime-does-not-support-microphone-forwarding = The runtime does not support microphone forwarding. Update the DLL or disable the microphone.
bridge-opus-encoder-unavailable-install-the-matching-runtime = Opus encoder unavailable. Install the matching runtime or disable audio and native haptics forwarding.
bridge-bridge-thread-failed-unexpectedly-resources-have-been = Bridge thread failed unexpectedly. Resources have been cleaned up; check the logs.
bridge-bridge-stopped = Bridge stopped
# Parameters: $error
bridge-could-not-start-bridge-thread = Could not start bridge thread: { $error }
bridge-some-callback-removals-failed-callbacks-have-been = Some callback removals failed; callbacks have been isolated before virtual-device removal
bridge-output-thread-failed-unexpectedly-stopping-the-session = Output thread failed unexpectedly; stopping the session
bridge-could-not-remove-virtual-device-check-the = Could not remove virtual device. Check the UDE driver.
bridge-the-selected-bluetooth-dualsense-was-not-found = The selected Bluetooth DualSense was not found. Check its connection and HidHide whitelist.
bridge-waiting-for-bluetooth-dualsense = Waiting for Bluetooth DualSense…
bridge-input-released-waiting-to-reconnect = Input released; waiting to reconnect
bridge-input-probe-timeout = The Bluetooth device opened but did not provide valid controller input. Hiding and virtual device creation have been deferred. Check the Bluetooth connection and whether another controller application is taking control.
# Parameters: $seconds
bridge-retry-backoff = Repeated connection failures; retrying in { $seconds } seconds to reduce device reconnections. If Steam is running, check its Desktop Layout and Guide Button Chord settings.
# Parameters: $pid
bridge-physical-pid-creating-a-virtual-usb-device = Physical PID={ $pid }. Creating a virtual USB device…
# Parameters: $report
bridge-feature-0x-has-an-invalid-bluetooth-crc = Feature 0x{ $report } has an invalid Bluetooth CRC; retrying…
# Parameters: $report, $error
bridge-could-not-read-feature-0x-retrying = Could not read feature 0x{ $report }: { $error }; retrying…
# Parameters: $report
bridge-could-not-obtain-physical-feature-0x-no = Could not obtain physical feature 0x{ $report }; no incomplete virtual identity was created
bridge-could-not-start-the-usb-server-check = Could not start the USB server. Check the UDE driver and port availability.
bridge-could-not-create-virtual-dualsense = Could not create virtual DualSense
bridge-viiper-does-not-support-the-usb-audio = VIIPER does not support the USB audio identity profile. Use the DLL from this release.
# Parameters: $firmware, $protocol
bridge-physical-controller-firmware-audio-protocol = Physical controller: firmware { $firmware }, audio protocol { $protocol }
bridge-read-failed-using-compatibility-mode = Read failed; using compatibility mode
# Parameters: $report
bridge-viiper-could-not-set-physical-feature-0x = VIIPER could not set physical feature 0x{ $report }
bridge-bluetooth-crc-verified-calibration-pairing-and-firmware = Bluetooth CRC verified; calibration, pairing and firmware features converted to USB and loaded before attachment
bridge-could-not-identify-the-physical-device-instance = Could not identify the physical device instance ID
# Parameters: $error
bridge-could-not-enable-full-input-mode = Could not enable full input mode: { $error }
bridge-callback-registration-incomplete = Callback registration incomplete
bridge-libviiper-does-not-support-output-callbacks-use = libVIIPER does not support output callbacks. Use the matching release.
bridge-libviiper-does-not-support-audio-haptics-or = libVIIPER does not support audio, haptics or configuration callbacks. Use the matching release.
# Parameters: $error
bridge-could-not-start-output-thread = Could not start output thread: { $error }
bridge-bridge-connected-input-output-and-audio-are = Bridge connected; input, output and audio are scheduled independently
bridge-could-not-configure-microphone-buffering-use-the = Could not configure microphone buffering. Use the matching libVIIPER.dll.
bridge-could-not-create-microphone-decoder = Could not create microphone decoder
bridge-could-not-update-audio-monitor-subscription = Could not update audio monitor subscription
bridge-repeated-writes-to-the-physical-controller-failed = Repeated writes to the physical controller failed
bridge-no-valid-controller-input-for-3-seconds = No valid controller input for 3 seconds; reconnecting
# Parameters: $milliseconds
bridge-microphone-safety-buffer-changed-to-ms = Microphone safety buffer changed to { $milliseconds } ms
bridge-could-not-submit-virtual-device-input = Could not submit virtual device input
# Parameters: $error
bridge-could-not-read-bluetooth-controller = Could not read Bluetooth controller: { $error }
# Parameters: $flags, $header, $packets, $samples, $frames
bridge-microphone-audio-flags-header-opus-packets-decoded = Microphone: audio flags={ $flags } header={ $header } Opus packets={ $packets } decoded samples={ $samples } USB frames={ $frames }
# Parameters: $full, $simple, $gyro, $callbacks, $rumble, $haptics, $audio, $flags, $motor_left, $motor_right, $outputs, $failures
bridge-diagnostics-full-input-simple-input-gyro-game = Diagnostics: full input={ $full } simple input={ $simple } gyro={ $gyro }; game feedback={ $callbacks } nonzero rumble={ $rumble } native haptics={ $haptics } audio callbacks={ $audio }; flags={ $flags } motors=({ $motor_left }, { $motor_right }) Bluetooth writes={ $outputs } write failures={ $failures }
# Parameters: $bytes, $rejected, $nonzero, $failures
bridge-audio-batches-max-b-rejected-nonzero-encoding = Audio batches: max={ $bytes } B rejected={ $rejected } nonzero={ $nonzero } encoding failures={ $failures }

## config

# Parameters: $error
config-could-not-read-configuration = Could not read configuration: { $error }
# Parameters: $line
config-configuration-line-is-missing = Configuration line { $line } is missing =
# Parameters: $key, $value
config-invalid-configuration = Invalid configuration { $key }: { $value }
# Parameters: $key
config-configuration-must-be-0-1-or-true = Configuration { $key } must be 0/1 or true/false
config-configuration-values-must-not-contain-line-breaks = Configuration values must not contain line breaks or NUL characters.
# Parameters: $error
config-could-not-save-configuration = Could not save configuration: { $error }
config-dsx-udp-automatic-dsx-launch-and-xbox = DSX UDP, automatic DSX launch, and Xbox/DS4 identities are not supported. Disable these options.
config-local-bridging-requires-auto-attach-true = Local bridging requires auto_attach=true.

## dependencies

# Parameters: $usbip, $hidhide, $viiper, $opus, $path
dependencies-usb-ip-win2-ude-hidhide-libviiper-opus = USB/IP Win2 UDE: { $usbip }
    HidHide: { $hidhide }
    libVIIPER: { $viiper }
    Opus: { $opus }
    Configuration: { $path }
dependencies-service-and-attachment-tool-available = Service and attachment tool available
dependencies-service-running-usbip-exe-is-missing = Driver running; usbip.exe not found. Choose its directory.
dependencies-not-running-or-not-installed-install-reboot = Driver not running. Install if missing; reboot if prompted.
dependencies-enabled-hiding-the-physical-controller = Enabled; hiding the physical controller
dependencies-accessible = Accessible
dependencies-not-accessible-check-driver-and-permissions = Driver inaccessible. Check installation, reboot and permissions.
dependencies-available = Available
dependencies-matching-dll-missing-place-it-beside-opends = Matching DLL missing. Place it beside opends.exe.
dependencies-libopus-0-dll-missing-audio-and-native = libopus-0.dll missing; audio and native haptics unavailable
# Parameters: $error
dependencies-could-not-read-installation-result = Could not read installation result: { $error }
dependencies-unsupported-driver-component = Unsupported driver component
dependencies-could-not-locate-the-application-directory = Could not locate the application directory
dependencies-scripts-install-drivers-ps1-is-missing-extract = scripts/Install-Drivers.ps1 is missing. Extract the complete release package again.
dependencies-could-not-locate-the-installation-log-directory = Could not locate the installation log directory
# Parameters: $error
dependencies-could-not-start-installer = Could not start installer: { $error }

## gui

# Parameters: $milliseconds, $samples
gui-ms-samples = { $milliseconds } ms · { $samples } samples
gui-start-bridge = Start bridge
gui-stop = Stop
gui-save-settings = Save settings
gui-refresh-devices = Refresh devices
gui-microphone-test = Microphone test…
gui-restore-controller-visibility = Restore controller visibility
gui-minimize-to-tray = Minimize to tray
gui-exit-opends = Exit OpenDS
gui-start-bridging-when-opends-opens = Start bridging when OpenDS opens
gui-run-in-the-tray-at-windows-sign = Run in the tray at Windows sign-in
gui-reconnect-automatically = Reconnect automatically
gui-hide-the-physical-controller-to-avoid-duplicate = Hide the physical controller to avoid duplicate input
gui-forward-game-audio = Forward game audio
gui-forward-native-stereo-haptics = Forward native stereo haptics
gui-forward-controller-microphone = Forward controller microphone
gui-ask = Ask
gui-exit-application = Exit application
gui-auto-select-bluetooth-dualsense = Auto-select Bluetooth DualSense
gui-automatic-game-setting-headphone-connection = Automatic (game setting / headphone connection)
gui-controller-speaker = Controller speaker
gui-headphones = Headphones
gui-enable-hidhide-device-hiding = Enable HidHide device hiding
gui-add-application = Add application
gui-remove-selected = Remove selected
gui-allow-opends-access = Allow OpenDS access
gui-check-again = Check again
gui-install-usb-ip-win2 = Install USB/IP Win2
gui-install-hidhide = Install HidHide
gui-overview = Overview
gui-live-monitor = Live monitor
gui-connection-feedback = Connection & feedback
gui-device-hiding = Device hiding
gui-diagnostics-logs = Diagnostics & logs
gui-stop-bridge = Stop bridge
gui-open-settings = Open settings
gui-stop-bridging-to-download-and-open-the = Stop bridging to download and open the official USB/IP Win2 installer
gui-stop-bridging-to-download-and-open-the-2 = Stop bridging to download and open the official HidHide installer
gui-installation-requires-administrator-approval-dependencies-will-be = Directories are saved immediately. HidHide uses its installed driver directly; its configuration tool is optional. Installing drivers requires administrator approval.
gui-first-install-the-missing-drivers-on-this = Some dependencies are not ready. Review the status above. If already installed, select the installation directory; install drivers only when missing.
gui-dualsense-control-center = DualSense control center
gui-bluetooth-connection-usb-experience = Bluetooth connection · USB experience
gui-connection-status-and-feedback-at-a-glance = Connection status and feedback at a glance.
gui-view-controller-input-and-audio-start-capture = View controller input and audio. Start capture manually.
gui-choose-your-controller-and-adjust-audio-haptics = Choose your controller and adjust audio, haptics and connection settings.
gui-let-games-see-only-the-virtual-controller = Let games see only the virtual controller.
gui-check-dependencies-and-review-connection-and-recovery = Check dependencies and review connection and recovery logs.
gui-native-feedback-wireless-freedom = Native feedback. Wireless freedom.
gui-dualsense-bluetooth-local-virtual-usb-controller = DualSense Bluetooth → local virtual USB controller
gui-input-reports = Input reports
gui-output-feedback = Output feedback
gui-audio-blocks = Audio blocks
gui-recent-activity = Recent activity
gui-ready-start-bridging-to-see-connection-and = Ready. Start bridging to see connection and recovery events here.
gui-on-close = On close
gui-physical-controller = Physical controller
gui-audio-output = Audio output
gui-speaker-volume-0-255 = Speaker volume · 0–255
gui-haptics-gain-10-20 = Haptics gain · 10–20
# Parameters: $gain
gui-microphone-db-0-50 = Microphone +{ $gain } dB · 0–50
gui-local-service-port = Local service port
gui-speaker-haptics-buffer-blocks = Speaker / haptics buffer · blocks
gui-microphone-safety-buffer-ms = Microphone safety buffer · ms
gui-application-whitelist = Application whitelist
gui-listed-applications-can-still-see-the-physical = Listed applications can still see the physical controller. Games usually do not need to be added.
gui-stopping-the-bridge-restores-the-hiding-settings = Stopping the bridge restores the hiding settings changed by OpenDS.
gui-application-log = Application log
gui-port-must-be-1-65535 = Port must be 1–65535
gui-buffer-must-be-1-128-blocks = Buffer must be 1–128 blocks
gui-speaker-volume-must-be-0-255 = Speaker volume must be 0–255
gui-haptics-gain-must-be-10-20 = Haptics gain must be 10–20
gui-port-buffer-or-haptics-gain-is-out = Port, buffer or haptics gain is out of range
gui-driver-installation-is-in-progress-wait-for = Driver installation is in progress. Wait for it to finish before starting the bridge.
gui-saved-controller-currently-disconnected = Saved controller (currently disconnected)
gui-another-driver-installation-is-in-progress-wait = Another driver installation is in progress. Wait for it to finish.
gui-stop-bridging-before-installing-drivers = Stop bridging before installing drivers.
# Parameters: $component
gui-preparing-complete-the-installation-in-the-official = Preparing { $component }. Complete the installation in the official wizard.
# Parameters: $message, $path
gui-installation-log = { $message } Installation log: { $path }
# Parameters: $component
gui-installed-dependencies-are-ready = { $component } installed; dependencies are ready.
# Parameters: $component
gui-wizard-finished-but-dependencies-are-not-ready = { $component } wizard finished, but dependencies are not ready. Reboot if prompted, then check again.
# Parameters: $component
gui-installation-failed-or-was-cancelled-see-the = { $component } installation failed or was cancelled. See the log.
gui-stopping-and-restoring-devices = Stopping and restoring devices…
gui-connected-forwarding = ● Connected · forwarding
gui-bridge-started-waiting-for-controller = ● Bridge started · waiting for controller…
# Parameters: $error
gui-bridge-stopped = ○ Bridge stopped · { $error }
gui-bridge-not-started = ○ Bridge not started
gui-bridging-is-running-stop-it-before-installing = Stop bridging before changing dependency directories or installing drivers.
gui-ready-start-bridging-to-see-connection-feedback = Ready.
    Start bridging to see connection, feedback and recovery events here.
gui-some-dependencies-are-missing-or-inaccessible-open = Some dependencies are missing or inaccessible. Open Diagnostics to check.
gui-dependencies-are-ready = Dependencies are ready
gui-settings-saved-applied-the-next-time-bridging = Settings saved; applied the next time bridging starts
gui-select-a-whitelist-entry-first = Select a whitelist entry first
gui-could-not-change-hidhide-state = Could not change HidHide state
gui-opends-ui-test-simulated-signals = OpenDS · UI test (simulated signals)
gui-waiting-for-controller = waiting for controller
gui-tray-startup-did-not-display-the-waiting = Tray startup did not display the waiting state
gui-state-unavailable = State unavailable
gui-ui-smoke-error = UI smoke error
gui-overview-did-not-display-the-bridge-error = Overview did not display the bridge error
gui-opening-from-tray-did-not-refresh-the = Opening from tray did not refresh the status
gui-main-window-is-missing-its-minimize-button = Main window is missing its minimize button
gui-minimizing-did-not-preserve-the-taskbar-window = Minimizing did not preserve the taskbar window
gui-minimizing-to-tray-did-not-hide-the = Minimizing to tray did not hide the window or incorrectly requested exit
gui-restoring-from-tray-did-not-restore-the = Restoring from tray did not restore the minimized window
gui-cancelling-close-changed-the-window-or-exit = Cancelling close changed the window or exit state
gui-close-preference-was-not-persisted-or-synchronized = Close preference was not persisted or synchronized with settings
gui-remembered-tray-action-did-not-take-effect = Remembered tray action did not take effect
gui-could-not-restore-the-close-confirmation = Could not restore the close confirmation
gui-window-or-taskbar-icon-was-not-loaded = Window or taskbar icon was not loaded
gui-could-not-create-the-ui-status-timer = Could not create the UI status timer

## gui close

gui-close-close-the-opends-window = Close the OpenDS window?
gui-close-minimizing-to-the-tray-keeps-bridging-and = Minimizing to the tray keeps bridging and capture running.
    Exiting stops bridging and file recording, and restores device visibility.
gui-close-remember-my-choice = Remember my choice
# Parameters: $code
gui-close-could-not-open-close-options-0x-use = Could not open close options (0x{ $code }). Use the tray menu or sidebar buttons.
gui-close-close-action-or-remember-choice-test-failed = Close action or remember-choice test failed
gui-close-close-cancellation-test-failed = Close cancellation test failed

## hid

hid-hid-closed = HID closed
hid-hid-disconnected = HID disconnected
hid-hid-not-opened-for-writing = HID not opened for writing
hid-invalid-hid-output-report-length = Invalid HID output report length
hid-hid-write-timed-out = HID write timed out
hid-incomplete-hid-write = Incomplete HID write
hid-invalid-feature-report-length = Invalid feature report length
hid-feature-report-read-timed-out = Feature report read timed out
hid-invalid-feature-report-response-length = Invalid feature report response length

## hidhide

hidhide-duplicate-active-key-in-hidhide-recovery-journal = Duplicate active key in HidHide recovery journal
hidhide-empty-hidhide-recovery-journal = Empty HidHide recovery journal
hidhide-invalid-hidhide-recovery-journal-version = Invalid HidHide recovery journal version
hidhide-corrupt-or-unsupported-hidhide-recovery-journal-file = Corrupt or unsupported HidHide recovery journal; file retained
hidhide-localappdata-is-not-set-cannot-locate-the = LOCALAPPDATA is not set; cannot locate the HidHide recovery journal
# Parameters: $error
hidhide-could-not-read-hidhide-recovery-journal = Could not read HidHide recovery journal: { $error }
hidhide-could-not-read-hidhide-device-blacklist-recovery = Could not read HidHide device blacklist; recovery journal retained
hidhide-could-not-read-hidhide-application-whitelist-recovery = Could not read HidHide application whitelist; recovery journal retained
hidhide-could-not-read-hidhide-state-recovery-journal = Could not read HidHide state; recovery journal retained
hidhide-could-not-restore-hidhide-state-recovery-journal = Could not restore HidHide state; recovery journal retained
hidhide-hidhide-recovery-verification-failed-recovery-journal-retained = HidHide recovery verification failed; recovery journal retained
hidhide-hidhide-unavailable-cannot-restore-yet-recovery-journal = HidHide unavailable; cannot restore yet. Recovery journal retained.
hidhide-could-not-read-the-recovery-path = Could not read the recovery path
# Parameters: $error
hidhide-hidhide-restored-but-its-recovery-journal-could = HidHide restored, but its recovery journal could not be deleted: { $error }
hidhide-localappdata-is-not-set-refusing-to-hide = LOCALAPPDATA is not set; refusing to hide devices without a recovery journal
hidhide-unfinished-hidhide-recovery-found-run-restore-first = Unfinished HidHide recovery found; run restore first
hidhide-hidhide-driver-unavailable = HidHide driver unavailable
hidhide-could-not-read-hidhide-state = Could not read HidHide state
hidhide-could-not-read-hidhide-device-blacklist = Could not read HidHide device blacklist
hidhide-could-not-read-hidhide-application-whitelist = Could not read HidHide application whitelist
# Parameters: $error
hidhide-could-not-save-recovery-journal-driver-unchanged = Could not save recovery journal; driver unchanged: { $error }
# Parameters: $result
hidhide-could-not-hide-the-physical-controller = Could not hide the physical controller: { $result }
hidhide-rolled-back = Rolled back
hidhide-invalid-characters-in-application-path = Invalid characters in application path
# Parameters: $error
hidhide-application-path-unavailable = Application path unavailable: { $error }
hidhide-select-an-application-on-a-local-disk = Select an application on a local disk; UNC paths are not supported
# Parameters: $error
hidhide-could-not-convert-application-path = Could not convert application path: { $error }
hidhide-invalid-application-path = Invalid application path
hidhide-could-not-read-application-whitelist = Could not read application whitelist
hidhide-could-not-write-or-verify-application-whitelist = Could not write or verify application whitelist

## main

main-use-startup-enable-disable-status = Use startup enable / disable / status
# Parameters: $state
main-windows-sign-in-startup = Windows sign-in startup: { $state }
main-enabled = Enabled
main-disabled = Disabled
main-opends-opends-gui-settings-and-tray-default = OpenDS
    { "  " }opends gui                 Settings and tray (default)
    { "  " }opends bridge [seconds]    Run bridge; omit or use 0 to run continuously, Ctrl+C to stop
    { "  " }opends restore             Recover an unfinished HidHide session
    { "  " }opends doctor              Check dependencies (read only)
    { "  " }opends list                List physical Bluetooth DualSense devices
    { "  " }opends probe               Read one input report (no device writes)
    { "  " }opends language check FILE Validate a Fluent language file
main-gui-creation-icons-event-loop-and-clean = GUI creation, icons, event loop and clean shutdown passed
main-no-input-received-within-500-ms = No input received within 500 ms
# Parameters: $command
main-unknown-command-use-opends-help = Unknown command: { $command }. Use opends --help.
main-hidhide-recovery-succeeded-and-was-verified = HidHide recovery succeeded and was verified
main-no-hidhide-session-needs-recovery = No HidHide session needs recovery
main-seconds-must-be-a-non-negative-integer = Seconds must be a non-negative integer
main-console-state-lock-is-poisoned = Console state lock is poisoned

## monitor

monitor-monitor-cache-would-exceed-32-mib-reduce = Monitor cache would exceed 32 MiB; reduce the sampling rate
monitor-monitor-cache-unavailable = Monitor cache unavailable
monitor-could-not-start-monitor-worker = Could not start monitor worker
monitor-start-capture-before-recording-to-a-file = Start capture before recording to a file
monitor-recording-would-exceed-the-monitor-cache-limit = Recording would exceed the monitor cache limit; reduce the sampling rate

## monitor recording

monitor-recording-invalid-drive-directory = Invalid drive directory
# Parameters: $error
monitor-recording-could-not-check-free-space-on-the = Could not check free space on the target disk: { $error }
# Parameters: $megabytes
monitor-recording-only-mib-remain-on-the-target-disk = Only { $megabytes } MiB remain on the target disk. Recording stopped to reserve 512 MiB; bridging and capture continue.
monitor-recording-recording-state-unavailable = Recording state unavailable
monitor-recording-recording-is-active-or-still-saving-try = Recording is active or still saving. Try again later.
# Parameters: $error
monitor-recording-csv-recording-failed = CSV recording failed: { $error }
monitor-recording-recording-queue-full-or-capture-delayed-too = Recording queue full or capture delayed too long. Recording stopped; received data is being saved. Capture and bridging continue.
# Parameters: $path, $error
monitor-recording-could-not-write-to-check-write-permissions = Could not write to { $path }: { $error }. Check write permissions for the selected location.

## monitor view

monitor-view-csv-files-csv-u0000-csv-u0000 = CSV files (*.csv)
monitor-view-choose-where-to-save-the-csv-recording = Choose where to save the CSV recording
# Parameters: $code
monitor-view-could-not-choose-save-location-0x = Could not choose save location: 0x{ $code }
# Parameters: $code
monitor-view-drawing-failed-0x = Drawing failed: 0x{ $code }
monitor-view-muted = Muted
monitor-view-touch-l = Touch L
monitor-view-touch-r = Touch R
monitor-view-could-not-create-monitor-window = Could not create monitor window
monitor-view-start-capture = Start capture
monitor-view-clear = Clear
monitor-view-source = Source
monitor-view-forwarded = Forwarded
monitor-view-separate-window = Separate window
monitor-view-always-on-top = Always on top
monitor-view-sample = Sample
monitor-view-refresh = Refresh
monitor-view-history = History
monitor-view-waiting-for-manual-start = Waiting for manual start
monitor-view-start-file-recording = Start file recording
# Parameters: $seconds
monitor-view-seconds = { $seconds } seconds
monitor-view-could-not-create-drawing-canvas = Could not create drawing canvas
# Parameters: $error
monitor-view-could-not-save-monitor-preferences = Could not save monitor preferences: { $error }
monitor-view-opends-live-monitor = OpenDS · Live monitor
monitor-view-could-not-create-separate-window = Could not create separate window
monitor-view-dock-window = Dock window
monitor-view-stop-capture = Stop capture
monitor-view-checking = Checking…
monitor-view-stop-file-recording = Stop file recording
monitor-view-saving = Saving…
monitor-view-csv-recording-failed = { " " }· CSV recording failed
monitor-view-checking-save-location = { " " }· Checking save location…
monitor-view-recording = Recording
monitor-view-saving-2 = Saving
monitor-view-saved = Saved
# Parameters: $count
monitor-view-dropped = { " " }/ dropped { $count }
# Parameters: $capture, $connection, $recording, $drops, $errors, $resets, $cache
monitor-view-dropped-decode-errors-stream-resets-cache-mib = { $capture } · { $connection }{ $recording } · dropped { $drops } · decode errors { $errors } · stream resets { $resets } · cache { $cache } MiB
monitor-view-capturing = Capturing
monitor-view-stopped-history-retained = Stopped / history retained
monitor-view-connected = Connected
monitor-view-waiting-for-controller = Waiting for controller
monitor-view-capture-stopped-last-input-retained = Capture stopped · last input retained
monitor-view-disconnected-waiting-to-reconnect = Disconnected · waiting to reconnect
monitor-view-waiting-for-new-input-reports = Waiting for new input reports
monitor-view-live-input-buttons-sticks-and-touchpad = Live input · buttons, sticks and touchpad
# Parameters: $lx, $ly, $rx, $ry
monitor-view-left-stick-x-y-right-stick-x = Left stick   X { $lx }   Y { $ly }
    Right stick  X { $rx }   Y { $ry }
monitor-view-blue-source-orange-dots-forwarded = Blue: source / orange dots: forwarded
monitor-view-speaker-front-l-r = Speaker · front L / R
monitor-view-haptics-rear-l-r = Haptics · rear L / R
monitor-view-microphone-mono = Microphone · mono
monitor-view-microphone-disabled = Microphone disabled
monitor-view-no-audio-signal-selected = No audio signal selected
monitor-view-no-data-received-yet = No data received yet
monitor-view-data-interrupted-500-ms = Data interrupted > 500 ms
monitor-view-silent-valid-zero-amplitude-pcm-received = Silent · valid zero-amplitude PCM received
# Parameters: $route, $volume
monitor-view-volume = { $route } · volume { $volume }
monitor-view-headphone-route = Headphone route
# Parameters: $gain, $limiter
monitor-view-source-original-level-forwarded-db = Source: original level · forwarded: +{ $gain } dB{ $limiter }
monitor-view-limited = { " " }/ limited
monitor-view-blue-source-orange-forwarded-fixed-100 = Blue: source / orange: forwarded · fixed ±100%
monitor-view-opends-file-recording-stopped = OpenDS · File recording stopped

## persistence

persistence-opends-is-already-running-use-its-existing = OpenDS is already running. Use its existing tray icon.
# Parameters: $error
persistence-could-not-create-process-lock = Could not create process lock: { $error }

## startup

# Parameters: $error
startup-windows-sign-in-startup-setup-failed = Windows sign-in startup setup failed: { $error }
startup-the-executable-path-is-not-valid-unicode = The executable path is not valid Unicode
startup-the-executable-path-contains-invalid-characters = The executable path contains invalid characters
startup-the-sign-in-startup-command-exceeds-the = The sign-in startup command exceeds the Windows 260-character limit. Use a shorter installation path.

# Appearance and shared dialogs
appearance-label = Appearance
appearance-system = Follow system
appearance-light = Light
appearance-dark = Dark
dialog-ok = &OK
dialog-yes = &Yes
dialog-no = &No

dependency-directory-invalid = The selected directory does not contain { $executable }: { $path }. Select the installed program directory.
dependency-usbip-custom-invalid = Driver running; selected directory has no usbip.exe. Choose again.
dependency-choose-directory = Browse…
dependency-use-auto = Auto
dependency-open-hidhide = Open HidHide settings
dependency-location = { $component } location ({ $mode })
dependency-mode-auto = automatic
dependency-mode-manual = manual
dependency-tool-not-found = Tool not found. Select an installation directory if installed.
dependency-selected-missing = { $executable } not found: { $path }
dependency-stop-before-changing = Stop bridging and wait for any installation to finish before changing directories.
dependency-directory-title = Choose the installation directory containing { $executable }
dependency-directory-encoding = The directory cannot be saved as UTF-8.
dependency-hidhide-not-found = HidHideClient.exe not found. Select the HidHide installation directory. The configuration tool is optional for bridging.
dependency-installer-finished = { $component } installer finished. Review dependency status above; select the installed directory or reboot if prompted.

# Input history
command-title = Input history
command-overlay = Overlay
command-lock = Lock
command-settings = Input history settings
command-clear = Clear
command-setting-value = Value
command-setting-color-value = Value (colors: #RRGGBB)
command-apply = Apply
command-color = Choose color
command-defaults = Restore defaults
command-settings-hint = Select a setting, edit its value, then Apply. Source: text / vector icon / hidden. Auto colors adapt black and white; turn off for separate light/dark palettes.
command-preview = Preview
command-invalid = Invalid setting
command-saved = Saved
command-connected = Connected
command-disconnected = Disconnected
command-gap = Input gap
command-boundary = Settings changed
command-source-text = Text
command-source-icon = Icon
command-source-hidden = Hidden
command-follow = Follow application
command-light = Light
command-dark = Dark
command-no = Off
command-yes = On
command-enabled = Record input history
command-right = Include right stick
command-source = Source labels
command-rows = Visible rows in monitor
command-overlay-rows = Visible rows in overlay
command-auto-colors = Automatic theme colors
command-theme = Overlay theme
command-bg-alpha = Background opacity (%)
command-fg-alpha = Content opacity (%)
command-topmost = Always on top
command-x = Window horizontal position
command-y = Window vertical position
command-width = Window width
command-height = Window height
command-neutral = Neutral color
command-dpad = D-pad color
command-button = Button color
command-connection = Connection event color
command-deadzone = Dead zone
command-medium = Medium
command-full = Full
command-gentle = Gentle
command-background = Background
command-text = Text
command-dpad-prefix = D-pad
command-empty = Waiting for input history
command-reset = Restore overlay
command-menu-show = Show / hide input overlay
command-menu-lock = Lock / unlock input overlay
command-short-settings = Settings
command-short-reset = Restore
command-file-failed = Input history file recording failed
command-unlock = Unlock

command-color-preview = Color preview

command-content-size = Content size (50–500%)

command-display-min-ms = Hide states shorter than (ms, 0=off)
command-record-min-ms = Skip states in command file below (ms, 0=off)

command-overlay-auto-rows = Fit overlay rows to window
command-separators = Show record separators
command-separator-alpha = Separator opacity (%)
command-separator-style = Separator end style
command-separator-color = Separator color
command-separator-flat = Flat
command-separator-fade = Fade
command-separator-taper = Taper

command-ignore-strength = Ignore strength
command-unified-color = Unified color

command-group-record = Recording and display
command-group-window = Window and layout
command-group-strength = Strength and dead zones
command-group-separator = Separators
command-group-colors = Colors and theme palettes

command-distinguish-strength = Distinguish strength
