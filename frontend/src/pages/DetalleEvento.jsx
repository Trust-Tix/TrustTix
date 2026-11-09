import { useEffect, useMemo, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import Encabezado from '../components/Encabezado'
import Portada from '../components/Portada'
import Disponibilidad from '../components/Disponibilidad'
import Modal from '../components/Modal'
import { IconoCalendario, IconoEscudo, IconoUbicacion } from '../components/Iconos'
import { abreviar, diaMes, fechaLarga, numero, precio } from '../utils/formato'
import { enlaceCuenta } from '../stellar/config'
import { patrocinioActivo } from '../stellar/contrato'
import useUbicacion from '../hooks/useUbicacion'
import useCoordenadas from '../hooks/useCoordenadas'
import { comoLlegar, distanciaKm, mapaEmbebido, textoDistancia } from '../utils/mapas'
import { claveLugar } from '../utils/lugares'

const xlmTexto = (n) => `${n.toLocaleString('es-MX', { maximumFractionDigits: 2 })} XLM`

export default function DetalleEvento() {
  const { id } = useParams()
  const navigate = useNavigate()
  const { eventos, usuario, billetera, infoBilletera, saldo, misBoletos, comprar, cargando } = useApp()
  const [confirmando, setConfirmando] = useState(false)
  const [firmando, setFirmando] = useState(false)
  const [error, setError] = useState('')
  const [sinComisiones, setSinComisiones] = useState(false)
  const { ubicacion } = useUbicacion()

  useEffect(() => {
    patrocinioActivo().then(setSinComisiones)
  }, [])

  const evento = eventos.find((ev) => ev.id === Number(id))
  const soloEste = useMemo(() => (evento ? [evento] : []), [evento])
  const coordenadasDe = useCoordenadas(soloEste)
  if (!evento && cargando) {
    return (
      <>
        <Encabezado titulo="Detalles" atras />
        <p className="vacio">Cargando el evento…</p>
      </>
    )
  }
  if (!evento) {
    return (
      <>
        <Encabezado titulo="Detalles" atras />
        <p className="vacio">Este evento no existe. <Link to="/inicio">Volver al inicio</Link></p>
      </>
    )
  }

  const miBoleto = misBoletos.find((b) => b.eventoId === evento.id)
  const agotado = evento.vendidos >= evento.capacidad
  const { dia, mes } = diaMes(evento.fecha)
  const mapa = `https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(`${evento.recinto}, ${evento.direccion}`)}`
  const punto = coordenadasDe(evento)
  const km = ubicacion && punto ? distanciaKm(ubicacion, punto) : null

  const precioXLM = Number(evento.precio) / 1e7
  const saldoActual = typeof saldo === 'string' ? Number(saldo) : null
  const sinSaldo = saldoActual != null && saldoActual < precioXLM

  const confirmarCompra = async () => {
    setError('')
    setFirmando(true)
    try {
      const { boletoId, hash } = await comprar(evento.id)
      navigate(`/boletos/${boletoId}`, { state: { recienComprado: true, hash } })
    } catch (e) {
      setError(e.message)
      setFirmando(false)
    }
  }

  let accion
  if (miBoleto) {
    accion = <Link to={`/boletos/${miBoleto.id}`} className="boton boton--claro">Ver mi boleto</Link>
  } else if (agotado) {
    accion = <span className="boton boton--apagado" aria-disabled="true">Agotado</span>
  } else if (!usuario) {
    accion = (
      <Link to="/login" state={{ desde: `/eventos/${evento.id}` }} className="boton boton--claro">
        Inicia sesión para comprar
      </Link>
    )
  } else {
    // Sin billetera también se compra: la billetera TrustTix se prepara al confirmar.
    accion = (
      <button type="button" className="boton boton--claro" onClick={() => setConfirmando(true)}>
        {evento.precio === 0n ? 'Obtener boleto' : 'Comprar boleto'}
      </button>
    )
  }

  return (
    <>
      <Encabezado titulo="Detalles" atras />

      <article className="detalle">
        <div className="detalle__columna">
          <div className="detalle__portada">
            <Portada evento={evento} alta />
          </div>

          {evento.descripcion && (
            <section className="detalle__bloque detalle__acerca">
              <h3 className="detalle__subtitulo">Acerca del evento</h3>
              <p className="detalle__descripcion">{evento.descripcion}</p>
            </section>
          )}

          <section className="detalle__bloque detalle__ubicacion">
            <h3 className="detalle__subtitulo">Ubicación</h3>
            <div className={`detalle__lugar ${punto ? 'detalle__lugar--mapa' : ''}`}>
              {punto ? (
                <iframe className="mapa mapa--marco" title={`Mapa de ${evento.recinto}`} src={mapaEmbebido(punto)} loading="lazy" />
              ) : (
                <a href={mapa} target="_blank" rel="noreferrer" className="mapa" aria-label="Abrir en Google Maps">
                  <span className="mapa__pin" aria-hidden="true" />
                  <span className="mapa__etiqueta">{evento.recinto}</span>
                </a>
              )}
              <div>
                <p className="detalle__recinto">
                  <Link to={`/lugares/${claveLugar(evento)}`}>{evento.recinto}</Link>
                </p>
                <p className="detalle__direccion">{evento.direccion}</p>
                {km != null && <p className="nota">{textoDistancia(km)} de tu ubicación</p>}
                <a href={punto ? comoLlegar(punto) : mapa} target="_blank" rel="noreferrer" className="enlace">
                  {punto ? 'Cómo llegar' : 'Abrir en Google Maps'}
                </a>
              </div>
            </div>
          </section>
        </div>

        <div className="detalle__columna">
          <header className="detalle__cabecera">
            <span className="etiqueta-categoria">{evento.categoria}</span>
            <h2 className="detalle__titulo">{evento.titulo}</h2>
            <p className="meta meta--grande">
              <IconoUbicacion width={16} height={16} /> {[evento.recinto, evento.ciudad].filter(Boolean).join(' · ')}
            </p>
          </header>

          <dl className="datos detalle__bloque">
            <div>
              <dt><IconoCalendario width={14} height={14} /> Fecha</dt>
              <dd>{fechaLarga(evento)}</dd>
            </div>
            <div>
              <dt>Organiza</dt>
              <dd>
                {evento.organizadorNombre ?? <span className="mono">{abreviar(evento.organizador)}</span>}
                <a className="verificado" href={enlaceCuenta(evento.organizador)} target="_blank" rel="noreferrer" title="Cuenta del organizador registrada en el contrato">
                  <IconoEscudo width={13} height={13} /> Verificado
                </a>
              </dd>
            </div>
          </dl>

          <section className="detalle__bloque detalle__lugares">
            <h3 className="detalle__subtitulo">Lugares</h3>
            <Disponibilidad evento={evento} detallada />
            <p className="sello">
              <IconoEscudo width={18} height={18} />
              <span>
                <strong>Capacidad protegida.</strong> Este evento no puede vender más de {numero(evento.capacidad)} boletos:
                la regla está en el contrato, no en la boletera.
              </span>
            </p>
          </section>

          <div className="detalle__compra">
            <div className="fecha-grande">
              <strong>{dia}</strong>
              <span>{mes}</span>
            </div>
            <div className="precio precio--grande">
              {precio(evento.precio)}
              {evento.precio > 0n && (
                <span className="precio__usdc">{sinComisiones ? 'Precio final, sin comisiones' : 'Saldo de prueba, sin dinero real'}</span>
              )}
            </div>
            {accion}
          </div>
        </div>
      </article>

      {confirmando && (
        <Modal titulo={evento.precio === 0n ? 'Confirma tu boleto' : 'Confirma tu compra'} onCerrar={() => !firmando && setConfirmando(false)}>
          <dl className="resumen">
            <div><dt>Evento</dt><dd>{evento.titulo}</dd></div>
            <div><dt>Total</dt><dd>{precio(evento.precio)}</dd></div>
            <div>
              <dt>Paga</dt>
              <dd>
                {!billetera
                  ? 'Billetera TrustTix, se crea al confirmar'
                  : infoBilletera?.tipo === 'trusttix'
                    ? 'Tu billetera TrustTix'
                    : `${infoBilletera?.nombre ?? 'Tu billetera'} (${abreviar(billetera)})`}
              </dd>
            </div>
            {saldoActual != null && (
              <div><dt>Saldo después</dt><dd>{xlmTexto(saldoActual - precioXLM)}</dd></div>
            )}
          </dl>
          <p className="nota">
            {!billetera || infoBilletera?.tipo === 'trusttix'
              ? 'Pagas con tu billetera TrustTix, sin ventanas extra.'
              : `${infoBilletera?.nombre ?? 'Tu billetera'} te pedirá confirmar el pago.`}{' '}
            {sinComisiones && 'TrustTix cubre la comisión de la red. '}
            El cobro y el boleto van juntos: si algo falla, no se cobra nada.
          </p>
          {sinSaldo && <p className="alerta alerta--error">Tu saldo no alcanza para este boleto.</p>}
          {error && <p className="alerta alerta--error" role="alert">{error}</p>}
          <div className="modal__acciones">
            <button type="button" className="boton boton--contorno" onClick={() => setConfirmando(false)} disabled={firmando}>
              Cancelar
            </button>
            <button type="button" className="boton boton--claro" onClick={confirmarCompra} disabled={firmando || sinSaldo}>
              {firmando ? (billetera ? 'Pagando…' : 'Preparando…') : evento.precio === 0n ? 'Confirmar' : 'Pagar'}
            </button>
          </div>
        </Modal>
      )}
    </>
  )
}
