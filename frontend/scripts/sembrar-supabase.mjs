// Sube las imágenes del catálogo al bucket "eventos" y guarda sus datos en la
// tabla eventos de Supabase. Se corre después de sembrar los eventos en el
// contrato (contracts/scripts/sembrar.ps1):
//
//   $env:SUPABASE_SERVICE_ROLE_KEY = "<service_role key>"
//   npm run sembrar
//
// Cada entrada del catálogo se liga a su evento por el nombre registrado en el
// contrato, así sirve para cualquier contrato sin cambiar números. Si hay dos
// eventos con el mismo nombre, se usa el más reciente.
//
// Usa VITE_SUPABASE_URL y VITE_CONTRATO_ID de .env.local. La llave
// service_role salta las reglas de acceso: solo va en la terminal, nunca en
// .env.local ni en Vercel. Se puede correr de nuevo; actualiza lo que ya exista.

import { readFile } from 'node:fs/promises'
import { extname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createClient } from '@supabase/supabase-js'
import { Networks, contract } from '@stellar/stellar-sdk'

try {
  process.loadEnvFile('.env.local')
} catch {
  // Sin .env.local: valen las variables de la terminal.
}

const SEMILLA = fileURLToPath(new URL('../../supabase/semilla/', import.meta.url))
// Mismo contrato que la app (src/stellar/config.js).
const CONTRATO = process.env.VITE_CONTRATO_ID ?? 'CDKVPZI5HC25H6ILACGF24PNTTNYMUDQBSAJGMSUKEQEFUYVSAAKKRD4'
const RPC_URL = process.env.VITE_RPC_URL ?? 'https://soroban-testnet.stellar.org'
const URL_SUPABASE = process.env.VITE_SUPABASE_URL
const LLAVE = process.env.SUPABASE_SERVICE_ROLE_KEY
const TIPOS = { '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.png': 'image/png', '.webp': 'image/webp', '.avif': 'image/avif' }

if (!URL_SUPABASE || !LLAVE) {
  console.error('Falta VITE_SUPABASE_URL (en .env.local) o SUPABASE_SERVICE_ROLE_KEY (en la terminal).')
  process.exit(1)
}

// Nombre de cada evento del contrato → su número (gana el más reciente).
async function eventosDelContrato() {
  const cliente = await contract.Client.from({ contractId: CONTRATO, networkPassphrase: Networks.TESTNET, rpcUrl: RPC_URL })
  const total = Number((await cliente.total_eventos()).result)
  const porNombre = new Map()
  for (let id = 1; id <= total; id++) {
    const { result } = await cliente.consultar_evento({ evento_id: id })
    porNombre.set(result.unwrap().nombre, id)
  }
  return porNombre
}

const supabase = createClient(URL_SUPABASE, LLAVE, { auth: { persistSession: false } })
const catalogo = JSON.parse(await readFile(join(SEMILLA, 'catalogo.json'), 'utf8'))
const ids = await eventosDelContrato()
console.log(`Contrato ${CONTRATO}: ${ids.size} eventos.`)

for (const { nombre, imagen, ...datos } of catalogo) {
  const eventoId = ids.get(nombre)
  if (!eventoId) {
    console.log(`Sin evento en el contrato para "${nombre}"; se omite.`)
    continue
  }

  let ruta = null
  if (imagen) {
    ruta = `catalogo/${CONTRATO}-${eventoId}${extname(imagen)}`
    const archivo = await readFile(join(SEMILLA, 'imagenes', imagen))
    const subida = await supabase.storage
      .from('eventos')
      .upload(ruta, archivo, { contentType: TIPOS[extname(imagen).toLowerCase()], upsert: true, cacheControl: '31536000' })
    if (subida.error) throw new Error(`${imagen}: ${subida.error.message}`)
  }

  const fila = await supabase
    .from('eventos')
    // Sin imagen en el catálogo no se toca la portada que ya tenga el evento.
    .upsert(
      { contrato: CONTRATO, evento_id: eventoId, ...datos, ...(ruta ? { imagen: ruta } : {}), creado_por: null },
      { onConflict: 'contrato,evento_id' },
    )
  if (fila.error) throw new Error(`${nombre}: ${fila.error.message}`)
  console.log(`Evento ${eventoId} "${nombre}" listo${ruta ? ` (${ruta})` : ''}`)
}
