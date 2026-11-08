// Los lugares (recintos) salen de los eventos: no hay un registro aparte en el
// contrato. Dos eventos son del mismo lugar si comparten recinto y ciudad.

const sinAcentos = (t) => t.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()

export const claveLugar = (evento) =>
  sinAcentos(`${evento.recinto} ${evento.ciudad ?? ''}`)
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')

// Lista de lugares con sus eventos, ordenada por cantidad de eventos.
export function agruparLugares(eventos) {
  const mapa = new Map()
  for (const ev of eventos) {
    const clave = claveLugar(ev)
    if (!mapa.has(clave)) {
      mapa.set(clave, { clave, nombre: ev.recinto, ciudad: ev.ciudad, direccion: ev.direccion, eventos: [] })
    }
    mapa.get(clave).eventos.push(ev)
  }
  return [...mapa.values()].sort((a, b) => b.eventos.length - a.eventos.length || a.nombre.localeCompare(b.nombre))
}
