param([switch]$Editor, [switch]$Test, [switch]$Render, [string]$GodotPath)
$ErrorActionPreference = 'Stop'
if (-not $GodotPath) {
    $GodotPath = Join-Path $PSScriptRoot 'tools\engine\Godot_v4.5.1-stable_win64_console.exe'
}
if (-not (Test-Path -LiteralPath $GodotPath)) {
    throw 'Godot 4.5.1 is missing. Run tools\setup.ps1 or pass -GodotPath.'
}
if (-not $Editor -and -not (Test-Path -LiteralPath (Join-Path $PSScriptRoot '.godot\imported'))) {
    & $GodotPath --headless --path $PSScriptRoot --editor --import --quit
    if ($LASTEXITCODE -ne 0) { throw 'Godot asset import failed.' }
}
$arguments = @('--path', $PSScriptRoot)
if ($Editor) { $arguments += '--editor' }
if ($Test) { $arguments += @('--headless', '--script', 'res://tests/test_port.gd') }
if ($Render) { $arguments += @('--resolution', '1280x720', '--script', 'res://tests/render_port.gd') }
if ($Test -or $Render) {
    $arguments += @('--', ('--data-dir=' + (Join-Path $PSScriptRoot 'test-output\isolated')))
}
& $GodotPath @arguments
exit $LASTEXITCODE
