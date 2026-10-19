#Requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$ApplicationFolder,
    [Parameter(Mandatory)][string]$OutputPath,
    [string]$RuntimeVersion = '10.0.13'
)
$ErrorActionPreference = 'Stop'
$sourceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$package = (Get-Item -LiteralPath $ApplicationFolder).FullName
$output = [IO.Path]::GetFullPath($OutputPath)
$version = [regex]::Match((Get-Content -LiteralPath (Join-Path $sourceRoot 'Cargo.toml') -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if (-not (Test-Path -LiteralPath (Join-Path $package 'Duck-Workshop.exe'))) { throw 'The application package is missing Duck-Workshop.exe.' }
$scratch = Join-Path $sourceRoot '.build'
New-Item -ItemType Directory -Path $scratch -Force | Out-Null
$env:DOTNET_CLI_HOME = Join-Path $scratch 'dotnet'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
$tests = Join-Path $sourceRoot 'Installer.Tests/Installer.Tests.csproj'
& dotnet run --project $tests --configuration Release -- (Join-Path $scratch ('install-tests-' + [Guid]::NewGuid().ToString('N')))
if ($LASTEXITCODE -ne 0) { throw 'Installer checks failed.' }
$hashes = [ordered]@{}
$files = Get-ChildItem -LiteralPath $package -File -Recurse
foreach ($file in $files) {
    $relative = [IO.Path]::GetRelativePath($package, $file.FullName).Replace('\','/')
    if ($relative -notmatch '^(Duck-Workshop\.exe|README\.md|LICENSE|RELEASING\.md|CHANGELOG\.md|SECURITY\.md|CONTRIBUTING\.md|docs/.+|assets/.+|licenses/.+)$' -or $relative -match '(?i)(\.private\.|\.(pfx|p12)$)') {
        throw "Unexpected file in application package: $relative"
    }
    $hashes[$relative] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
}
$payload = Join-Path $scratch 'setup-payload.zip'
$metadata = Join-Path $scratch 'setup-payload.json'
[IO.File]::WriteAllText($metadata, (@{Version=$version; Files=$hashes} | ConvertTo-Json -Depth 5))
if (Test-Path -LiteralPath $payload) { Remove-Item -LiteralPath $payload }
$archive = [IO.Compression.ZipFile]::Open($payload, [IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($relative in $hashes.Keys) {
        $entry = $archive.CreateEntry($relative, [IO.Compression.CompressionLevel]::Optimal)
        $input = [IO.File]::OpenRead((Join-Path $package $relative))
        $stream = $entry.Open()
        try { $input.CopyTo($stream) } finally { $input.Dispose(); $stream.Dispose() }
    }
} finally { $archive.Dispose() }
$publish = Join-Path $scratch 'setup-publish'
& dotnet publish (Join-Path $sourceRoot 'Installer/DuckWorkshop.Setup.csproj') -c Release -r win-x64 --self-contained true "-p:RuntimeFrameworkVersion=$RuntimeVersion" "-p:Version=$version" -p:DebugType=None -p:DebugSymbols=false -p:NuGetAudit=true -p:NuGetAuditMode=all '-warnaserror:NU1900,NU1901,NU1902,NU1903,NU1904,NU1905' "-p:PayloadPath=$payload" "-p:PayloadMetadataPath=$metadata" -o $publish
if ($LASTEXITCODE -ne 0) { throw 'Setup publish failed.' }
New-Item -ItemType Directory -Path (Split-Path -Parent $output) -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $publish 'DuckWorkshopSetup.exe') -Destination $output -Force
Write-Output "Setup prepared: $output"
