import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react'
import { portadaGenerica } from '../data/eventos'
import * as red from '../stellar/contrato'
import { conectar, reconectar } from '../stellar/billetera'
import { salirDeGoogle } from '../auth/google'
import { entrarConGoogle, guardarMetadatos, leerMetadatos, salirDeSupabase, subirImagen, supabaseListo } from '../supabase'

// Estado de la app. Todo lo que define la validez de un boleto (eventos,
// capacidad, vendidos, dueños y entradas) se lee del contrato TrustTix en
// Testnet. Los datos descriptivos de cada evento (imagen, categoría, dirección,
// descripción) se leen de Supabase. En el navegador solo se guardan la sesión
// de Google, el personal autorizado desde aquí, los enlaces a las transacciones
// hechas desde aquí y, si Supabase no está configurado, los datos de los
// eventos creados aquí.

// Cambia junto con el contrato: los números de evento y boleto de otro
// contrato no corresponden a los de este.
const CLAVE = 'trusttix-v6'
const AppContext = createContext(null)

function cargarLocal() {
  try {
    return JSON.parse(localStorage.getItem(CLAVE)) ?? {}
  } catch {
    return {}
  }
}

// Une lo que viene de la red con los datos descriptivos.
function armarEvento(id, enRed, datos) {
  const meta = datos[id] ?? {}
  const [fecha, hora = '00:00'] = enRed.fecha.split('T')
  return {
    id,
    titulo: enRed.nombre,
    recinto: enRed.recinto,
    fecha,
    hora,
    capacidad: Number(enRed.capacidad),
    vendidos: Number(enRed.vendidos),
    precio: BigInt(enRed.precio),
    organizador: enRed.organizador,
    categoria: meta.categoria ?? 'Eventos',
    ciudad: meta.ciudad ?? '',
    direccion: meta.direccion ?? enRed.recinto,
    descripcion: meta.descripcion ?? '',
    organizadorNombre: meta.organizadorNombre ?? null,
    fechaFin: meta.fechaFin,
    imagen: meta.imagen,
    coordenadas: meta.lat != null ? { lat: meta.lat, lng: meta.lng } : null,
    portada: portadaGenerica(enRed.nombre),
  }
}

function armarBoleto(id, enRed) {
  return {
    id,
    eventoId: Number(enRed.evento),
    dueno: enRed.dueno,
    estado: enRed.usado ? 'usado' : 'valido',
    usadoEn: enRed.usado ? Number(enRed.usado_en) : null,
  }
}

export function AppProvider({ children }) {
  const inicial = cargarLocal()
  const [usuario, setUsuario] = useState(inicial.usuario ?? null)
  const [metaLocal, setMetaLocal] = useState(inicial.metaLocal ?? {})
  const [metaRemota, setMetaRemota] = useState({})
  const [registro, setRegistro] = useState(inicial.registro ?? {})
  const [personal, setPersonal] = useState(inicial.personal ?? {})
  const [billetera, setBilletera] = useState(null)
  const [eventos, setEventos] = useState([])
  const [misBoletos, setMisBoletos] = useState([])
  const [cargando, setCargando] = useState(true)
  const [errorRed, setErrorRed] = useState('')

  useEffect(() => {
    try {
      localStorage.setItem(CLAVE, JSON.stringify({ usuario, metaLocal, registro, personal }))
    } catch {
      // Sin almacenamiento: la app sigue funcionando, solo pierde la sesión.
    }
  }, [usuario, metaLocal, registro, personal])

  const anotar = useCallback((boletoId, tipo, hash) => {
    setRegistro((r) => ({ ...r, [boletoId]: [...(r[boletoId] ?? []), { tipo, hash, fecha: new Date().toISOString() }] }))
  }, [])

  // Lo guardado en Supabase manda sobre lo guardado en este navegador.
  const recargarMetadatos = useCallback(async () => {
    try {
      setMetaRemota(await leerMetadatos())
    } catch (e) {
      console.warn(e.message)
    }
  }, [])

  const metadatos = useMemo(() => ({ ...metaLocal, ...metaRemota }), [metaLocal, metaRemota])

  const recargarEventos = useCallback(async () => {
    try {
      const total = await red.totalEventos()
      const lista = await Promise.all(
        Array.from({ length: total }, (_, i) => i + 1).map(async (id) => armarEvento(id, await red.consultarEvento(id), metadatos)),
      )
      setEventos(lista.reverse())
      setErrorRed('')
    } catch (e) {
      setErrorRed(`No pudimos leer la red de Stellar: ${e.message}`)
    } finally {
      setCargando(false)
    }
  }, [metadatos])

  const recargarMisBoletos = useCallback(async (cuenta = billetera) => {
    if (!cuenta) {
      setMisBoletos([])
      return
    }
    const ids = await red.boletosDe(cuenta)
    const lista = await Promise.all(ids.map(async (id) => armarBoleto(id, await red.consultarBoleto(id))))
    setMisBoletos(lista.reverse())
  }, [billetera])

  useEffect(() => {
    recargarMetadatos()
  }, [recargarMetadatos])

  useEffect(() => {
    recargarEventos()
  }, [recargarEventos])

  useEffect(() => {
    reconectar().then((cuenta) => cuenta && setBilletera(cuenta))
  }, [])

  useEffect(() => {
    recargarMisBoletos(billetera).catch(() => {})
  }, [billetera, recargarMisBoletos])

  // perfil: { id, nombre, correo, foto } de la cuenta de Google. Con el mismo
  // token se abre la sesión de Supabase; si falla, la app sigue y solo no se
  // podrán guardar datos de eventos.
  const iniciarSesion = useCallback((perfil, token) => {
    setUsuario(perfil)
    if (token) entrarConGoogle(token).catch((e) => console.warn(e.message))
  }, [])

  const cerrarSesion = useCallback(() => {
    salirDeGoogle()
    salirDeSupabase().catch(() => {})
    setUsuario(null)
    setBilletera(null)
  }, [])

  const conectarBilletera = useCallback(async () => {
    const cuenta = await conectar()
    setBilletera(cuenta)
    return cuenta
  }, [])

  const desconectarBilletera = useCallback(() => setBilletera(null), [])

  // comprar: cobra en XLM y emite el boleto en una sola transacción.
  const comprar = useCallback(
    async (eventoId) => {
      if (!billetera) throw new Error('Conecta tu billetera para comprar.')
      const { resultado, hash } = await red.comprar(billetera, eventoId)
      const boletoId = Number(resultado)
      anotar(boletoId, 'compra', hash)
      await Promise.all([recargarEventos(), recargarMisBoletos()])
      return { boletoId, hash }
    },
    [billetera, anotar, recargarEventos, recargarMisBoletos],
  )

  const transferir = useCallback(
    async (boletoId, destino) => {
      if (!billetera) throw new Error('Conecta tu billetera.')
      const { hash } = await red.transferir(billetera, boletoId, destino)
      anotar(boletoId, 'transferencia', hash)
      await recargarMisBoletos()
      return hash
    },
    [billetera, anotar, recargarMisBoletos],
  )

  // consultar_boleto: lectura pública, no requiere billetera.
  const consultarBoleto = useCallback(
    async (boletoId) => {
      try {
        const boleto = armarBoleto(boletoId, await red.consultarBoleto(boletoId))
        const evento =
          eventos.find((ev) => ev.id === boleto.eventoId) ??
          armarEvento(boleto.eventoId, await red.consultarEvento(boleto.eventoId), metadatos)
        return { boleto, evento }
      } catch (e) {
        if (/no existe/i.test(e.message)) return null
        throw e
      }
    },
    [eventos, metadatos],
  )

  const registrarEntrada = useCallback(
    async (boletoId) => {
      if (!billetera) throw new Error('Conecta la billetera del personal de acceso.')
      const { hash } = await red.registrarEntrada(billetera, boletoId)
      anotar(boletoId, 'entrada', hash)
      return hash
    },
    [billetera, anotar],
  )

  // Por defecto pregunta por la billetera conectada; con `cuenta`, por cualquier otra.
  const esPersonal = useCallback(
    (eventoId, cuenta = billetera) => (cuenta ? red.esPersonal(eventoId, cuenta) : Promise.resolve(false)),
    [billetera],
  )

  // El contrato no tiene una lista del personal, así que el organizador
  // recuerda aquí a quién autorizó y la app lo confirma contra el contrato.
  const autorizarPersonal = useCallback(
    async (eventoId, cuenta) => {
      if (!billetera) throw new Error('Conecta la billetera del organizador.')
      const { hash } = await red.autorizarPersonal(billetera, eventoId, cuenta)
      setPersonal((p) => ({ ...p, [eventoId]: [...new Set([...(p[eventoId] ?? []), cuenta])] }))
      return hash
    },
    [billetera],
  )

  const quitarPersonal = useCallback(
    async (eventoId, cuenta) => {
      if (!billetera) throw new Error('Conecta la billetera del organizador.')
      const { hash } = await red.quitarPersonal(billetera, eventoId, cuenta)
      setPersonal((p) => ({ ...p, [eventoId]: (p[eventoId] ?? []).filter((c) => c !== cuenta) }))
      return hash
    },
    [billetera],
  )

  // Boletos de un evento: recorre todos los emitidos (suficiente para el MVP).
  const boletosDeEvento = useCallback(async (eventoId) => {
    const total = await red.totalBoletos()
    const todos = await Promise.all(
      Array.from({ length: total }, (_, i) => i + 1).map(async (id) => armarBoleto(id, await red.consultarBoleto(id))),
    )
    return todos.filter((b) => b.eventoId === eventoId)
  }, [])

  const crearEvento = useCallback(
    async (datos) => {
      if (!billetera) throw new Error('Conecta tu billetera.')
      const { resultado, hash } = await red.crearEvento(billetera, {
        nombre: datos.titulo,
        recinto: datos.recinto,
        fecha: `${datos.fecha}T${datos.hora}`,
        capacidad: datos.capacidad,
        precio: datos.precio,
      })
      const id = Number(resultado)
      const meta = {
        categoria: datos.categoria,
        ciudad: datos.ciudad,
        direccion: datos.direccion,
        descripcion: datos.descripcion,
        organizadorNombre: datos.organizadorNombre || undefined,
        lat: datos.coordenadas?.lat,
        lng: datos.coordenadas?.lng,
      }
      // El evento ya existe en la red. Si la base de datos falla, se avisa y
      // los datos se pueden volver a guardar desde el panel del evento.
      let aviso = null
      if (supabaseListo) {
        try {
          const imagen = datos.imagen ? await subirImagen(id, datos.imagen) : undefined
          await guardarMetadatos(id, { ...meta, imagen })
          await recargarMetadatos()
        } catch (e) {
          aviso = `El evento quedó en la red, pero ${e.message.charAt(0).toLowerCase()}${e.message.slice(1)}`
          setMetaLocal((m) => ({ ...m, [id]: meta }))
        }
      } else {
        setMetaLocal((m) => ({ ...m, [id]: meta }))
      }
      await recargarEventos()
      return { id, hash, aviso }
    },
    [billetera, recargarMetadatos, recargarEventos],
  )

  // Cambia la portada de un evento ya creado.
  const cambiarPortada = useCallback(
    async (eventoId, imagen) => {
      if (!supabaseListo) throw new Error('Falta configurar Supabase para guardar imágenes.')
      const ruta = await subirImagen(eventoId, imagen)
      await guardarMetadatos(eventoId, { imagen: ruta })
      await recargarMetadatos()
    },
    [recargarMetadatos],
  )

  const valor = useMemo(
    () => ({
      usuario,
      billetera,
      eventos,
      misBoletos,
      registro,
      personal,
      cargando,
      errorRed,
      iniciarSesion,
      cerrarSesion,
      conectarBilletera,
      desconectarBilletera,
      recargarEventos,
      recargarMisBoletos,
      comprar,
      transferir,
      consultarBoleto,
      registrarEntrada,
      esPersonal,
      autorizarPersonal,
      quitarPersonal,
      boletosDeEvento,
      crearEvento,
      cambiarPortada,
    }),
    [usuario, billetera, eventos, misBoletos, registro, personal, cargando, errorRed, iniciarSesion, cerrarSesion, conectarBilletera, desconectarBilletera, recargarEventos, recargarMisBoletos, comprar, transferir, consultarBoleto, registrarEntrada, esPersonal, autorizarPersonal, quitarPersonal, boletosDeEvento, crearEvento, cambiarPortada],
  )

  return <AppContext.Provider value={valor}>{children}</AppContext.Provider>
}

export function useApp() {
  return useContext(AppContext)
}
