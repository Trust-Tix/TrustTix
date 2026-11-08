import { useMemo } from 'react'
import { Link } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import Encabezado from '../components/Encabezado'
import Portada from '../components/Portada'
import { IconoUbicacion } from '../components/Iconos'
import { agruparLugares } from '../utils/lugares'

// Recintos con eventos en TrustTix; cada uno lleva a su mapa y su cartelera.
export default function Lugares() {
  const { eventos, cargando } = useApp()
  const lugares = useMemo(() => agruparLugares(eventos), [eventos])

  return (
    <>
      <Encabezado titulo="Lugares" />
      {cargando && !lugares.length ? (
        <p className="vacio">Leyendo los lugares desde Stellar Testnet…</p>
      ) : (
        <div className="rejilla">
          {lugares.map((l) => (
            <Link key={l.clave} to={`/lugares/${l.clave}`} className="lugar">
              <Portada evento={l.eventos.find((ev) => ev.imagen) ?? l.eventos[0]} />
              <div className="lugar__texto">
                <h3>{l.nombre}</h3>
                <p className="meta">
                  <IconoUbicacion width={13} height={13} />
                  <span className="meta__texto">{l.ciudad}</span>
                </p>
                <p className="nota">{l.eventos.length === 1 ? '1 evento' : `${l.eventos.length} eventos`}</p>
              </div>
            </Link>
          ))}
        </div>
      )}
    </>
  )
}
