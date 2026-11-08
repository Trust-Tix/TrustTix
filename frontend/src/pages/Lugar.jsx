import { useMemo } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import Encabezado from '../components/Encabezado'
import TarjetaEvento from '../components/TarjetaEvento'
import useCoordenadas from '../hooks/useCoordenadas'
import { claveLugar } from '../utils/lugares'
import { comoLlegar, mapaEmbebido } from '../utils/mapas'

// Un recinto: dónde está y qué eventos tiene.
export default function Lugar() {
  const { clave } = useParams()
  const { eventos, cargando } = useApp()
  const delLugar = useMemo(() => eventos.filter((ev) => claveLugar(ev) === clave), [eventos, clave])
  const primero = useMemo(() => delLugar.slice(0, 1), [delLugar])
  const coordenadasDe = useCoordenadas(primero)

  if (!delLugar.length) {
    return (
      <>
        <Encabezado titulo="Lugar" atras />
        <p className="vacio">
          {cargando ? 'Leyendo el lugar desde Stellar…' : <>No hay eventos en este lugar. <Link to="/lugares">Ver lugares</Link></>}
        </p>
      </>
    )
  }

  const lugar = delLugar[0]
  const punto = coordenadasDe(lugar)
  const busqueda = `https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(`${lugar.recinto}, ${lugar.direccion ?? lugar.ciudad}`)}`

  return (
    <>
      <Encabezado titulo={lugar.recinto} atras />

      <section className="lugar-detalle">
        {punto && <iframe className="mapa mapa--marco lugar-detalle__mapa" title={`Mapa de ${lugar.recinto}`} src={mapaEmbebido(punto)} loading="lazy" />}
        <div className="lugar-detalle__texto">
          <p className="detalle__recinto">{lugar.recinto}</p>
          {lugar.direccion && <p className="detalle__direccion">{lugar.direccion}</p>}
          {!lugar.direccion && lugar.ciudad && <p className="detalle__direccion">{lugar.ciudad}</p>}
          <a href={punto ? comoLlegar(punto) : busqueda} target="_blank" rel="noreferrer" className="enlace">
            {punto ? 'Cómo llegar' : 'Abrir en Google Maps'}
          </a>
        </div>
      </section>

      <h2 className="subtitulo">Próximos eventos</h2>
      <div className="rejilla">
        {delLugar.map((ev) => (
          <TarjetaEvento key={ev.id} evento={ev} />
        ))}
      </div>
    </>
  )
}
