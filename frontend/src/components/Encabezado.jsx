import { Link, useLocation, useNavigate } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import Logo from './Logo'
import BotonBilletera from './BotonBilletera'
import { IconoAtras } from './Iconos'

// Encabezado de cada pantalla: logo (o flecha de regreso), título y billetera.
// Sin sesión, en lugar de la billetera aparece la invitación a entrar.
// `portada` lo oculta en la tienda de escritorio, donde la barra superior ya da el contexto.
export default function Encabezado({ titulo, atras = false, billetera = true, portada = false }) {
  const navigate = useNavigate()
  const { pathname } = useLocation()
  const { usuario } = useApp()
  return (
    <header className={`encabezado ${portada ? 'encabezado--portada' : ''}`}>
      {atras ? (
        <button type="button" className="encabezado__atras" onClick={() => navigate(-1)} aria-label="Regresar">
          <IconoAtras width={24} height={24} />
        </button>
      ) : (
        <Logo size={44} className="encabezado__logo" />
      )}
      <h1 className="encabezado__titulo">{titulo}</h1>
      {billetera && (usuario ? <BotonBilletera /> : <Link to="/login" state={{ desde: pathname }} className="billetera">Entrar</Link>)}
    </header>
  )
}
