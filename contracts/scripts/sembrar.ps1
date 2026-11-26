# Crea los eventos de ejemplo de TrustTix en el contrato v2 de Testnet y genera
# transacciones reales de cada tipo: crear evento, compra, transferencia,
# entrada, tope de reventa y boletos publicados en reventa.
#
# Uso (desde cualquier carpeta):
#   .\contracts\scripts\sembrar.ps1
#   .\contracts\scripts\sembrar.ps1 -Contrato <ID_DEL_CONTRATO>
#
# Sin -Contrato usa el de scripts\contrato-id.txt (el que guarda desplegar-v2.ps1).
# Requiere la identidad "trusttix" (organizador) creada con:
#   stellar keys generate trusttix --network testnet --fund
#
# Después, para las portadas y descripciones en Supabase:
#   cd frontend; npm run sembrar

param(
    [string]$Contrato = '',
    [string]$Organizador = 'trusttix',
    [string]$Bitacora = (Join-Path $PSScriptRoot 'transacciones.md')
)

$ErrorActionPreference = 'Continue'
$red = 'testnet'
$XLM = 10000000  # stroops por XLM

if (-not $Contrato) {
    $Contrato = (Get-Content (Join-Path $PSScriptRoot 'contrato-id.txt') -TotalCount 1).Trim()
}
if ($Contrato -notmatch '^C[A-Z2-7]{55}$') {
    Write-Host "ERROR: '$Contrato' no es un ID de contrato válido." -ForegroundColor Red
    exit 1
}
Write-Host "Contrato: $Contrato"

"# Transacciones de ejemplo en Testnet`n" | Set-Content $Bitacora -Encoding utf8
"Contrato v2: https://stellar.expert/explorer/testnet/contract/$Contrato`n" | Add-Content $Bitacora -Encoding utf8

# La entrada va vacía para que la CLI nunca se quede esperando una respuesta.
function Invocar([string]$cuenta, [string]$descripcion, [string[]]$argumentos) {
    $salida = @() | & stellar contract invoke --id $Contrato --source-account $cuenta --network $red --send=yes -- @argumentos 2>&1 | ForEach-Object { "$_" } | Out-String
    $enlace = [regex]::Match($salida, 'https://stellar\.expert/explorer/testnet/tx/[0-9a-f]+').Value
    $resultado = ($salida -split "`n" | Where-Object { $_ -match '^\s*("?[\w\-]+"?|\d+)\s*$' } | Select-Object -Last 1)
    if ($enlace) {
        "- $descripcion`: $enlace" | Add-Content $Bitacora -Encoding utf8
        Write-Host "OK  $descripcion" -ForegroundColor Green
    } else {
        Write-Host "ERR $descripcion" -ForegroundColor Red
        Write-Host $salida
    }
    return ($resultado -replace '"', '').Trim()
}

function Cuenta([string]$nombre) {
    $existe = & stellar keys address $nombre 2>$null
    if (-not $existe) { & stellar keys generate $nombre --network $red --fund 2>&1 | Out-Null }
    return (& stellar keys address $nombre).Trim()
}

$org = (& stellar keys address $Organizador).Trim()
$puerta = Cuenta 'ttx-puerta'
$fans = 1..4 | ForEach-Object { Cuenta "ttx-fan$_" }

# nombre, recinto, fecha, capacidad, precio en XLM. Los datos descriptivos y las
# portadas están en supabase\semilla\catalogo.json, ligados por el nombre.
$eventos = @(
    @('Caifanes en concierto', 'Centro Cultural Teopanzolco', '2026-12-19T22:00', 300, 100),
    @('AWS Summit Mexico City', 'Centro CitiBanamex', '2026-11-27T09:00', 300, 0),
    @('GOYA HACK · UNAM', 'Universidad Nacional Autónoma de México', '2026-09-21T10:00', 3, 0),
    @('Bruno Mars en concierto', 'Estadio GNP Seguros', '2026-12-07T20:30', 40, 90),
    @('El Rey León, el musical', 'Teatro Telcel', '2026-12-12T20:00', 120, 25)
)

$ids = @()
foreach ($e in $eventos) {
    $precio = [int64]$e[4] * $XLM
    $id = Invocar $Organizador "Crear evento: $($e[0])" @(
        'crear_evento', '--organizador', $org, '--nombre', $e[0], '--recinto', $e[1],
        '--fecha', $e[2], '--capacidad', "$($e[3])", '--precio', "$precio")
    $ids += $id
    Invocar $Organizador "Autorizar personal de acceso en: $($e[0])" @(
        'autorizar_personal', '--evento_id', $id, '--cuenta', $puerta) | Out-Null
}

# Goya Hack: se agota (3 de 3).
$goya = $ids[2]
$boletosGoya = @()
foreach ($i in 0..2) {
    $boletosGoya += Invocar "ttx-fan$($i + 1)" "Compra en GOYA HACK (fan $($i + 1))" @(
        'comprar', '--comprador', $fans[$i], '--evento_id', $goya)
}

# Una transferencia y una entrada real.
Invocar 'ttx-fan1' 'Transferencia de un boleto de GOYA HACK (fan 1 a fan 4)' @(
    'transferir', '--boleto_id', $boletosGoya[0], '--de', $fans[0], '--a', $fans[3]) | Out-Null
Invocar 'ttx-puerta' 'Entrada registrada en la puerta (GOYA HACK)' @(
    'registrar_entrada', '--boleto_id', $boletosGoya[1], '--personal', $puerta) | Out-Null

# Bruno Mars: 28 de 40 vendidos para que queden 12.
$bruno = $ids[3]
$boletosBruno = @()
foreach ($n in 1..28) {
    $fan = (($n - 1) % 4) + 1
    $boletosBruno += Invocar "ttx-fan$fan" "Compra en Bruno Mars ($n de 28)" @(
        'comprar', '--comprador', $fans[$fan - 1], '--evento_id', $bruno)
}

# Reventa en Bruno Mars: el organizador sube el tope a 120 % (108 XLM) y dos
# fans publican su boleto, uno a precio original y otro en el tope.
Invocar $Organizador 'Tope de reventa de Bruno Mars en 120 %' @(
    'fijar_tope_reventa', '--evento_id', $bruno, '--tope', '12000') | Out-Null
Invocar 'ttx-fan1' 'Fan 1 revende su boleto de Bruno Mars en 90 XLM' @(
    'publicar_reventa', '--boleto_id', $boletosBruno[0], '--vendedor', $fans[0], '--precio', "$(90 * $XLM)") | Out-Null
Invocar 'ttx-fan2' 'Fan 2 revende su boleto de Bruno Mars en 108 XLM' @(
    'publicar_reventa', '--boleto_id', $boletosBruno[1], '--vendedor', $fans[1], '--precio', "$(108 * $XLM)") | Out-Null

# Algunas compras en los demás eventos.
Invocar 'ttx-fan2' 'Compra en Caifanes (100 XLM)' @('comprar', '--comprador', $fans[1], '--evento_id', $ids[0]) | Out-Null
Invocar 'ttx-fan3' 'Compra en El Rey León (25 XLM)' @('comprar', '--comprador', $fans[2], '--evento_id', $ids[4]) | Out-Null
Invocar 'ttx-fan4' 'Compra en AWS Summit (gratis)' @('comprar', '--comprador', $fans[3], '--evento_id', $ids[1]) | Out-Null

"`nPersonal de acceso: $puerta" | Add-Content $Bitacora -Encoding utf8
Write-Host "`nListo. Enlaces guardados en $Bitacora"
