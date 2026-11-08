import { useEffect, useMemo, useRef, useState } from 'react'
import { Link, useLocation, useParams } from 'react-router-dom'
import QrScanner from 'qr-scanner'
import { useApp } from '../context/AppContext'
import Encabezado from '../components/Encabezado'
import Disponibilidad from '../components/Disponibilidad'
import BotonBilletera from '../components/BotonBilletera'
import { IconoEscaner, IconoExterno } from '../components/Iconos'
import { enlaceTx } from '../stellar/config'
import { abreviar, direccionValida, fechaLarga, numero, precio } from '../utils/formato'
import { DOMINIO, urlPortal } from '../portal'
import Portada from '../components/Portada'
import { supabaseListo } from '../supabase'
import { ANCHO_MINIMO, prepararImagen } from '../utils/imagen'

const CUENTA = /G[A-Z2-7]{55}/

// Panel de un evento para su organizador: ventas, entradas y personal de acceso.
export default function AdminEvento() {
  const { id } = useParams()
  const eventoId = Number(id)
  const { eventos, billetera, cargando, personal, esPersonal, autorizarPersonal, quitarPersonal, boletosDeEvento, cambiarPortada } = useApp()
  const { state } = useLocation()
  const [portada, setPortada] = useState({ subiendo: false, error: '' })
  const [boletos, setBoletos] = useState(null)
  const [confirmadas, setConfirmadas] = useState({})
  const [cuenta, setCuenta] = useState('')
  const [enviando, setEnviando] = useState(null)
  const [aviso, setAviso] = useState(null)
  const [camara, setCamara] = useState(false)
  const video = useRef(null)

  const evento = eventos.find((ev) => ev.id === eventoId)
  const lista = useMemo(() => personal[eventoId] ?? [], [personal, eventoId])

  useEffect(() => {
    let vigente = true
    boletosDeEvento(eventoId).then((l) => vigente && setBoletos(l)).catch(() => vigente && setBoletos([]))
    return () => {
      vigente = false
    }
  }, [eventoId, boletosDeEvento])

  // Cada cuenta recordada se confirma contra el contrato.
  useEffect(() => {
    let vigente = true
    for (const c of lista) {
      esPersonal(eventoId, c).then((v) => vigente && setConfirmadas((m) => ({ ...m, [c]: v }))).catch(() => {})
    }
    return () => {
      vigente = false
    }
  }, [eventoId, lista, esPersonal])

  // Cámara para leer el código de personal que muestra el portal staff.
  useEffect(() => {
    if (!camara || !video.current) return undefined
    const lector = new QrScanner(
      video.current,
      ({ data }) => {
        const m = CUENTA.exec(data)
        if (!m) return
        setCuenta(m[0])
        setCamara(false)
      },
      { returnDetailedScanResult: true, preferredCamera: 'environment', highlightScanRegion: false, maxScansPerSecond: 8 },
    )
    lector.start().catch((e) => {
      setAviso({ tipo: 'error', texto: `No pudimos abrir la cámara: ${e?.message ?? e}` })
      setCamara(false)
    })
    return () => {
      lector.stop()
      lector.destroy()
    }
  }, [camara])

  if (!evento) {
    return (
      <>
        <Encabezado titulo="Evento" atras />
        <p className="vacio">
          {cargando ? 'Leyendo el evento desde Stellar…' : <>Este evento no existe. <Link to="/organizador">Ver mis eventos</Link></>}
        </p>
      </>
    )
  }

  const autorizar = async (e) => {
    e.preventDefault()
    const c = cuenta.trim()
    if (!direccionValida(c)) {
      setAviso({ tipo: 'error', texto: 'Esa no es una cuenta Stellar válida. Empieza con G y tiene 56 caracteres.' })
      return
    }
    if (c === evento.organizador) {
      setAviso({ tipo: 'error', texto: 'Esa es tu cuenta de organizador y ya puede registrar entradas.' })
      return
    }
    setEnviando(c)
    setAviso(null)
    try {
      const hash = await autorizarPersonal(eventoId, c)
      setConfirmadas((m) => ({ ...m, [c]: true }))
      setCuenta('')
      setAviso({ tipo: 'exito', texto: `${abreviar(c)} ya puede registrar entradas en este evento.`, hash })
    } catch (err) {
      setAviso({ tipo: 'error', texto: err.message })
    }
    setEnviando(null)
  }

  const quitar = async (c) => {
    setEnviando(c)
    setAviso(null)
    try {
      const hash = await quitarPersonal(eventoId, c)
      setAviso({ tipo: 'exito', texto: `${abreviar(c)} ya no puede registrar entradas.`, hash })
    } catch (err) {
      setAviso({ tipo: 'error', texto: err.message })
    }
    setEnviando(null)
  }

  const elegirPortada = async (e) => {
    const archivo = e.target.files?.[0]
    e.target.value = ''
    if (!archivo) return
    setPortada({ subiendo: true, error: '' })
    try {
      const { blob, vista } = await prepararImagen(archivo)
      URL.revokeObjectURL(vista)
      await cambiarPortada(eventoId, blob)
      setPortada({ subiendo: false, error: '' })
    } catch (err) {
      setPortada({ subiendo: false, error: err.message })
    }
  }

  const usados = boletos?.filter((b) => b.estado === 'usado').length
  const ingresos = BigInt(evento.vendidos) * evento.precio

  return (
    <>
      <Encabezado titulo={evento.titulo} atras />
      <p className="introduccion">{fechaLarga(evento)}. {evento.recinto}.</p>
      {state?.aviso && <p className="alerta alerta--error" role="status">{state.aviso}</p>}

      {!billetera ? (
        <div className="vacio vacio--caja">
          <p>Conecta la billetera con la que creaste este evento.</p>
          <BotonBilletera grande />
        </div>
      ) : billetera !== evento.organizador ? (
        <p className="alerta alerta--error">
          La cuenta <span className="mono">{abreviar(billetera)}</span> no organiza este evento. Conecta la billetera con la que lo creaste.
        </p>
      ) : (
        <>
          <section className="tarjeta">
            <h3 className="tarjeta__titulo">Ventas</h3>
            <Disponibilidad evento={evento} detallada />
            <dl className="datos">
              <div>
                <dt>Recaudado</dt>
                <dd>{evento.precio === 0n ? 'Evento gratuito' : precio(ingresos)}</dd>
              </div>
              <div>
                <dt>Entradas registradas</dt>
                <dd>{usados == null ? 'Leyendo…' : `${numero(usados)} de ${numero(evento.vendidos)}`}</dd>
              </div>
            </dl>
            {evento.precio > 0n && <p className="nota">En esta versión cada pago llega a tu cuenta en el momento de la compra.</p>}
          </section>

          {supabaseListo && (
            <section className="tarjeta">
              <h3 className="tarjeta__titulo">Portada</h3>
              <Portada evento={evento} />
              <label className={`boton boton--contorno ${portada.subiendo ? 'boton--apagado' : ''}`}>
                {portada.subiendo ? 'Subiendo…' : evento.imagen ? 'Cambiar imagen' : 'Subir imagen'}
                <input
                  type="file"
                  accept="image/jpeg,image/png,image/webp,image/avif"
                  onChange={elegirPortada}
                  disabled={portada.subiendo}
                  className="sr-only"
                />
              </label>
              <p className={portada.error ? 'alerta alerta--error' : 'nota'}>
                {portada.error || `Horizontal, de al menos ${ANCHO_MINIMO} px de ancho.`}
              </p>
            </section>
          )}

          <section className="tarjeta">
            <h3 className="tarjeta__titulo">Personal de acceso</h3>
            <p className="nota">
              Quien vaya a escanear boletos abre staff.{DOMINIO}, conecta su billetera y te muestra su código. Escanéalo aquí o pega su
              cuenta y firma para autorizarla en el contrato.
            </p>

            <form className="personal__form" onSubmit={autorizar}>
              <label className="campo">
                <span>Cuenta Stellar del personal</span>
                <input
                  className="mono"
                  value={cuenta}
                  onChange={(e) => setCuenta(e.target.value)}
                  placeholder="G…"
                  autoComplete="off"
                  spellCheck={false}
                />
              </label>
              <div className="acciones">
                <button type="button" className="boton boton--contorno" onClick={() => setCamara((c) => !c)}>
                  <IconoEscaner width={18} height={18} /> {camara ? 'Apagar cámara' : 'Escanear código'}
                </button>
                <button type="submit" className="boton boton--claro" disabled={Boolean(enviando) || !cuenta.trim()}>
                  {enviando && enviando === cuenta.trim() ? 'Esperando a la red…' : 'Autorizar'}
                </button>
              </div>
            </form>

            {camara && (
              <div className="escaner__visor escaner__visor--activo">
                <video ref={video} className="escaner__video" muted playsInline />
              </div>
            )}

            {aviso && (
              <p className={`alerta alerta--${aviso.tipo}`} role="status">
                {aviso.texto}{' '}
                {aviso.hash && (
                  <a className="enlace enlace--externo" href={enlaceTx(aviso.hash)} target="_blank" rel="noreferrer">
                    Ver la transacción <IconoExterno width={12} height={12} />
                  </a>
                )}
              </p>
            )}

            {lista.length === 0 ? (
              <p className="nota">Todavía no autorizas a nadie desde este navegador. Tu cuenta de organizador siempre puede registrar entradas.</p>
            ) : (
              <ul className="lista personal__lista">
                {lista.map((c) => (
                  <li key={c} className="personal__item">
                    <span className="mono" title={c}>{abreviar(c)}</span>
                    <span className={`estado ${confirmadas[c] ? 'estado--valido' : 'estado--usado'}`}>
                      {confirmadas[c] == null ? 'Revisando' : confirmadas[c] ? 'Autorizada' : 'Sin permiso'}
                    </span>
                    {confirmadas[c] && (
                      <button type="button" className="boton boton--peligro boton--chico" onClick={() => quitar(c)} disabled={Boolean(enviando)}>
                        {enviando === c ? 'Firmando…' : 'Quitar'}
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </section>
        </>
      )}

      <section className="menu">
        <a href={urlPortal('app',`/eventos/${evento.id}`)} className="menu__opcion">
          <IconoExterno />
          <span><strong>Página del evento</strong><small>Lo que ven los compradores</small></span>
        </a>
        <a href={urlPortal('staff', `/acceso/${evento.id}`)} className="menu__opcion">
          <IconoEscaner />
          <span><strong>Escáner de este evento</strong><small>Abre la app de acceso en la puerta</small></span>
        </a>
      </section>
    </>
  )
}
