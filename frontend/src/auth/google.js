// Inicio de sesión con Google Identity Services (botón oficial de Google).
//
// Google devuelve un token de identidad (JWT) con el nombre, el correo y la
// foto de la cuenta. La app solo lo usa para mostrar quién entró: lo que da
// permisos dentro de TrustTix (comprar, crear eventos, registrar entradas) es
// la firma de la billetera. Si algún día el servidor necesita confiar en esta
// identidad, tendrá que verificar el token con las llaves públicas de Google.

export const GOOGLE_CLIENT_ID = import.meta.env.VITE_GOOGLE_CLIENT_ID ?? ''

let carga = null
let alEntrar = () => {}
let iniciado = false

function cargarGoogle() {
  if (window.google?.accounts?.id) return Promise.resolve(window.google)
  if (!carga) {
    carga = new Promise((resolve, reject) => {
      const script = document.createElement('script')
      script.src = 'https://accounts.google.com/gsi/client'
      script.async = true
      script.onload = () => resolve(window.google)
      script.onerror = () => {
        carga = null
        reject(new Error('No pudimos cargar el inicio de sesión de Google. Revisa tu conexión y recarga la página.'))
      }
      document.head.appendChild(script)
    })
  }
  return carga
}

function leerToken(credencial) {
  const base64 = credencial.split('.')[1].replace(/-/g, '+').replace(/_/g, '/')
  const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0))
  return JSON.parse(new TextDecoder().decode(bytes))
}

// Dibuja el botón de Google dentro de `contenedor` y llama a `callback` con el
// perfil y el token, que también abre la sesión de Supabase.
export async function mostrarBotonGoogle(contenedor, callback) {
  if (!GOOGLE_CLIENT_ID) throw new Error('Falta configurar VITE_GOOGLE_CLIENT_ID para iniciar sesión con Google.')
  const google = await cargarGoogle()
  alEntrar = callback
  if (!iniciado) {
    google.accounts.id.initialize({
      client_id: GOOGLE_CLIENT_ID,
      ux_mode: 'popup',
      callback: ({ credential }) => {
        const datos = leerToken(credential)
        alEntrar({ id: datos.sub, nombre: datos.name ?? datos.email, correo: datos.email, foto: datos.picture ?? null }, credential)
      },
    })
    iniciado = true
  }
  google.accounts.id.renderButton(contenedor, {
    type: 'standard',
    theme: 'outline',
    size: 'large',
    shape: 'pill',
    text: 'continue_with',
    logo_alignment: 'center',
    locale: 'es-419',
    width: Math.min(Math.max(contenedor.clientWidth, 200), 400),
  })
}

// Evita que Google vuelva a entrar solo con la misma cuenta después de cerrar sesión.
export function salirDeGoogle() {
  window.google?.accounts?.id?.disableAutoSelect()
}
