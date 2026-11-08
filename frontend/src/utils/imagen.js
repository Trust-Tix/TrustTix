// Portadas de eventos. El carrusel de la tienda ocupa hasta ~1,200 px de ancho y
// en pantallas de alta densidad se ve al doble, así que una imagen chica se
// pixelea. Se exige un ancho mínimo y se guarda en WebP a un tamaño razonable.

export const ANCHO_MINIMO = 1600
const ANCHO_MAXIMO = 2400
const CALIDAD = 0.85

export async function prepararImagen(archivo) {
  if (!archivo.type.startsWith('image/')) throw new Error('El archivo no es una imagen.')
  let mapa
  try {
    mapa = await createImageBitmap(archivo)
  } catch {
    throw new Error('No pudimos leer esa imagen. Prueba con un JPG, PNG o WebP.')
  }
  if (mapa.width < ANCHO_MINIMO) {
    mapa.close()
    throw new Error(`La imagen mide ${mapa.width} px de ancho. Usa una de al menos ${ANCHO_MINIMO} px para que no se pixelee.`)
  }
  const escala = Math.min(1, ANCHO_MAXIMO / mapa.width)
  const lienzo = document.createElement('canvas')
  lienzo.width = Math.round(mapa.width * escala)
  lienzo.height = Math.round(mapa.height * escala)
  lienzo.getContext('2d').drawImage(mapa, 0, 0, lienzo.width, lienzo.height)
  mapa.close()
  const blob = await new Promise((listo) => lienzo.toBlob(listo, 'image/webp', CALIDAD))
  if (!blob) throw new Error('No pudimos preparar la imagen en este navegador.')
  return { blob, vista: URL.createObjectURL(blob), ancho: lienzo.width, alto: lienzo.height }
}
