#![no_std]

//! Pagos TrustTix: suscripciones con cargo automático, pago al personal que
//! se libera con el tiempo y cashback de los comercios.
//!
//! Reglas que hace cumplir la red:
//! 1. Una suscripción cobra el primer periodo al suscribirse y deja una
//!    autorización del token para los siguientes; el comercio solo cobra
//!    cuando ya venció el periodo, y nunca más periodos de los acordados.
//! 2. El pago al personal se libera de forma lineal en el tiempo: cada
//!    quien cobra solo lo que ya devengó y, si se cancela, recibe eso y el
//!    resto regresa al organizador.
//! 3. El cashback sale de la reserva que fondeó el comercio, nunca de otros.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env, Symbol,
};

const DIA: u32 = 17_280;
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;
/// Cashback: 5 % de cada pago, en puntos base.
const CASHBACK: i128 = 500;
const PUNTOS_BASE: i128 = 10_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NoExiste = 1,
    MontoInvalido = 2,
    MuyPronto = 3,
    SinPeriodos = 4,
    Cerrado = 5,
    NadaQueCobrar = 6,
    SinReserva = 7,
    MismaCuenta = 8,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suscripcion {
    pub cliente: Address,
    pub comercio: Address,
    pub monto: i128,
    /// Duración de cada periodo, en segundos.
    pub periodo: u64,
    pub proximo_cobro: u64,
    pub restantes: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagoProgramado {
    pub organizador: Address,
    pub persona: Address,
    pub total: i128,
    pub cobrado: i128,
    pub inicio: u64,
    pub duracion: u64,
    pub cancelado: bool,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Token,
    Total(Symbol),
    Suscripcion(u32),
    Pago(u32),
    Reserva(Address),
}

#[contractevent]
pub struct Movimiento {
    #[topic]
    pub tipo: Symbol,
    pub de: Address,
    pub para: Address,
    pub monto: i128,
}

#[contract]
pub struct Pagos;

#[contractimpl]
impl Pagos {
    pub fn __constructor(env: Env, token: Address) {
        env.storage().instance().set(&Clave::Token, &token);
    }

    // ---------- Suscripciones ----------

    /// Se suscribe a un comercio (por ejemplo, el pase mensual de un club):
    /// paga el primer periodo y autoriza al contrato a cobrar los demás
    /// hasta el ledger `vence` (el cliente lo fija, así su firma no cambia).
    /// Devuelve el número de suscripción.
    pub fn suscribirse(
        env: Env,
        cliente: Address,
        comercio: Address,
        monto: i128,
        periodo: u64,
        periodos: u32,
        vence: u32,
    ) -> Result<u32, Error> {
        cliente.require_auth();
        if monto <= 0 || periodo == 0 || periodos < 2 || vence <= env.ledger().sequence() {
            return Err(Error::MontoInvalido);
        }
        if cliente == comercio {
            return Err(Error::MismaCuenta);
        }
        let xlm = token(&env);
        xlm.transfer(&cliente, &comercio, &monto);
        xlm.approve(
            &cliente,
            &env.current_contract_address(),
            &(monto * (periodos - 1) as i128),
            &vence,
        );
        let id = siguiente(&env, "suscripcion");
        let suscripcion = Suscripcion {
            cliente: cliente.clone(),
            comercio: comercio.clone(),
            monto,
            periodo,
            proximo_cobro: env.ledger().timestamp() + periodo,
            restantes: periodos - 1,
        };
        guardar(&env, &Clave::Suscripcion(id), &suscripcion);
        publicar(&env, "suscripcion", &cliente, &comercio, monto);
        Ok(id)
    }

    /// El comercio cobra el siguiente periodo, solo si ya venció, con la
    /// autorización que dejó el cliente. Devuelve los periodos que quedan.
    pub fn cobrar_suscripcion(env: Env, suscripcion_id: u32) -> Result<u32, Error> {
        let mut suscripcion: Suscripcion = env
            .storage()
            .persistent()
            .get(&Clave::Suscripcion(suscripcion_id))
            .ok_or(Error::NoExiste)?;
        suscripcion.comercio.require_auth();
        if suscripcion.restantes == 0 {
            return Err(Error::SinPeriodos);
        }
        if env.ledger().timestamp() < suscripcion.proximo_cobro {
            return Err(Error::MuyPronto);
        }
        token(&env).transfer_from(
            &env.current_contract_address(),
            &suscripcion.cliente,
            &suscripcion.comercio,
            &suscripcion.monto,
        );
        suscripcion.restantes -= 1;
        suscripcion.proximo_cobro += suscripcion.periodo;
        guardar(&env, &Clave::Suscripcion(suscripcion_id), &suscripcion);
        publicar(
            &env,
            "cargo",
            &suscripcion.cliente,
            &suscripcion.comercio,
            suscripcion.monto,
        );
        Ok(suscripcion.restantes)
    }

    // ---------- Pago al personal que se libera con el tiempo ----------

    /// El organizador deposita el pago de una persona del staff, que se
    /// libera de forma lineal durante `duracion` segundos.
    pub fn crear_pago_programado(
        env: Env,
        organizador: Address,
        persona: Address,
        total: i128,
        duracion: u64,
    ) -> Result<u32, Error> {
        organizador.require_auth();
        if total <= 0 || duracion == 0 {
            return Err(Error::MontoInvalido);
        }
        if organizador == persona {
            return Err(Error::MismaCuenta);
        }
        token(&env).transfer(&organizador, env.current_contract_address(), &total);
        let id = siguiente(&env, "pago");
        let pago = PagoProgramado {
            organizador: organizador.clone(),
            persona: persona.clone(),
            total,
            cobrado: 0,
            inicio: env.ledger().timestamp(),
            duracion,
            cancelado: false,
        };
        guardar(&env, &Clave::Pago(id), &pago);
        publicar(&env, "programado", &organizador, &persona, total);
        Ok(id)
    }

    /// La persona cobra lo que ya se liberó y aún no ha cobrado.
    pub fn cobrar_devengado(env: Env, pago_id: u32) -> Result<i128, Error> {
        let mut pago = pago_activo(&env, pago_id)?;
        pago.persona.require_auth();
        let monto = devengado(&env, &pago) - pago.cobrado;
        if monto <= 0 {
            return Err(Error::NadaQueCobrar);
        }
        enviar(&env, &pago.persona, monto);
        pago.cobrado += monto;
        guardar(&env, &Clave::Pago(pago_id), &pago);
        publicar(&env, "devengado", &pago.organizador, &pago.persona, monto);
        Ok(monto)
    }

    /// El organizador termina el pago: la persona recibe lo devengado que
    /// le falte y el resto regresa al organizador. Devuelve lo regresado.
    pub fn terminar_pago_programado(env: Env, pago_id: u32) -> Result<i128, Error> {
        let mut pago = pago_activo(&env, pago_id)?;
        pago.organizador.require_auth();
        let ganado = devengado(&env, &pago);
        let pendiente = ganado - pago.cobrado;
        if pendiente > 0 {
            enviar(&env, &pago.persona, pendiente);
        }
        let regreso = pago.total - ganado;
        if regreso > 0 {
            enviar(&env, &pago.organizador, regreso);
        }
        pago.cobrado = ganado;
        pago.cancelado = true;
        guardar(&env, &Clave::Pago(pago_id), &pago);
        publicar(
            &env,
            "terminado",
            &pago.organizador,
            &pago.persona,
            pendiente,
        );
        Ok(regreso)
    }

    // ---------- Cashback ----------

    /// El comercio deposita la reserva de la que sale el cashback de sus
    /// clientes. Devuelve la reserva total.
    pub fn fondear_cashback(env: Env, comercio: Address, monto: i128) -> Result<i128, Error> {
        comercio.require_auth();
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        token(&env).transfer(&comercio, env.current_contract_address(), &monto);
        let nueva = reserva(&env, &comercio) + monto;
        guardar(&env, &Clave::Reserva(comercio.clone()), &nueva);
        Ok(nueva)
    }

    /// El cliente paga al comercio y recibe el 5 % de regreso desde la
    /// reserva del comercio, en la misma transacción. Devuelve el cashback.
    pub fn pagar_con_cashback(
        env: Env,
        cliente: Address,
        comercio: Address,
        monto: i128,
    ) -> Result<i128, Error> {
        cliente.require_auth();
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        let regreso = monto * CASHBACK / PUNTOS_BASE;
        let disponible = reserva(&env, &comercio);
        if regreso > disponible {
            return Err(Error::SinReserva);
        }
        token(&env).transfer(&cliente, &comercio, &monto);
        if regreso > 0 {
            enviar(&env, &cliente, regreso);
        }
        guardar(
            &env,
            &Clave::Reserva(comercio.clone()),
            &(disponible - regreso),
        );
        publicar(&env, "cashback", &comercio, &cliente, regreso);
        Ok(regreso)
    }

    // ---------- Consultas ----------

    pub fn consultar_suscripcion(env: Env, suscripcion_id: u32) -> Result<Suscripcion, Error> {
        env.storage()
            .persistent()
            .get(&Clave::Suscripcion(suscripcion_id))
            .ok_or(Error::NoExiste)
    }

    pub fn reserva_de(env: Env, comercio: Address) -> i128 {
        reserva(&env, &comercio)
    }
}

// ---------- Funciones internas ----------

fn token(env: &Env) -> token::Client<'_> {
    let id: Address = env.storage().instance().get(&Clave::Token).unwrap();
    token::Client::new(env, &id)
}

fn guardar<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, clave: &Clave, valor: &V) {
    env.storage().persistent().set(clave, valor);
    env.storage()
        .persistent()
        .extend_ttl(clave, UMBRAL_TTL, EXTENDER_TTL);
}

fn siguiente(env: &Env, tipo: &str) -> u32 {
    let clave = Clave::Total(Symbol::new(env, tipo));
    let id: u32 = env.storage().instance().get(&clave).unwrap_or(0) + 1;
    env.storage().instance().set(&clave, &id);
    id
}

fn publicar(env: &Env, tipo: &str, de: &Address, para: &Address, monto: i128) {
    Movimiento {
        tipo: Symbol::new(env, tipo),
        de: de.clone(),
        para: para.clone(),
        monto,
    }
    .publish(env);
}

fn enviar(env: &Env, para: &Address, monto: i128) {
    token(env).transfer(&env.current_contract_address(), para, &monto);
}

fn pago_activo(env: &Env, id: u32) -> Result<PagoProgramado, Error> {
    let pago: PagoProgramado = env
        .storage()
        .persistent()
        .get(&Clave::Pago(id))
        .ok_or(Error::NoExiste)?;
    if pago.cancelado {
        return Err(Error::Cerrado);
    }
    Ok(pago)
}

/// Parte del total que ya se liberó según el tiempo transcurrido.
fn devengado(env: &Env, pago: &PagoProgramado) -> i128 {
    let pasado = env
        .ledger()
        .timestamp()
        .saturating_sub(pago.inicio)
        .min(pago.duracion);
    pago.total * pasado as i128 / pago.duracion as i128
}

fn reserva(env: &Env, comercio: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&Clave::Reserva(comercio.clone()))
        .unwrap_or(0)
}

mod test;
