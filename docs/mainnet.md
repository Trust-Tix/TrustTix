# TrustTix en Mainnet

TrustTix funciona en la red principal de Stellar en **https://mainnet.trusttix.xyz**, con XLM real. La app de Testnet (https://app.trusttix.xyz) sigue para demos y pruebas.

## Contrato

| Dato | Valor |
| --- | --- |
| Contrato | [`CBEPJEJDV2C7HV3PO7V7Z4P5XIADQCXFES2Q2P6DGDVB3LKS2QSPXSJ5`](https://stellar.expert/explorer/public/contract/CBEPJEJDV2C7HV3PO7V7Z4P5XIADQCXFES2Q2P6DGDVB3LKS2QSPXSJ5) |
| Código | [`contracts/trusttix-mainnet`](../contracts/trusttix-mainnet): el núcleo que usa la app más `actualizar` |
| Despliegue | [subida del código](https://stellar.expert/explorer/public/tx/a8736c15fc24024cb782338e26625fa4de29168bea5e1daa78c5073d14645886) y [creación del contrato](https://stellar.expert/explorer/public/tx/0b6cd66ae60528b53b37581a9765276e50844050af4d63011101323639a25199), con [`desplegar-mainnet.ps1`](../contracts/scripts/desplegar-mainnet.ps1) |
| Pago | XLM nativo (Stellar Asset Contract `CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA`) |
| Comisiones | 3 % por compra y 2 % por reventa, para la cuenta administradora de TrustTix |

## Cómo se conecta la app a Mainnet

La misma app se publica dos veces. La variable `VITE_RED=mainnet` ([`frontend/src/stellar/config.js`](../frontend/src/stellar/config.js)) cambia el contrato, el RPC (`https://mainnet.sorobanrpc.com`), Horizon y el explorador, y además:

- **Solo billeteras propias** (Freighter, xBull, LOBSTR y otras, con [Stellar Wallets Kit](../frontend/src/stellar/billetera.js)). La billetera integrada de Testnet no existe en Mainnet porque guarda llaves.
- **La billetera es la identidad:** comprar, revender y ver tus boletos no piden cuenta de Google.
- **Sin patrocinio ni Friendbot:** cada quien paga su comisión de red (una fracción de centavo).
- **Aviso visible:** la barra y el encabezado de cada pantalla dicen MAINNET en amarillo.

Todas las llamadas al contrato pasan por [`frontend/src/stellar/contrato.js`](../frontend/src/stellar/contrato.js): la app simula la llamada (si el contrato la rechazaría, avisa antes de pedir la firma), la firma la billetera conectada, se envía con una comisión de inclusión de 0.001 XLM y la app espera la confirmación de la red.

## Las 5 funciones que usa el frontend

Cada una tiene una transacción real en Mainnet hecha desde la app.

### 1. `comprar`: comprar un boleto

- **Dónde:** página del evento, botón **Comprar boleto** y **Pagar** en la ventana de confirmación ([`DetalleEvento.jsx`](../frontend/src/pages/DetalleEvento.jsx)).
- **Cómo:** `comprar` en [`AppContext.jsx`](../frontend/src/context/AppContext.jsx) llama a `red.comprar(cuenta, eventoId)` de `contrato.js`. El contrato cobra el precio, paga el 97 % al organizador y el 3 % a TrustTix, y emite el boleto a nombre de quien paga, en una sola transacción. La app lleva a **Tu boleto** con el QR y el enlace al comprobante.
- **Transacción:** [8feebb36…](https://stellar.expert/explorer/public/tx/8feebb36a6f45098e2f16a1827de960bdcbe9f4bc799ae6a301ce07fb5d84dba) (boleto TTX-000002 del evento "Lanzamiento de TrustTix en Mainnet", 1 XLM).

### 2. `publicar_reventa`: poner un boleto a la venta con precio tope

- **Dónde:** **Tu boleto**, sección **Reventa**, botón **Revender** ([`Revender.jsx`](../frontend/src/components/Revender.jsx), dentro de [`DetalleBoleto.jsx`](../frontend/src/pages/DetalleBoleto.jsx)).
- **Cómo:** la ventana muestra el precio máximo (110 % del original), la comisión y lo que recibe quien vende; no deja publicar arriba del tope. `publicarReventa` de `AppContext.jsx` llama a `red.publicarReventa(billetera, boletoId, precio)`. Volver a publicar cambia el precio (**Cambiar precio**).
- **Transacción:** [62be3d76…](https://stellar.expert/explorer/public/tx/62be3d76021aef8a07600cd67442c6314641baef43de555f780baef9c8e2e806) (TTX-000002 en 1.1 XLM).

### 3. `cancelar_reventa`: retirar el boleto de la venta

- **Dónde:** **Tu boleto**, sección **Reventa**, botón **Retirar** (aparece cuando el boleto está a la venta; [`Revender.jsx`](../frontend/src/components/Revender.jsx)).
- **Cómo:** `cancelarReventa` de `AppContext.jsx` llama a `red.cancelarReventa(billetera, boletoId)`. Solo el dueño puede hacerlo; el boleto deja de aparecer en la página del evento.
- **Transacción:** [c6dfa8a6…](https://stellar.expert/explorer/public/tx/c6dfa8a68f2690f03f4171a9c9a9fcf97f5b6a9210afcadf3e5066b9030025a9).

### 4. `comprar_reventa`: comprar un boleto en reventa

- **Dónde:** página del evento, sección **Reventa**: lista de boletos a la venta, del más barato al más caro, con botón **Comprar** y **Pagar** ([`Reventas.jsx`](../frontend/src/components/Reventas.jsx)).
- **Cómo:** `reventasDeEvento` de `AppContext.jsx` arma la lista con `consultar_reventa`; `comprarReventa` llama a `red.comprarReventa(cuenta, boletoId)`. El contrato cobra a quien compra, paga a quien vende menos el 2 % y cambia el dueño en la misma transacción: nadie se queda con el dinero y con el boleto.
- **Transacción:** [2c3894c4…](https://stellar.expert/explorer/public/tx/2c3894c4f7fb2d1a9f224cfd118ebd41551e9a0f6e6ac1475f23800685e66614) (otra cuenta compra TTX-000002 en 1.1 XLM; antes se volvió a publicar con [d2b48255…](https://stellar.expert/explorer/public/tx/d2b48255ae12701e3e13a358d3bba04cba66be869d13863ae467184d54cf2fa2)).

### 5. `transferir`: regalar o pasar un boleto

- **Dónde:** **Tu boleto**, botón **Transferir**, que abre [`Transferir.jsx`](../frontend/src/pages/Transferir.jsx): se escribe o se escanea con la cámara la cuenta de quien recibe.
- **Cómo:** `transferir` de `AppContext.jsx` llama a `red.transferir(billetera, boletoId, destino)`. Solo el dueño puede transferir y nunca un boleto ya usado; si estaba en reventa, la oferta se retira sola.
- **Transacción:** [70830154…](https://stellar.expert/explorer/public/tx/70830154d7bde594bcab02da1dccad73060b27e86ca0fd6e74d69386963892da) (TTX-000002 regresa a la primera cuenta).

## Otras funciones del contrato que la app ya usa

Consultas sin firma: `total_eventos`, `consultar_evento`, `consultar_boleto`, `consultar_reventa`, `boletos_de`, `comisiones`, `es_personal`. Con firma, en los portales de organizador y personal: `crear_evento`, `autorizar_personal`, `quitar_personal` y `registrar_entrada` (escáner de la puerta).

## Límites

El contrato no tiene auditoría externa. En Mainnet los eventos de TrustTix usan precios simbólicos mientras tanto. Las funciones nuevas (cortesías por grupo, apartados, preventa y demás) se prueban primero en el contrato de Testnet ([`contracts/trusttix`](../contracts/trusttix)) y se agregan a Mainnet con `actualizar` cuando la app las use.
