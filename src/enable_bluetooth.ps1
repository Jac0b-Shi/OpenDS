$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$radio = [Windows.Devices.Radios.Radio,Windows.System.Devices,ContentType=WindowsRuntime]
$asTask = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
    $_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetParameters().Count -eq 1 -and
    $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
} | Select-Object -First 1
function Wait-RadioOperation($operation, $type) {
    $task = $asTask.MakeGenericMethod($type).Invoke($null, @($operation))
    $task.Wait()
    $task.Result
}
$radios = Wait-RadioOperation ($radio::GetRadiosAsync()) ([Collections.Generic.IReadOnlyList[Windows.Devices.Radios.Radio]])
$bluetooth = @($radios | Where-Object Kind -eq Bluetooth)
if (!$bluetooth.Count) { exit 1 }
foreach ($device in $bluetooth) {
    if ($device.State -eq 'On') { exit 0 }
}
$access = [Windows.Devices.Radios.RadioAccessStatus]
if ((Wait-RadioOperation ($radio::RequestAccessAsync()) $access) -ne 'Allowed') { exit 1 }
foreach ($device in $bluetooth) {
    if ((Wait-RadioOperation ($device.SetStateAsync('On')) $access) -eq 'Allowed') { exit 0 }
}
exit 1
