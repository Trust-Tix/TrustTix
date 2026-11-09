import { createClient } from '@supabase/supabase-js'
import { CONTRATO_ID } from './stellar/config'

// Base de datos de Supabase para lo que no vive en el contrato: imagen,
// categoría, dirección, coordenadas y descripción de cada evento.
// Sin VITE_SUPABASE_URL y VITE_SUPABASE_ANON_KEY la app sigue funcionando con
// lo que hay en la red, y los datos de un evento nuevo quedan en el navegador.

const URL_SUPABASE = import.meta.env.VITE_SUPABASE_URL ?? ''
const CLAVE_PUBLICA = import.meta.env.VITE_SUPABASE_ANON_KEY ?? ''
const BUCKET = 'eventos'

export const supabaseListo = Boolean(URL_SUPABASE && CLAVE_PUBLICA)

const cliente = supabaseListo ? createClient(URL_SUPABASE, CLAVE_PUBLICA) : null

// La sesión de Supabase se abre con el mismo token de Google del inicio de sesión.
export async function entrarConGoogle(token) {
  if (!cliente) return
  const { error } = await cliente.auth.signInWithIdToken({ provider: 'google', token })
  if (error) throw new Error(`No pudimos abrir la sesión en la base de datos: ${error.message}`)
}

export async function salirDeSupabase() {
  await cliente?.auth.signOut()
}

// ---------- Billetera TrustTix de la cuenta ----------

// Hay sesión de Supabase (la abre el login de Google): sin ella la billetera
// solo vive en este navegador.
export async function haySesion() {
  if (!cliente) return false
  const { data } = await cliente.auth.getUser().catch(() => ({ data: {} }))
  return Boolean(data?.user)
}

// La billetera guardada en la cuenta, o null si la cuenta todavía no tiene.
export async function leerBilleteraDeCuenta() {
  const { data, error } = await cliente.from('billeteras').select('direccion, secreta').maybeSingle()
  if (error) throw new Error(`No pudimos leer tu billetera de la cuenta: ${error.message}`)
  return data
}

// Guarda (o reemplaza) la billetera de la cuenta.
export async function guardarBilleteraEnCuenta(direccion, secreta) {
  const usuario = await usuarioActual()
  const { error } = await cliente.from('billeteras').upsert({ usuario: usuario.id, direccion, secreta }, { onConflict: 'usuario' })
  if (error) throw new Error(`No pudimos guardar tu billetera en tu cuenta: ${error.message}`)
}

const urlImagen = (ruta) => {
  if (!ruta) return undefined
  if (/^https?:\/\//.test(ruta)) return ruta
  return cliente.storage.from(BUCKET).getPublicUrl(ruta).data.publicUrl
}

// Datos de todos los eventos de este contrato, por número de evento.
export async function leerMetadatos() {
  if (!cliente) return {}
  const { data, error } = await cliente.from('eventos').select('*').eq('contrato', CONTRATO_ID)
  if (error) throw new Error(`No pudimos leer los datos de los eventos: ${error.message}`)
  return Object.fromEntries(
    data.map((f) => [
      f.evento_id,
      {
        categoria: f.categoria,
        ciudad: f.ciudad,
        direccion: f.direccion,
        descripcion: f.descripcion,
        organizadorNombre: f.organizador_nombre,
        fechaFin: f.fecha_fin ?? undefined,
        lat: f.lat,
        lng: f.lng,
        imagen: urlImagen(f.imagen),
      },
    ]),
  )
}

async function usuarioActual() {
  const { data } = await cliente.auth.getUser()
  if (!data.user) throw new Error('Tu sesión de la base de datos venció. Cierra sesión y vuelve a entrar con Google.')
  return data.user
}

// Sube la imagen a la carpeta del usuario y devuelve su ruta en el bucket.
export async function subirImagen(eventoId, imagen) {
  const usuario = await usuarioActual()
  const ruta = `${usuario.id}/${CONTRATO_ID}-${eventoId}-${Date.now()}.webp`
  const { error } = await cliente.storage.from(BUCKET).upload(ruta, imagen, { contentType: 'image/webp', cacheControl: '31536000' })
  if (error) throw new Error(`No pudimos subir la imagen: ${error.message}`)
  return ruta
}

// Crea o actualiza los datos de un evento. Solo quien los registró puede cambiarlos.
export async function guardarMetadatos(eventoId, datos) {
  await usuarioActual()
  const fila = { contrato: CONTRATO_ID, evento_id: eventoId }
  const columnas = {
    categoria: 'categoria',
    ciudad: 'ciudad',
    direccion: 'direccion',
    descripcion: 'descripcion',
    organizadorNombre: 'organizador_nombre',
    lat: 'lat',
    lng: 'lng',
    imagen: 'imagen',
  }
  for (const [clave, columna] of Object.entries(columnas)) {
    if (datos[clave] !== undefined) fila[columna] = datos[clave]
  }
  const { error } = await cliente.from('eventos').upsert(fila, { onConflict: 'contrato,evento_id' })
  if (error) {
    const permiso = /row-level security/i.test(error.message)
    throw new Error(permiso ? 'Solo la cuenta que registró los datos de este evento puede cambiarlos.' : `No pudimos guardar los datos: ${error.message}`)
  }
}
