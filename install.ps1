param(
    [string]$Version = "latest"
)

$Repo = "ris-xeryx/nepix"
$Bin = "nepix.exe"

$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "i686" }
$OS = "windows"

if ($Version -eq "latest") {
    $Url = "https://github.com/${Repo}/releases/latest/download/${Bin}"
} else {
    $Url = "https://github.com/${Repo}/releases/download/${Version}/nepix-${OS}-${Arch}.exe"
}

$InstallDir = "$env:LOCALAPPDATA\nepix"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

Write-Host "Descargando nepix $Version... ($OS-$Arch)"
Invoke-WebRequest -Uri $Url -OutFile "$InstallDir\$Bin"

$Path = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
if ($Path -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$Path;$InstallDir", [EnvironmentVariableTarget]::User)
    Write-Host "Agregado a PATH. Reinicia la terminal para usar nepix."
}

Write-Host "Instalado en $InstallDir\$Bin"
