$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$destination = Join-Path $PSScriptRoot 'engine'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
$zip = Join-Path $destination 'portable.zip'
curl.exe --fail --location --connect-timeout 15 --max-time 180 --output $zip 'https://github.com/godotengine/godot-builds/releases/download/4.5.1-stable/Godot_v4.5.1-stable_win64.exe.zip'
if ($LASTEXITCODE -ne 0) { throw 'Godot download failed.' }
Expand-Archive -LiteralPath $zip -DestinationPath $destination -Force
Write-Output 'Portable Godot 4.5.1 is ready. No system installation was performed.'
