import { useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import { tieneBilleteraPropia } from '../stellar/billetera'
import { IconoBilletera, IconoExterno } from './Iconos'

// Las dos formas de conectar: la billetera TrustTix (integrada, recomendada) o
// cualquier billetera de Stellar con Stellar Wallets Kit.
// `antesDeExterna` se llama antes de abrir el selector del kit, para cerrar un
// modal propio y que no queden dos encimados; como este componente se desmonta,
// el error de la billetera externa se entrega con `alFallar`.
export default function ElegirBilletera({ alConectar, antesDeExterna, alFallar }) {
  const { usuario, conectarBilletera } = useApp()
  const { pathname } = useLocation()
  const [ocupado, setOcupado] = useState(null)
  const [error, setError] = useState('')
  const yaTiene = tieneBilleteraPropia(usuario?.id)

  const conectar = async (tipo) => {
    setError('')
    setOcupado(tipo)
    if (tipo === 'externa') antesDeExterna?.()
    try {
      await conectarBilletera(tipo)
      alConectar?.()
    } catch (e) {
      if (e.cancelado) {
        // Cerró el selector: no hay nada que avisar.
      } else if (tipo === 'externa' && alFallar) alFallar(e.message)
      else setError(e.message)
    }
    setOcupado(null)
  }

  return (
    <div className="elegir-billetera">
      <button
        type="button"
        className="opcion-billetera opcion-billetera--principal"
        onClick={() => conectar('trusttix')}
        disabled={!usuario || Boolean(ocupado)}
      >
        <IconoBilletera width={24} height={24} />
        <span>
          <strong>{ocupado === 'trusttix' ? (yaTiene ? 'Abriendo…' : 'Creando y activando…') : 'Billetera TrustTix'}</strong>
          <small>
            {yaTiene
              ? 'Tu billetera de esta cuenta. Pagas al instante, también en el celular.'
              : 'Se crea al instante para tu cuenta y recibe XLM de prueba. No instalas nada y funciona en el celular.'}
          </small>
        </span>
        <span className="opcion-billetera__sello">Recomendada</span>
      </button>
      {!usuario && (
        <p className="nota">
          <Link to="/login" state={{ desde: pathname }}>Inicia sesión</Link> para usar la billetera TrustTix.
        </p>
      )}

      <button type="button" className="opcion-billetera" onClick={() => conectar('externa')} disabled={Boolean(ocupado)}>
        <IconoExterno width={22} height={22} />
        <span>
          <strong>{ocupado === 'externa' ? 'Esperando a tu billetera…' : 'Otra billetera'}</strong>
          <small>Varial (antes Freighter), xBull, Albedo, LOBSTR, Hana, Rabet y más. Debe estar en Testnet.</small>
        </span>
      </button>

      {error && <p className="alerta alerta--error" role="alert">{error}</p>}
    </div>
  )
}
