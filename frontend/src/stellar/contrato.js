import { contract, rpc, scValToNative } from '@stellar/stellar-sdk'
import { CONTRATO_ID, RED, RPC_URL } from './config'
import { firmar } from './billetera'

// Llamadas al contrato TrustTix. Las consultas se simulan contra la red
// (gratis, sin firma); las acciones se firman con la billetera activa y se envían.

const MENSAJES = {
  1: 'Ese evento no existe en la red.',
  2: 'Ese boleto no existe en la red.',
  3: 'Ya no quedan lugares. La red rechazó la compra y no se cobró nada.',
  4: 'Solo el dueño actual puede transferir este boleto.',
  5: 'Este boleto ya fue utilizado.',
  6: 'Tu cuenta no está autorizada para registrar entradas en este evento.',
  7: 'La capacidad debe ser mayor que cero.',
  8: 'El precio no puede ser negativo.',
  9: 'No puedes transferirte el boleto a ti mismo.',
}

let especificacion = null

async function cliente(direccion) {
  const opciones = {
    contractId: CONTRATO_ID,
    networkPassphrase: RED,
    rpcUrl: RPC_URL,
    publicKey: direccion ?? undefined,
    signTransaction: direccion ? firmar : undefined,
  }
  if (!especificacion) {
    const base = await contract.Client.from(opciones)
    especificacion = base.spec
    if (!direccion) return base
  }
  return new contract.Client(especificacion, opciones)
}

function traducir(error, tx) {
  const textos = [error?.message, String(error ?? ''), String(tx?.simulation?.error ?? '')]
  for (const t of textos) {
    const m = /Error\(Contract, #(\d+)\)/.exec(t)
    if (m && MENSAJES[m[1]]) return new Error(MENSAJES[m[1]])
  }
  if (/balance|underfunded|insufficient/i.test(textos.join(' '))) {
    return new Error('No tienes saldo suficiente en XLM para esta compra.')
  }
  if (/User declined|rejected|cancel/i.test(textos.join(' '))) {
    return new Error('Cancelaste la firma en tu billetera.')
  }
  if (/account not found|Account .* not found|does not exist/i.test(textos.join(' '))) {
    return new Error('Tu billetera todavía no está activada en Testnet. Actívala con XLM de prueba en la página Billetera.')
  }
  return error instanceof Error ? error : new Error(String(error))
}

// Convierte Ok/Err del SDK en un valor o en un error legible.
function valor(tx, resultado) {
  if (resultado && typeof resultado.isErr === 'function') {
    if (resultado.isErr()) throw traducir(null, tx)
    return resultado.unwrap()
  }
  return resultado
}

async function leer(metodo, args = {}) {
  const c = await cliente()
  let tx
  try {
    tx = await c[metodo](args)
    return valor(tx, tx.result)
  } catch (e) {
    throw traducir(e, tx)
  }
}

const servidor = new rpc.Server(RPC_URL)

// ¿TrustTix paga las comisiones de red? Se pregunta una vez a /api/patrocinar.
let consultaPatrocinio = null
export function patrocinioActivo() {
  if (!consultaPatrocinio) {
    consultaPatrocinio = fetch('/api/patrocinar')
      .then((r) => (r.ok && r.headers.get('content-type')?.includes('json') ? r.json() : { activo: false }))
      .then((d) => Boolean(d.activo))
      .catch(() => false)
  }
  return consultaPatrocinio
}

// Envía una transacción ya firmada. Primero pide a /api/patrocinar que TrustTix
// pague la comisión; si el patrocinio no está disponible (en local, o sin la
// cuenta patrocinadora configurada), la envía directo y la comisión sale de la
// billetera, como antes.
async function enviarFirmada(transaccion) {
  try {
    const r = await fetch('/api/patrocinar', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ xdr: transaccion.toXDR() }),
    })
    if (r.ok && r.headers.get('content-type')?.includes('json')) {
      const { hash } = await r.json()
      if (hash) return { hash, patrocinada: true }
    }
  } catch {
    // Sin patrocinio: sigue con el envío directo.
  }
  const enviada = await servidor.sendTransaction(transaccion)
  if (enviada.status === 'ERROR' || enviada.status === 'TRY_AGAIN_LATER') {
    throw new Error(`La red no aceptó la transacción (${enviada.status}).`)
  }
  return { hash: enviada.hash, patrocinada: false }
}

// Espera a que la red confirme la transacción y devuelve su resultado.
async function confirmar(hash) {
  for (let intento = 0; intento < 60; intento++) {
    const r = await servidor.getTransaction(hash)
    if (r.status === 'SUCCESS') return r.returnValue ? scValToNative(r.returnValue) : undefined
    if (r.status === 'FAILED') throw new Error('La red rechazó la transacción.')
    await new Promise((listo) => setTimeout(listo, 1000))
  }
  throw new Error('La red tardó demasiado en confirmar. Revisa tu billetera en unos minutos.')
}

async function escribir(direccion, metodo, args) {
  const c = await cliente(direccion)
  let tx
  try {
    tx = await c[metodo](args)
    valor(tx, tx.result) // detecta errores del contrato antes de pedir la firma
    await tx.sign()
    const { hash, patrocinada } = await enviarFirmada(tx.signed)
    return { resultado: await confirmar(hash), hash, patrocinada }
  } catch (e) {
    throw traducir(e, tx)
  }
}

// ---------- Consultas ----------

export const totalEventos = () => leer('total_eventos').then(Number)
export const totalBoletos = () => leer('total_boletos').then(Number)
export const consultarEvento = (id) => leer('consultar_evento', { evento_id: id })
export const consultarBoleto = (id) => leer('consultar_boleto', { boleto_id: id })
export const boletosDe = (cuenta) => leer('boletos_de', { cuenta }).then((l) => l.map(Number))
export const esPersonal = (eventoId, cuenta) => leer('es_personal', { evento_id: eventoId, cuenta })

// ---------- Acciones firmadas ----------

export const crearEvento = (organizador, { nombre, recinto, fecha, capacidad, precio }) =>
  escribir(organizador, 'crear_evento', { organizador, nombre, recinto, fecha, capacidad, precio })

export const comprar = (comprador, eventoId) =>
  escribir(comprador, 'comprar', { comprador, evento_id: eventoId })

export const transferir = (de, boletoId, a) =>
  escribir(de, 'transferir', { boleto_id: boletoId, de, a })

export const registrarEntrada = (personal, boletoId) =>
  escribir(personal, 'registrar_entrada', { boleto_id: boletoId, personal })

export const autorizarPersonal = (organizador, eventoId, cuenta) =>
  escribir(organizador, 'autorizar_personal', { evento_id: eventoId, cuenta })

export const quitarPersonal = (organizador, eventoId, cuenta) =>
  escribir(organizador, 'quitar_personal', { evento_id: eventoId, cuenta })
