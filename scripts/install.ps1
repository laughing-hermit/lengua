<#
Installs lengua from its release archive, on Windows: checks that this computer is the system
and CPU the archive is for (its TARGET), copies the plugin where Claude Code finds plugins, a
marketplace folder of lengua's own, which it adds to Claude Code when the claude command is
installed (and says the commands that do, when it isn't), then runs lengua-mcp doctor, which
says whether lengua has what it needs here. The archive ships every part lengua needs, the
solvers included, so nothing else is installed. Its exit code is doctor's: 0 when nothing is
wrong. It runs in Windows PowerShell 5.1 and in PowerShell 7.

usage: powershell -ExecutionPolicy Bypass -File .\install.ps1 [-Check] [-Prefix <folder>]
  -Check            only check this computer; install nothing
  -Prefix <folder>  make <folder> lengua's marketplace folder, the plugin in a lengua folder in
                    it, instead of lengua's own; what it replaces there must be lengua's (an
                    earlier install), or not there yet
#>
param(
    [switch]$Check,
    [string]$Prefix = ''
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2

# A target's name as people say it.
function Get-Described([string]$Name) {
    switch ($Name) {
        'linux-x86_64' { 'Linux on x86-64' }
        'linux-aarch64' { 'Linux on ARM64' }
        'macos-aarch64' { 'macOS on ARM64 (Apple silicon)' }
        'windows-x86_64' { 'Windows on x86-64' }
        'windows-aarch64' { 'Windows on ARM64' }
        default { $Name }
    }
}

# Runs a command quietly: whether it succeeded. A native command's error output must not stop
# the script, which Windows PowerShell 5.1 does when errors stop it.
function Test-Quietly([scriptblock]$Command) {
    $saved = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & $Command *> $null
        return ($LASTEXITCODE -eq 0)
    } catch {
        return $false
    } finally {
        $ErrorActionPreference = $saved
    }
}

$here = $PSScriptRoot
$targetFile = Join-Path $here 'TARGET'
if (-not (Test-Path -LiteralPath $targetFile)) {
    [Console]::Error.WriteLine('This folder has no TARGET file: install lengua from its release archive, unpacked.')
    exit 1
}
$target = (Get-Content -Raw -LiteralPath $targetFile).Trim()
$version = (Get-Content -Raw -LiteralPath (Join-Path $here 'plugin.json') | ConvertFrom-Json).version

# PowerShell 7 runs on Linux and macOS too, which no archive with install.ps1 is for. Windows
# PowerShell 5.1 has no $IsWindows, and runs on Windows alone.
if ((Test-Path -LiteralPath 'variable:IsWindows') -and -not $IsWindows) {
    [Console]::Error.WriteLine("This archive is lengua for $(Get-Described $target), and this computer isn't Windows: use the archive for its system and CPU (linux-x86_64, linux-aarch64 or macos-aarch64), and its install.sh.")
    exit 1
}

# The CPU Windows itself runs on, as the system's own environment names it: an x64 PowerShell
# is emulated on ARM64, where its process's PROCESSOR_ARCHITECTURE says AMD64.
$environment = 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment'
$native = (Get-ItemProperty -LiteralPath $environment).PROCESSOR_ARCHITECTURE
$cpu = switch ($native) {
    'AMD64' { 'x86_64' }
    'ARM64' { 'aarch64' }
    default { $native }
}
$computer = "windows-$cpu"
if ($computer -ne $target) {
    [Console]::Error.WriteLine("This archive is lengua for $(Get-Described $target), and this computer is $(Get-Described $computer): use the archive for $computer, if lengua is built for it (linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64 or windows-aarch64).")
    exit 1
}

if ($Check) {
    Write-Output "This computer is $(Get-Described $computer), which this archive is for."
    Write-Output ''
    & (Join-Path $here 'server\lengua-mcp.exe') doctor
    exit $LASTEXITCODE
}

# lengua's marketplace folder: the plugin in lengua\, and .claude-plugin\marketplace.json,
# which names it for Claude Code.
if ($Prefix) {
    $market = $Prefix
} else {
    $market = Join-Path $env:LOCALAPPDATA 'lengua\plugins'
}
$dest = Join-Path $market 'lengua'
$staging = "$dest.new"
$marketplace = Join-Path $market '.claude-plugin\marketplace.json'
# What the install replaces must be lengua's: an earlier install, which holds its TARGET file,
# and the marketplace.json that names lengua, or nothing yet.
foreach ($folder in $dest, $staging) {
    if ((Test-Path -LiteralPath $folder) -and -not (Test-Path -LiteralPath (Join-Path $folder 'TARGET'))) {
        [Console]::Error.WriteLine("$folder is there and isn't lengua (it has no TARGET file), so the install won't replace it: give -Prefix a folder of lengua's own, or move $folder away.")
        exit 1
    }
}
if (Test-Path -LiteralPath $marketplace) {
    $named = $null
    try {
        $named = (Get-Content -Raw -LiteralPath $marketplace | ConvertFrom-Json).name
    } catch {
        $named = $null
    }
    if ($named -ne 'lengua') {
        [Console]::Error.WriteLine("$marketplace is another marketplace's, so the install won't replace it: give -Prefix a folder of lengua's own.")
        exit 1
    }
}
New-Item -ItemType Directory -Force -Path (Join-Path $market '.claude-plugin') | Out-Null
if (Test-Path -LiteralPath $staging) {
    Remove-Item -Recurse -Force -LiteralPath $staging
}
Copy-Item -Recurse -Force -LiteralPath $here -Destination $staging
foreach ($own in 'install.sh', 'install.ps1', 'INSTALL.md') {
    $file = Join-Path $staging $own
    if (Test-Path -LiteralPath $file) {
        Remove-Item -Force -LiteralPath $file
    }
}
if (Test-Path -LiteralPath $dest) {
    Remove-Item -Recurse -Force -LiteralPath $dest
}
Move-Item -LiteralPath $staging -Destination $dest
# A browser marks what it downloads, and Windows may stop an unsigned program so marked.
Get-ChildItem -Recurse -Force -File -LiteralPath $dest | Unblock-File
$manifest = @"
{
  "name": "lengua",
  "owner": {
    "name": "lengua"
  },
  "plugins": [
    {
      "name": "lengua",
      "source": "./lengua",
      "version": "$version"
    }
  ]
}
"@
# UTF-8 without a byte order mark, which a JSON reader may refuse.
[System.IO.File]::WriteAllText($marketplace, $manifest, (New-Object System.Text.UTF8Encoding $false))
$server = Join-Path $dest 'server\lengua-mcp.exe'

Write-Output "Installed lengua $version for $(Get-Described $target) in $dest."
Write-Output ''
# Claude Code installs a plugin from a marketplace it knows: lengua's, added once and updated
# after.
$registered = $false
if (Get-Command claude -ErrorAction SilentlyContinue) {
    $known = (Test-Quietly { claude plugin marketplace add $market }) -or (Test-Quietly { claude plugin marketplace update lengua })
    if ($known) {
        $registered = (Test-Quietly { claude plugin update lengua@lengua }) -or (Test-Quietly { claude plugin install lengua@lengua })
    }
}
if ($registered) {
    Write-Output 'Claude Code has lengua now: start a new session, or run /reload-plugins in one.'
} else {
    Write-Output 'Add it to Claude Code:'
    Write-Output "    claude plugin marketplace add `"$market`""
    Write-Output '    claude plugin install lengua@lengua'
    Write-Output "or load it for one session: claude --plugin-dir `"$dest`""
}
Write-Output "Other agent harnesses load the plugin folder $dest."
Write-Output 'Then describe a program: "Use lengua to write a program that ...".'
Write-Output ''
Write-Output 'Run a program it built from a shell, script or service:'
Write-Output "    & `"$server`" run <program folder> -- <arguments>"
Write-Output ''
& $server doctor
exit $LASTEXITCODE
