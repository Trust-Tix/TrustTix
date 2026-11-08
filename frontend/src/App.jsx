import { useEffect } from 'react'
import { Navigate, Outlet, Route, Routes, useLocation } from 'react-router-dom'
import { useApp } from './context/AppContext'
import { PORTAL, subdominioViejo, urlPortal } from './portal'
import Navegacion from './components/Navegacion'
import Landing from './pages/Landing'
import Login from './pages/Login'
import Inicio from './pages/Inicio'
import DetalleEvento from './pages/DetalleEvento'
import Lugares from './pages/Lugares'
import Lugar from './pages/Lugar'
import MisBoletos from './pages/MisBoletos'
import DetalleBoleto from './pages/DetalleBoleto'
import Transferir from './pages/Transferir'
import Verificar from './pages/Verificar'
import Perfil from './pages/Perfil'
import Organizador from './pages/Organizador'
import CrearEvento from './pages/CrearEvento'
import AdminEvento from './pages/AdminEvento'
import Acceso from './pages/Acceso'
import Escaner from './pages/Escaner'

function Plantilla() {
  const { pathname } = useLocation()
  useEffect(() => {
    window.scrollTo(0, 0)
  }, [pathname])
  return (
    <div className={`plantilla ${PORTAL === 'app' ? 'plantilla--tienda' : ''}`}>
      <Navegacion />
      <main className="contenido">
        <Outlet />
      </main>
    </div>
  )
}

function ConSesion({ children }) {
  const { usuario } = useApp()
  const location = useLocation()
  if (!usuario) return <Navigate to="/login" replace state={{ desde: location.pathname }} />
  return children
}

// Manda la misma ruta al portal que le corresponde (enlaces viejos o de otro sitio).
function IrAPortal({ portal }) {
  const { pathname } = useLocation()
  useEffect(() => {
    window.location.replace(urlPortal(portal, pathname))
  }, [portal, pathname])
  return null
}

const INICIO = { app: '/inicio', organizer: '/organizador', staff: '/acceso' }

function rutas() {
  if (PORTAL === 'organizer') {
    return (
      <>
        <Route path="/organizador" element={<ConSesion><Organizador /></ConSesion>} />
        <Route path="/organizador/crear" element={<ConSesion><CrearEvento /></ConSesion>} />
        <Route path="/organizador/:id" element={<ConSesion><AdminEvento /></ConSesion>} />
        <Route path="/perfil" element={<ConSesion><Perfil /></ConSesion>} />
      </>
    )
  }
  if (PORTAL === 'staff') {
    return (
      <>
        <Route path="/acceso" element={<ConSesion><Acceso /></ConSesion>} />
        <Route path="/acceso/:eventoId" element={<ConSesion><Escaner /></ConSesion>} />
        <Route path="/perfil" element={<ConSesion><Perfil /></ConSesion>} />
      </>
    )
  }
  return (
    <>
      {/* Los eventos y la verificación se ven sin cuenta; comprar y guardar boletos la piden. */}
      <Route path="/inicio" element={<Inicio />} />
      <Route path="/eventos/:id" element={<DetalleEvento />} />
      <Route path="/lugares" element={<Lugares />} />
      <Route path="/lugares/:clave" element={<Lugar />} />
      <Route path="/verificar" element={<Verificar />} />
      <Route path="/boletos" element={<ConSesion><MisBoletos /></ConSesion>} />
      <Route path="/boletos/:id" element={<ConSesion><DetalleBoleto /></ConSesion>} />
      <Route path="/boletos/:id/transferir" element={<ConSesion><Transferir /></ConSesion>} />
      <Route path="/perfil" element={<ConSesion><Perfil /></ConSesion>} />
    </>
  )
}

export default function App() {
  if (subdominioViejo()) {
    return (
      <Routes>
        <Route path="*" element={<IrAPortal portal="organizer" />} />
      </Routes>
    )
  }

  if (PORTAL === 'landing') {
    return (
      <Routes>
        <Route path="/" element={<Landing />} />
        <Route path="/organizador/*" element={<IrAPortal portal="organizer" />} />
        <Route path="/acceso/*" element={<IrAPortal portal="staff" />} />
        {/* Antes la app vivía en la raíz: sus rutas siguen llevando a app. */}
        <Route path="*" element={<IrAPortal portal="app" />} />
      </Routes>
    )
  }

  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route element={<Plantilla />}>{rutas()}</Route>
      {PORTAL === 'app' && <Route path="/organizador/*" element={<IrAPortal portal="organizer" />} />}
      {PORTAL === 'app' && <Route path="/acceso/*" element={<IrAPortal portal="staff" />} />}
      <Route path="*" element={<Navigate to={INICIO[PORTAL]} replace />} />
    </Routes>
  )
}
