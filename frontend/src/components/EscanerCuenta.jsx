import { useEffect, useRef, useState } from 'react'
import QrScanner from 'qr-scanner'
import { IconoEscaner } from './Iconos'

const CUENTA = /G[A-Z2-7]{55}/

// Botón que abre la cámara y devuelve la primera cuenta Stellar que lea en un
// QR (el de "Recibir" de otra billetera, o el código de personal del staff).
export default function EscanerCuenta({ alLeer, texto = 'Escanear QR', deshabilitado = false }) {
  const [abierta, setAbierta] = useState(false)
  const [error, setError] = useState('')
  const video = useRef(null)
  const devolver = useRef(alLeer)

  useEffect(() => {
    devolver.current = alLeer
  }, [alLeer])

  useEffect(() => {
    if (!abierta || !video.current) return undefined
    const lector = new QrScanner(
      video.current,
      ({ data }) => {
        const m = CUENTA.exec(data)
        if (!m) return
        navigator.vibrate?.(60)
        devolver.current(m[0])
        setAbierta(false)
      },
      { returnDetailedScanResult: true, preferredCamera: 'environment', highlightScanRegion: false, maxScansPerSecond: 8 },
    )
    lector.start().catch((e) => {
      setError(`No pudimos abrir la cámara: ${e?.message ?? e}`)
      setAbierta(false)
    })
    return () => {
      lector.stop()
      lector.destroy()
    }
  }, [abierta])

  return (
    <>
      <button
        type="button"
        className="boton boton--contorno"
        onClick={() => {
          setError('')
          setAbierta((a) => !a)
        }}
        disabled={deshabilitado}
      >
        <IconoEscaner width={18} height={18} /> {abierta ? 'Cerrar cámara' : texto}
      </button>
      {abierta && (
        <div className="escaner__visor escaner__visor--activo">
          <video ref={video} className="escaner__video" muted playsInline />
        </div>
      )}
      {error && <p className="alerta alerta--error">{error}</p>}
    </>
  )
}
