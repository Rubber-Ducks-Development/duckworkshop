#Requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$ApplicationPath,
    [Parameter(Mandatory)][string]$SigningKeyPath,
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [string]$DownloadUrl,
    [string]$Notes = 'Duck Workshop update',
    [string]$OutputPath = (Join-Path $PSScriptRoot '../release/latest.json')
)
$ErrorActionPreference = 'Stop'
if (-not $DownloadUrl) { $DownloadUrl = "https://github.com/Rubber-Ducks-Development/duckworkshop/releases/download/v$Version/Duck-Workshop.exe" }
$uri = [Uri]$DownloadUrl
if (-not $uri.IsAbsoluteUri -or $uri.Scheme -ne 'https' -or $uri.UserInfo -or $uri.Fragment) { throw 'Download URL must use HTTPS without credentials or fragments.' }
$file = Get-Item -LiteralPath $ApplicationPath
if ($file.Length -lt 1 -or $file.Length -gt 268435456) { throw 'Application must be between 1 byte and 256 MiB.' }
if ([Text.Encoding]::UTF8.GetByteCount($Notes) -gt 16384) { throw 'Release notes exceed the manifest limit.' }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$private = [IO.Path]::GetFullPath($SigningKeyPath)
$output = [IO.Path]::GetFullPath($OutputPath)
if ($private.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Keep the private key outside the source repository.' }
foreach ($protected in @($private, $file.FullName, (Join-Path $root 'assets/UpdatePublicKey.pem'), (Join-Path $root 'assets/UpdatePublicKey.der'))) {
    if ($output.Equals($protected, [StringComparison]::OrdinalIgnoreCase)) { throw 'Manifest output must not overwrite an executable or signing key.' }
}
$key = [Security.Cryptography.RSA]::Create()
$public = [Security.Cryptography.RSA]::Create()
try {
    $key.ImportFromPem([IO.File]::ReadAllText($private))
    $public.ImportFromPem([IO.File]::ReadAllText((Join-Path $root 'assets/UpdatePublicKey.pem')))
    $expected = [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $root 'assets/UpdatePublicKey.der')))
    if ([Convert]::ToBase64String($key.ExportRSAPublicKey()) -ne $expected -or [Convert]::ToBase64String($public.ExportRSAPublicKey()) -ne $expected) { throw 'Signing key and both pinned public keys must match.' }
    $release = [ordered]@{Product='duck-workshop'; Version=$Version; DownloadUrl=$uri.AbsoluteUri; Sha256=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash; Size=$file.Length; Notes=$Notes}
    $bytes = [Text.Encoding]::UTF8.GetBytes(($release | ConvertTo-Json -Compress))
    $signature = $key.SignData($bytes, [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pss)
    if (-not $public.VerifyData($bytes, $signature, [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pss)) { throw 'Signature self-check failed.' }
    $json = [ordered]@{Payload=[Convert]::ToBase64String($bytes); Signature=[Convert]::ToBase64String($signature)} | ConvertTo-Json
    if ([Text.Encoding]::UTF8.GetByteCount($json) -gt 131072) { throw 'Signed manifest is too large.' }
    New-Item -ItemType Directory -Path (Split-Path -Parent $output) -Force | Out-Null
    [IO.File]::WriteAllText($output, $json, [Text.UTF8Encoding]::new($false))
    Write-Output "Signed feed: $output"
} finally { $key.Dispose(); $public.Dispose() }
