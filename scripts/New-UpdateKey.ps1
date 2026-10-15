#Requires -Version 7.0
param([Parameter(Mandatory)][string]$PrivateKeyPath, [switch]$ReplacePublicKey)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$private = [IO.Path]::GetFullPath($PrivateKeyPath)
if ($private.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Keep the private key outside the source repository.' }
if (Test-Path -LiteralPath $private) { throw 'Private key already exists; refusing to overwrite it.' }
if ((Test-Path -LiteralPath (Join-Path $root 'assets/UpdatePublicKey.pem')) -and -not $ReplacePublicKey) { throw 'This source already pins a release key. Use the matching private key. Intentional rotation requires -ReplacePublicKey and a manual app rollout.' }
$rsa = [Security.Cryptography.RSA]::Create(3072)
try {
    New-Item -ItemType Directory -Path (Split-Path -Parent $private) -Force | Out-Null
    [IO.File]::WriteAllText($private, $rsa.ExportPkcs8PrivateKeyPem())
    [IO.File]::WriteAllText((Join-Path $root 'assets/UpdatePublicKey.pem'), $rsa.ExportSubjectPublicKeyInfoPem())
    [IO.File]::WriteAllBytes((Join-Path $root 'assets/UpdatePublicKey.der'), $rsa.ExportRSAPublicKey())
    Write-Output 'Created signing key. Back up the private key securely; it is required for updates to existing copies.'
} finally { $rsa.Dispose() }
