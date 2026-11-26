# Casos de prueba de TrustTix

Pasos para probar a mano la compra, la reventa, la entrada y la cancelación en la app, con el resultado que debe verse. Sirven para revisar cada versión antes de publicarla y para que cualquier persona del jurado pruebe la app sin ayuda.

**Dónde:** <https://app.trusttix.xyz> (compradores), <https://organizer.trusttix.xyz> (organizadores) y <https://staff.trusttix.xyz> (personal de acceso). En local, <http://localhost:5173/?portal=app> con `npm run dev`.

**Red:** Stellar Testnet, contrato v2 `CDKVPZI5HC25H6ILACGF24PNTTNYMUDQBSAJGMSUKEQEFUYVSAAKKRD4`. No se usa dinero real: la billetera TrustTix recibe XLM de prueba al crearse.

**Datos de ejemplo** (creados con `contracts/scripts/sembrar.ps1`):

| Folio | Evento | Estado |
| --- | --- | --- |
| TTX-000001 | Prueba de cancelacion | Evento cancelado, boleto reembolsado |
| TTX-000002 | Noche de reventa justa | En reventa en 27.5 XLM |
| TTX-000004 | GOYA HACK · UNAM | Ya entró al evento |
| TTX-000006 | Bruno Mars en concierto | En reventa en 90 XLM (tope del evento: 120 %) |
| TTX-000007 | Bruno Mars en concierto | En reventa en 108 XLM, justo en el tope |

## 1. Compra

| # | Caso | Pasos | Resultado esperado |
| --- | --- | --- | --- |
| 1.1 | Ver eventos sin cuenta | Abrir app.trusttix.xyz sin iniciar sesión. | Se ven los eventos con precio y lugares disponibles. El evento cancelado no aparece. |
| 1.2 | Comprar con billetera nueva | Iniciar sesión con Google, abrir Caifanes en concierto, tocar Comprar boleto y Pagar. | Se crea la billetera TrustTix con XLM de prueba, se cobra 100 XLM y el boleto aparece en Mis boletos con su QR. |
| 1.3 | Comisión del 3 % | Revisar la transacción de la compra en stellar.expert (enlace Ver comprobante). | El organizador recibe 97 XLM y la cuenta de TrustTix 3 XLM, en la misma transacción. |
| 1.4 | Evento agotado | Abrir GOYA HACK · UNAM. | El botón dice Agotado y no deja comprar. |
| 1.5 | Sin saldo suficiente | Con una billetera con menos saldo que el precio, intentar comprar. | La app avisa que el saldo no alcanza y no se cobra nada. |
| 1.6 | Evento gratis | Obtener boleto de AWS Summit Mexico City. | Se emite el boleto sin cobro ni comisión. |

## 2. Reventa

| # | Caso | Pasos | Resultado esperado |
| --- | --- | --- | --- |
| 2.1 | Ver reventas | Abrir Bruno Mars en concierto. | La sección Reventa lista TTX-000006 (90 XLM) y TTX-000007 (108 XLM), del más barato al más caro. |
| 2.2 | Comprar en reventa | Con sesión, tocar Comprar en TTX-000006 y Pagar. | Se cobran 90 XLM, el vendedor recibe 88.2 XLM y TrustTix 1.8 XLM. El boleto pasa a tu cuenta y sale de la lista. |
| 2.3 | Publicar en reventa | En Tu boleto de un evento de pago, tocar Revender, escribir un precio dentro del tope y Publicar. | El boleto aparece como "Está a la venta" con lo que recibirías, y sale en la página del evento. |
| 2.4 | Precio arriba del tope | En el mismo formulario, escribir un precio mayor al máximo. | La app avisa que pasa del tope y no deja publicar. La red también lo rechazaría. |
| 2.5 | Cambiar precio y retirar | En un boleto en reventa, tocar Cambiar precio y luego Retirar. | El precio se actualiza; al retirarlo deja de aparecer en la página del evento. |
| 2.6 | Transferir quita la reventa | Transferir un boleto que estaba en reventa. | La oferta desaparece: era del dueño anterior. |
| 2.7 | No comprar tu propio boleto | Abrir un evento donde tienes un boleto en reventa. | Tu boleto aparece como "Tu boleto" con botón Ver, sin opción de compra. |

## 3. Entrada y verificación

| # | Caso | Pasos | Resultado esperado |
| --- | --- | --- | --- |
| 3.1 | Verificar un boleto válido | En Verificar, buscar TTX-000006. | Boleto válido, con el dueño actual y enlace al registro público. |
| 3.2 | Verificar un boleto usado | Buscar TTX-000004. | Boleto ya utilizado, con la fecha en que entró. |
| 3.3 | Verificar un boleto reembolsado | Buscar TTX-000001. | Boleto reembolsado: el evento se canceló y no debe comprarse. |
| 3.4 | Autorizar personal | En organizer, abrir el evento, escanear el código de la cuenta de personal y firmar. | La cuenta queda autorizada y puede abrir el escáner de ese evento en staff. |
| 3.5 | Registrar una entrada | En staff, escanear el QR de un boleto válido y tocar registrar. | La entrada queda registrada y el boleto pasa a Usado. |
| 3.6 | QR repetido | Escanear otra vez el mismo QR. | Se rechaza: el boleto ya fue utilizado. |
| 3.7 | Boleto de otro evento | Escanear el QR de un boleto de otro evento. | El escáner avisa que es de otro evento y no deja registrarlo. |

## 4. Cancelación y reembolso

Las funciones `cancelar_evento` y `reembolsar` todavía no tienen botón en el panel del organizador; se prueban con la CLI de Stellar y se revisan en la app.

| # | Caso | Pasos | Resultado esperado |
| --- | --- | --- | --- |
| 4.1 | Evento cancelado | Abrir `/eventos/1` en la app. | Aviso de evento cancelado y botón Cancelado, sin compra ni reventa. |
| 4.2 | Boleto reembolsado | Con la cuenta dueña de TTX-000001, abrir Tu boleto. | Sello REEMBOLSADO, sin QR, sin Transferir ni Revender. |
| 4.3 | Reglas en el contrato | `cd contracts` y `cargo test`. | Pasan las pruebas de cancelación: no vende, no revende, no registra entradas y reembolsa una sola vez. |

## 5. Billetera

| # | Caso | Pasos | Resultado esperado |
| --- | --- | --- | --- |
| 5.1 | Misma billetera en otro dispositivo | Iniciar sesión con la misma cuenta de Google en otro navegador. | Aparece la misma billetera con los mismos boletos. |
| 5.2 | Enviar XLM con nota | En Billetera, enviar XLM a otra cuenta con una nota. | La transacción lleva la nota (memo) y se ve en stellar.expert. |
| 5.3 | Comisión de red patrocinada | Comprar desde app.trusttix.xyz. | La comisión de la red la paga la cuenta de TrustTix, no la billetera de quien compra. |
