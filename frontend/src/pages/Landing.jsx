import { useApp } from '../context/AppContext'
import Logo from '../components/Logo'
import { urlPortal } from '../portal'

const REPOSITORIO = 'https://github.com/DanielRosalesAlanis/TrustTix'

// Página de trusttix.xyz: el negocio en pocas líneas. Las apps viven en sus subdominios.
export default function Landing() {
  const { eventos } = useApp()
  const cinta = eventos.filter((ev) => ev.imagen)

  return (
    <div className="landing">
      <header className="landing__barra">
        <a href="#inicio" className="landing__marca">
          <Logo size={34} />
          <span>TrustTix</span>
        </a>
        <nav className="landing__enlaces" aria-label="Secciones">
          <a href="#organizadores">Organizadores</a>
          <a href={urlPortal('app', '/inicio')}>Eventos</a>
        </nav>
        <a href={urlPortal('organizer', '/organizador')} className="boton boton--claro boton--chico">Vender boletos</a>
      </header>

      <main>
        <section id="inicio" className="landing__portada">
          <h1 className="landing__titulo">Tu lugar<br />es tuyo.</h1>
          <p className="landing__bajada">
            La boletera donde cada boleto es único. No se clona, no se sobrevende y siempre se sabe de quién es.
          </p>
          <div className="landing__acciones">
            <a href={urlPortal('app', '/inicio')} className="boton boton--claro">Ver eventos</a>
            <a href={urlPortal('organizer', '/organizador')} className="boton boton--contorno">Vender con TrustTix</a>
          </div>
        </section>

        {/* La cinta se duplica para que el desplazamiento no tenga corte. */}
        {cinta.length > 0 && (
          <div className="landing__cinta" aria-hidden="true">
            <div className="landing__cinta-pista">
              {[...cinta, ...cinta].map((ev, i) => (
                <img key={i} src={ev.imagen} alt="" className="landing__cartel" loading="lazy" />
              ))}
            </div>
          </div>
        )}

        <section className="landing__frase">
          <p>
            Cada boleto tiene un dueño. <span>Uno solo.</span>
          </p>
        </section>

        <section id="organizadores" className="landing__lados">
          <div className="landing__lado">
            <p className="landing__rotulo">Si organizas</p>
            <h2>Vende sin sorpresas en la puerta.</h2>
            <ul>
              <li>Cobras en el momento de cada venta.</li>
              <li>Nadie vende más lugares de los que tienes.</li>
              <li>Tu staff escanea en fila desde su celular.</li>
            </ul>
            <a href={urlPortal('organizer', '/organizador')} className="landing__flecha">Crear mi evento</a>
          </div>
          <div className="landing__lado">
            <p className="landing__rotulo">Si asistes</p>
            <h2>Compra tranquilo, incluso en reventa.</h2>
            <ul>
              <li>Tu boleto queda a tu nombre.</li>
              <li>Si no vas, lo transfieres.</li>
              <li>Antes de pagar, verificas quién es el dueño.</li>
            </ul>
            <a href={urlPortal('app', '/inicio')} className="landing__flecha">Ver eventos</a>
          </div>
        </section>

        <section className="landing__cierre">
          <h2>Hecho en México.<br />Construido en Stellar.</h2>
          <p>Nacimos en Builders 101. Hoy funcionamos como demo en Stellar Testnet, sin dinero real.</p>
        </section>
      </main>

      <footer className="landing__pie">
        <div className="landing__marca">
          <Logo size={28} />
          <span>TrustTix</span>
        </div>
        <nav aria-label="Apps">
          <a href={urlPortal('app', '/inicio')}>Eventos</a>
          <a href={urlPortal('organizer', '/organizador')}>Organizadores</a>
          <a href={urlPortal('staff', '/acceso')}>Staff</a>
          <a href={REPOSITORIO} target="_blank" rel="noreferrer">GitHub</a>
        </nav>
      </footer>
    </div>
  )
}
