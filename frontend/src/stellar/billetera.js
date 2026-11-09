import { Keypair, TransactionBuilder } from '@stellar/stellar-sdk'
import { StellarWalletsKit, SwkAppDarkTheme } from '@creit.tech/stellar-wallets-kit'
import { defaultModules } from '@creit.tech/stellar-wallets-kit/modules/utils'
import { RED } from './config'
import { guardarBilleteraEnCuenta, haySesion, leerBilleteraDeCuenta, supabaseListo } from '../supabase'

// Módulo de billeteras. Hay dos tipos:
//
//   trusttix   Billetera integrada: una cuenta de Testnet que se crea para la
//              cuenta de Google y se activa con Friendbot. Sirve en el celular
//              y no necesita instalar nada. Su llave se guarda en la cuenta
//              (Supabase) y en el navegador, por eso es SOLO para Testnet.
//   externa    Cualquier billetera de Stellar Wallets Kit: Varial (antes
//              Freighter), xBull, Albedo, LOBSTR, Hana, Rabet y otras.
//
// Todo el resto de la app firma con `firmar`, sin saber qué tipo está activo.
//
// Modo grabación: cuando un navegador automatizado inyecta
// window.__TRUSTTIX_DEMO__, la app usa esa cuenta de prueba en lugar de una
// billetera. Solo sirve para grabar demos; las llaves nunca están en la app.
const demo = () => (typeof window !== 'undefined' ? window.__TRUSTTIX_DEMO__ : undefined)

const CLAVE_ACTIVA = 'trusttix-billetera'
const clavePropia = (usuarioId) => `trusttix-billetera-${usuarioId}`

let kitListo = false
function kit() {
  if (!kitListo) {
    StellarWalletsKit.init({ modules: defaultModules(), network: RED, theme: SwkAppDarkTheme })
    kitListo = true
  }
  return StellarWalletsKit
}

function leer(clave) {
  try {
    return JSON.parse(localStorage.getItem(clave))
  } catch {
    return null
  }
}

function guardar(clave, valor) {
  try {
    if (valor == null) localStorage.removeItem(clave)
    else localStorage.setItem(clave, JSON.stringify(valor))
  } catch {
    // Sin almacenamiento la billetera dura lo que dure la pestaña.
  }
}

// Billetera activa en memoria: { tipo, direccion, nombre, par? }.
let activa = null

const describir = (b) => (b ? { tipo: b.tipo, direccion: b.direccion, nombre: b.nombre } : null)

export const billeteraActiva = () => describir(activa)

// ---------- Billetera integrada ----------

export const tieneBilleteraPropia = (usuarioId) => Boolean(usuarioId && leer(clavePropia(usuarioId))?.secreta)

function activarPropia(secreta, enCuenta = false) {
  const par = Keypair.fromSecret(secreta)
  activa = { tipo: 'trusttix', direccion: par.publicKey(), nombre: 'Billetera TrustTix', par, enCuenta }
  return activa
}

// Abre la billetera integrada de la cuenta; si no existe, la crea. Con sesión
// de Supabase, la billetera vive en la cuenta y es la misma en cualquier
// dispositivo o subdominio; sin ella, solo en este navegador.
//   1. La de la cuenta, si ya hay una (gana sobre la de este navegador).
//   2. La de este navegador, que además se sube a la cuenta.
//   3. Una nueva, guardada en los dos lugares.
export async function usarBilleteraPropia(usuarioId) {
  if (!usuarioId) throw new Error('Inicia sesión con Google para usar la billetera TrustTix.')
  const conCuenta = supabaseListo && (await haySesion())
  const remota = conCuenta ? await leerBilleteraDeCuenta().catch(() => undefined) : undefined
  let local = leer(clavePropia(usuarioId))
  let nueva = false

  if (remota?.secreta) {
    local = { secreta: remota.secreta, creada: local?.creada ?? new Date().toISOString() }
    guardar(clavePropia(usuarioId), local)
  } else if (!local?.secreta) {
    local = { secreta: Keypair.random().secret(), creada: new Date().toISOString() }
    guardar(clavePropia(usuarioId), local)
    nueva = true
  }

  // `remota === null` = la cuenta respondió y no tiene billetera: se sube esta.
  let enCuenta = Boolean(remota?.secreta)
  if (conCuenta && remota === null) {
    const par = Keypair.fromSecret(local.secreta)
    enCuenta = await guardarBilleteraEnCuenta(par.publicKey(), local.secreta).then(() => true, () => false)
  }

  const b = activarPropia(local.secreta, enCuenta)
  guardar(CLAVE_ACTIVA, { tipo: 'trusttix', usuarioId })
  return { ...describir(b), nueva }
}

// Usa una billetera creada en otro lugar (con su clave secreta). Con sesión,
// pasa a ser la billetera de la cuenta en todos los dispositivos.
export async function importarBilleteraPropia(usuarioId, secreta) {
  const limpia = secreta.trim()
  let par
  try {
    par = Keypair.fromSecret(limpia)
  } catch {
    throw new Error('Esa no es una clave secreta válida. Empieza con S y tiene 56 caracteres.')
  }
  let enCuenta = false
  if (supabaseListo && (await haySesion())) {
    await guardarBilleteraEnCuenta(par.publicKey(), limpia)
    enCuenta = true
  }
  guardar(clavePropia(usuarioId), { secreta: limpia, creada: new Date().toISOString(), importada: true })
  guardar(CLAVE_ACTIVA, { tipo: 'trusttix', usuarioId })
  activarPropia(par.secret(), enCuenta)
  return describir(activa)
}

// La billetera TrustTix activa está respaldada en la cuenta.
export const billeteraEnCuenta = () => Boolean(activa?.enCuenta)

// Para respaldar la billetera o llevarla a Freighter. Solo la integrada tiene clave aquí.
export function claveSecretaPropia(usuarioId) {
  return leer(clavePropia(usuarioId))?.secreta ?? null
}

// ---------- Billeteras externas ----------

async function revisarRedExterna() {
  let red
  try {
    red = await kit().getNetwork()
  } catch {
    return // Algunas billeteras no informan su red; la transacción ya va firmada para Testnet.
  }
  if (red?.networkPassphrase && red.networkPassphrase !== RED) {
    throw new Error('Tu billetera no está en Testnet. Cámbiala a Testnet en su configuración y vuelve a conectar.')
  }
}

// Abre el selector de Stellar Wallets Kit y conecta la billetera elegida.
export async function conectarExterna() {
  let address
  try {
    ;({ address } = await kit().authModal())
  } catch (e) {
    // Cerrar el selector es una decisión de la persona, no un error: se marca para no mostrar alerta.
    if (!e?.message || /clos|cancel|reject|dismiss/i.test(e.message)) {
      throw Object.assign(new Error('Se cerró el selector sin conectar una billetera.'), { cancelado: true })
    }
    throw new Error(e.message)
  }
  await revisarRedExterna()
  const modulo = kit().selectedModule
  activa = { tipo: 'externa', direccion: address, nombre: modulo?.productName ?? 'Billetera externa', id: modulo?.productId }
  guardar(CLAVE_ACTIVA, { tipo: 'externa', id: activa.id })
  return describir(activa)
}

// ---------- Comunes ----------

// Recupera la billetera usada la última vez sin pedir permiso de nuevo.
export async function reconectar(usuarioId) {
  try {
    if (demo()) {
      const direccion = await demo().cuenta()
      activa = { tipo: 'demo', direccion, nombre: 'Cuenta de demostración' }
      return describir(activa)
    }
    const ultima = leer(CLAVE_ACTIVA)
    if (ultima?.tipo === 'trusttix' && ultima.usuarioId === usuarioId) {
      // Pasa por la cuenta: si en otro dispositivo se importó otra clave, esa manda.
      return describir(await usarBilleteraPropia(usuarioId))
    }
    if (ultima?.tipo === 'externa' && ultima.id) {
      kit().setWallet(ultima.id)
      const { address } = await kit().fetchAddress()
      if (!address) return null
      activa = { tipo: 'externa', direccion: address, nombre: kit().selectedModule?.productName ?? 'Billetera externa', id: ultima.id }
      return describir(activa)
    }
  } catch {
    // La billetera ya no da permiso o no está instalada: se vuelve a elegir.
  }
  return null
}

// Deja de usar la billetera. La integrada no se borra: vuelve al elegirla.
// `manual` recuerda que la persona la desconectó a propósito, para no volver a
// crearle la billetera TrustTix automáticamente; al cerrar sesión se olvida.
export async function desconectar({ manual = true } = {}) {
  if (activa?.tipo === 'externa') await kit().disconnect().catch(() => {})
  activa = null
  guardar(CLAVE_ACTIVA, manual ? { tipo: 'ninguna' } : null)
}

// Sin una elección previa, la app abre la billetera TrustTix sola al iniciar sesión.
export const eligioOtraBilletera = () => {
  const ultima = leer(CLAVE_ACTIVA)
  return ultima?.tipo === 'ninguna' || ultima?.tipo === 'externa'
}

// Firma usada por el cliente del contrato y por los pagos.
export async function firmar(xdr, opciones = {}) {
  if (demo()) {
    const cuenta = await demo().cuenta()
    return { signedTxXdr: await demo().firmar(xdr), signerAddress: cuenta }
  }
  if (!activa) throw new Error('Conecta una billetera para firmar.')
  const red = opciones.networkPassphrase ?? RED
  if (activa.tipo === 'trusttix') {
    const tx = TransactionBuilder.fromXDR(xdr, red)
    tx.sign(activa.par)
    return { signedTxXdr: tx.toXDR(), signerAddress: activa.direccion }
  }
  try {
    const r = await kit().signTransaction(xdr, { networkPassphrase: red, address: opciones.address ?? activa.direccion })
    return { signedTxXdr: r.signedTxXdr, signerAddress: r.signerAddress ?? activa.direccion }
  } catch (e) {
    throw new Error(e?.message ? `La billetera no firmó: ${e.message}` : 'Se canceló la firma en la billetera.')
  }
}
