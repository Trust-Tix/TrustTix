# Despliega el contrato TrustTix en Stellar MAINNET (dinero real) y guarda su ID.
#
# Usa contracts/trusttix-mainnet: el núcleo que usa la app más `actualizar`.
# Antes de gastar, revisa el saldo y simula cuánto cuesta subir el código; si
# no alcanza, se detiene sin pagar nada. Pide confirmación escrita, salvo con
# -Confirmar.
#
# Uso (desde cualquier carpeta):
#   .\contracts\scripts\desplegar-mainnet.ps1
#   .\contracts\scripts\desplegar-mainnet.ps1 -Fuente trusttix-mainnet -Confirmar
#
# -Fuente  Identidad de la CLI que paga y queda como administradora (recibe
#          las comisiones y es la única que puede actualizar el contrato).
#          Necesita XLM real: subir el código cuesta unos 27 XLM.
# La clave secreta de la identidad se queda en esta computadora; respáldala.

param(
    [string]$Fuente = 'trusttix-mainnet',
    [ValidateRange(0, 1000)][int]$Compra = 300,
    [ValidateRange(0, 1000)][int]$Reventa = 200,
    [string]$Rpc = 'https://mainnet.sorobanrpc.com',
    # Comisión de inclusión en stroops. La de la CLI (100) puede quedar debajo
    # del mínimo de Mainnet y la transacción nunca entra; 10,000 son 0.001 XLM.
    [int]$InclusionFee = 10000,
    [switch]$Confirmar
)

$ErrorActionPreference = 'Continue'
$pp = 'Public Global Stellar Network ; September 2015'
$contratos = Split-Path -Parent $PSScriptRoot
$wasm = Join-Path $contratos 'target\wasm32v1-none\release\trusttix_mainnet.wasm'
$archivoId = Join-Path $PSScriptRoot 'contrato-mainnet.txt'
$explorador = 'https://stellar.expert/explorer/public'

function Fallar([string]$mensaje, [string]$detalle = '') {
    Write-Host "ERROR: $mensaje" -ForegroundColor Red
    if ($detalle) { Write-Host $detalle }
    exit 1
}

function Invoke-StellarCli([string[]]$argumentos) {
    $salida = @() | & stellar.exe @argumentos 2>&1 | ForEach-Object { "$_" } | Out-String
    return @{ Ok = ($LASTEXITCODE -eq 0); Texto = $salida }
}

function SaldoXlm([string]$cuenta) {
    try {
        $c = Invoke-RestMethod "https://horizon.stellar.org/accounts/$cuenta" -TimeoutSec 30
        return [double](($c.balances | Where-Object asset_type -eq 'native').balance)
    } catch {
        return 0
    }
}

$red = @('--rpc-url', $Rpc, '--network-passphrase', $pp)
$enviar = @('--inclusion-fee', "$InclusionFee")

# 1. Cuenta que paga y administra.
$r = Invoke-StellarCli @('keys', 'address', $Fuente)
if (-not $r.Ok) { Fallar "No existe la identidad '$Fuente'. Créala con: stellar keys generate $Fuente" }
$admin = $r.Texto.Trim()
$saldo = SaldoXlm $admin
Write-Host "Admin:      $Fuente ($admin)"
Write-Host "Saldo:      $saldo XLM"
Write-Host "Comisiones: compra $Compra pb ($($Compra / 100) %), reventa $Reventa pb ($($Reventa / 100) %)"

# 2. Token de pago: XLM nativo en Mainnet.
$r = Invoke-StellarCli (@('contract', 'id', 'asset', '--asset', 'native') + $red)
$token = [regex]::Match($r.Texto, 'C[A-Z2-7]{55}').Value
if (-not $token) { Fallar 'No se pudo obtener el contrato del XLM en Mainnet.' $r.Texto }
Write-Host "Token XLM:  $token"

# 3. Compilar.
Write-Host "`nCompilando contracts/trusttix-mainnet..."
Push-Location $contratos
$r = Invoke-StellarCli @('contract', 'build', '--package', 'trusttix-mainnet')
Pop-Location
if (-not $r.Ok -or -not (Test-Path $wasm)) { Fallar 'No se pudo compilar el contrato.' $r.Texto }
$huella = (Get-FileHash $wasm -Algorithm SHA256).Hash.ToLower()
Write-Host "Código:     $huella ($((Get-Item $wasm).Length) bytes)"

# 4. Simular cuánto cuesta subir el código, sin pagar.
$x = (@() | & stellar.exe contract upload --wasm $wasm --source-account $Fuente @red --build-only 2>$null | Out-String).Trim()
$cuerpo = @{ jsonrpc = '2.0'; id = 1; method = 'simulateTransaction'; params = @{ transaction = $x } } | ConvertTo-Json -Depth 5
$sim = Invoke-RestMethod -Uri $Rpc -Method Post -ContentType 'application/json' -Body $cuerpo -TimeoutSec 60
$costo = if ($sim.result.minResourceFee) { [double]$sim.result.minResourceFee / 1e7 } else { 0 }
Write-Host "Subir el código cuesta: $costo XLM (si ya estaba subido, casi nada)"
if ($saldo -lt $costo + 5) { Fallar "Saldo insuficiente: hacen falta unos $([math]::Ceiling($costo + 5)) XLM y hay $saldo." }

if (-not $Confirmar) {
    $respuesta = Read-Host "`nEsto gasta XLM REAL. Escribe DESPLEGAR para continuar"
    if ($respuesta -ne 'DESPLEGAR') { Fallar 'Cancelado: no se gastó nada.' }
}

# 5. Subir el código (si ya estaba subido, la red lo reutiliza).
Write-Host "`nSubiendo el código a Mainnet..."
$r = Invoke-StellarCli (@('contract', 'upload', '--wasm', $wasm, '--source-account', $Fuente) + $red + $enviar)
if (-not $r.Ok) { Fallar 'No se pudo subir el código.' $r.Texto }

# 6. Crear el contrato con su constructor.
Write-Host 'Creando el contrato...'
$r = Invoke-StellarCli (@(
        'contract', 'deploy', '--wasm-hash', $huella, '--source-account', $Fuente) + $red + $enviar + @(
        '--', '--token', $token, '--admin', $admin, '--compra', "$Compra", '--reventa', "$Reventa"))
$id = [regex]::Match($r.Texto, 'C[A-Z2-7]{55}').Value
if (-not $r.Ok -or -not $id) { Fallar 'No se pudo crear el contrato.' $r.Texto }
$tx = [regex]::Matches($r.Texto, 'Signing transaction: ([0-9a-f]{64})') | Select-Object -Last 1 | ForEach-Object { $_.Groups[1].Value }

# 7. Comprobar que responde con lo que se le dio.
$c = Invoke-StellarCli (@('contract', 'invoke', '--id', $id, '--source-account', $Fuente) + $red + @('--', 'comisiones'))
$a = Invoke-StellarCli (@('contract', 'invoke', '--id', $id, '--source-account', $Fuente) + $red + @('--', 'admin'))
$comisionesOk = $c.Texto -match "`"compra`":\s*$Compra\b" -and $c.Texto -match "`"reventa`":\s*$Reventa\b"
if (-not $comisionesOk -or $a.Texto -notmatch $admin) { Fallar "El contrato $id no respondió lo esperado." "$($c.Texto)`n$($a.Texto)" }

# 8. Guardar el ID.
$texto = "$id`r`n# Contrato TrustTix en Mainnet: $explorador/contract/$id`r`n# Admin: $admin. Comisiones: $Compra pb compra, $Reventa pb reventa. Código: $huella`r`n"
[System.IO.File]::WriteAllText($archivoId, $texto)

Write-Host "`nContrato en Mainnet: $id" -ForegroundColor Green
Write-Host "Contrato:   $explorador/contract/$id"
if ($tx) { Write-Host "Despliegue: $explorador/tx/$tx" }
Write-Host "Saldo que queda: $(SaldoXlm $admin) XLM"
Write-Host "ID guardado en $archivoId"
