import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import Modal from './Modal'
import { IconoEscudo } from './Iconos'
import { abreviar, folio, porcentaje, precio } from '../utils/formato'
import { ES_MAINNET } from '../stellar/config'

// Boletos que sus dueños revenden dentro de TrustTix. Cada uno se compra en una
// sola transacción: pago al vendedor, comisión y cambio de dueño, o nada.
export default function Reventas({ evento }) {
  const navigate = useNavigate()
  const { usuario, billetera, reventasDeEvento, comprarReventa } = useApp()
  const [ofertas, setOfertas] = useState(null)
  const [elegida, setElegida] = useState(null)
  const [firmando, setFirmando] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    let vigente = true
    reventasDeEvento(evento.id).then(
      (lista) => vigente && setOfertas(lista),
      () => vigente && setOfertas([]),
    )
    return () => {
      vigente = false
    }
  }, [evento.id, reventasDeEvento])

  if (!ofertas?.length) return null

  const comprar = async () => {
    setError('')
    setFirmando(true)
    try {
      const hash = await comprarReventa(elegida.boleto.id)
      navigate(`/boletos/${elegida.boleto.id}`, { state: { recienComprado: true, hash } })
    } catch (e) {
      setError(e.message)
      setFirmando(false)
    }
  }

  return (
    <section className="detalle__bloque detalle__reventa">
      <h3 className="detalle__subtitulo">Reventa</h3>
      <ul className="reventas">
        {ofertas.map((o) => {
          const mio = o.boleto.dueno === billetera
          return (
            <li key={o.boleto.id} className="reventas__fila">
              <div>
                <strong className="mono">{folio(o.boleto.id)}</strong>
                <small>{mio ? 'Tu boleto' : `Lo vende ${abreviar(o.boleto.dueno)}`}</small>
              </div>
              <span className="precio">{precio(o.precio)}</span>
              {mio ? (
                <Link to={`/boletos/${o.boleto.id}`} className="boton boton--contorno">Ver</Link>
              ) : usuario || (ES_MAINNET && billetera) ? (
                <button type="button" className="boton boton--claro" onClick={() => setElegida(o)}>
                  Comprar
                </button>
              ) : (
                <Link to="/login" state={{ desde: `/eventos/${evento.id}` }} className="boton boton--contorno">
                  Entrar
                </Link>
              )}
            </li>
          )
        })}
      </ul>
      <p className="sello">
        <IconoEscudo width={18} height={18} />
        <span>
          <strong>Reventa con tope.</strong> Ninguno pasa del {porcentaje(evento.topeReventa)} del precio original, y el boleto
          cambia de dueño en el mismo momento en que pagas.
        </span>
      </p>

      {elegida && (
        <Modal titulo="Compra en reventa" onCerrar={() => !firmando && setElegida(null)}>
          <dl className="resumen">
            <div><dt>Evento</dt><dd>{evento.titulo}</dd></div>
            <div><dt>Boleto</dt><dd className="mono">{folio(elegida.boleto.id)}</dd></div>
            <div><dt>Lo vende</dt><dd className="mono">{abreviar(elegida.boleto.dueno)}</dd></div>
            <div><dt>Total</dt><dd>{precio(elegida.precio)}</dd></div>
          </dl>
          <p className="nota">El pago y el cambio de dueño van juntos: si algo falla, no se cobra nada y el boleto sigue con su dueño.</p>
          {error && <p className="alerta alerta--error" role="alert">{error}</p>}
          <div className="modal__acciones">
            <button type="button" className="boton boton--contorno" onClick={() => setElegida(null)} disabled={firmando}>
              Cancelar
            </button>
            <button type="button" className="boton boton--claro" onClick={comprar} disabled={firmando}>
              {firmando ? 'Pagando…' : 'Pagar'}
            </button>
          </div>
        </Modal>
      )}
    </section>
  )
}
