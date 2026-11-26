# Despliega la Bóveda TrustTix en Stellar Testnet en un solo paso: compila,
# despliega con el código promocional, la llena de XLM, hace un reclamo de
# prueba y deja el ID listo en la app (frontend/src/stellar/boveda-id.json).
#
# Uso (desde cualquier carpeta):
#   .\contracts\scripts\desplegar-boveda.ps1
#   .\contracts\scripts\desplegar-boveda.ps1 -Codigo 'Ship It' -MontoXlm 5555 -Veces 4
#   .\contracts\scripts\desplegar-boveda.ps1 -Ensayo -MontoXlm 5 -Veces 3
#
# -Codigo    Código que libera los XLM. Se compara sin mayúsculas ni espacios a los lados.
# -MontoXlm  XLM que recibe cada cuenta que reclama (una vez por cuenta).
# -Veces     Para cuántos reclamos se llena la bóveda (uno lo usa la prueba del script).
# -Ensayo    No toca la app: solo despliega y prueba.
#
# Las cuentas que usa (ttx-boveda y ttx-boveda-fondoN) se crean y se fondean
# solas con Friendbot. Sus llaves se quedan en esta computadora.
# Requiere la CLI de Stellar y Rust con el target wasm32v1-none.

param(
    [string]$Codigo = 'Ship It',
    [ValidateRange(1, 9000)][int]$MontoXlm = 5555,
    [ValidateRange(1, 10)][int]$Veces = 4,
    [switch]$Ensayo,
    # Stellar Asset Contract de XLM nativo en Testnet.
    [string]$Token = 'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC'
)

$ErrorActionPreference = 'Continue'
$red = 'testnet'
$XLM = 10000000  # stroops por XLM
$raiz = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$contratos = Join-Path $raiz 'contracts'
$wasm = Join-Path $contratos 'target\wasm32v1-none\release\boveda.wasm'
$archivoApp = Join-Path $raiz 'frontend\src\stellar\boveda-id.json'
$bitacora = Join-Path $PSScriptRoot 'boveda.md'
$inicio = Get-Date

function Fallar([string]$mensaje, [string]$detalle = '') {
    Write-Host "ERROR: $mensaje" -ForegroundColor Red
    if ($detalle) { Write-Host $detalle }
    exit 1
}

# La CLI escribe su progreso en stderr; se junta todo en texto. La entrada va
# vacía para que nunca se quede esperando una respuesta.
function Invoke-StellarCli([string[]]$argumentos) {
    $salida = @() | & stellar.exe @argumentos 2>&1 | ForEach-Object { "$_" } | Out-String
    return @{ Ok = ($LASTEXITCODE -eq 0); Texto = $salida }
}

function Enlace([string]$texto) {
    return [regex]::Match($texto, 'https://stellar\.expert/explorer/testnet/tx/[0-9a-f]+').Value
}

function Resultado([string]$texto) {
    $linea = $texto -split "`n" | Where-Object { $_ -match '^\s*"?-?[\w]+"?\s*$' } | Select-Object -Last 1
    return ("$linea" -replace '"', '').Trim()
}

# Crea la identidad si no existe (fondeada con Friendbot) y devuelve su cuenta G...
function Cuenta([string]$nombre) {
    $r = Invoke-StellarCli @('keys', 'address', $nombre)
    if (-not $r.Ok) {
        $g = Invoke-StellarCli @('keys', 'generate', $nombre, '--network', $red, '--fund')
        if (-not $g.Ok) { Fallar "No se pudo crear la cuenta '$nombre' con Friendbot." $g.Texto }
        $r = Invoke-StellarCli @('keys', 'address', $nombre)
    }
    return $r.Texto.Trim()
}

function SaldoXlm([string]$cuenta) {
    try {
        $c = Invoke-RestMethod "https://horizon-testnet.stellar.org/accounts/$cuenta"
        return [double](($c.balances | Where-Object asset_type -eq 'native').balance)
    } catch {
        return 0
    }
}

function Invocar([string]$quien, [string[]]$argumentos) {
    return Invoke-StellarCli (@('contract', 'invoke', '--id', $script:id, '--source-account', $quien, '--network', $red, '--send=yes', '--') + $argumentos)
}

if (-not (Get-Command stellar -ErrorAction SilentlyContinue)) {
    Fallar 'No se encontró la CLI de Stellar. Instálala con: cargo install --locked stellar-cli'
}

# 1. Huella del código: SHA-256 en minúsculas y sin espacios a los lados.
$normalizado = $Codigo.Trim().ToLowerInvariant()
if (-not $normalizado) { Fallar 'El código no puede ir vacío.' }
$sha = [System.Security.Cryptography.SHA256]::Create()
$hash = -join ($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($normalizado)) | ForEach-Object { $_.ToString('x2') })
$monto = [int64]$MontoXlm * $XLM

Write-Host "Código:  '$Codigo' (se compara como '$normalizado')"
Write-Host "Monto:   $MontoXlm XLM por cuenta, para $Veces reclamos"

# 2. Cuenta administradora.
$admin = Cuenta 'ttx-boveda'
Write-Host "Admin:   ttx-boveda ($admin)"

# 3. Compilar.
Write-Host "`nCompilando la bóveda..."
Push-Location $contratos
$r = Invoke-StellarCli @('contract', 'build', '--package', 'boveda')
Pop-Location
if (-not $r.Ok -or -not (Test-Path $wasm)) { Fallar 'No se pudo compilar la bóveda.' $r.Texto }

# 4. Desplegar.
Write-Host 'Desplegando en Testnet...'
$r = Invoke-StellarCli @(
    'contract', 'deploy', '--wasm', $wasm, '--source-account', 'ttx-boveda', '--network', $red,
    '--', '--admin', $admin, '--token', $Token, '--codigo_hash', $hash, '--monto', "$monto"
)
$script:id = [regex]::Match($r.Texto, 'C[A-Z2-7]{55}').Value
if (-not $r.Ok -or -not $script:id) { Fallar 'El despliegue falló.' $r.Texto }
$txDespliegue = Enlace $r.Texto
Write-Host "Bóveda:  $($script:id)" -ForegroundColor Green

"# Bóveda TrustTix en Testnet`n" | Set-Content $bitacora -Encoding utf8
"Contrato: https://stellar.expert/explorer/testnet/contract/$($script:id)" | Add-Content $bitacora -Encoding utf8
"Código: $Codigo. Monto por cuenta: $MontoXlm XLM.`n" | Add-Content $bitacora -Encoding utf8
"- Despliegue: $txDespliegue" | Add-Content $bitacora -Encoding utf8

# 5. Llenarla. Cada cuenta de Friendbot trae 10,000 XLM; se usan las que hagan falta.
$falta = [int64]$monto * $Veces
$n = 0
$txDeposito = ''
while ($falta -gt 0 -and $n -lt 12) {
    $n++
    $nombre = "ttx-boveda-fondo$n"
    $cuenta = Cuenta $nombre
    $usable = [int64][math]::Floor((SaldoXlm $cuenta) - 5) * $XLM
    if ($usable -lt 100 * $XLM) { continue }
    $cantidad = [math]::Min($usable, $falta)
    Write-Host "Depositando $($cantidad / $XLM) XLM desde $nombre..."
    $r = Invocar $nombre @('depositar', '--de', $cuenta, '--cantidad', "$cantidad")
    if (-not $r.Ok) { Fallar "No se pudo depositar desde $nombre." $r.Texto }
    $falta -= $cantidad
    if (-not $txDeposito) { $txDeposito = Enlace $r.Texto }
    "- depositar ($($cantidad / $XLM) XLM desde $nombre): $(Enlace $r.Texto)" | Add-Content $bitacora -Encoding utf8
}
if ($falta -gt 0) { Fallar "Faltaron $($falta / $XLM) XLM por depositar." }

# 6. Reclamo de prueba con una cuenta nueva: comprueba que el código paga.
$probador = "ttx-boveda-prueba-$(Get-Date -Format 'HHmmss')"
$cuentaPrueba = Cuenta $probador
Write-Host "Reclamo de prueba con $probador..."
$r = Invocar $probador @('reclamar', '--cuenta', $cuentaPrueba, '--codigo', $Codigo)
$pagado = Resultado $r.Texto
if (-not $r.Ok -or $pagado -ne "$monto") { Fallar 'El reclamo de prueba no pagó lo esperado.' $r.Texto }
$txReclamo = Enlace $r.Texto
"- reclamar (prueba con $probador): $txReclamo" | Add-Content $bitacora -Encoding utf8

$r = Invoke-StellarCli @('contract', 'invoke', '--id', $script:id, '--source-account', 'ttx-boveda', '--network', $red, '--', 'disponible')
$disponible = [int64](Resultado $r.Texto)
Write-Host "Disponible: $($disponible / $XLM) XLM ($([math]::Floor($disponible / $monto)) reclamos más)" -ForegroundColor Green

# 7. ID en la app.
$appUrl = "https://app.trusttix.xyz/canjear?boveda=$($script:id)"
if (-not $Ensayo) {
    [System.IO.File]::WriteAllText($archivoApp, "{ `"contrato`": `"$($script:id)`" }`n")
    Write-Host "ID guardado en $archivoApp"
}
"`nApp: $appUrl" | Add-Content $bitacora -Encoding utf8

$minutos = [math]::Round(((Get-Date) - $inicio).TotalMinutes, 1)
Write-Host "`n================ LISTO en $minutos min ================" -ForegroundColor Green
Write-Host "Link de la app:        $appUrl"
Write-Host "Contrato (stellar.expert): https://stellar.expert/explorer/testnet/contract/$($script:id)"
Write-Host "Función depositar:     $txDeposito"
Write-Host "Función reclamar:      $txReclamo"
Write-Host "Enlaces guardados en $bitacora"
if (-not $Ensayo) {
    # Mensaje en UTF-8 para que git conserve los acentos.
    $mensaje = Join-Path $env:TEMP 'mensaje-boveda.txt'
    $texto = "Bóveda en Testnet: la app apunta al contrato desplegado`n`nBóveda $($script:id) con $Veces reclamos de $MontoXlm XLM y el código del reto. Bitácora de transacciones en contracts/scripts/boveda.md.`n"
    [System.IO.File]::WriteAllText($mensaje, $texto, (New-Object System.Text.UTF8Encoding $false))
    Write-Host "`nDespués de entregar, sube el ID a la app (desde la raíz del repo):"
    Write-Host '  git add frontend/src/stellar/boveda-id.json contracts/scripts/boveda.md'
    Write-Host "  git commit -F `"$mensaje`""
    Write-Host '  git push'
}
