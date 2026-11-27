#![no_std]

//! Contrato TrustTix: boletos de eventos imposibles de duplicar.
//!
//! Reglas que hace cumplir la red:
//! 1. Un evento nunca vende más boletos que su capacidad.
//! 2. Cada boleto tiene un solo dueño, y solo ese dueño puede transferirlo.
//! 3. El pago al organizador, la comisión de TrustTix y la emisión del boleto
//!    ocurren en la misma transacción.
//! 4. Solo el personal autorizado por el organizador registra entradas, y cada
//!    boleto entra una sola vez.
//! 5. Un evento cancelado no vende, no revende ni registra entradas, y el
//!    organizador devuelve el precio de cada boleto a su dueño.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, BytesN, ContractExecutable, Env, String, Vec,
};

/// Ledgers aproximados en un día (un ledger cada ~5 segundos).
const DIA: u32 = 17_280;
/// Si a un registro le quedan menos de 7 días de vida, se extiende a 30.
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;

/// Las comisiones se expresan en puntos base: 300 = 3 %.
const PUNTOS_BASE: i128 = 10_000;
/// Ninguna comisión puede pasar del 10 %, ni siquiera por decisión del administrador.
const COMISION_MAXIMA: u32 = 1_000;
/// Tope inicial de reventa de cada evento: 110 % del precio original.
const TOPE_REVENTA_INICIAL: u32 = 11_000;
/// El organizador puede fijar el tope entre el 100 % (sin ganancia para quien
/// revende) y el 150 % del precio original.
const TOPE_REVENTA_MINIMO: u32 = 10_000;
const TOPE_REVENTA_MAXIMO: u32 = 15_000;
/// Boletos que una cuenta puede comprar por evento, sumando todas sus compras.
const LIMITE_POR_CUENTA: u32 = 10;
/// Boletos que se reembolsan como máximo en una sola transacción. Cada uno
/// escribe el boleto y el saldo de su dueño, y la red limita cuántos registros
/// cambia una transacción.
const LOTE_MAXIMO: u32 = 10;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    EventoNoExiste = 1,
    BoletoNoExiste = 2,
    Agotado = 3,
    NoEsDueno = 4,
    YaUsado = 5,
    NoAutorizado = 6,
    CapacidadInvalida = 7,
    PrecioInvalido = 8,
    MismoDueno = 9,
    ComisionInvalida = 10,
    SobreTope = 11,
    NoEnReventa = 12,
    TopeInvalido = 13,
    EventoCancelado = 14,
    EventoActivo = 15,
    YaReembolsado = 16,
    LimitePorCuenta = 17,
    CantidadInvalida = 18,
    NoHayOferta = 19,
}

/// Comisiones de TrustTix en puntos base: `compra` sobre cada boleto vendido y
/// `reventa` sobre cada boleto revendido.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comisiones {
    pub compra: u32,
    pub reventa: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Evento {
    pub organizador: Address,
    pub nombre: String,
    pub recinto: String,
    pub fecha: String,
    pub capacidad: u32,
    pub vendidos: u32,
    /// Precio en la unidad mínima del token de pago (stroops si es XLM).
    pub precio: i128,
    /// Precio máximo de reventa, en puntos base sobre `precio` (11,000 = 110 %).
    pub tope_reventa: u32,
    pub cancelado: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Boleto {
    pub evento: u32,
    pub dueno: Address,
    pub usado: bool,
    /// Marca de tiempo del ledger en que entró (0 si no se ha usado).
    pub usado_en: u64,
    /// El organizador ya devolvió su precio porque el evento se canceló.
    pub reembolsado: bool,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Token,
    Admin,
    Comisiones,
    TotalEventos,
    TotalBoletos,
    Evento(u32),
    Boleto(u32),
    Personal(u32, Address),
    BoletosDe(Address),
    /// Precio al que el dueño ofrece el boleto en reventa.
    Reventa(u32),
    /// Boletos que una cuenta ha pagado en un evento.
    Compras(u32, Address),
    /// Dinero en garantía que una cuenta ofrece por un boleto.
    Oferta(u32, Address),
}

// ---------- Eventos que quedan en el registro público ----------

#[contractevent]
pub struct EventoCreado {
    #[topic]
    pub evento: u32,
    pub organizador: Address,
    pub capacidad: u32,
}

#[contractevent]
pub struct Compra {
    #[topic]
    pub evento: u32,
    pub boleto: u32,
    pub comprador: Address,
    pub precio: i128,
    /// Parte del precio que fue a TrustTix.
    pub comision: i128,
}

#[contractevent]
pub struct CompraVarios {
    #[topic]
    pub evento: u32,
    pub comprador: Address,
    pub boletos: Vec<u32>,
    pub total: i128,
    /// Parte del total que fue a TrustTix.
    pub comision: i128,
}

#[contractevent]
pub struct Regalo {
    #[topic]
    pub boleto: u32,
    pub de: Address,
    pub para: Address,
}

#[contractevent]
pub struct Devolucion {
    #[topic]
    pub boleto: u32,
    pub evento: u32,
    pub dueno: Address,
}

#[contractevent]
pub struct OfertaHecha {
    #[topic]
    pub boleto: u32,
    pub comprador: Address,
    pub precio: i128,
}

#[contractevent]
pub struct OfertaRetirada {
    #[topic]
    pub boleto: u32,
    pub comprador: Address,
}

#[contractevent]
pub struct Actualizacion {
    pub wasm: BytesN<32>,
}

#[contractevent]
pub struct ReventaPublicada {
    #[topic]
    pub boleto: u32,
    pub vendedor: Address,
    pub precio: i128,
}

#[contractevent]
pub struct Reventa {
    #[topic]
    pub boleto: u32,
    pub vendedor: Address,
    pub comprador: Address,
    pub precio: i128,
    /// Parte del precio que fue a TrustTix.
    pub comision: i128,
}

#[contractevent]
pub struct ReventaCancelada {
    #[topic]
    pub boleto: u32,
}

#[contractevent]
pub struct TopeReventaActualizado {
    #[topic]
    pub evento: u32,
    /// Nuevo tope en puntos base sobre el precio original.
    pub tope: u32,
}

#[contractevent]
pub struct Cancelacion {
    #[topic]
    pub evento: u32,
    pub vendidos: u32,
}

#[contractevent]
pub struct Reembolso {
    #[topic]
    pub boleto: u32,
    pub evento: u32,
    pub dueno: Address,
    pub monto: i128,
}

#[contractevent]
pub struct ComisionesActualizadas {
    pub compra: u32,
    pub reventa: u32,
}

#[contractevent]
pub struct Transferencia {
    #[topic]
    pub boleto: u32,
    pub de: Address,
    pub a: Address,
}

#[contractevent]
pub struct Entrada {
    #[topic]
    pub boleto: u32,
    pub evento: u32,
    pub personal: Address,
}

#[contract]
pub struct TrustTix;

#[contractimpl]
impl TrustTix {
    /// Se ejecuta una sola vez al desplegar. `token` es el contrato del activo
    /// con el que se paga (en Testnet, el Stellar Asset Contract de XLM).
    /// `admin` es la cuenta de TrustTix: recibe las comisiones y es la única que
    /// puede cambiarlas.
    pub fn __constructor(env: Env, token: Address, admin: Address, compra: u32, reventa: u32) {
        let comisiones = validar_comisiones(&env, compra, reventa);
        env.storage().instance().set(&Clave::Token, &token);
        env.storage().instance().set(&Clave::Admin, &admin);
        env.storage()
            .instance()
            .set(&Clave::Comisiones, &comisiones);
        env.storage().instance().set(&Clave::TotalEventos, &0u32);
        env.storage().instance().set(&Clave::TotalBoletos, &0u32);
    }

    // ---------- Administrador ----------

    /// Cambia las comisiones de TrustTix. Solo el administrador, y nunca arriba del 10 %.
    pub fn configurar_comision(env: Env, compra: u32, reventa: u32) -> Result<(), Error> {
        admin(&env).require_auth();
        if compra > COMISION_MAXIMA || reventa > COMISION_MAXIMA {
            return Err(Error::ComisionInvalida);
        }
        env.storage()
            .instance()
            .set(&Clave::Comisiones, &Comisiones { compra, reventa });
        ComisionesActualizadas { compra, reventa }.publish(&env);
        Ok(())
    }

    /// Cambia el código del contrato por uno ya subido a la red, sin perder
    /// eventos, boletos ni saldos. Solo el administrador.
    pub fn actualizar(env: Env, wasm: BytesN<32>) {
        admin(&env).require_auth();
        env.deployer()
            .update_current_contract(ContractExecutable::Wasm(wasm.clone()));
        Actualizacion { wasm }.publish(&env);
    }

    // ---------- Organizador ----------

    /// Registra un evento. La capacidad queda fija y no se puede rebasar.
    pub fn crear_evento(
        env: Env,
        organizador: Address,
        nombre: String,
        recinto: String,
        fecha: String,
        capacidad: u32,
        precio: i128,
    ) -> Result<u32, Error> {
        organizador.require_auth();
        if capacidad == 0 {
            return Err(Error::CapacidadInvalida);
        }
        if precio < 0 {
            return Err(Error::PrecioInvalido);
        }

        let id = siguiente(&env, Clave::TotalEventos);
        let evento = Evento {
            organizador: organizador.clone(),
            nombre,
            recinto,
            fecha,
            capacidad,
            vendidos: 0,
            precio,
            tope_reventa: TOPE_REVENTA_INICIAL,
            cancelado: false,
        };
        guardar(&env, &Clave::Evento(id), &evento);
        EventoCreado {
            evento: id,
            organizador,
            capacidad,
        }
        .publish(&env);
        Ok(id)
    }

    /// El organizador autoriza una cuenta para registrar entradas en su evento.
    pub fn autorizar_personal(env: Env, evento_id: u32, cuenta: Address) -> Result<(), Error> {
        let evento = leer_evento(&env, evento_id)?;
        evento.organizador.require_auth();
        guardar(&env, &Clave::Personal(evento_id, cuenta), &true);
        Ok(())
    }

    /// El organizador fija el precio máximo de reventa de su evento, en puntos
    /// base sobre el precio original (10,000 = 100 %, 15,000 = 150 %). Aplica
    /// también a los boletos que ya estaban en reventa.
    pub fn fijar_tope_reventa(env: Env, evento_id: u32, tope: u32) -> Result<(), Error> {
        let mut evento = leer_evento(&env, evento_id)?;
        evento.organizador.require_auth();
        if evento.cancelado {
            return Err(Error::EventoCancelado);
        }
        if !(TOPE_REVENTA_MINIMO..=TOPE_REVENTA_MAXIMO).contains(&tope) {
            return Err(Error::TopeInvalido);
        }
        evento.tope_reventa = tope;
        guardar(&env, &Clave::Evento(evento_id), &evento);
        TopeReventaActualizado {
            evento: evento_id,
            tope,
        }
        .publish(&env);
        Ok(())
    }

    /// El organizador cancela su evento: desde ese momento no se vende, no se
    /// revende ni se registran entradas. Los boletos se reembolsan uno por uno
    /// con `reembolsar`.
    pub fn cancelar_evento(env: Env, evento_id: u32) -> Result<(), Error> {
        let mut evento = leer_evento(&env, evento_id)?;
        evento.organizador.require_auth();
        if evento.cancelado {
            return Err(Error::EventoCancelado);
        }
        evento.cancelado = true;
        guardar(&env, &Clave::Evento(evento_id), &evento);
        Cancelacion {
            evento: evento_id,
            vendidos: evento.vendidos,
        }
        .publish(&env);
        Ok(())
    }

    /// En un evento cancelado, el organizador devuelve el precio original del
    /// boleto a su dueño actual. Cada boleto se reembolsa una sola vez y queda
    /// sin validez para entrar, transferir o revender.
    pub fn reembolsar(env: Env, boleto_id: u32) -> Result<i128, Error> {
        let mut boleto = leer_boleto(&env, boleto_id)?;
        let evento = leer_evento(&env, boleto.evento)?;
        evento.organizador.require_auth();
        if !evento.cancelado {
            return Err(Error::EventoActivo);
        }
        if boleto.reembolsado {
            return Err(Error::YaReembolsado);
        }
        if boleto.usado {
            return Err(Error::YaUsado);
        }
        devolver_precio(&env, &evento, boleto_id, &mut boleto);
        Ok(evento.precio)
    }

    /// Reembolsa de una vez los boletos de un evento cancelado entre los
    /// números `desde` y `hasta` (hasta 10 por transacción). Se saltan los de
    /// otros eventos, los ya reembolsados y los que ya entraron. Devuelve
    /// cuántos reembolsó.
    pub fn reembolsar_lote(env: Env, evento_id: u32, desde: u32, hasta: u32) -> Result<u32, Error> {
        let evento = leer_evento(&env, evento_id)?;
        evento.organizador.require_auth();
        if !evento.cancelado {
            return Err(Error::EventoActivo);
        }
        if desde == 0 || hasta < desde || hasta - desde >= LOTE_MAXIMO {
            return Err(Error::CantidadInvalida);
        }

        let mut reembolsados = 0;
        for boleto_id in desde..=hasta {
            if let Ok(mut boleto) = leer_boleto(&env, boleto_id) {
                if boleto.evento == evento_id && !boleto.usado && !boleto.reembolsado {
                    devolver_precio(&env, &evento, boleto_id, &mut boleto);
                    reembolsados += 1;
                }
            }
        }
        Ok(reembolsados)
    }

    /// El organizador retira la autorización de una cuenta.
    pub fn quitar_personal(env: Env, evento_id: u32, cuenta: Address) -> Result<(), Error> {
        let evento = leer_evento(&env, evento_id)?;
        evento.organizador.require_auth();
        env.storage()
            .persistent()
            .remove(&Clave::Personal(evento_id, cuenta));
        Ok(())
    }

    // ---------- Asistente ----------

    /// Cobra el precio al comprador, paga al organizador, separa la comisión de
    /// TrustTix y emite el boleto, todo en la misma transacción. Falla si ya no
    /// quedan lugares.
    pub fn comprar(env: Env, comprador: Address, evento_id: u32) -> Result<u32, Error> {
        comprador.require_auth();
        let mut evento = preparar_compra(&env, &comprador, evento_id, 1)?;
        let comision = cobrar(
            &env,
            &comprador,
            &evento.organizador,
            evento.precio,
            comisiones(&env).compra,
        );
        let id = emitir(&env, evento_id, &mut evento, &comprador);
        guardar(&env, &Clave::Evento(evento_id), &evento);

        Compra {
            evento: evento_id,
            boleto: id,
            comprador,
            precio: evento.precio,
            comision,
        }
        .publish(&env);
        Ok(id)
    }

    /// Compra varios boletos del mismo evento en una sola transacción: un cobro
    /// por el total, una comisión y todos los boletos a nombre del comprador.
    /// Ninguna cuenta pasa de 10 boletos por evento.
    pub fn comprar_varios(
        env: Env,
        comprador: Address,
        evento_id: u32,
        cantidad: u32,
    ) -> Result<Vec<u32>, Error> {
        comprador.require_auth();
        if cantidad == 0 {
            return Err(Error::CantidadInvalida);
        }
        let mut evento = preparar_compra(&env, &comprador, evento_id, cantidad)?;
        let total = evento.precio * cantidad as i128;
        let comision = cobrar(
            &env,
            &comprador,
            &evento.organizador,
            total,
            comisiones(&env).compra,
        );

        let mut boletos = Vec::new(&env);
        for _ in 0..cantidad {
            boletos.push_back(emitir(&env, evento_id, &mut evento, &comprador));
        }
        guardar(&env, &Clave::Evento(evento_id), &evento);

        CompraVarios {
            evento: evento_id,
            comprador,
            boletos: boletos.clone(),
            total,
            comision,
        }
        .publish(&env);
        Ok(boletos)
    }

    /// Compra un boleto de regalo: paga `comprador` y el boleto queda desde el
    /// inicio a nombre de `para`. El regalo cuenta en el límite de quien paga.
    pub fn comprar_para(
        env: Env,
        comprador: Address,
        para: Address,
        evento_id: u32,
    ) -> Result<u32, Error> {
        comprador.require_auth();
        if comprador == para {
            return Err(Error::MismoDueno);
        }
        let mut evento = preparar_compra(&env, &comprador, evento_id, 1)?;
        let comision = cobrar(
            &env,
            &comprador,
            &evento.organizador,
            evento.precio,
            comisiones(&env).compra,
        );
        let id = emitir(&env, evento_id, &mut evento, &para);
        guardar(&env, &Clave::Evento(evento_id), &evento);

        Compra {
            evento: evento_id,
            boleto: id,
            comprador: comprador.clone(),
            precio: evento.precio,
            comision,
        }
        .publish(&env);
        Regalo {
            boleto: id,
            de: comprador,
            para,
        }
        .publish(&env);
        Ok(id)
    }

    /// Solo el dueño actual puede transferir, y nunca un boleto ya usado.
    pub fn transferir(env: Env, boleto_id: u32, de: Address, a: Address) -> Result<(), Error> {
        de.require_auth();
        mover(&env, boleto_id, &de, &a)
    }

    /// Devolución acordada: el dueño entrega su boleto y el organizador le
    /// regresa el precio original. Firman los dos. El boleto queda sin validez
    /// y su lugar vuelve a la venta. Devuelve el monto regresado.
    pub fn devolver_boleto(env: Env, boleto_id: u32) -> Result<i128, Error> {
        let mut boleto = leer_boleto(&env, boleto_id)?;
        let mut evento = leer_evento(&env, boleto.evento)?;
        boleto.dueno.require_auth();
        evento.organizador.require_auth();
        if evento.cancelado {
            return Err(Error::EventoCancelado);
        }
        if boleto.usado {
            return Err(Error::YaUsado);
        }
        if boleto.reembolsado {
            return Err(Error::YaReembolsado);
        }

        devolver_precio(&env, &evento, boleto_id, &mut boleto);
        evento.vendidos -= 1;
        guardar(&env, &Clave::Evento(boleto.evento), &evento);
        Devolucion {
            boleto: boleto_id,
            evento: boleto.evento,
            dueno: boleto.dueno,
        }
        .publish(&env);
        Ok(evento.precio)
    }

    // ---------- Ofertas ----------

    /// Ofrece comprar un boleto que no está en reventa. El dinero queda en
    /// garantía en el contrato hasta que el dueño acepte o quien oferta la
    /// retire. Ofertar otra vez por el mismo boleto reemplaza la oferta.
    pub fn ofertar(
        env: Env,
        comprador: Address,
        boleto_id: u32,
        precio: i128,
    ) -> Result<(), Error> {
        comprador.require_auth();
        let boleto = leer_boleto(&env, boleto_id)?;
        if boleto.usado {
            return Err(Error::YaUsado);
        }
        if boleto.reembolsado {
            return Err(Error::YaReembolsado);
        }
        if boleto.dueno == comprador {
            return Err(Error::MismoDueno);
        }
        let evento = leer_evento(&env, boleto.evento)?;
        if evento.cancelado {
            return Err(Error::EventoCancelado);
        }
        if precio <= 0 {
            return Err(Error::PrecioInvalido);
        }
        if precio > precio_maximo(&evento) {
            return Err(Error::SobreTope);
        }

        let token = token::Client::new(&env, &token_de_pago(&env));
        let contrato = env.current_contract_address();
        let clave = Clave::Oferta(boleto_id, comprador.clone());
        if let Some(anterior) = env.storage().persistent().get::<_, i128>(&clave) {
            token.transfer(&contrato, &comprador, &anterior);
        }
        token.transfer(&comprador, &contrato, &precio);
        guardar(&env, &clave, &precio);
        OfertaHecha {
            boleto: boleto_id,
            comprador,
            precio,
        }
        .publish(&env);
        Ok(())
    }

    /// El dueño acepta una oferta: recibe el precio menos la comisión de
    /// reventa y el boleto pasa a quien ofertó, en una sola transacción.
    pub fn aceptar_oferta(
        env: Env,
        boleto_id: u32,
        vendedor: Address,
        comprador: Address,
    ) -> Result<(), Error> {
        vendedor.require_auth();
        let clave = Clave::Oferta(boleto_id, comprador.clone());
        let precio: i128 = env
            .storage()
            .persistent()
            .get(&clave)
            .ok_or(Error::NoHayOferta)?;
        let boleto = leer_boleto(&env, boleto_id)?;
        let evento = leer_evento(&env, boleto.evento)?;
        if evento.cancelado {
            return Err(Error::EventoCancelado);
        }
        if precio > precio_maximo(&evento) {
            return Err(Error::SobreTope);
        }

        mover(&env, boleto_id, &vendedor, &comprador)?;
        env.storage().persistent().remove(&clave);
        let comision = cobrar(
            &env,
            &env.current_contract_address(),
            &vendedor,
            precio,
            comisiones(&env).reventa,
        );
        Reventa {
            boleto: boleto_id,
            vendedor,
            comprador,
            precio,
            comision,
        }
        .publish(&env);
        Ok(())
    }

    /// Quien ofertó recupera su dinero si nadie aceptó. Devuelve el monto.
    pub fn retirar_oferta(env: Env, boleto_id: u32, comprador: Address) -> Result<i128, Error> {
        comprador.require_auth();
        let clave = Clave::Oferta(boleto_id, comprador.clone());
        let precio: i128 = env
            .storage()
            .persistent()
            .get(&clave)
            .ok_or(Error::NoHayOferta)?;
        env.storage().persistent().remove(&clave);
        token::Client::new(&env, &token_de_pago(&env)).transfer(
            &env.current_contract_address(),
            &comprador,
            &precio,
        );
        OfertaRetirada {
            boleto: boleto_id,
            comprador,
        }
        .publish(&env);
        Ok(precio)
    }

    /// Monto que `comprador` tiene ofertado por un boleto.
    pub fn consultar_oferta(env: Env, boleto_id: u32, comprador: Address) -> Result<i128, Error> {
        env.storage()
            .persistent()
            .get(&Clave::Oferta(boleto_id, comprador))
            .ok_or(Error::NoHayOferta)
    }

    // ---------- Reventa ----------

    /// El dueño ofrece su boleto a un precio que no puede pasar del tope del
    /// evento. Publicarlo otra vez cambia el precio.
    pub fn publicar_reventa(
        env: Env,
        boleto_id: u32,
        vendedor: Address,
        precio: i128,
    ) -> Result<(), Error> {
        vendedor.require_auth();
        let boleto = leer_boleto(&env, boleto_id)?;
        if boleto.dueno != vendedor {
            return Err(Error::NoEsDueno);
        }
        if boleto.usado {
            return Err(Error::YaUsado);
        }
        if boleto.reembolsado {
            return Err(Error::YaReembolsado);
        }
        if precio < 0 {
            return Err(Error::PrecioInvalido);
        }
        let evento = leer_evento(&env, boleto.evento)?;
        if evento.cancelado {
            return Err(Error::EventoCancelado);
        }
        if precio > precio_maximo(&evento) {
            return Err(Error::SobreTope);
        }

        guardar(&env, &Clave::Reventa(boleto_id), &precio);
        ReventaPublicada {
            boleto: boleto_id,
            vendedor,
            precio,
        }
        .publish(&env);
        Ok(())
    }

    /// El dueño retira su boleto de la reventa.
    pub fn cancelar_reventa(env: Env, boleto_id: u32, vendedor: Address) -> Result<(), Error> {
        vendedor.require_auth();
        let boleto = leer_boleto(&env, boleto_id)?;
        if boleto.dueno != vendedor {
            return Err(Error::NoEsDueno);
        }
        if !env.storage().persistent().has(&Clave::Reventa(boleto_id)) {
            return Err(Error::NoEnReventa);
        }
        retirar_reventa(&env, boleto_id);
        Ok(())
    }

    /// Compra un boleto en reventa: paga al vendedor, separa la comisión de
    /// TrustTix y cambia el dueño, todo en la misma transacción.
    pub fn comprar_reventa(env: Env, boleto_id: u32, comprador: Address) -> Result<(), Error> {
        comprador.require_auth();
        let mut boleto = leer_boleto(&env, boleto_id)?;
        let precio: i128 = env
            .storage()
            .persistent()
            .get(&Clave::Reventa(boleto_id))
            .ok_or(Error::NoEnReventa)?;
        if boleto.usado {
            return Err(Error::YaUsado);
        }
        let evento = leer_evento(&env, boleto.evento)?;
        if evento.cancelado {
            return Err(Error::EventoCancelado);
        }
        // Si el organizador bajó el tope después de publicarse, la oferta ya no vale.
        if precio > precio_maximo(&evento) {
            return Err(Error::SobreTope);
        }
        let vendedor = boleto.dueno.clone();
        if vendedor == comprador {
            return Err(Error::MismoDueno);
        }

        let comision = cobrar(
            &env,
            &comprador,
            &vendedor,
            precio,
            comisiones(&env).reventa,
        );

        boleto.dueno = comprador.clone();
        guardar(&env, &Clave::Boleto(boleto_id), &boleto);
        quitar_de(&env, &vendedor, boleto_id);
        agregar_a(&env, &comprador, boleto_id);
        env.storage()
            .persistent()
            .remove(&Clave::Reventa(boleto_id));

        Reventa {
            boleto: boleto_id,
            vendedor,
            comprador,
            precio,
            comision,
        }
        .publish(&env);
        Ok(())
    }

    // ---------- Personal de acceso ----------

    /// Marca el boleto como usado. Un segundo intento se rechaza.
    pub fn registrar_entrada(env: Env, boleto_id: u32, personal: Address) -> Result<(), Error> {
        personal.require_auth();
        let mut boleto = leer_boleto(&env, boleto_id)?;
        let evento = leer_evento(&env, boleto.evento)?;
        let autorizado = personal == evento.organizador
            || env
                .storage()
                .persistent()
                .get::<_, bool>(&Clave::Personal(boleto.evento, personal.clone()))
                .unwrap_or(false);
        if !autorizado {
            return Err(Error::NoAutorizado);
        }
        if evento.cancelado {
            return Err(Error::EventoCancelado);
        }
        if boleto.usado {
            return Err(Error::YaUsado);
        }
        if boleto.reembolsado {
            return Err(Error::YaReembolsado);
        }

        boleto.usado = true;
        boleto.usado_en = env.ledger().timestamp();
        guardar(&env, &Clave::Boleto(boleto_id), &boleto);
        // Un boleto que ya entró no se puede seguir vendiendo.
        retirar_reventa(&env, boleto_id);

        Entrada {
            boleto: boleto_id,
            evento: boleto.evento,
            personal,
        }
        .publish(&env);
        Ok(())
    }

    // ---------- Consultas públicas (no requieren firma) ----------

    pub fn consultar_evento(env: Env, evento_id: u32) -> Result<Evento, Error> {
        leer_evento(&env, evento_id)
    }

    pub fn consultar_boleto(env: Env, boleto_id: u32) -> Result<Boleto, Error> {
        leer_boleto(&env, boleto_id)
    }

    /// Precio al que está en reventa un boleto.
    pub fn consultar_reventa(env: Env, boleto_id: u32) -> Result<i128, Error> {
        env.storage()
            .persistent()
            .get(&Clave::Reventa(boleto_id))
            .ok_or(Error::NoEnReventa)
    }

    pub fn total_eventos(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&Clave::TotalEventos)
            .unwrap_or(0)
    }

    pub fn total_boletos(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&Clave::TotalBoletos)
            .unwrap_or(0)
    }

    pub fn boletos_de(env: Env, cuenta: Address) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&Clave::BoletosDe(cuenta))
            .unwrap_or(Vec::new(&env))
    }

    pub fn es_personal(env: Env, evento_id: u32, cuenta: Address) -> bool {
        match leer_evento(&env, evento_id) {
            Ok(evento) if evento.organizador == cuenta => true,
            Ok(_) => env
                .storage()
                .persistent()
                .get(&Clave::Personal(evento_id, cuenta))
                .unwrap_or(false),
            Err(_) => false,
        }
    }

    pub fn token(env: Env) -> Address {
        token_de_pago(&env)
    }

    pub fn admin(env: Env) -> Address {
        admin(&env)
    }

    pub fn comisiones(env: Env) -> Comisiones {
        comisiones(&env)
    }
}

// ---------- Funciones internas ----------

fn validar_comisiones(env: &Env, compra: u32, reventa: u32) -> Comisiones {
    if compra > COMISION_MAXIMA || reventa > COMISION_MAXIMA {
        panic_with_error!(env, Error::ComisionInvalida);
    }
    Comisiones { compra, reventa }
}

fn admin(env: &Env) -> Address {
    env.storage().instance().get(&Clave::Admin).unwrap()
}

fn comisiones(env: &Env) -> Comisiones {
    env.storage().instance().get(&Clave::Comisiones).unwrap()
}

fn token_de_pago(env: &Env) -> Address {
    env.storage().instance().get(&Clave::Token).unwrap()
}

/// Cobra `precio` a `de`: la comisión (en puntos base) va al administrador y
/// el resto a `para`. Devuelve la comisión cobrada. Con precio 0 no mueve nada.
fn cobrar(env: &Env, de: &Address, para: &Address, precio: i128, puntos: u32) -> i128 {
    if precio <= 0 {
        return 0;
    }
    let token = token::Client::new(env, &token_de_pago(env));
    let comision = precio * puntos as i128 / PUNTOS_BASE;
    if comision > 0 {
        token.transfer(de, admin(env), &comision);
    }
    token.transfer(de, para, &(precio - comision));
    comision
}

/// Revisa que `comprador` pueda pagar `cantidad` boletos más del evento: que no
/// esté cancelado, que queden lugares y que no pase del límite por cuenta.
/// Deja anotadas sus compras y devuelve el evento para emitir los boletos.
fn preparar_compra(
    env: &Env,
    comprador: &Address,
    evento_id: u32,
    cantidad: u32,
) -> Result<Evento, Error> {
    let evento = leer_evento(env, evento_id)?;
    if evento.cancelado {
        return Err(Error::EventoCancelado);
    }
    if evento.vendidos + cantidad > evento.capacidad {
        return Err(Error::Agotado);
    }
    let clave = Clave::Compras(evento_id, comprador.clone());
    let previas: u32 = env.storage().persistent().get(&clave).unwrap_or(0);
    if previas + cantidad > LIMITE_POR_CUENTA {
        return Err(Error::LimitePorCuenta);
    }
    guardar(env, &clave, &(previas + cantidad));
    Ok(evento)
}

/// Emite un boleto del evento a nombre de `dueno` y suma un vendido.
/// Quien llama guarda el evento al terminar.
fn emitir(env: &Env, evento_id: u32, evento: &mut Evento, dueno: &Address) -> u32 {
    let id = siguiente(env, Clave::TotalBoletos);
    let boleto = Boleto {
        evento: evento_id,
        dueno: dueno.clone(),
        usado: false,
        usado_en: 0,
        reembolsado: false,
    };
    guardar(env, &Clave::Boleto(id), &boleto);
    agregar_a(env, dueno, id);
    evento.vendidos += 1;
    id
}

/// El organizador devuelve el precio original del boleto a su dueño actual y
/// el boleto queda reembolsado y fuera de la reventa.
fn devolver_precio(env: &Env, evento: &Evento, boleto_id: u32, boleto: &mut Boleto) {
    if evento.precio > 0 {
        token::Client::new(env, &token_de_pago(env)).transfer(
            &evento.organizador,
            &boleto.dueno,
            &evento.precio,
        );
    }
    boleto.reembolsado = true;
    guardar(env, &Clave::Boleto(boleto_id), boleto);
    retirar_reventa(env, boleto_id);
    Reembolso {
        boleto: boleto_id,
        evento: boleto.evento,
        dueno: boleto.dueno.clone(),
        monto: evento.precio,
    }
    .publish(env);
}

/// Cambia el dueño de un boleto con las reglas de una transferencia: solo el
/// dueño actual, nunca un boleto usado o reembolsado, y nunca a sí mismo.
fn mover(env: &Env, boleto_id: u32, de: &Address, a: &Address) -> Result<(), Error> {
    let mut boleto = leer_boleto(env, boleto_id)?;
    if boleto.dueno != *de {
        return Err(Error::NoEsDueno);
    }
    if boleto.usado {
        return Err(Error::YaUsado);
    }
    if boleto.reembolsado {
        return Err(Error::YaReembolsado);
    }
    if de == a {
        return Err(Error::MismoDueno);
    }

    boleto.dueno = a.clone();
    guardar(env, &Clave::Boleto(boleto_id), &boleto);
    quitar_de(env, de, boleto_id);
    agregar_a(env, a, boleto_id);
    // Si estaba en reventa, la oferta era del dueño anterior: se retira.
    retirar_reventa(env, boleto_id);

    Transferencia {
        boleto: boleto_id,
        de: de.clone(),
        a: a.clone(),
    }
    .publish(env);
    Ok(())
}

/// Precio más alto al que se puede revender un boleto del evento.
fn precio_maximo(evento: &Evento) -> i128 {
    evento.precio * evento.tope_reventa as i128 / PUNTOS_BASE
}

fn siguiente(env: &Env, clave: Clave) -> u32 {
    let id: u32 = env.storage().instance().get(&clave).unwrap_or(0) + 1;
    env.storage().instance().set(&clave, &id);
    env.storage()
        .instance()
        .extend_ttl(UMBRAL_TTL, EXTENDER_TTL);
    id
}

fn guardar<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, clave: &Clave, valor: &V) {
    env.storage().persistent().set(clave, valor);
    env.storage()
        .persistent()
        .extend_ttl(clave, UMBRAL_TTL, EXTENDER_TTL);
}

fn leer_evento(env: &Env, id: u32) -> Result<Evento, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Evento(id))
        .ok_or(Error::EventoNoExiste)
}

fn leer_boleto(env: &Env, id: u32) -> Result<Boleto, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Boleto(id))
        .ok_or(Error::BoletoNoExiste)
}

/// Quita la oferta de reventa de un boleto, si la hay.
fn retirar_reventa(env: &Env, boleto_id: u32) {
    let clave = Clave::Reventa(boleto_id);
    if env.storage().persistent().has(&clave) {
        env.storage().persistent().remove(&clave);
        ReventaCancelada { boleto: boleto_id }.publish(env);
    }
}

fn agregar_a(env: &Env, cuenta: &Address, boleto_id: u32) {
    let clave = Clave::BoletosDe(cuenta.clone());
    let mut lista: Vec<u32> = env
        .storage()
        .persistent()
        .get(&clave)
        .unwrap_or(Vec::new(env));
    lista.push_back(boleto_id);
    guardar(env, &clave, &lista);
}

fn quitar_de(env: &Env, cuenta: &Address, boleto_id: u32) {
    let clave = Clave::BoletosDe(cuenta.clone());
    let lista: Vec<u32> = env
        .storage()
        .persistent()
        .get(&clave)
        .unwrap_or(Vec::new(env));
    if let Some(i) = lista.first_index_of(boleto_id) {
        let mut lista = lista;
        lista.remove(i);
        guardar(env, &clave, &lista);
    }
}

mod test;
