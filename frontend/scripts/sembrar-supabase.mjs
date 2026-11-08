// Sube las imágenes del catálogo al bucket "eventos" y guarda sus datos en la
// tabla eventos de Supabase. Se corre una vez, después de aplicar la migración:
//
//   $env:SUPABASE_SERVICE_ROLE_KEY = "<service_role key>"
//   npm run sembrar
//
// Usa VITE_SUPABASE_URL y VITE_CONTRATO_ID de .env.local. La llave
// service_role salta las reglas de acceso: solo va en la terminal, nunca en
// .env.local ni en Vercel. Se puede correr de nuevo; actualiza lo que ya exista.

import { readFile } from 'node:fs/promises'
import { extname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createClient } from '@supabase/supabase-js'

try {
  process.loadEnvFile('.env.local')
} catch {
  // Sin .env.local: valen las variables de la terminal.
}

const SEMILLA = fileURLToPath(new URL('../../supabase/semilla/', import.meta.url))
const CONTRATO = process.env.VITE_CONTRATO_ID ?? 'CCWWMLZANZV54WMJQ5JQ6XP2AAUE5BGKK3Q5CPQZEJMREJ4OYRVN36GD'
const URL_SUPABASE = process.env.VITE_SUPABASE_URL
const LLAVE = process.env.SUPABASE_SERVICE_ROLE_KEY
const TIPOS = { '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.png': 'image/png', '.webp': 'image/webp', '.avif': 'image/avif' }

if (!URL_SUPABASE || !LLAVE) {
  console.error('Falta VITE_SUPABASE_URL (en .env.local) o SUPABASE_SERVICE_ROLE_KEY (en la terminal).')
  process.exit(1)
}

const supabase = createClient(URL_SUPABASE, LLAVE, { auth: { persistSession: false } })
const catalogo = JSON.parse(await readFile(join(SEMILLA, 'catalogo.json'), 'utf8'))

for (const { imagen, ...datos } of catalogo) {
  const ruta = `catalogo/${CONTRATO}-${datos.evento_id}${extname(imagen)}`
  const archivo = await readFile(join(SEMILLA, 'imagenes', imagen))
  const subida = await supabase.storage
    .from('eventos')
    .upload(ruta, archivo, { contentType: TIPOS[extname(imagen).toLowerCase()], upsert: true, cacheControl: '31536000' })
  if (subida.error) throw new Error(`${imagen}: ${subida.error.message}`)

  const fila = await supabase
    .from('eventos')
    .upsert({ contrato: CONTRATO, ...datos, imagen: ruta, creado_por: null }, { onConflict: 'contrato,evento_id' })
  if (fila.error) throw new Error(`Evento ${datos.evento_id}: ${fila.error.message}`)
  console.log(`Evento ${datos.evento_id} listo (${ruta})`)
}
