import { Asset, BASE_FEE, Horizon, Operation, StrKey, TransactionBuilder } from '@stellar/stellar-sdk'
import { FRIENDBOT_URL, HORIZON_URL, RED } from './config'
import { firmar } from './billetera'

// Operaciones de cuenta en Testnet a través de Horizon: saldo, activación,
// envío de XLM e historial de pagos.

const horizon = new Horizon.Server(HORIZON_URL)

const noExiste = (e) => e?.response?.status === 404 || e?.name === 'NotFoundError'

// Saldo en XLM como texto ("123.4500000"), o null si la cuenta aún no existe en la red.
export async function saldoXLM(direccion) {
  try {
    const cuenta = await horizon.loadAccount(direccion)
    return cuenta.balances.find((b) => b.asset_type === 'native')?.balance ?? '0'
  } catch (e) {
    if (noExiste(e)) return null
    throw new Error(`No pudimos leer el saldo: ${e.message}`)
  }
}

// En Testnet, Friendbot crea la cuenta y le deposita XLM de prueba. Si ya hay
// una activación en camino para esa cuenta, se espera esa misma.
const activaciones = new Map()

export function activarConFriendbot(direccion) {
  if (!activaciones.has(direccion)) {
    const pedido = (async () => {
      const r = await fetch(`${FRIENDBOT_URL}?addr=${encodeURIComponent(direccion)}`)
      if (!r.ok) {
        const cuerpo = await r.json().catch(() => ({}))
        if (/already funded|op_already_exists|createAccountAlreadyExist/i.test(JSON.stringify(cuerpo))) return
        throw new Error('Friendbot no respondió. Intenta de nuevo en unos segundos.')
      }
    })().finally(() => activaciones.delete(direccion))
    activaciones.set(direccion, pedido)
  }
  return activaciones.get(direccion)
}

// Envía XLM desde la billetera activa. Si el destino no existe todavía, lo crea.
export async function enviarXLM(origen, destino, monto) {
  const para = destino.trim()
  if (!StrKey.isValidEd25519PublicKey(para)) throw new Error('La cuenta destino no es válida. Empieza con G y tiene 56 caracteres.')
  if (para === origen) throw new Error('No puedes enviarte XLM a ti mismo.')
  const cantidad = Number(monto)
  if (!(cantidad > 0)) throw new Error('El monto debe ser mayor que cero.')
  const texto = cantidad.toFixed(7)

  const cuenta = await horizon.loadAccount(origen)
  const existe = await horizon.loadAccount(para).then(() => true, (e) => (noExiste(e) ? false : Promise.reject(e)))
  if (!existe && cantidad < 1) throw new Error('Para una cuenta nueva el mínimo es 1 XLM, que la red pide como reserva.')

  const operacion = existe
    ? Operation.payment({ destination: para, asset: Asset.native(), amount: texto })
    : Operation.createAccount({ destination: para, startingBalance: texto })
  const tx = new TransactionBuilder(cuenta, { fee: BASE_FEE, networkPassphrase: RED }).addOperation(operacion).setTimeout(120).build()

  const { signedTxXdr } = await firmar(tx.toXDR(), { address: origen })
  try {
    const r = await horizon.submitTransaction(TransactionBuilder.fromXDR(signedTxXdr, RED))
    return r.hash
  } catch (e) {
    const codigos = e?.response?.data?.extras?.result_codes
    if (JSON.stringify(codigos ?? '').includes('underfunded')) throw new Error('No te alcanza el saldo. Recuerda que la red reserva 1 XLM por cuenta.')
    throw new Error(`La red rechazó el envío${codigos ? ` (${JSON.stringify(codigos)})` : ''}.`)
  }
}

// Últimos movimientos de XLM de la cuenta, del más nuevo al más viejo. Incluye
// los cobros del contrato (compras de boletos), que Horizon reporta como
// llamadas a contrato con cambios de saldo.
export async function movimientos(direccion, limite = 10) {
  try {
    const r = await horizon.payments().forAccount(direccion).order('desc').limit(limite).call()
    return r.records.flatMap((p) => {
      const base = { id: p.id, hash: p.transaction_hash, fecha: p.created_at, contrato: false }
      if (p.type === 'create_account') {
        const entra = p.account === direccion
        return [{ ...base, entra, monto: p.starting_balance, contraparte: entra ? p.funder : p.account }]
      }
      if (p.type === 'payment' && p.asset_type === 'native') {
        const entra = p.to === direccion
        return [{ ...base, entra, monto: p.amount, contraparte: entra ? p.from : p.to }]
      }
      if (p.type === 'invoke_host_function') {
        return (p.asset_balance_changes ?? [])
          .filter((c) => c.asset_type === 'native' && (c.from === direccion || c.to === direccion))
          .map((c, i) => ({
            ...base,
            id: `${p.id}-${i}`,
            contrato: true,
            entra: c.to === direccion,
            monto: c.amount,
            contraparte: c.to === direccion ? c.from : c.to,
          }))
      }
      return []
    })
  } catch (e) {
    if (noExiste(e)) return []
    throw new Error(`No pudimos leer los movimientos: ${e.message}`)
  }
}
