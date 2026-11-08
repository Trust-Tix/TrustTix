import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import { abreviar } from '../utils/formato'
import { IconoBilletera } from './Iconos'
import Modal from './Modal'
import ElegirBilletera from './ElegirBilletera'

const formatoSaldo = (texto) => `${Number(texto).toLocaleString('es-MX', { maximumFractionDigits: 2 })} XLM`

// Botón de billetera. Conectada, muestra el saldo y lleva a la página
// Billetera; sin conectar, abre el selector de billeteras.
export default function BotonBilletera({ grande = false }) {
  const { billetera, infoBilletera, saldo, preparando } = useApp()
  const [eligiendo, setEligiendo] = useState(false)
  const [error, setError] = useState('')

  if (preparando && !billetera) {
    return (
      <span className={`billetera ${grande ? 'boton boton--apagado boton--bloque' : ''}`} role="status">
        <IconoBilletera width={16} height={16} /> Preparando tu billetera…
      </span>
    )
  }

  if (billetera) {
    return (
      <Link
        to="/billetera"
        className={`billetera billetera--conectada ${grande ? 'billetera--grande' : ''}`}
        title={`${infoBilletera?.nombre ?? 'Billetera'}: ${billetera}`}
      >
        <span className="billetera__punto" aria-hidden="true" />
        {typeof saldo === 'string' ? formatoSaldo(saldo) : abreviar(billetera)}
      </Link>
    )
  }

  return (
    <div className={grande ? 'billetera-bloque' : undefined}>
      <button
        type="button"
        className={`billetera ${grande ? 'boton boton--claro boton--bloque' : ''}`}
        onClick={() => {
          setError('')
          setEligiendo(true)
        }}
      >
        <IconoBilletera width={16} height={16} />
        {grande ? 'Conectar billetera' : 'Conectar'}
      </button>
      {grande && error && <p className="alerta alerta--error" role="alert">{error}</p>}
      {eligiendo && (
        <Modal titulo="Elige tu billetera" onCerrar={() => setEligiendo(false)}>
          <ElegirBilletera
            alConectar={() => setEligiendo(false)}
            antesDeExterna={() => setEligiendo(false)}
            alFallar={(mensaje) => (grande ? setError(mensaje) : window.alert(mensaje))}
          />
        </Modal>
      )}
    </div>
  )
}
