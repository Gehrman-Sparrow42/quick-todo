param([switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot
$localCargo = Join-Path $PSScriptRoot '.toolchain\cargo\bin\cargo.exe'
if (Test-Path -LiteralPath $localCargo) {
    $env:CARGO_HOME = Join-Path $PSScriptRoot '.toolchain\cargo'
    $env:RUSTUP_HOME = Join-Path $PSScriptRoot '.toolchain\rustup'
    $env:PATH = "$(Split-Path -Parent $localCargo);$env:PATH"
    $assemblerTools = Join-Path $PSScriptRoot '.toolchain\w64devkit\bin'
    if (Test-Path -LiteralPath $assemblerTools -PathType Container) {
        $env:PATH = "$assemblerTools;$env:PATH"
        $env:CARGO_ENCODED_RUSTFLAGS = (@('-C', "dlltool=$(Join-Path $assemblerTools 'dlltool.exe')", '-C', 'link-self-contained=yes') -join [char]0x1f)
    }
    $gnuTools = Join-Path $env:RUSTUP_HOME 'toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained'
    if (Test-Path -LiteralPath (Join-Path $gnuTools 'dlltool.exe')) {
        $env:PATH = "$env:PATH;$gnuTools"
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $assemblerTools 'gcc.exe'
    }
    $cargo = $localCargo
} else {
    $cargo = (Get-Command cargo -ErrorAction Stop).Source
}
& $cargo fmt --check
if ($LASTEXITCODE -ne 0) { throw 'Rust formatting check failed.' }
& $cargo clippy --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'Rust lint check failed.' }
& $cargo test
if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
& $cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed.' }
if (-not $CheckOnly) {
    $running = Get-Process -Name 'quick-todo' -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq (Join-Path $PSScriptRoot 'quick-todo.exe') }
    if ($running) { throw 'Quit Quick Todo from its tray menu before replacing the executable. Your build is ready in target\release.' }
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'target\release\quick-todo.exe') -Destination (Join-Path $PSScriptRoot 'quick-todo.exe')
    Write-Output 'Built and deployed quick-todo.exe.'
}
