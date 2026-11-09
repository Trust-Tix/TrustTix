import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import { CATEGORIAS } from '../data/eventos'
import Encabezado from '../components/Encabezado'
import BotonBilletera from '../components/BotonBilletera'
import useUbicacion from '../hooks/useUbicacion'
import { aStroops } from '../utils/formato'
import { buscarLugar, mapaEmbebido } from '../utils/mapas'
import { ANCHO_MINIMO, prepararImagen } from '../utils/imagen'
import { supabaseListo } from '../supabase'

const VACIO = {
  titulo: '',
  categoria: 'Conciertos',
  recinto: '',
  ciudad: '',
  direccion: '',
  fecha: '',
  hora: '20:00',
  capacidad: '',
  precioXLM: '',
  descripcion: '',
  organizadorNombre: '',
}

export default function CrearEvento() {
  const { billetera, crearEvento } = useApp()
  const navigate = useNavigate()
  const { ubicacion } = useUbicacion()
  const [datos, setDatos] = useState(() => ({ ...VACIO, ciudad: ubicacion?.ciudad ?? '' }))
  const [punto, setPunto] = useState(null)
  const [ubicando, setUbicando] = useState(false)
  const [errorMapa, setErrorMapa] = useState('')
  const [enviando, setEnviando] = useState(false)
  const [error, setError] = useState('')
  const [imagen, setImagen] = useState(null)
  const [errorImagen, setErrorImagen] = useState('')

  const elegirImagen = async (e) => {
    const archivo = e.target.files?.[0]
    if (!archivo) return
    setErrorImagen('')
    try {
      const lista = await prepararImagen(archivo)
      setImagen((anterior) => {
        if (anterior) URL.revokeObjectURL(anterior.vista)
        return lista
      })
    } catch (err) {
      setErrorImagen(err.message)
      e.target.value = ''
    }
  }

  // La ciudad se propone con la ubicación del organizador, sin pisar lo que ya escribió.
  useEffect(() => {
    if (ubicacion?.ciudad) setDatos((d) => (d.ciudad ? d : { ...d, ciudad: ubicacion.ciudad }))
  }, [ubicacion])

  const cambiar = (campo) => (e) => {
    setDatos((d) => ({ ...d, [campo]: e.target.value }))
    if (campo === 'direccion' || campo === 'ciudad' || campo === 'recinto') setPunto(null)
  }

  // Primero la dirección; si no aparece, el nombre del recinto en esa ciudad.
  const ubicar = async () => {
    const consultas = [[datos.direccion, datos.ciudad], [datos.recinto, datos.ciudad]]
      .map((partes) => partes.filter((p) => p.trim()).join(', '))
      .filter(Boolean)
    for (const consulta of consultas) {
      const encontrado = await buscarLugar(consulta)
      if (encontrado) return encontrado
    }
    return null
  }

  const verEnMapa = async () => {
    setUbicando(true)
    setErrorMapa('')
    try {
      const encontrado = await ubicar()
      setPunto(encontrado)
      if (!encontrado) setErrorMapa('No encontramos esa dirección en el mapa. Revisa la calle y la ciudad.')
    } catch (e) {
      setErrorMapa(e.message)
    }
    setUbicando(false)
  }

  const enviar = async (e) => {
    e.preventDefault()
    const capacidad = Number(datos.capacidad)
    const precioXLM = Number(datos.precioXLM || 0)
    if (!Number.isInteger(capacidad) || capacidad < 1) {
      setError('La capacidad debe ser un número entero mayor que cero.')
      return
    }
    if (!(precioXLM >= 0)) {
      setError('El precio no puede ser negativo.')
      return
    }
    setError('')
    setEnviando(true)
    try {
      const coordenadas = punto ?? (await ubicar().catch(() => null))
      const { id, aviso } = await crearEvento({
        ...datos,
        capacidad,
        precio: aStroops(precioXLM),
        coordenadas,
        imagen: imagen?.blob,
      })
      navigate(`/organizador/${id}`, { state: { aviso } })
    } catch (err) {
      setError(err.message)
      setEnviando(false)
    }
  }

  return (
    <>
      <Encabezado titulo="Crear evento" atras />

      {!billetera ? (
        <div className="vacio vacio--caja">
          <p>El evento se registra con la cuenta de tu billetera, que será la única que pueda administrarlo.</p>
          <BotonBilletera grande />
        </div>
      ) : (
        <form className="formulario tarjeta" onSubmit={enviar}>
          <label className="campo">
            <span>Nombre del evento</span>
            <input required value={datos.titulo} onChange={cambiar('titulo')} placeholder="Ej. Noche de rock" />
          </label>
          {supabaseListo && (
            <div className="campo">
              <span>Portada</span>
              {imagen && <img className="portada-previa" src={imagen.vista} alt="Vista previa de la portada" />}
              <label className="boton boton--contorno">
                {imagen ? 'Cambiar imagen' : 'Elegir imagen'}
                <input type="file" accept="image/jpeg,image/png,image/webp,image/avif" onChange={elegirImagen} className="sr-only" />
              </label>
              <p className="nota">
                {errorImagen || `Horizontal, de al menos ${ANCHO_MINIMO} px de ancho. Se guarda optimizada para que no se pixelee.`}
              </p>
            </div>
          )}
          <label className="campo">
            <span>Organiza (opcional)</span>
            <input value={datos.organizadorNombre} onChange={cambiar('organizadorNombre')} placeholder="Ej. Producciones Morelos" />
          </label>
          <div className="campo-doble">
            <label className="campo">
              <span>Categoría</span>
              <select value={datos.categoria} onChange={cambiar('categoria')}>
                {CATEGORIAS.filter((c) => c !== 'Todos').map((c) => <option key={c}>{c}</option>)}
              </select>
            </label>
            <label className="campo">
              <span>Ciudad</span>
              <input required value={datos.ciudad} onChange={cambiar('ciudad')} placeholder="Ej. Cuernavaca, Morelos" />
            </label>
          </div>
          <label className="campo">
            <span>Recinto</span>
            <input required value={datos.recinto} onChange={cambiar('recinto')} placeholder="Ej. Jardín Borda" />
          </label>
          <label className="campo">
            <span>Dirección</span>
            <input required value={datos.direccion} onChange={cambiar('direccion')} placeholder="Calle, número y colonia" />
          </label>
          <div className="campo">
            <button
              type="button"
              className="boton boton--contorno"
              onClick={verEnMapa}
              disabled={ubicando || !(datos.direccion.trim() || datos.recinto.trim())}
            >
              {ubicando ? 'Buscando en el mapa…' : punto ? 'Volver a buscar en el mapa' : 'Ver en el mapa'}
            </button>
            {punto && (
              <iframe className="mapa mapa--marco" title="Ubicación del evento" src={mapaEmbebido(punto)} loading="lazy" />
            )}
            {punto && <p className="nota">Si el punto no es el correcto, ajusta la dirección antes de crear el evento.</p>}
            {errorMapa && <p className="nota">{errorMapa}</p>}
          </div>
          <div className="campo-doble">
            <label className="campo">
              <span>Fecha</span>
              <input required type="date" value={datos.fecha} onChange={cambiar('fecha')} />
            </label>
            <label className="campo">
              <span>Hora</span>
              <input required type="time" value={datos.hora} onChange={cambiar('hora')} />
            </label>
          </div>
          <div className="campo-doble">
            <label className="campo">
              <span>Capacidad máxima</span>
              <input required type="number" min="1" step="1" inputMode="numeric" value={datos.capacidad} onChange={cambiar('capacidad')} />
            </label>
            <label className="campo">
              <span>Precio en XLM (0 = gratis)</span>
              <input type="number" min="0" step="0.1" inputMode="decimal" value={datos.precioXLM} onChange={cambiar('precioXLM')} placeholder="0" />
            </label>
          </div>
          <label className="campo">
            <span>Descripción</span>
            <textarea rows={3} value={datos.descripcion} onChange={cambiar('descripcion')} />
          </label>

          <p className="sello">
            <span>
              <strong>La capacidad no se podrá aumentar después.</strong> El contrato rechazará cualquier venta por encima de ese número.
            </span>
          </p>
          <p className="nota">
            En el contrato quedan el nombre, el recinto, la fecha, la capacidad y el precio.{' '}
            {supabaseListo
              ? 'La portada, la categoría, la dirección y la descripción se guardan en la base de datos de TrustTix.'
              : 'La categoría, la dirección y la descripción se guardan en este navegador.'}
          </p>

          {error && <p className="alerta alerta--error" role="alert">{error}</p>}

          <button type="submit" className="boton boton--claro boton--bloque" disabled={enviando}>
            {enviando ? 'Creando evento…' : 'Crear evento'}
          </button>
        </form>
      )}
    </>
  )
}
