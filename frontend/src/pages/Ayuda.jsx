import { Link } from 'react-router-dom'
import Encabezado from '../components/Encabezado'

// Preguntas frecuentes de compradores. Se ve sin cuenta.
const PREGUNTAS = [
  {
    pregunta: '¿Qué es TrustTix?',
    respuesta: (
      <>
        Una boletera donde cada boleto queda registrado a tu nombre en un registro público. Así nadie puede venderte un boleto
        falso ni el mismo boleto dos veces, y en la entrada cada boleto pasa una sola vez.
      </>
    ),
  },
  {
    pregunta: '¿Necesito saber de criptomonedas?',
    respuesta: (
      <>
        No. Inicias sesión con Google y la app te crea una billetera TrustTix al instante, sin instalar nada. Con ella pagas y
        guardas tus boletos. Si ya tienes una billetera de Stellar, también puedes conectarla desde la página{' '}
        <Link to="/billetera">Billetera</Link>.
      </>
    ),
  },
  {
    pregunta: '¿Cuánto cuesta?',
    respuesta: (
      <>
        Hoy TrustTix funciona en la red de prueba de Stellar: tu billetera recibe saldo de prueba y no se usa dinero real. Pagas
        el precio del boleto y nada más; la comisión de la red la cubre TrustTix.
      </>
    ),
  },
  {
    pregunta: '¿Cómo compro un boleto?',
    respuesta: (
      <>
        Abre el evento en <Link to="/inicio">Inicio</Link>, toca <strong>Comprar boleto</strong> y confirma. El pago y tu
        boleto van en el mismo paso: si algo falla, no se cobra nada. Tus boletos quedan en{' '}
        <Link to="/boletos">Mis boletos</Link>.
      </>
    ),
  },
  {
    pregunta: 'Ya no puedo ir. ¿Cómo revendo mi boleto?',
    respuesta: (
      <>
        En <strong>Tu boleto</strong>, toca <strong>Revender</strong> y elige el precio. No puede pasar del tope que fija el
        organizador (normalmente el 110 % del precio original). Cuando alguien lo compra, recibes el dinero al momento,
        menos el 2 % de comisión, y el boleto pasa a su cuenta. Mientras nadie lo compre sigue siendo tuyo y puedes retirarlo.
      </>
    ),
  },
  {
    pregunta: '¿Cómo compro en reventa sin que me estafen?',
    respuesta: (
      <>
        Compra en la sección <strong>Reventa</strong> de la página del evento: el pago y el cambio de dueño ocurren juntos, así
        que no hay forma de pagar y quedarte sin boleto. Si alguien te ofrece un boleto fuera de TrustTix, revisa antes su
        número en <Link to="/verificar">Verificar</Link>: ves quién es el dueño y si ya se usó.
      </>
    ),
  },
  {
    pregunta: '¿Puedo regalar mi boleto?',
    respuesta: (
      <>
        Sí. En <strong>Tu boleto</strong>, toca <strong>Transferir</strong> y escanea el código de la billetera de quien lo
        recibe, o pega su cuenta. No tiene costo. Desde ese momento el boleto es suyo y tú ya no puedes usarlo.
      </>
    ),
  },
  {
    pregunta: '¿Qué pasa si pierdo mi celular?',
    respuesta: (
      <>
        Tus boletos no viven en el celular sino en el registro público, a nombre de tu billetera. La billetera TrustTix queda
        guardada en tu cuenta: entra con el mismo Google en otro dispositivo y ahí estarán. Si usas una billetera propia,
        guarda su frase o clave de respaldo.
      </>
    ),
  },
  {
    pregunta: '¿Y si alguien le toma foto a mi QR?',
    respuesta: (
      <>
        Cada boleto entra una sola vez: la primera persona que lo presenta pasa y cualquier copia se rechaza. No compartas tu
        QR y muéstralo solo en la entrada.
      </>
    ),
  },
  {
    pregunta: '¿Qué pasa si se cancela el evento?',
    respuesta: (
      <>
        El evento deja de vender y revender, y el organizador devuelve el precio original de cada boleto a la billetera de
        quien lo tenga. En <strong>Tu boleto</strong> verás cuando ya se reembolsó.
      </>
    ),
  },
]

export default function Ayuda() {
  return (
    <>
      <Encabezado titulo="Ayuda" atras />
      <p className="introduccion">Lo que más nos preguntan quienes compran boletos en TrustTix.</p>
      <div className="preguntas">
        {PREGUNTAS.map(({ pregunta, respuesta }) => (
          <details key={pregunta} className="pregunta">
            <summary>{pregunta}</summary>
            <p>{respuesta}</p>
          </details>
        ))}
      </div>
    </>
  )
}
