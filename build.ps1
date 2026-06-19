$keyPath = "$HOME\.tauri\corvus.key"

if (-not (Test-Path $keyPath)) {
    Write-Error "Private key not found: $keyPath"
    exit 1
}

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Read-Host "Key password" -AsSecureString | `
    ForEach-Object { [System.Runtime.InteropServices.Marshal]::PtrToStringAuto(
        [System.Runtime.InteropServices.Marshal]::SecureStringToBSTR($_)) }

cargo tauri build
