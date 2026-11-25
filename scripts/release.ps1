<#
  Makes a new version: sets the version in the three places it lives, commits, tags and pushes.
  GitHub Actions (.github/workflows/release.yml) then builds, signs and publishes the release,
  and every installed Ilha updates itself.

  Usage:  .\scripts\release.ps1 0.2.0
#>
param([Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

if (git status --porcelain) { throw 'Há mudanças sem commit. Faça o commit antes de lançar uma versão.' }
if ((git rev-parse --abbrev-ref HEAD) -ne 'main') { throw 'Lance versões a partir da branch main.' }
if (git tag --list "v$Version") { throw "A tag v$Version já existe." }

# package.json and tauri.conf.json
foreach ($file in 'package.json', 'src-tauri/tauri.conf.json') {
  $text = Get-Content $file -Raw
  $text = [regex]::Replace($text, '"version":\s*"[^"]+"', "`"version`": `"$Version`"", 1)
  Set-Content $file $text -NoNewline -Encoding utf8
}
# Cargo.toml (only the package's own version, the first one)
$cargo = Get-Content 'src-tauri/Cargo.toml' -Raw
$cargo = [regex]::Replace($cargo, '(?m)^version = "[^"]+"', "version = `"$Version`"", 1)
Set-Content 'src-tauri/Cargo.toml' $cargo -NoNewline -Encoding utf8
# Cargo.lock follows
Push-Location src-tauri; cargo update -p ilha --offline 2>$null; Pop-Location

if (-not (Select-String -Path CHANGELOG.md -Pattern "## \[$Version\]" -Quiet)) {
  Write-Warning "O CHANGELOG.md ainda não tem uma seção ## [$Version]. Escreva o que mudou e rode de novo."
  git checkout -- package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
  exit 1
}

git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "Versão $Version"
git tag -a "v$Version" -m "Ilha v$Version"
git push origin main
git push origin "v$Version"
Write-Host "Pronto: v$Version enviada. Acompanhe o build em https://github.com/josevictoremiliano/island/actions" -ForegroundColor Green
