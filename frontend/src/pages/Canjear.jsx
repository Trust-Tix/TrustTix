import { useCallback, useEffect, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import Encabezado from '../components/Encabezado'
import BotonBilletera from '../components/BotonBilletera'
import { IconoCheck, IconoExterno } from '../components/Iconos'
import { BOVEDA_ID, enlaceContrato, enlaceTx } from '../stellar/config'
import { estadoBoveda, idValido, reclamar } from '../stellar/boveda'
import { abreviar, precio } from '../utils/formato'

// Canje de códigos promocionales: conectas una billetera de Testnet, escribes
// el código y la bóveda te paga en una sola transacción. Se ve sin cuenta.
// La bóveda puede venir en el enlace (?boveda=C…) o en la configuración.
export default function Canjear() {
  const { billetera, recargarSaldo } = useApp()
  const [params] = useSearchParams()
  const id = idValido(params.get('boveda')) ? params.get('boveda') : BOVEDA_ID
  const [estado, setEstado] = useState(null)
  const [codigo, setCodigo] = useState('')
  const [enviando, setEnviando] = useState(false)
  const [error, setError] = useState('')
  const [listo, setListo] = useState(null)

  const cargar = useCallback(() => {
    if (!idValido(id)) return
    estadoBoveda(id, billetera).then(setEstado, (e) => setError(e.message))
  }, [id, billetera])

  useEffect(() => {
    cargar()
  }, [cargar])

  if (!idValido(id)) {
    return (
      <>
        <Encabezado titulo="Canjear" atras />
        <p className="vacio">Todavía no hay códigos para canjear. Vuelve pronto.</p>
      </>
    )
  }

  const quedan = estado && estado.monto > 0n ? Number(estado.disponible / estado.monto) : null

  const enviar = async (e) => {
    e.preventDefault()
    setError('')
    setEnviando(true)
    try {
      const r = await reclamar(id, billetera, codigo)
      setListo(r)
      recargarSaldo().catch(() => {})
      cargar()
    } catch (err) {
      setError(err.message)
    }
    setEnviando(false)
  }

  return (
    <>
      <Encabezado titulo="Canjear" atras />
      <p className="introduccion">
        Escribe tu código y recibe XLM de prueba en tu billetera de Testnet. La bóveda paga en una sola transacción y cada
        billetera canjea una vez.
      </p>

      <section className="tarjeta">
        <dl className="resumen">
          <div><dt>Premio por billetera</dt><dd>{estado ? precio(estado.monto) : '…'}</dd></div>
          <div><dt>Premios disponibles</dt><dd>{quedan ?? '…'}</dd></div>
          {billetera && <div><dt>Tu billetera</dt><dd className="mono">{abreviar(billetera)}</dd></div>}
        </dl>

        {listo ? (
          <div className="resultado resultado--valido" role="status">
            <span className="resultado__icono"><IconoCheck width={40} height={40} /></span>
            <h2>¡Recibiste {precio(listo.monto)}!</h2>
            <p>Ya están en tu billetera.</p>
            <a className="enlace enlace--externo" href={enlaceTx(listo.hash)} target="_blank" rel="noreferrer">
              Ver la transacción <IconoExterno width={12} height={12} />
            </a>
          </div>
        ) : !billetera ? (
          <>
            <p className="nota">Conecta una billetera de Testnet para recibir el premio: Freighter, xBull, Albedo o la billetera TrustTix.</p>
            <BotonBilletera grande />
          </>
        ) : estado?.yaReclamo ? (
          <p className="alerta" role="status">Esta billetera ya canjeó su premio.</p>
        ) : (
          <form className="formulario" onSubmit={enviar}>
            <label className="campo">
              <span>Código</span>
              <input
                value={codigo}
                onChange={(e) => setCodigo(e.target.value)}
                placeholder="Escribe tu código"
                autoComplete="off"
                autoCapitalize="none"
                maxLength={64}
                required
              />
            </label>
            {error && <p className="alerta alerta--error" role="alert">{error}</p>}
            <button type="submit" className="boton boton--claro boton--bloque" disabled={enviando || !codigo.trim() || quedan === 0}>
              {enviando ? 'Canjeando…' : quedan === 0 ? 'Sin premios disponibles' : 'Canjear'}
            </button>
          </form>
        )}
        {error && (listo || !billetera) && <p className="alerta alerta--error" role="alert">{error}</p>}
      </section>

      <p className="nota nota--centro">
        <a className="enlace enlace--externo" href={enlaceContrato(id)} target="_blank" rel="noreferrer">
          Ver la bóveda en stellar.expert <IconoExterno width={12} height={12} />
        </a>
      </p>
    </>
  )
}
