import { contract } from '@stellar/stellar-sdk'
import { COMISION_INCLUSION, RED, RPC_URL } from './config'
import { firmar } from './billetera'
import { confirmar, enviarFirmada } from './contrato'

// Bóveda de códigos promocionales (contracts/boveda): guarda XLM y paga un
// monto fijo a cada cuenta que presenta el código correcto, una sola vez.

const MENSAJES = {
  1: 'Ese código no es válido. Revísalo e inténtalo de nuevo.',
  2: 'Esta billetera ya reclamó su premio.',
  3: 'La bóveda ya no tiene fondos para otro premio.',
  4: 'Monto inválido.',
}

export const idValido = (id) => /^C[A-Z2-7]{55}$/.test(id ?? '')

// La especificación del contrato se lee una vez por bóveda.
const especificaciones = new Map()

async function cliente(id, direccion) {
  const opciones = {
    contractId: id,
    networkPassphrase: RED,
    rpcUrl: RPC_URL,
    publicKey: direccion ?? undefined,
    signTransaction: direccion ? firmar : undefined,
  }
  if (!especificaciones.has(id)) {
    const base = await contract.Client.from(opciones)
    especificaciones.set(id, base.spec)
    if (!direccion) return base
  }
  return new contract.Client(especificaciones.get(id), opciones)
}

function traducir(error, tx) {
  const textos = [error?.message, String(error ?? ''), String(tx?.simulation?.error ?? '')]
  for (const t of textos) {
    const m = /Error\(Contract, #(\d+)\)/.exec(t)
    if (m && MENSAJES[m[1]]) return new Error(MENSAJES[m[1]])
  }
  const todo = textos.join(' ')
  if (/account not found|Account .* not found|does not exist/i.test(todo)) {
    return new Error('Tu billetera todavía no está activada en Testnet. Fondéala con Friendbot y vuelve a intentarlo.')
  }
  if (/User declined|rejected|cancel/i.test(todo)) return new Error('Cancelaste la firma en tu billetera.')
  return error instanceof Error ? error : new Error(String(error))
}

function valor(tx) {
  const r = tx.result
  if (r && typeof r.isErr === 'function') {
    if (r.isErr()) throw traducir(null, tx)
    return r.unwrap()
  }
  return r
}

async function leer(id, metodo, args = {}) {
  const c = await cliente(id)
  let tx
  try {
    tx = await c[metodo](args)
    return valor(tx)
  } catch (e) {
    throw traducir(e, tx)
  }
}

// Monto por cuenta y lo disponible, en stroops; y si `cuenta` ya reclamó.
export async function estadoBoveda(id, cuenta) {
  const [monto, disponible, reclamos, yaReclamo] = await Promise.all([
    leer(id, 'monto'),
    leer(id, 'disponible'),
    leer(id, 'reclamos'),
    cuenta ? leer(id, 'ya_reclamo', { cuenta }) : false,
  ])
  return { monto: BigInt(monto), disponible: BigInt(disponible), reclamos: Number(reclamos), yaReclamo: Boolean(yaReclamo) }
}

// Firma el reclamo con la billetera activa y espera a que la red lo confirme.
export async function reclamar(id, cuenta, codigo) {
  const c = await cliente(id, cuenta)
  let tx
  try {
    tx = await c.reclamar({ cuenta, codigo }, { fee: COMISION_INCLUSION })
    valor(tx) // código incorrecto, ya reclamado o sin fondos: falla antes de pedir la firma
    await tx.sign()
    const { hash } = await enviarFirmada(tx.signed)
    const monto = await confirmar(hash)
    return { monto: BigInt(monto ?? 0), hash }
  } catch (e) {
    throw traducir(e, tx)
  }
}
