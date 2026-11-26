import { useState } from 'react'
import { useApp } from '../context/AppContext'
import Modal from './Modal'
import { aStroops, porcentaje, precio, xlm } from '../utils/formato'

// El dueño pone su boleto a la venta dentro de TrustTix, sin pasar del tope que
// fijó el organizador. El contrato cobra, paga y cambia el dueño en una sola
// transacción cuando alguien lo compra.
// `reventa`: precio actual en stroops, o null si no está a la venta.
export default function Revender({ boleto, evento, reventa, alCambiar }) {
  const { publicarReventa, cancelarReventa, comisiones } = useApp()
  const [abierto, setAbierto] = useState(false)
  const [texto, setTexto] = useState('')
  const [firmando, setFirmando] = useState(false)
  const [error, setError] = useState('')

  const maximo = (evento.precio * BigInt(evento.topeReventa)) / 10_000n
  const comision = comisiones?.reventa ?? 0
  const neto = (p) => p - (p * BigInt(comision)) / 10_000n

  const abrir = () => {
    setTexto(xlm(reventa ?? evento.precio).replace(/,/g, ''))
    setError('')
    setAbierto(true)
  }

  const montoTexto = Number(texto)
  const monto = texto && montoTexto > 0 ? aStroops(texto) : null
  const sobreTope = monto != null && monto > maximo

  const publicar = async () => {
    setError('')
    setFirmando(true)
    try {
      await publicarReventa(boleto.id, monto)
      setAbierto(false)
      await alCambiar()
    } catch (e) {
      setError(e.message)
    }
    setFirmando(false)
  }

  const quitar = async () => {
    setError('')
    setFirmando(true)
    try {
      await cancelarReventa(boleto.id)
      await alCambiar()
    } catch (e) {
      setError(e.message)
    }
    setFirmando(false)
  }

  return (
    <section className="tarjeta">
      <h3 className="tarjeta__titulo">Reventa</h3>
      {reventa != null ? (
        <>
          <p className="nota">
            Está a la venta en <strong>{precio(reventa)}</strong>. Cuando alguien lo compre recibes {precio(neto(reventa))} al
            momento y el boleto pasa a su cuenta.
          </p>
          <div className="acciones">
            <button type="button" className="boton boton--contorno" onClick={abrir} disabled={firmando}>
              Cambiar precio
            </button>
            <button type="button" className="boton boton--contorno" onClick={quitar} disabled={firmando}>
              {firmando && !abierto ? 'Retirando…' : 'Retirar'}
            </button>
          </div>
        </>
      ) : (
        <>
          <p className="nota">
            ¿Ya no puedes ir? Véndelo aquí mismo hasta en {precio(maximo)}, el {porcentaje(evento.topeReventa)} del precio
            original. Nadie puede revenderlo más caro.
          </p>
          <button type="button" className="boton boton--claro" onClick={abrir}>
            Revender
          </button>
        </>
      )}
      {error && !abierto && <p className="alerta alerta--error" role="alert">{error}</p>}

      {abierto && (
        <Modal titulo={reventa != null ? 'Cambia el precio' : 'Pon tu boleto en reventa'} onCerrar={() => !firmando && setAbierto(false)}>
          <label className="campo">
            <span>Precio en XLM</span>
            <input
              type="number"
              min="0"
              step="0.1"
              inputMode="decimal"
              value={texto}
              onChange={(e) => setTexto(e.target.value)}
              autoFocus
            />
          </label>
          <dl className="resumen">
            <div><dt>Precio máximo</dt><dd>{precio(maximo)}</dd></div>
            {monto != null && !sobreTope && (
              <>
                <div><dt>Comisión TrustTix ({porcentaje(comision)})</dt><dd>{precio(monto - neto(monto))}</dd></div>
                <div><dt>Recibes</dt><dd>{precio(neto(monto))}</dd></div>
              </>
            )}
          </dl>
          {sobreTope && <p className="alerta alerta--error">Ese precio pasa del tope de reventa de este evento.</p>}
          <p className="nota">Mientras esté a la venta sigue siendo tuyo: puedes usarlo, transferirlo o quitarlo cuando quieras.</p>
          {error && <p className="alerta alerta--error" role="alert">{error}</p>}
          <div className="modal__acciones">
            <button type="button" className="boton boton--contorno" onClick={() => setAbierto(false)} disabled={firmando}>
              Cancelar
            </button>
            <button type="button" className="boton boton--claro" onClick={publicar} disabled={firmando || monto == null || sobreTope}>
              {firmando ? 'Publicando…' : 'Publicar'}
            </button>
          </div>
        </Modal>
      )}
    </section>
  )
}
