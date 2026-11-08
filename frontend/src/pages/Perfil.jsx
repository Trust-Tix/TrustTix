import { useNavigate } from 'react-router-dom'
import { useApp } from '../context/AppContext'
import Encabezado from '../components/Encabezado'
import BotonBilletera from '../components/BotonBilletera'
import CodigoCuenta from '../components/CodigoCuenta'
import { IconoCalendario, IconoEscaner, IconoExterno, IconoInicio } from '../components/Iconos'
import { enlaceContrato, enlaceCuenta } from '../stellar/config'
import { PORTAL, urlPortal } from '../portal'

// Accesos a los otros portales; cada uno abre su propio subdominio.
const MENU = {
  app: [
    { portal: 'organizer', ruta: '/organizador', Icono: IconoCalendario, titulo: 'Panel del organizador', texto: 'Crea eventos y asigna al personal de acceso' },
    { portal: 'staff', ruta: '/acceso', Icono: IconoEscaner, titulo: 'App de acceso', texto: 'Escanea boletos en la puerta' },
  ],
  organizer: [
    { portal: 'app', ruta: '/inicio', Icono: IconoInicio, titulo: 'Ir a TrustTix', texto: 'Mira tus eventos como los ven los compradores' },
    { portal: 'staff', ruta: '/acceso', Icono: IconoEscaner, titulo: 'App de acceso', texto: 'Escanea boletos en la puerta' },
  ],
  staff: [
    { portal: 'app', ruta: '/inicio', Icono: IconoInicio, titulo: 'Ir a TrustTix', texto: 'Compra y consulta boletos' },
  ],
}

export default function Perfil() {
  const { usuario, billetera, misBoletos, desconectarBilletera, cerrarSesion } = useApp()
  const navigate = useNavigate()

  return (
    <>
      <Encabezado titulo={PORTAL === 'staff' ? 'Mi cuenta' : 'Perfil'} billetera={false} />

      <section className="tarjeta perfil">
        {usuario.foto ? (
          <img className="perfil__avatar" src={usuario.foto} alt="" referrerPolicy="no-referrer" />
        ) : (
          <span className="perfil__avatar" aria-hidden="true">{usuario.nombre?.[0]}</span>
        )}
        <div>
          <h2 className="perfil__nombre">{usuario.nombre}</h2>
          <p className="nota">{usuario.correo}</p>
        </div>
      </section>

      <section className="tarjeta">
        <h3 className="tarjeta__titulo">Billetera</h3>
        {billetera ? (
          <>
            <p className="mono rompe">{billetera}</p>
            <p className="nota">
              Freighter, Stellar Testnet{PORTAL === 'app' &&`, ${misBoletos.length} boleto(s)`}
            </p>
            <a className="enlace enlace--externo" href={enlaceCuenta(billetera)} target="_blank" rel="noreferrer">
              Ver mi cuenta en stellar.expert <IconoExterno width={12} height={12} />
            </a>
            <button type="button" className="boton boton--contorno" onClick={desconectarBilletera}>Desconectar</button>
          </>
        ) : (
          <>
            <p className="nota">
              {PORTAL === 'organizer'
                ? 'Conecta Freighter (en Testnet) con la cuenta con la que organizas tus eventos.'
                : PORTAL === 'staff'
                  ? 'Conecta Freighter (en Testnet) con la cuenta que el organizador va a autorizar.'
                  : 'Conecta Freighter (en Testnet) para comprar, ver y transferir boletos.'}
            </p>
            <BotonBilletera grande />
          </>
        )}
      </section>

      {PORTAL === 'staff' && billetera && (
        <section className="tarjeta">
          <h3 className="tarjeta__titulo">Tu código de personal</h3>
          <p className="nota">Muéstraselo al organizador. Lo escanea desde su panel y tu cuenta queda autorizada en el contrato.</p>
          <CodigoCuenta cuenta={billetera} />
        </section>
      )}

      <section className="menu">
        {MENU[PORTAL].map(({ portal, ruta, Icono, titulo, texto }) => (
          <a key={portal} href={urlPortal(portal, ruta)} className="menu__opcion">
            <Icono />
            <span><strong>{titulo}</strong><small>{texto}</small></span>
          </a>
        ))}
        <a href={enlaceContrato()} target="_blank" rel="noreferrer" className="menu__opcion">
          <IconoExterno />
          <span><strong>Contrato TrustTix</strong><small>Ver el contrato y sus transacciones en Testnet</small></span>
        </a>
      </section>

      <div className="acciones acciones--columna">
        <button type="button" className="boton boton--peligro" onClick={() => { cerrarSesion(); navigate('/login') }}>
          Cerrar sesión
        </button>
      </div>
    </>
  )
}
