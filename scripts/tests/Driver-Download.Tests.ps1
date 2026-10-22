param([string]$Origin, [string]$FixtureProxy, [string]$BrokenProxy, [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1')
. (Join-Path $PSScriptRoot '../Driver-Download.ps1')
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
function Assert($Condition, [string]$Message) { if (!$Condition) { throw $Message } }
function Test-Asset($Item, [string]$Path) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -ine $Item.sha256) { throw 'SHA256 mismatch in test fixture' }
}
$bytes = [Text.Encoding]::UTF8.GetBytes('OpenDS local driver download fixture')
$sha = [Security.Cryptography.SHA256]::Create()
$hash = ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-','').ToLowerInvariant()
$sha.Dispose()
function New-Asset([string]$Name, [string]$Url) { return @{file=$Name;url=$Url;sha256=$hash} }
function Check-Saved($Asset) {
    $path = Join-Path $OutputDirectory $Asset.file
    Assert (Test-Path -LiteralPath $path) 'Downloaded file missing'
    Test-Asset $Asset $path
}

# Set poisoned defaults inside this child process only. Direct must bypass them.
$env:HTTP_PROXY = $BrokenProxy
$env:HTTPS_PROXY = $BrokenProxy
$env:ALL_PROXY = $BrokenProxy
$env:NO_PROXY = ''
[Net.WebRequest]::DefaultWebProxy = New-Object Net.WebProxy($BrokenProxy)
if ($PSVersionTable.PSVersion.Major -ge 7) {
    [Net.Http.HttpClient]::DefaultProxy = New-Object Net.WebProxy($BrokenProxy)
}
$script:SystemCalls = 0
function New-DriverSystemProxy { $script:SystemCalls++; throw 'System proxy resolution failure fixture' }
$asset = New-Asset 'explicit.exe' 'http://driver.opends.invalid/asset'
Save-DriverAsset $asset (Join-Path $OutputDirectory $asset.file) ($FixtureProxy -replace '^http://','')
Check-Saved $asset
Assert ($script:SystemCalls -eq 0) 'Explicit proxy did not take priority'

function New-DriverSystemProxy { $script:SystemCalls++; return (New-Object Net.WebProxy($FixtureProxy)) }
$asset = New-Asset 'system.exe' 'http://driver.opends.invalid/asset'
Save-DriverAsset $asset (Join-Path $OutputDirectory $asset.file) $BrokenProxy
Check-Saved $asset
Assert ($script:SystemCalls -eq 1) 'System proxy fallback not used'

function New-DriverSystemProxy { return (New-Object Net.WebProxy($BrokenProxy)) }
$asset = New-Asset 'direct.exe' ($Origin + '/asset')
Save-DriverAsset $asset (Join-Path $OutputDirectory $asset.file) ''
Check-Saved $asset

# A server that stops sending after its headers must respect the total timeout,
# especially on .NET Framework where stream cancellation alone is insufficient.
$timer = [Diagnostics.Stopwatch]::StartNew()
$timedOut = $false
try { Receive-DriverDownload -Uri ($Origin + '/stall') -OutFile (Join-Path $OutputDirectory 'timeout.tmp') -Route Direct -TimeoutSeconds 1 }
catch { $timedOut = $true }
Assert $timedOut 'Stalled body did not time out'
Assert ($timer.Elapsed.TotalSeconds -lt 4) 'Stalled body exceeded its deadline'

# No proxy at all is a valid network configuration.
$env:HTTP_PROXY = $null; $env:HTTPS_PROXY = $null; $env:ALL_PROXY = $null
function New-DriverSystemProxy { return (New-Object Net.WebProxy) }
$asset = New-Asset 'no-proxy.exe' ($Origin + '/asset')
Save-DriverAsset $asset (Join-Path $OutputDirectory $asset.file) ''
Check-Saved $asset

# A destination-aware system resolver is retained across redirects. A scraped
# fixed PAC address would fail this case: the first URL must be DIRECT and the
# redirect's .invalid host can only be served by the fixture proxy.
Add-Type -TypeDefinition @'
using System;
using System.Net;
public sealed class DestinationAwareTestProxy : IWebProxy {
    private readonly Uri address;
    public DestinationAwareTestProxy(string value) { address = new Uri(value); }
    public ICredentials Credentials { get; set; }
    public bool IsBypassed(Uri destination) { return destination.IsLoopback; }
    public Uri GetProxy(Uri destination) { return IsBypassed(destination) ? destination : address; }
}
'@
function New-DriverSystemProxy { return (New-Object DestinationAwareTestProxy($FixtureProxy)) }
$asset = New-Asset 'redirect.exe' ($Origin + '/redirect')
Save-DriverAsset $asset (Join-Path $OutputDirectory $asset.file) ''
Check-Saved $asset

function New-DriverSystemProxy { return (New-Object Net.WebProxy) }
foreach ($case in @('tampered','missing','truncated')) {
    $asset = New-Asset ($case + '.exe') ($Origin + '/' + $case)
    $failed = $false
    try { Save-DriverAsset $asset (Join-Path $OutputDirectory $asset.file) '' }
    catch {
        $failed = $true
        Assert ($_.Exception.Message.Contains('Download it manually: ' + $asset.url)) 'Missing manual fallback URL'
    }
    Assert $failed ('Bad download unexpectedly succeeded: ' + $case)
    Assert (!(Test-Path -LiteralPath (Join-Path $OutputDirectory $asset.file))) 'Bad download was promoted'
}
Assert (@(Get-ChildItem -LiteralPath $OutputDirectory -Filter '.partial-*' -Force).Count -eq 0) 'Partial download was not cleaned up'
Write-Host ('PASS: driver download network cases on PowerShell ' + $PSVersionTable.PSVersion)
