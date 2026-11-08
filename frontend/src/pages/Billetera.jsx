import { useCallback, useEffect, useState } from 'react'
import { useApp } from '../context/AppContext'
import Encabezado from '../components/Encabezado'
import CodigoCuenta from '../components/CodigoCuenta'
import ElegirBilletera from '../components/ElegirBilletera'
import { IconoExterno } from '../components/Iconos'
import { claveSecretaPropia } from '../stellar/billetera'
import { activarConFriendbot, enviarXLM, movimientos } from '../stellar/cuenta'
import EscanerCuenta from '../components/EscanerCuenta'
import { enlaceCuenta, enlaceTx } from '../stellar/config'
import { abreviar, fechaHora } from '../utils/formato'

const formatoXLM = (texto) => Number(texto).toLocaleString('es-MX', { maximumFractionDigits: 2 })

// Billetera de la cuenta: saldo, recibir, enviar XLM, movimientos y respaldo.
export default function Billetera() {
  const { usuario, billetera, infoBilletera, saldo, recargarSaldo, preparando, desconectarBilletera, importarBilletera } = useApp()
  const [lista, setLista] = useState(null)
  const [error, setError] = useState('')
  const [activando, setActivando] = useState(false)
  const [envio, setEnvio] = useState({ destino: '', monto: '' })
  const [enviando, setEnviando] = useState(false)
  const [aviso, setAviso] = useState(null)
  const [verClave, setVerClave] = useState(false)
  const [importando, setImportando] = useState({ abierto: false, secreta: '', error: '' })

  const integrada = infoBilletera?.tipo === 'trusttix'

  const cargar = useCallback(async () => {
    if (!billetera) return
    setError('')
    try {
      const [, m] = await Promise.all([recargarSaldo(billetera), movimientos(billetera)])
      setLista(m)
    } catch (e) {
      setError(e.message)
    }
  }, [billetera, recargarSaldo])

  useEffect(() => {
    setLista(null)
    setVerClave(false)
    cargar()
  }, [cargar])

  const activar = async () => {
    setActivando(true)
    setError('')
    try {
      await activarConFriendbot(billetera)
      await cargar()
    } catch (e) {
      setError(e.message)
    }
    setActivando(false)
  }

  const enviar = async (e) => {
    e.preventDefault()
    setEnviando(true)
    setAviso(null)
    try {
      const hash = await enviarXLM(billetera, envio.destino, envio.monto)
      setAviso({ tipo: 'exito', texto: `Enviaste ${envio.monto} XLM a ${abreviar(envio.destino.trim())}.`, hash })
      setEnvio({ destino: '', monto: '' })
      await cargar()
    } catch (err) {
      setAviso({ tipo: 'error', texto: err.message })
    }
    setEnviando(false)
  }

  const importar = async (e) => {
    e.preventDefault()
    try {
      await importarBilletera(importando.secreta)
      setImportando({ abierto: false, secreta: '', error: '' })
    } catch (err) {
      setImportando((i) => ({ ...i, error: err.message }))
    }
  }

  if (!billetera && preparando) {
    return (
      <>
        <Encabezado titulo="Billetera" billetera={false} />
        <p className="vacio">Creando tu billetera TrustTix y activándola con XLM de prueba…</p>
      </>
    )
  }

  if (!billetera) {
    return (
      <>
        <Encabezado titulo="Billetera" billetera={false} />
        <p className="introduccion">
          Con una billetera compras boletos, los transfieres y firmas entradas. Todo ocurre en Stellar Testnet, con XLM de prueba.
        </p>
        <section className="tarjeta">
          <ElegirBilletera />
        </section>
        {usuario && (
          <section className="tarjeta">
            <button type="button" className="enlace" onClick={() => setImportando((i) => ({ ...i, abierto: !i.abierto }))}>
              ¿Ya tienes una billetera TrustTix en otro dispositivo?
            </button>
            {importando.abierto && (
              <form className="personal__form" onSubmit={importar}>
                <label className="campo">
                  <span>Clave secreta (empieza con S)</span>
                  <input
                    className="mono"
                    type="password"
                    value={importando.secreta}
                    onChange={(e) => setImportando((i) => ({ ...i, secreta: e.target.value, error: '' }))}
                    autoComplete="off"
                    spellCheck={false}
                  />
                </label>
                <button type="submit" className="boton boton--claro" disabled={!importando.secreta.trim()}>Importar billetera</button>
                {importando.error && <p className="alerta alerta--error">{importando.error}</p>}
              </form>
            )}
          </section>
        )}
      </>
    )
  }

  return (
    <>
      <Encabezado titulo="Billetera" billetera={false} />

      <section className="tarjeta saldo">
        <div className="saldo__cabecera">
          <span className="etiqueta-categoria">{infoBilletera?.nombre ?? 'Billetera'}</span>
          <span className="estado-red">Stellar Testnet</span>
        </div>
        <p className="saldo__monto">
          {saldo === undefined ? '…' : saldo === null ? 'Sin activar' : `${formatoXLM(saldo)} XLM`}
        </p>
        <p className="nota mono" title={billetera}>{abreviar(billetera)}</p>
        <div className="saldo__acciones">
          {saldo === null && (
            <button type="button" className="boton boton--claro" onClick={activar} disabled={activando}>
              {activando ? 'Activando…' : 'Activar con XLM de prueba'}
            </button>
          )}
          <button type="button" className="boton boton--contorno boton--chico" onClick={cargar}>Actualizar</button>
          <a className="enlace enlace--externo" href={enlaceCuenta(billetera)} target="_blank" rel="noreferrer">
            Ver en stellar.expert <IconoExterno width={12} height={12} />
          </a>
        </div>
        {saldo === null && (
          <p className="nota">La cuenta todavía no existe en la red. Friendbot, el servicio de Testnet, la crea y le deposita XLM de prueba.</p>
        )}
        {error && <p className="alerta alerta--error">{error}</p>}
      </section>

      <div className="billetera-rejilla">
        <section className="tarjeta">
          <h3 className="tarjeta__titulo">Recibir</h3>
          <p className="nota">Comparte este código para recibir XLM o boletos transferidos.</p>
          <CodigoCuenta cuenta={billetera} />
        </section>

        <section className="tarjeta">
          <h3 className="tarjeta__titulo">Enviar XLM</h3>
          <form className="personal__form" onSubmit={enviar}>
            <label className="campo">
              <span>Cuenta destino</span>
              <input
                className="mono"
                value={envio.destino}
                onChange={(e) => setEnvio((v) => ({ ...v, destino: e.target.value }))}
                placeholder="G…"
                autoComplete="off"
                spellCheck={false}
              />
            </label>
            <EscanerCuenta texto="Escanear su QR" alLeer={(cuenta) => setEnvio((v) => ({ ...v, destino: cuenta }))} />
            <label className="campo">
              <span>Monto en XLM</span>
              <input
                type="number"
                min="0"
                step="0.0000001"
                inputMode="decimal"
                value={envio.monto}
                onChange={(e) => setEnvio((v) => ({ ...v, monto: e.target.value }))}
              />
            </label>
            <button type="submit" className="boton boton--claro" disabled={enviando || !envio.destino.trim() || !envio.monto || saldo === null}>
              {enviando ? (integrada ? 'Enviando…' : 'Firma en tu billetera…') : 'Enviar'}
            </button>
          </form>
          {aviso && (
            <p className={`alerta alerta--${aviso.tipo}`} role="status">
              {aviso.texto}{' '}
              {aviso.hash && (
                <a className="enlace enlace--externo" href={enlaceTx(aviso.hash)} target="_blank" rel="noreferrer">
                  Ver la transacción <IconoExterno width={12} height={12} />
                </a>
              )}
            </p>
          )}
        </section>
      </div>

      <section className="tarjeta">
        <h3 className="tarjeta__titulo">Movimientos</h3>
        {lista === null ? (
          <p className="nota">Leyendo movimientos…</p>
        ) : lista.length === 0 ? (
          <p className="nota">Todavía no hay movimientos.</p>
        ) : (
          <ul className="movimientos">
            {lista.map((m) => (
              <li key={m.id} className="movimiento">
                <span className={`movimiento__monto ${m.entra ? 'movimiento__monto--entra' : ''}`}>
                  {m.entra ? '+' : '−'}{formatoXLM(m.monto)} XLM
                </span>
                <span className="movimiento__detalle">
                  {m.contrato ? (m.entra ? 'Cobro de boletos' : 'Compra de boleto') : m.entra ? 'Recibido' : 'Enviado'}
                  {' '}
                  <span className="mono">{m.contrato && !m.entra ? 'TrustTix' : abreviar(m.contraparte)}</span>
                </span>
                <a className="enlace" href={enlaceTx(m.hash)} target="_blank" rel="noreferrer">{fechaHora(m.fecha)}</a>
              </li>
            ))}
          </ul>
        )}
      </section>

      {integrada && (
        <section className="tarjeta">
          <h3 className="tarjeta__titulo">Respaldo</h3>
          <p className="nota">
            Tu billetera TrustTix vive en este navegador. Guarda la clave secreta para usarla en otro dispositivo o importarla en
            Varial (antes Freighter). Quien la tenga controla la cuenta. Es solo para Testnet.
          </p>
          {verClave ? (
            <p className="mono rompe clave-secreta">{claveSecretaPropia(usuario?.id)}</p>
          ) : (
            <button type="button" className="boton boton--contorno boton--chico" onClick={() => setVerClave(true)}>
              Mostrar clave secreta
            </button>
          )}
        </section>
      )}

      <div className="acciones acciones--columna">
        <button type="button" className="boton boton--contorno" onClick={desconectarBilletera}>
          {integrada ? 'Usar otra billetera' : 'Desconectar billetera'}
        </button>
      </div>
    </>
  )
}
