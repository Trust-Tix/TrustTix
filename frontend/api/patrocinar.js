// Patrocinio de comisiones (función de Vercel).
//
// La app firma la transacción con la billetera de la persona y la manda aquí.
// Esta función la envuelve en un "fee bump" firmado por la cuenta
// patrocinadora de TrustTix y la envía a la red: la comisión la paga TrustTix y
// la persona solo paga el boleto.
//
// Solo patrocina llamadas al contrato TrustTix, con un tope de comisión, para
// que nadie use la cuenta patrocinadora para otra cosa.
//
// Variables de entorno en Vercel:
//   TRUSTTIX_PATROCINADOR_SECRETA   clave secreta (S…) de la cuenta que paga
//   VITE_CONTRATO_ID                opcional, el contrato TrustTix

import { FeeBumpTransaction, Keypair, Networks, TransactionBuilder, rpc } from '@stellar/stellar-sdk'

const CONTRATO = process.env.VITE_CONTRATO_ID ?? 'CCWWMLZANZV54WMJQ5JQ6XP2AAUE5BGKK3Q5CPQZEJMREJ4OYRVN36GD'
const RPC_URL = process.env.VITE_RPC_URL ?? 'https://soroban-testnet.stellar.org'
const RED = Networks.TESTNET
// 1 XLM: muy por encima de lo que cuesta una llamada normal al contrato.
const COMISION_MAXIMA = 10_000_000

// La transacción es exactamente una llamada al contrato TrustTix. El SDK 17
// representa la función como JSON: {"invoke_contract":{"contract_address":"C…"}}.
function esDelContrato(tx) {
  if (tx.operations.length !== 1) return false
  const op = tx.operations[0]
  if (op.type !== 'invokeHostFunction') return false
  try {
    const funcion = JSON.parse(JSON.stringify(op.func, (_, v) => (typeof v === 'bigint' ? String(v) : v)))
    return funcion?.invoke_contract?.contract_address === CONTRATO
  } catch {
    return false
  }
}

export default async function handler(req, res) {
  const secreta = process.env.TRUSTTIX_PATROCINADOR_SECRETA
  // GET: la app pregunta si hay patrocinio, para decir "sin comisiones" solo cuando es cierto.
  if (req.method === 'GET') return res.status(200).json({ activo: Boolean(secreta) })
  if (req.method !== 'POST') return res.status(405).json({ error: 'Usa GET o POST.' })

  if (!secreta) return res.status(503).json({ error: 'El patrocinio de comisiones no está configurado.' })

  let interna
  try {
    const cuerpo = typeof req.body === 'string' ? JSON.parse(req.body) : req.body
    interna = TransactionBuilder.fromXDR(cuerpo?.xdr, RED)
  } catch {
    return res.status(400).json({ error: 'La transacción no es válida.' })
  }

  if (interna instanceof FeeBumpTransaction) return res.status(400).json({ error: 'La transacción ya está patrocinada.' })
  if (!esDelContrato(interna)) return res.status(403).json({ error: 'Solo se patrocinan llamadas al contrato TrustTix.' })
  if (Number(interna.fee) > COMISION_MAXIMA) return res.status(403).json({ error: 'La comisión supera el tope de patrocinio.' })
  if (!interna.signatures.length) return res.status(400).json({ error: 'Falta la firma de la billetera.' })

  try {
    const patrocinador = Keypair.fromSecret(secreta)
    // La comisión externa cubre de sobra la interna (incluido el costo de recursos de Soroban).
    const envuelta = TransactionBuilder.buildFeeBumpTransaction(patrocinador, interna.fee, interna, RED)
    envuelta.sign(patrocinador)
    const enviada = await new rpc.Server(RPC_URL).sendTransaction(envuelta)
    if (enviada.status === 'ERROR' || enviada.status === 'TRY_AGAIN_LATER') {
      return res.status(502).json({ error: `La red no aceptó la transacción (${enviada.status}).` })
    }
    return res.status(200).json({ hash: enviada.hash })
  } catch (e) {
    return res.status(500).json({ error: `No pudimos patrocinar la transacción: ${e.message}` })
  }
}
