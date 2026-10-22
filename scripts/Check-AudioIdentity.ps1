<#
Read-only snapshot of connected DualSense audio endpoints and speaker settings.
Run before and after a bridge restart with -CompareTo to detect identity churn.
Does not change audio defaults, registry values, or device configuration.
#>
[CmdletBinding()]
param(
    [string]$OutputPath,
    [string]$CompareTo
)
$ErrorActionPreference = 'Stop'
$endpointKeys = @{
    FullRange = '{1da5d803-d492-4edd-8c23-e0c0ffee7f0e},6'
    Physical = '{1da5d803-d492-4edd-8c23-e0c0ffee7f0e},3'
    Device = '{b3f8fa53-0004-438e-9003-51a46e139bfc},2'
}
$endpoints = @(Get-PnpDevice -PresentOnly -Class AudioEndpoint |
    Where-Object FriendlyName -Match 'DualSense' | ForEach-Object {
        $endpoint = $_
        $flow = if ($endpoint.InstanceId -match '\{0\.0\.0\.') { 'Render' } else { 'Capture' }
        $guidMatch = [regex]::Match($endpoint.InstanceId, '\{[0-9a-f-]+\}$', 'IgnoreCase')
        if (-not $guidMatch.Success) { throw "Invalid endpoint ID: $($endpoint.InstanceId)" }
        $propertyPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\$flow\$($guidMatch.Value)\Properties"
        $properties = Get-ItemProperty -LiteralPath $propertyPath
        [pscustomobject]@{
            Id = $endpoint.InstanceId
            Name = $endpoint.FriendlyName
            Flow = $flow
            AudioDevice = $properties.($endpointKeys.Device)
            FullRange = $properties.($endpointKeys.FullRange)
            Physical = $properties.($endpointKeys.Physical)
        }
    } | Sort-Object Id)
if (-not $endpoints.Count) { throw 'No connected DualSense audio endpoint found' }
$snapshot = [pscustomobject]@{
    Time = (Get-Date).ToString('o')
    Endpoints = $endpoints
}
$json = $snapshot | ConvertTo-Json -Depth 5
if ($OutputPath) { $json | Set-Content -LiteralPath $OutputPath -Encoding utf8 }
$json
if ($CompareTo) {
    $baseline = Get-Content -LiteralPath $CompareTo -Raw | ConvertFrom-Json
    $fields = @('Id', 'Name', 'Flow', 'AudioDevice', 'FullRange', 'Physical')
    $before = @($baseline.Endpoints | Sort-Object Id | Select-Object -Property $fields) | ConvertTo-Json -Depth 5 -Compress
    $after = @($endpoints | Select-Object -Property $fields) | ConvertTo-Json -Depth 5 -Compress
    if ($before -cne $after) { throw 'DualSense audio identity or speaker settings changed' }
    Write-Host 'PASS: endpoint IDs, names, audio devices and speaker settings unchanged'
}
