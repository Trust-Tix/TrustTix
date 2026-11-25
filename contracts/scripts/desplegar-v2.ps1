# Compila, despliega e inicializa el contrato TrustTix v2 en Stellar Testnet,
# comprueba que responde y guarda su ID en scripts\contrato-id.txt.
#
# Uso (desde cualquier carpeta):
#   .\contracts\scripts\desplegar-v2.ps1
#   .\contracts\scripts\desplegar-v2.ps1 -Fuente trusttix-admin -Admin G...
#
# -Fuente  Identidad de la CLI de Stellar que firma y paga el despliegue.
#          Si no existe, se crea y se fondea con Friendbot.
# -Admin   Cuenta G... de TrustTix: recibe las comisiones y es la única que
#          puede cambiarlas. Por defecto, la cuenta de -Fuente.
#
# Requiere la CLI de Stellar: https://developers.stellar.org/docs/tools/cli
# La clave secreta de la identidad se queda en esta computadora; nunca va al repo.

param(
    [string]$Fuente = 'trusttix-admin',
    [string]$Admin = '',
    [ValidateRange(0, 1000)][int]$Compra = 300,
    [ValidateRange(0, 1000)][int]$Reventa = 200,
    # Stellar Asset Contract de XLM nativo en Testnet.
    [string]$Token = 'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC'
)

$ErrorActionPreference = 'Continue'
$red = 'testnet'
$contratos = Split-Path -Parent $PSScriptRoot
$wasm = Join-Path $contratos 'target\wasm32v1-none\release\trusttix.wasm'
$archivoId = Join-Path $PSScriptRoot 'contrato-id.txt'

function Fallar([string]$mensaje, [string]$detalle = '') {
    Write-Host "ERROR: $mensaje" -ForegroundColor Red
    if ($detalle) { Write-Host $detalle }
    exit 1
}

# La CLI escribe su progreso en stderr; se junta todo en texto para leerlo.
# La entrada va vacía para que la CLI nunca se quede esperando una respuesta.
function Invoke-StellarCli([string[]]$argumentos) {
    $salida = @() | & stellar.exe @argumentos 2>&1 | ForEach-Object { "$_" } | Out-String
    return @{ Ok = ($LASTEXITCODE -eq 0); Texto = $salida }
}

if (-not (Get-Command stellar -ErrorAction SilentlyContinue)) {
    Fallar 'No se encontró la CLI de Stellar. Instálala con: cargo install --locked stellar-cli'
}

# 1. Identidad que despliega.
$fuenteDir = (Invoke-StellarCli @('keys', 'address', $Fuente))
if (-not $fuenteDir.Ok) {
    Write-Host "Creando la identidad '$Fuente' y fondeándola en Testnet..."
    $r = Invoke-StellarCli @('keys', 'generate', $Fuente, '--network', $red, '--fund')
    if (-not $r.Ok) { Fallar "No se pudo crear la identidad '$Fuente'." $r.Texto }
    $fuenteDir = Invoke-StellarCli @('keys', 'address', $Fuente)
}
$cuentaFuente = $fuenteDir.Texto.Trim()
if (-not $Admin) { $Admin = $cuentaFuente }
if ($Admin -notmatch '^G[A-Z2-7]{55}$') { Fallar "-Admin debe ser una cuenta G... válida: '$Admin'." }

Write-Host "Despliega:  $Fuente ($cuentaFuente)"
Write-Host "Admin:      $Admin"
Write-Host "Comisiones: compra $Compra pb ($($Compra / 100) %), reventa $Reventa pb ($($Reventa / 100) %)"

# 2. Compilar.
Write-Host "`nCompilando el contrato..."
Push-Location $contratos
$r = Invoke-StellarCli @('contract', 'build')
Pop-Location
if (-not $r.Ok -or -not (Test-Path $wasm)) { Fallar 'No se pudo compilar el contrato.' $r.Texto }

# 3. Desplegar e inicializar (el constructor recibe token, admin y comisiones).
Write-Host 'Desplegando en Testnet...'
$r = Invoke-StellarCli @(
    'contract', 'deploy', '--wasm', $wasm, '--source-account', $Fuente, '--network', $red,
    '--', '--token', $Token, '--admin', $Admin, '--compra', "$Compra", '--reventa', "$Reventa"
)
$id = [regex]::Match($r.Texto, 'C[A-Z2-7]{55}').Value
if (-not $r.Ok -or -not $id) { Fallar 'El despliegue falló.' $r.Texto }
$tx = [regex]::Match($r.Texto, 'https://stellar\.expert/explorer/testnet/tx/[0-9a-f]+').Value

# 4. Comprobar que el contrato responde con lo que se le dio.
Write-Host 'Comprobando el contrato...'
$c = Invoke-StellarCli @('contract', 'invoke', '--id', $id, '--source-account', $Fuente, '--network', $red, '--', 'comisiones')
$a = Invoke-StellarCli @('contract', 'invoke', '--id', $id, '--source-account', $Fuente, '--network', $red, '--', 'admin')
$comisionesOk = $c.Texto -match "`"compra`":\s*$Compra\b" -and $c.Texto -match "`"reventa`":\s*$Reventa\b"
if (-not $c.Ok -or -not $comisionesOk) { Fallar "El contrato $id no devolvió las comisiones esperadas." $c.Texto }
if (-not $a.Ok -or $a.Texto -notmatch $Admin) { Fallar "El contrato $id no devolvió el admin esperado." $a.Texto }

# 5. Guardar el ID, solo, en una línea.
[System.IO.File]::WriteAllText($archivoId, "$id`r`n")

Write-Host "`nContrato v2 desplegado: $id" -ForegroundColor Green
Write-Host "Comisiones: $([regex]::Match($c.Texto, '\{.*\}').Value)"
Write-Host "Contrato:   https://stellar.expert/explorer/testnet/contract/$id"
if ($tx) { Write-Host "Despliegue: $tx" }
Write-Host "ID guardado en $archivoId"
