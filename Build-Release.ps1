#Requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$SigningKeyPath,
    [string]$Notes = 'Duck Workshop update',
    [string]$CertificateThumbprint,
    [string]$SignToolPath = 'signtool.exe'
)
$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    foreach ($arguments in @(@('fmt','--all','--','--check'), @('test','--all-targets','--locked'), @('clippy','--all-targets','--locked','--','-D','warnings'), @('build','--release','--locked'))) {
        & cargo @arguments
        if ($LASTEXITCODE -ne 0) { throw "cargo $arguments failed." }
    }
    $version = [regex]::Match((Get-Content -LiteralPath Cargo.toml -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    $release = Join-Path $PSScriptRoot 'release'
    $package = Join-Path $release "Duck-Workshop-$version-Windows-x64"
    New-Item -ItemType Directory -Path $package -Force | Out-Null
    $application = Join-Path $release 'Duck-Workshop.exe'
    Copy-Item -LiteralPath target/release/duck-workshop.exe -Destination $application -Force
    if ($CertificateThumbprint) {
        & $SignToolPath sign /sha1 $CertificateThumbprint /fd SHA256 /tr https://timestamp.digicert.com /td SHA256 $application
        if ($LASTEXITCODE -ne 0) { throw 'Windows publisher signing failed.' }
        & $SignToolPath verify /pa $application
        if ($LASTEXITCODE -ne 0) { throw 'Windows publisher signature verification failed.' }
    }
    & ./scripts/Sign-Update.ps1 -ApplicationPath $application -SigningKeyPath $SigningKeyPath -Version $version -Notes $Notes
    Copy-Item -LiteralPath $application -Destination $package -Force
    foreach ($name in @('README.md','LICENSE','RELEASING.md','CHANGELOG.md','SECURITY.md','CONTRIBUTING.md')) { Copy-Item -LiteralPath $name -Destination $package -Force }
    Copy-Item -LiteralPath docs -Destination $package -Recurse -Force
    Copy-Item -LiteralPath assets -Destination $package -Recurse -Force
    $licenses = Join-Path $package 'licenses'
    New-Item -ItemType Directory -Path $licenses -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Installer/LICENSE') -Destination (Join-Path $licenses 'DuckWorkshopSetup-LICENSE.txt') -Force
    $setup = Join-Path $release 'DuckWorkshopSetup.exe'
    & (Join-Path $PSScriptRoot 'scripts/Build-Setup.ps1') -ApplicationFolder $package -OutputPath $setup
    Get-ChildItem -LiteralPath $package -Recurse -File | Where-Object LastWriteTime -lt ([DateTime]::new(1980,1,1)) | ForEach-Object { $_.LastWriteTime = [DateTime]::new(1980,1,1) }
    $zip = Join-Path $release "Duck-Workshop-$version-Windows-x64.zip"
    Compress-Archive -LiteralPath $package -DestinationPath $zip -Force
    $sums = foreach ($item in @($application, $setup, $zip, (Join-Path $release 'latest.json'))) { $hash = Get-FileHash -LiteralPath $item -Algorithm SHA256; "$($hash.Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($item))" }
    [IO.File]::WriteAllLines((Join-Path $release 'SHA256SUMS.txt'), $sums)
    Write-Output "Release prepared in $release. Upload all five release assets after reviewing them."
} finally { Pop-Location }
