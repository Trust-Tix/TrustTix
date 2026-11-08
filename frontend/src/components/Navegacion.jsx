import { Link, NavLink, useLocation } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import Logo from './Logo'
import BotonBilletera from './BotonBilletera'
import { IconoBoleto, IconoCalendario, IconoEscaner, IconoEscudo, IconoInicio, IconoMas, IconoPerfil, IconoUbicacion } from './Iconos'
import { NOMBRE_PORTAL, PORTAL } from '../portal'

const ENLACES = {
  app: [
    { a: '/inicio', texto: 'Inicio', Icono: IconoInicio },
    { a: '/lugares', texto: 'Lugares', Icono: IconoUbicacion },
    { a: '/boletos', texto: 'Mis boletos', Icono: IconoBoleto },
    { a: '/verificar', texto: 'Verificar', Icono: IconoEscudo },
    { a: '/perfil', texto: 'Perfil', Icono: IconoPerfil },
  ],
  organizer: [
    { a: '/organizador', texto: 'Mis eventos', Icono: IconoCalendario, exacto: true },
    { a: '/organizador/crear', texto: 'Crear evento', Icono: IconoMas },
    { a: '/perfil', texto: 'Perfil', Icono: IconoPerfil },
  ],
  staff: [
    { a: '/acceso', texto: 'Eventos', Icono: IconoEscaner },
    { a: '/perfil', texto: 'Mi cuenta', Icono: IconoPerfil },
  ],
}

// Sin sesión, el lugar del perfil lleva a iniciar sesión.
const ENTRAR = { a: '/login', texto: 'Entrar', Icono: IconoPerfil }

// En celular es una barra inferior. En pantallas grandes, la tienda (app) usa
// una barra superior con la cuenta a la derecha; los demás portales, una lateral.
export default function Navegacion() {
  const { usuario } = useApp()
  const { pathname } = useLocation()
  const enlaces = ENLACES[PORTAL].map((e) => (!usuario && e.a === '/perfil' ? ENTRAR : e))
  const tienda = PORTAL === 'app'

  return (
    <nav className={`navegacion ${tienda ? 'navegacion--tienda' : ''}`} aria-label="Principal">
      <Link to={tienda ? '/inicio' : '/'} className="navegacion__marca">
        <Logo size={40} />
        <span>
          TrustTix
          {!tienda && <small>{NOMBRE_PORTAL[PORTAL]}</small>}
        </span>
      </Link>
      {enlaces.map(({ a, texto, Icono, exacto }) => (
        <NavLink
          key={a}
          to={a}
          end={exacto}
          state={a === '/login' ? { desde: pathname } : undefined}
          className={`navegacion__enlace ${tienda && (a === '/perfil' || a === '/login') ? 'navegacion__enlace--celular' : ''}`}
        >
          <Icono width={22} height={22} />
          <span>{texto}</span>
        </NavLink>
      ))}
      {tienda && (
        <div className="navegacion__cuenta">
          {usuario ? (
            <>
              <BotonBilletera />
              <Link to="/perfil" className="navegacion__avatar" aria-label="Perfil">
                {usuario.foto ? <img src={usuario.foto} alt="" referrerPolicy="no-referrer" /> : usuario.nombre?.[0]}
              </Link>
            </>
          ) : (
            <Link to="/login" state={{ desde: pathname }} className="boton boton--claro boton--chico">Iniciar sesión</Link>
          )}
        </div>
      )}
    </nav>
  )
}
