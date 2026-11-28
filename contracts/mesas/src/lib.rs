#![no_std]

//! Mesas TrustTix: mesas VIP con consumo mínimo, regalos programados y
//! compras con árbitro entre fans.
//!
//! Reglas que hace cumplir la red:
//! 1. Una mesa VIP se reserva pagando el consumo mínimo; el recinto carga lo
//!    consumido y al cerrar cobra lo que falte para el mínimo.
//! 2. Un regalo programado solo lo reclama quien lo recibe, a partir de su
//!    fecha; antes de esa fecha quien lo dio lo puede revocar.
//! 3. En una compra con árbitro el pago queda en garantía: el comprador lo
//!    libera al vendedor o el árbitro decide cómo repartirlo.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env, Symbol,
};

const DIA: u32 = 17_280;
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;
const PUNTOS_BASE: u32 = 10_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NoExiste = 1,
    MontoInvalido = 2,
    Cerrado = 3,
    SinSaldo = 4,
    MuyPronto = 5,
    YaDisponible = 6,
    MismaCuenta = 7,
    ParteInvalida = 8,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mesa {
    pub cliente: Address,
    pub recinto: Address,
    /// Consumo mínimo pagado al reservar.
    pub minimo: i128,
    /// Lo que ya se cargó a la mesa.
    pub consumido: i128,
    pub cerrada: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Regalo {
    pub de: Address,
    pub para: Address,
    pub monto: i128,
    /// Marca de tiempo desde la que se puede reclamar.
    pub fecha: u64,
    pub cerrado: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compra {
    pub comprador: Address,
    pub vendedor: Address,
    pub arbitro: Address,
    pub monto: i128,
    pub cerrada: bool,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Token,
    Total(Symbol),
    Mesa(u32),
    Regalo(u32),
    Compra(u32),
}

#[contractevent]
pub struct Pago {
    #[topic]
    pub tipo: Symbol,
    pub de: Address,
    pub para: Address,
    pub monto: i128,
}

#[contract]
pub struct Mesas;

#[contractimpl]
impl Mesas {
    pub fn __constructor(env: Env, token: Address) {
        env.storage().instance().set(&Clave::Token, &token);
    }

    // ---------- Mesas VIP ----------

    /// Reserva una mesa VIP pagando el consumo mínimo, que queda en
    /// garantía. Devuelve el número de mesa.
    pub fn reservar_mesa(
        env: Env,
        cliente: Address,
        recinto: Address,
        minimo: i128,
    ) -> Result<u32, Error> {
        cliente.require_auth();
        if minimo <= 0 {
            return Err(Error::MontoInvalido);
        }
        if cliente == recinto {
            return Err(Error::MismaCuenta);
        }
        token(&env).transfer(&cliente, env.current_contract_address(), &minimo);
        let id = siguiente(&env, "mesa");
        let mesa = Mesa {
            cliente: cliente.clone(),
            recinto: recinto.clone(),
            minimo,
            consumido: 0,
            cerrada: false,
        };
        guardar(&env, &Clave::Mesa(id), &mesa);
        publicar(&env, "mesa", &cliente, &recinto, minimo);
        Ok(id)
    }

    /// El recinto carga un consumo a la mesa y lo cobra de la garantía; no
    /// puede cargar más de lo que queda. Devuelve el consumo total.
    pub fn cargar_consumo(env: Env, mesa_id: u32, monto: i128) -> Result<i128, Error> {
        let mut mesa = mesa_abierta(&env, mesa_id)?;
        mesa.recinto.require_auth();
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        if monto > mesa.minimo - mesa.consumido {
            return Err(Error::SinSaldo);
        }
        enviar(&env, &mesa.recinto, monto);
        mesa.consumido += monto;
        guardar(&env, &Clave::Mesa(mesa_id), &mesa);
        publicar(&env, "consumo", &mesa.cliente, &mesa.recinto, monto);
        Ok(mesa.consumido)
    }

    /// El recinto cierra la mesa: lo que no se consumió del mínimo se queda
    /// el recinto, como se acordó al reservar. Devuelve ese monto.
    pub fn cerrar_mesa(env: Env, mesa_id: u32) -> Result<i128, Error> {
        let mut mesa = mesa_abierta(&env, mesa_id)?;
        mesa.recinto.require_auth();
        let resto = (mesa.minimo - mesa.consumido).max(0);
        if resto > 0 {
            enviar(&env, &mesa.recinto, resto);
        }
        mesa.cerrada = true;
        guardar(&env, &Clave::Mesa(mesa_id), &mesa);
        publicar(&env, "cierre", &mesa.cliente, &mesa.recinto, resto);
        Ok(resto)
    }

    // ---------- Regalos programados ----------

    /// Programa un regalo en XLM para otra persona, que podrá reclamarlo a
    /// partir de `fecha`. El dinero queda en el contrato.
    pub fn programar_regalo(
        env: Env,
        de: Address,
        para: Address,
        monto: i128,
        fecha: u64,
    ) -> Result<u32, Error> {
        de.require_auth();
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        if de == para {
            return Err(Error::MismaCuenta);
        }
        token(&env).transfer(&de, env.current_contract_address(), &monto);
        let id = siguiente(&env, "regalo");
        let regalo = Regalo {
            de: de.clone(),
            para: para.clone(),
            monto,
            fecha,
            cerrado: false,
        };
        guardar(&env, &Clave::Regalo(id), &regalo);
        publicar(&env, "regalo", &de, &para, monto);
        Ok(id)
    }

    /// Quien recibe el regalo lo cobra a partir de su fecha.
    pub fn reclamar_regalo(env: Env, regalo_id: u32) -> Result<i128, Error> {
        let mut regalo = regalo_abierto(&env, regalo_id)?;
        regalo.para.require_auth();
        if env.ledger().timestamp() < regalo.fecha {
            return Err(Error::MuyPronto);
        }
        enviar(&env, &regalo.para, regalo.monto);
        regalo.cerrado = true;
        guardar(&env, &Clave::Regalo(regalo_id), &regalo);
        Ok(regalo.monto)
    }

    /// Antes de la fecha, quien dio el regalo lo revoca y recupera su dinero.
    pub fn revocar_regalo(env: Env, regalo_id: u32) -> Result<i128, Error> {
        let mut regalo = regalo_abierto(&env, regalo_id)?;
        regalo.de.require_auth();
        if env.ledger().timestamp() >= regalo.fecha {
            return Err(Error::YaDisponible);
        }
        enviar(&env, &regalo.de, regalo.monto);
        regalo.cerrado = true;
        guardar(&env, &Clave::Regalo(regalo_id), &regalo);
        publicar(&env, "revocado", &regalo.para, &regalo.de, regalo.monto);
        Ok(regalo.monto)
    }

    // ---------- Compras con árbitro ----------

    /// El comprador paga a un vendedor con un árbitro de confianza; el pago
    /// queda en garantía. Devuelve el número de compra.
    pub fn pagar_con_arbitro(
        env: Env,
        comprador: Address,
        vendedor: Address,
        arbitro: Address,
        monto: i128,
    ) -> Result<u32, Error> {
        comprador.require_auth();
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        if comprador == vendedor || arbitro == comprador || arbitro == vendedor {
            return Err(Error::MismaCuenta);
        }
        token(&env).transfer(&comprador, env.current_contract_address(), &monto);
        let id = siguiente(&env, "compra");
        let compra = Compra {
            comprador: comprador.clone(),
            vendedor: vendedor.clone(),
            arbitro,
            monto,
            cerrada: false,
        };
        guardar(&env, &Clave::Compra(id), &compra);
        publicar(&env, "garantia", &comprador, &vendedor, monto);
        Ok(id)
    }

    /// El comprador recibió lo acordado y acepta: el pago va al vendedor.
    pub fn aceptar_compra(env: Env, compra_id: u32) -> Result<i128, Error> {
        let mut compra = compra_abierta(&env, compra_id)?;
        compra.comprador.require_auth();
        enviar(&env, &compra.vendedor, compra.monto);
        compra.cerrada = true;
        guardar(&env, &Clave::Compra(compra_id), &compra);
        publicar(
            &env,
            "venta",
            &compra.comprador,
            &compra.vendedor,
            compra.monto,
        );
        Ok(compra.monto)
    }

    /// Si hay desacuerdo, el árbitro decide qué parte (en puntos base) va al
    /// vendedor; el resto regresa al comprador. Devuelve lo del vendedor.
    pub fn resolver_disputa(env: Env, compra_id: u32, parte_vendedor: u32) -> Result<i128, Error> {
        let mut compra = compra_abierta(&env, compra_id)?;
        compra.arbitro.require_auth();
        if parte_vendedor > PUNTOS_BASE {
            return Err(Error::ParteInvalida);
        }
        let al_vendedor = compra.monto * parte_vendedor as i128 / PUNTOS_BASE as i128;
        if al_vendedor > 0 {
            enviar(&env, &compra.vendedor, al_vendedor);
        }
        if compra.monto > al_vendedor {
            enviar(&env, &compra.comprador, compra.monto - al_vendedor);
        }
        compra.cerrada = true;
        guardar(&env, &Clave::Compra(compra_id), &compra);
        publicar(
            &env,
            "arbitraje",
            &compra.arbitro,
            &compra.vendedor,
            al_vendedor,
        );
        Ok(al_vendedor)
    }

    // ---------- Consultas ----------

    pub fn consultar_mesa(env: Env, mesa_id: u32) -> Result<Mesa, Error> {
        env.storage()
            .persistent()
            .get(&Clave::Mesa(mesa_id))
            .ok_or(Error::NoExiste)
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
    Pago {
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

fn mesa_abierta(env: &Env, id: u32) -> Result<Mesa, Error> {
    let mesa: Mesa = env
        .storage()
        .persistent()
        .get(&Clave::Mesa(id))
        .ok_or(Error::NoExiste)?;
    if mesa.cerrada {
        return Err(Error::Cerrado);
    }
    Ok(mesa)
}

fn regalo_abierto(env: &Env, id: u32) -> Result<Regalo, Error> {
    let regalo: Regalo = env
        .storage()
        .persistent()
        .get(&Clave::Regalo(id))
        .ok_or(Error::NoExiste)?;
    if regalo.cerrado {
        return Err(Error::Cerrado);
    }
    Ok(regalo)
}

fn compra_abierta(env: &Env, id: u32) -> Result<Compra, Error> {
    let compra: Compra = env
        .storage()
        .persistent()
        .get(&Clave::Compra(id))
        .ok_or(Error::NoExiste)?;
    if compra.cerrada {
        return Err(Error::Cerrado);
    }
    Ok(compra)
}

mod test;
