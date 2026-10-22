[CmdletBinding()]
param([string]$Source = (Join-Path $PSScriptRoot '../vendor/viiper-source'))
$ErrorActionPreference = 'Stop'
$sourcePath = (Resolve-Path -LiteralPath $Source).Path
$utf8 = New-Object Text.UTF8Encoding($false, $true)
$sha = [Security.Cryptography.SHA256]::Create()
try {
    $files = @{}
    foreach ($file in (Get-ChildItem -LiteralPath $sourcePath -Recurse -File -Force)) {
        $relative = $file.FullName.Substring($sourcePath.Length + 1).Replace('\','/')
        if ($relative -eq 'OPENDS-PATCH.md' -or $relative.StartsWith('.git/')) { continue }
        $files[$relative] = $file.FullName
    }
    [string[]]$paths = @($files.Keys)
    [Array]::Sort($paths, [StringComparer]::Ordinal)
    $manifest = New-Object Text.StringBuilder
    foreach ($relative in $paths) {
        $bytes = [IO.File]::ReadAllBytes($files[$relative])
        if ($bytes -notcontains 0) {
            try { $bytes = $utf8.GetBytes($utf8.GetString($bytes).Replace("`r`n","`n")) }
            catch [Text.DecoderFallbackException] { } # Non-UTF-8 files are hashed as bytes.
        }
        $hash = ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-','').ToLowerInvariant()
        [void]$manifest.Append($hash).Append('  ').Append($relative).Append("`n")
    }
    ([BitConverter]::ToString($sha.ComputeHash($utf8.GetBytes($manifest.ToString())))).Replace('-','').ToLowerInvariant()
} finally { $sha.Dispose() }
