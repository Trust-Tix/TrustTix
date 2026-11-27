import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react'
import { portadaGenerica } from '../data/eventos'
import * as red from '../stellar/contrato'
import {
  billeteraIntegradaDisponible,
  conectarExterna,
  desconectar,
  eligioOtraBilletera,
  importarBilleteraPropia,
  reconectar,
  usarBilleteraPropia,
} from '../stellar/billetera'
import { activarConFriendbot, saldoXLM } from '../stellar/cuenta'
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
const CLAVE = 'trusttix-v7'
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
    // Precio máximo de reventa en puntos base sobre el precio (11,000 = 110 %).
    topeReventa: Number(enRed.tope_reventa),
    cancelado: Boolean(enRed.cancelado),
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
    // Solo un boleto 'valido' sirve para entrar, transferir o revender.
    estado: enRed.usado ? 'usado' : enRed.reembolsado ? 'reembolsado' : 'valido',
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
  // { tipo: 'trusttix' | 'externa' | 'demo', nombre } de la billetera conectada.
  const [infoBilletera, setInfoBilletera] = useState(null)
  const [saldo, setSaldo] = useState(undefined)
  // Mientras se crea y activa la billetera TrustTix, y el aviso de que quedó lista.
  const [preparando, setPreparando] = useState(false)
  const [avisoBilletera, setAvisoBilletera] = useState('')
  const [eventosEnRed, setEventosEnRed] = useState([])
  const [misBoletos, setMisBoletos] = useState([])
  const [cargando, setCargando] = useState(true)
  const [errorRed, setErrorRed] = useState('')
  // Comisiones vigentes de TrustTix, leídas del contrato (puntos base).
  const [comisiones, setComisiones] = useState(null)

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

  // Lo que viene de la red se guarda tal cual; las portadas y demás datos de
  // Supabase se le agregan después, sin volver a leer la red cuando llegan.
  const recargarEventos = useCallback(async () => {
    try {
      const total = await red.totalEventos()
      const lista = await Promise.all(
        Array.from({ length: total }, (_, i) => i + 1).map(async (id) => ({ id, enRed: await red.consultarEvento(id) })),
      )
      setEventosEnRed(lista.reverse())
      setErrorRed('')
    } catch (e) {
      setErrorRed(`No pudimos leer la red de Stellar: ${e.message}`)
    } finally {
      setCargando(false)
    }
  }, [])

  const eventos = useMemo(
    () => eventosEnRed.map(({ id, enRed }) => armarEvento(id, enRed, metadatos)),
    [eventosEnRed, metadatos],
  )
  // Lo que se ofrece en la tienda: los eventos cancelados ya no se anuncian.
  const cartelera = useMemo(() => eventos.filter((ev) => !ev.cancelado), [eventos])

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
    red.comisiones().then(setComisiones, (e) => console.warn(e.message))
  }, [])

  const ponerBilletera = useCallback((b) => {
    setBilletera(b?.direccion ?? null)
    setInfoBilletera(b ? { tipo: b.tipo, nombre: b.nombre } : null)
  }, [])

  const usuarioId = usuario?.id

  // Saldo en XLM de la billetera conectada: texto, null si la cuenta aún no
  // existe en la red, undefined mientras se lee.
  const recargarSaldo = useCallback(async (cuenta = billetera) => {
    if (!cuenta) {
      setSaldo(undefined)
      return undefined
    }
    try {
      const s = await saldoXLM(cuenta)
      setSaldo(s)
      return s
    } catch {
      return undefined
    }
  }, [billetera])

  useEffect(() => {
    recargarMisBoletos(billetera).catch(() => {})
    setSaldo(undefined)
    recargarSaldo(billetera)
  }, [billetera, recargarMisBoletos, recargarSaldo])

  // perfil: { id, nombre, correo, foto } de la cuenta de Google. Con el mismo
  // token se abre la sesión de Supabase ANTES de marcar la sesión iniciada: la
  // billetera de la cuenta se busca ahí, y sin esperar se crearía otra. Si
  // Supabase falla, la app sigue con la billetera de este navegador.
  const iniciarSesion = useCallback(async (perfil, token) => {
    if (token) await entrarConGoogle(token).catch((e) => console.warn(e.message))
    setUsuario(perfil)
  }, [])

  const cerrarSesion = useCallback(() => {
    salirDeGoogle()
    salirDeSupabase().catch(() => {})
    desconectar({ manual: false }).catch(() => {})
    setUsuario(null)
    ponerBilletera(null)
  }, [ponerBilletera])

  // tipo 'trusttix': la billetera integrada de la cuenta de Google (se crea y
  // se activa con Friendbot la primera vez). tipo 'externa': el selector de
  // Stellar Wallets Kit.
  const conectarBilletera = useCallback(
    async (tipo = 'externa') => {
      let b
      if (tipo === 'trusttix') {
        b = await usarBilleteraPropia(usuarioId)
        if (b.nueva) {
          setPreparando(true)
          // Si Friendbot falla, la billetera queda conectada sin saldo y se
          // activa al comprar o desde la página Billetera.
          const activada = await activarConFriendbot(b.direccion).then(() => true, () => false)
          setPreparando(false)
          if (activada) setAvisoBilletera('Creamos tu billetera TrustTix con 10,000 XLM de prueba. Ya puedes comprar.')
        }
      } else {
        b = await conectarExterna()
      }
      ponerBilletera(b)
      return b.direccion
    },
    [usuarioId, ponerBilletera],
  )

  // Al entrar, recupera la billetera de la última vez. Si la persona nunca eligió
  // otra, abre (o crea) su billetera TrustTix sin pedirle nada.
  useEffect(() => {
    let vigente = true
    reconectar(usuarioId).then((b) => {
      if (!vigente) return
      if (b) ponerBilletera(b)
      else if (usuarioId && !eligioOtraBilletera() && billeteraIntegradaDisponible) conectarBilletera('trusttix').catch(() => {})
    })
    return () => {
      vigente = false
    }
    // conectarBilletera cambia con usuarioId; basta con reaccionar a la cuenta.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [usuarioId, ponerBilletera])

  // Billetera lista para firmar y con saldo: la conectada, o la TrustTix si no hay.
  // En Mainnet no hay billetera integrada: hay que conectar una propia antes.
  const prepararBilletera = useCallback(async () => {
    if (!billetera && !billeteraIntegradaDisponible) {
      throw new Error('Conecta tu billetera de Stellar (Freighter, xBull, LOBSTR y otras) para continuar.')
    }
    const cuenta = billetera ?? (await conectarBilletera('trusttix'))
    if (saldo == null && (await recargarSaldo(cuenta)) === null) {
      await activarConFriendbot(cuenta)
      await recargarSaldo(cuenta)
    }
    return cuenta
  }, [billetera, saldo, conectarBilletera, recargarSaldo])

  const importarBilletera = useCallback(
    async (secreta) => {
      const b = await importarBilleteraPropia(usuarioId, secreta)
      ponerBilletera(b)
      return b.direccion
    },
    [usuarioId, ponerBilletera],
  )

  const desconectarBilletera = useCallback(async () => {
    await desconectar()
    ponerBilletera(null)
  }, [ponerBilletera])

  // comprar: cobra en XLM y emite el boleto en una sola transacción. Si no hay
  // billetera, prepara la TrustTix en el mismo paso.
  const comprar = useCallback(
    async (eventoId) => {
      if (!usuarioId && !billetera) throw new Error('Inicia sesión para comprar.')
      const cuenta = await prepararBilletera()
      const { resultado, hash } = await red.comprar(cuenta, eventoId)
      const boletoId = Number(resultado)
      anotar(boletoId, 'compra', hash)
      // El boleto ya existe en la red; lo demás se actualiza mientras se muestra.
      Promise.all([recargarEventos(), recargarMisBoletos(cuenta), recargarSaldo(cuenta)]).catch(() => {})
      return { boletoId, hash }
    },
    [usuarioId, billetera, prepararBilletera, anotar, recargarEventos, recargarMisBoletos, recargarSaldo],
  )

  const transferir = useCallback(
    async (boletoId, destino) => {
      if (!billetera) throw new Error('Conecta tu billetera.')
      const { hash } = await red.transferir(billetera, boletoId, destino)
      anotar(boletoId, 'transferencia', hash)
      Promise.all([recargarMisBoletos(), recargarSaldo()]).catch(() => {})
      return hash
    },
    [billetera, anotar, recargarMisBoletos, recargarSaldo],
  )

  // Compra en reventa: igual que comprar, prepara la billetera TrustTix si hace falta.
  const comprarReventa = useCallback(
    async (boletoId) => {
      if (!usuarioId && !billetera) throw new Error('Inicia sesión para comprar.')
      const cuenta = await prepararBilletera()
      const { hash } = await red.comprarReventa(cuenta, boletoId)
      anotar(boletoId, 'compra-reventa', hash)
      Promise.all([recargarMisBoletos(cuenta), recargarSaldo(cuenta)]).catch(() => {})
      return hash
    },
    [usuarioId, billetera, prepararBilletera, anotar, recargarMisBoletos, recargarSaldo],
  )

  // El dueño pone su boleto en reventa (o le cambia el precio) y lo retira.
  const publicarReventa = useCallback(
    async (boletoId, precio) => {
      if (!billetera) throw new Error('Conecta tu billetera.')
      const { hash } = await red.publicarReventa(billetera, boletoId, precio)
      anotar(boletoId, 'reventa', hash)
      return hash
    },
    [billetera, anotar],
  )

  const cancelarReventa = useCallback(
    async (boletoId) => {
      if (!billetera) throw new Error('Conecta tu billetera.')
      const { hash } = await red.cancelarReventa(billetera, boletoId)
      anotar(boletoId, 'reventa-cancelada', hash)
      return hash
    },
    [billetera, anotar],
  )

  // consultar_boleto: lectura pública, no requiere billetera.
  const consultarBoleto = useCallback(
    async (boletoId) => {
      try {
        const boleto = armarBoleto(boletoId, await red.consultarBoleto(boletoId))
        const [evento, reventa] = await Promise.all([
          eventos.find((ev) => ev.id === boleto.eventoId) ??
            red.consultarEvento(boleto.eventoId).then((enRed) => armarEvento(boleto.eventoId, enRed, metadatos)),
          // Precio en stroops si su dueño lo tiene a la venta, o null.
          boleto.estado === 'valido' ? red.consultarReventa(boletoId).catch(() => null) : null,
        ])
        return { boleto, evento, reventa }
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

  // Boletos de un evento que sus dueños tienen a la venta, del más barato al más caro.
  const reventasDeEvento = useCallback(
    async (eventoId) => {
      const vigentes = (await boletosDeEvento(eventoId)).filter((b) => b.estado === 'valido')
      const ofertas = await Promise.all(
        vigentes.map(async (boleto) => ({ boleto, precio: await red.consultarReventa(boleto.id) })),
      )
      return ofertas.filter((o) => o.precio != null).sort((a, b) => (a.precio < b.precio ? -1 : a.precio > b.precio ? 1 : 0))
    },
    [boletosDeEvento],
  )

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
      infoBilletera,
      importarBilletera,
      saldo,
      recargarSaldo,
      preparando,
      avisoBilletera,
      setAvisoBilletera,
      eventos,
      cartelera,
      comisiones,
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
      comprarReventa,
      reventasDeEvento,
      publicarReventa,
      cancelarReventa,
      consultarBoleto,
      registrarEntrada,
      esPersonal,
      autorizarPersonal,
      quitarPersonal,
      boletosDeEvento,
      crearEvento,
      cambiarPortada,
    }),
    [usuario, billetera, infoBilletera, importarBilletera, saldo, recargarSaldo, preparando, avisoBilletera, eventos, cartelera, comisiones, misBoletos, registro, personal, cargando, errorRed, iniciarSesion, cerrarSesion, conectarBilletera, desconectarBilletera, recargarEventos, recargarMisBoletos, comprar, transferir, comprarReventa, reventasDeEvento, publicarReventa, cancelarReventa, consultarBoleto, registrarEntrada, esPersonal, autorizarPersonal, quitarPersonal, boletosDeEvento, crearEvento, cambiarPortada],
  )

  return <AppContext.Provider value={valor}>{children}</AppContext.Provider>
}

export function useApp() {
  return useContext(AppContext)
}
