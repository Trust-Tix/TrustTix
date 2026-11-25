# Reglas de negocio de TrustTix

Este documento explica, sin código, cómo se cobra y se reparte el dinero en TrustTix: la compra de un boleto, la reventa y las comisiones. Está escrito para organizadores e inversionistas.

Estas reglas no dependen de la buena voluntad de nadie: las hace cumplir el contrato de TrustTix en la red Stellar. Si una operación rompe una regla, la red la rechaza completa y no se mueve dinero.

**Estado:** son las reglas del contrato versión 2, que se publica en Stellar Testnet (red de prueba, sin dinero real). La app de trusttix.xyz todavía usa la versión 1, que no tiene comisión ni reventa.

## Resumen

| Regla | Valor |
| --- | --- |
| Comisión de TrustTix en cada compra | 3 % del precio |
| Comisión de TrustTix en cada reventa | 2 % del precio de reventa |
| Comisión máxima permitida | 10 %, para compra y para reventa |
| Precio máximo de reventa | 110 % del precio original |
| Comisión en boletos gratis | Ninguna |
| Lugares que se pueden vender | Nunca más que la capacidad del evento |

## 1. Compra de un boleto

- **El comprador paga el precio publicado y nada más.** La comisión no se suma al precio: sale de él.
- Del precio, el **97 % va al organizador** y el **3 % a TrustTix**, en el mismo momento de la compra. El organizador no espera liquidaciones.
- **Ejemplo:** un boleto de 100 XLM. El comprador paga 100 XLM; el organizador recibe 97 XLM y TrustTix 3 XLM.
- El cobro, el pago al organizador, la comisión y la emisión del boleto ocurren en **una sola transacción**. Si algo falla, no se cobra nada y no se emite el boleto.
- Los **boletos gratis** no pagan comisión.
- La **capacidad** del evento queda fija al crearlo. Cuando se vende el último lugar, la red rechaza cualquier otra compra: no hay sobreventa.
- Si al calcular la comisión quedan fracciones, se redondea a favor del organizador.

Aparte está la comisión de la red Stellar por cada operación, que es una fracción de centavo. La app de TrustTix puede pagarla por el usuario.

## 2. Reventa

### Publicar un boleto en reventa

- Solo el **dueño actual** puede poner su boleto en reventa.
- El precio **no puede pasar del 110 % del precio original**. Un boleto de 100 XLM se puede revender en 110 XLM como máximo. La red rechaza cualquier precio más alto.
- El dueño puede **cambiar el precio** volviéndolo a publicar, o **retirar** su boleto de la reventa cuando quiera.
- Un boleto que ya se usó para entrar **no se puede revender**.

### Comprar un boleto en reventa

- El comprador paga el precio de reventa publicado.
- De ese precio, el **98 % va al vendedor** y el **2 % a TrustTix**.
- **Ejemplo:** un boleto revendido en 110 XLM. El vendedor recibe 107.80 XLM y TrustTix 2.20 XLM.
- El pago, la comisión y el **cambio de dueño** ocurren en una sola transacción. El vendedor no puede quedarse con el dinero y con el boleto.
- Nadie puede comprar su propio boleto.
- Hoy el organizador **no recibe** una parte de la reventa.

### Cuándo un boleto sale de la reventa

Un boleto deja de estar a la venta, sin que nadie tenga que hacer nada más, cuando:

1. Se vende en la reventa.
2. Su dueño lo **transfiere** a otra persona: la oferta era del dueño anterior.
3. Se **registra la entrada** al evento: un boleto usado ya no vale para entrar.
4. Su dueño la **cancela**.

## 3. Boletos usados y transferencias

- Cada boleto **entra una sola vez**. Solo el organizador o el personal que él autorizó puede registrar la entrada.
- Un boleto usado **no se puede transferir ni revender**.
- Transferir un boleto (regalarlo) **no paga comisión**.

## 4. Quién puede cambiar las comisiones

- Solo la **cuenta administradora de TrustTix** puede cambiar las comisiones, y nunca arriba del **10 %**, ni siquiera por error.
- Cada cambio queda **registrado públicamente** en la red, con los valores nuevos.
- Una venta paga la comisión vigente en el momento en que ocurre. Cambiar la comisión no afecta las ventas anteriores.

## 5. Todo se puede verificar

Cualquier persona, sin cuenta, puede consultar en la red:

- Los datos de un evento: capacidad, boletos vendidos, precio y tope de reventa.
- Quién es el dueño de un boleto y si ya se usó.
- Si un boleto está en reventa y a qué precio.
- Las comisiones vigentes.

Así, antes de comprar en reventa, el comprador puede comprobar que el boleto existe, que no se ha usado y quién lo vende.

## 6. Lo que viene

En desarrollo, todavía **no** está en el contrato. Cuando llegue, este documento se actualizará:

- **Tope de reventa por evento:** hoy todos los eventos tienen el mismo tope de 110 %. El organizador podrá fijar el suyo.
- **Cancelar un evento y reembolsar:** el organizador podrá cancelar su evento. Un evento cancelado no vende ni revende, y los compradores reciben su reembolso.
