param(
    [string]$TodoPath,
    [switch]$StartAtLogin,
    [switch]$NoLaunch
)
$ErrorActionPreference = 'Stop'
$exePath = Join-Path $PSScriptRoot 'quick-todo.exe'
$configPath = Join-Path $PSScriptRoot 'config.txt'
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) {
    throw 'Build Quick Todo with build.ps1 before installing shortcuts.'
}
if ($TodoPath) {
    $todoFile = Get-Item -LiteralPath $TodoPath -ErrorAction Stop
    if ($todoFile.PSIsContainer -or $todoFile.Extension -ne '.md') {
        throw 'TodoPath must point to an existing Markdown file.'
    }
    [IO.File]::WriteAllText($configPath, "target=$($todoFile.FullName)`r`n", [Text.UTF8Encoding]::new($false))
} elseif (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) {
    throw 'Specify -TodoPath with the path of your existing Todo.md file.'
}
$shell = New-Object -ComObject WScript.Shell
$programsPath = [Environment]::GetFolderPath('Programs')
$menuShortcutPath = Join-Path $programsPath 'Quick Todo.lnk'
$menuShortcut = $shell.CreateShortcut($menuShortcutPath)
$menuShortcut.TargetPath = $exePath
$menuShortcut.WorkingDirectory = $PSScriptRoot
$menuShortcut.Description = 'Quick Todo - capture app ideas with Ctrl+Alt+T'
$menuShortcut.IconLocation = "$exePath,0"
$menuShortcut.Save()
Write-Output "Start menu shortcut: $menuShortcutPath"
if ($StartAtLogin) {
    $startupPath = [Environment]::GetFolderPath('Startup')
    $startupShortcutPath = Join-Path $startupPath 'Quick Todo.lnk'
    $startupShortcut = $shell.CreateShortcut($startupShortcutPath)
    $startupShortcut.TargetPath = $exePath
    $startupShortcut.Arguments = '--hidden'
    $startupShortcut.WorkingDirectory = $PSScriptRoot
    $startupShortcut.Description = 'Enable the Quick Todo global keyboard shortcut at sign-in'
    $startupShortcut.IconLocation = "$exePath,0"
    $startupShortcut.Save()
    Write-Output "Sign-in shortcut: $startupShortcutPath"
}
if (-not $NoLaunch) {
    Start-Process -FilePath $exePath -ArgumentList '--hidden' -WorkingDirectory $PSScriptRoot -WindowStyle Hidden
}
