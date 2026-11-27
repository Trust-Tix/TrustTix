// TrustTix es una sola app que se publica como cuatro sitios:
//   trusttix.xyz             landing: quiénes somos y qué ofrecemos
//   app.trusttix.xyz         compradores (ver eventos sin cuenta; comprar y guardar boletos con cuenta)
//   organizer.trusttix.xyz   organizadores (crear eventos y asignar personal)
//   staff.trusttix.xyz       personal de acceso (escanear boletos en la puerta)
// Fuera de ese dominio (localhost, vistas previas de Vercel) el portal se elige
// con ?portal=app, ?portal=organizer o ?portal=staff, y se recuerda mientras la
// pestaña siga abierta.

//
// La publicación de Mainnet (mainnet.trusttix.xyz) es un solo sitio: abre la
// tienda y los portales de organizadores y personal se eligen con ?portal=,
// para que nunca se salte a los subdominios de Testnet.

import { ES_MAINNET } from './stellar/config'

export const DOMINIO = import.meta.env.VITE_DOMINIO ?? 'trusttix.xyz'

const PORTALES = ['landing', 'app', 'organizer', 'staff']
const SUBDOMINIOS = ['app', 'organizer', 'staff']
// Nombres de la versión anterior, para que los enlaces viejos sigan funcionando.
const ANTERIORES = { admin: 'organizer', publico: 'app' }
const CLAVE = 'trusttix-portal'

function detectar() {
  const sub = window.location.hostname.split('.')[0]
  if (!ES_MAINNET && SUBDOMINIOS.includes(sub)) return sub
  if (!ES_MAINNET && sub === 'admin') return 'organizer'
  const crudo = new URLSearchParams(window.location.search).get('portal')
  const pedido = ANTERIORES[crudo] ?? crudo
  try {
    if (PORTALES.includes(pedido)) sessionStorage.setItem(CLAVE, pedido)
    const guardado = sessionStorage.getItem(CLAVE)
    if (PORTALES.includes(guardado)) return guardado
  } catch {
    // Sin almacenamiento: vale lo que diga la dirección.
  }
  if (PORTALES.includes(pedido)) return pedido
  // En Mainnet no hay landing propia: se abre directo la tienda.
  return ES_MAINNET ? 'app' : 'landing'
}

export const PORTAL = detectar()

export const NOMBRE_PORTAL = {
  landing: 'TrustTix',
  app: 'TrustTix',
  organizer: 'Organizadores',
  staff: 'Personal de acceso',
}

// Dirección de una ruta en otro portal: subdominio en el dominio real,
// ?portal= en cualquier otro host.
export function urlPortal(portal, ruta = '/') {
  const { protocol, hostname, origin } = window.location
  const base = hostname.replace(/^(app|organizer|staff|admin|www)\./, '')
  if (base === DOMINIO && !ES_MAINNET) return `${protocol}//${portal === 'landing' ? '' : `${portal}.`}${base}${ruta}`
  return `${origin}${ruta}?portal=${portal}`
}

// admin.trusttix.xyz era el portal de organizadores: se manda a organizer.
export function subdominioViejo() {
  return !ES_MAINNET && window.location.hostname.startsWith('admin.')
}
