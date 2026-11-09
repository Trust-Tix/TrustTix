import { useMemo, useState } from 'react'
import { useApp } from '../context/AppContext'
import { CATEGORIAS } from '../data/eventos'
import Encabezado from '../components/Encabezado'
import TarjetaEvento from '../components/TarjetaEvento'
import Destacados from '../components/Destacados'
import { IconoBuscar, IconoUbicacion } from '../components/Iconos'
import useUbicacion from '../hooks/useUbicacion'
import useCoordenadas from '../hooks/useCoordenadas'
import { distanciaKm } from '../utils/mapas'

const sinAcentos = (t) => t.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()

const TEXTO_UBICACION = {
  inactiva: 'Usar mi ubicación',
  buscando: 'Buscando tu ubicación…',
  denegada: 'Ubicación bloqueada. Actívala en los permisos del sitio',
  error: 'No pudimos ubicarte. Intentar de nuevo',
}

export default function Inicio() {
  const { eventos, cargando, errorRed } = useApp()
  const [busqueda, setBusqueda] = useState('')
  const [categoria, setCategoria] = useState('Todos')
  const { ubicacion, estado, localizar } = useUbicacion()
  const coordenadasDe = useCoordenadas(eventos, Boolean(ubicacion))

  // Con ubicación, los eventos van del más cercano al más lejano; los que no
  // se pudieron ubicar quedan al final en su orden original.
  const visibles = useMemo(() => {
    const q = sinAcentos(busqueda.trim())
    const lista = eventos
      .filter(
        (ev) =>
          (categoria === 'Todos' || ev.categoria === categoria) &&
          (!q || sinAcentos(`${ev.titulo} ${ev.recinto} ${ev.ciudad}`).includes(q)),
      )
      .map((ev) => {
        const punto = ubicacion ? coordenadasDe(ev) : null
        return { evento: ev, km: punto ? distanciaKm(ubicacion, punto) : null }
      })
    if (ubicacion) lista.sort((a, b) => (a.km ?? Infinity) - (b.km ?? Infinity) || 0)
    return lista
  }, [eventos, busqueda, categoria, ubicacion, coordenadasDe])

  return (
    <>
      <Encabezado titulo="Inicio" portada />

      <Destacados eventos={eventos} />

      <section className="seccion">
        <h2 className="subtitulo">{ubicacion ? 'Eventos cerca de ti' : 'Principales eventos'}</h2>
        <div className="ubicacion">
          <IconoUbicacion width={14} height={14} />
          {estado === 'lista' ? (
            <button type="button" className="ubicacion__boton" onClick={localizar} title="Actualizar mi ubicación">
              {ubicacion.ciudad ?? 'Tu ubicación actual'}
            </button>
          ) : (
            <button
              type="button"
              className="ubicacion__boton ubicacion__boton--accion"
              onClick={localizar}
              disabled={estado === 'buscando'}
            >
              {TEXTO_UBICACION[estado]}
            </button>
          )}
        </div>

        <label className="buscador">
          <span className="sr-only">Buscar eventos</span>
          <input
            type="search"
            placeholder="Buscar eventos"
            value={busqueda}
            onChange={(e) => setBusqueda(e.target.value)}
          />
          <IconoBuscar />
        </label>

        <div className="chips" role="tablist" aria-label="Categorías">
          {CATEGORIAS.map((c) => (
            <button
              key={c}
              type="button"
              role="tab"
              aria-selected={categoria === c}
              className={`chip ${categoria === c ? 'chip--activo' : ''}`}
              onClick={() => setCategoria(c)}
            >
              {c}
            </button>
          ))}
        </div>
      </section>

      {errorRed && <p className="alerta alerta--error">{errorRed}</p>}
      {cargando ? (
        <p className="vacio">Cargando eventos…</p>
      ) : visibles.length > 0 ? (
        <div className="rejilla">
          {visibles.map(({ evento, km }) => (
            <TarjetaEvento key={evento.id} evento={evento} km={km} />
          ))}
        </div>
      ) : (
        <p className="vacio">No encontramos eventos con esa búsqueda.</p>
      )}
    </>
  )
}
