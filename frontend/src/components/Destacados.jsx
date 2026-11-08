import { useRef } from 'react'
import { Link } from 'react-router-dom'
import { IconoAtras } from './Iconos'
import { fechaCorta, precio } from '../utils/formato'

// Carrusel de eventos destacados al inicio de la tienda: los que tienen
// imagen y todavía tienen lugares, del más próximo al más lejano.
export default function Destacados({ eventos }) {
  const pista = useRef(null)
  const lista = eventos
    .filter((ev) => ev.imagen && ev.vendidos < ev.capacidad)
    .sort((a, b) => a.fecha.localeCompare(b.fecha))
    .slice(0, 5)

  if (!lista.length) return null

  const mover = (dir) => pista.current?.scrollBy({ left: dir * pista.current.clientWidth, behavior: 'smooth' })

  return (
    <section className="destacados" aria-label="Eventos destacados">
      <div ref={pista} className="destacados__pista">
        {lista.map((ev) => (
          <Link key={ev.id} to={`/eventos/${ev.id}`} className="destacado">
            <img src={ev.imagen} alt="" className="destacado__imagen" />
            <div className="destacado__texto">
              <span className="etiqueta-categoria">{ev.categoria}</span>
              <h2>{ev.titulo}</h2>
              <p>{fechaCorta(ev)}. {ev.recinto}</p>
              <span className="destacado__accion">
                {ev.precio === 0n ? 'Entrada gratis' : `Desde ${precio(ev.precio)}`}
                <span className="boton boton--claro boton--chico">Ver boletos</span>
              </span>
            </div>
          </Link>
        ))}
      </div>
      {lista.length > 1 && (
        <div className="destacados__flechas">
          <button type="button" onClick={() => mover(-1)} aria-label="Anterior"><IconoAtras width={20} height={20} /></button>
          <button type="button" onClick={() => mover(1)} aria-label="Siguiente"><IconoAtras width={20} height={20} style={{ transform: 'scaleX(-1)' }} /></button>
        </div>
      )}
    </section>
  )
}
