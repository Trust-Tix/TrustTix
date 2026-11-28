#![no_std]

//! Subastas TrustTix: subastas de lugares VIP, rifas de meet and greet,
//! fondeo colectivo de eventos y garantía de asistencia para eventos gratis.
//!
//! Reglas que hace cumplir la red:
//! 1. En una subasta, la mejor puja queda en garantía y la anterior regresa
//!    completa a su dueño en la misma transacción.
//! 2. En una rifa, el ganador recibe la mitad de lo recaudado y el
//!    organizador el resto; si se cancela, cada número se reembolsa.
//! 3. Una campaña solo se cobra si llegó a su meta; si no, cada quien
//!    recupera su apoyo al cierre.
//! 4. La garantía de asistencia regresa a quien asistió y es del organizador
//!    si no se presentó.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env,
    Symbol, Vec,
};

const DIA: u32 = 17_280;
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;
/// Números que puede tener una rifa.
const NUMEROS_MAXIMOS: u32 = 100;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NoExiste = 1,
    Cerrada = 2,
    PujaBaja = 3,
    MontoInvalido = 4,
    SinCompraYa = 5,
    SinParticipantes = 6,
    RifaLlena = 7,
    CampanaVigente = 8,
    CampanaTerminada = 9,
    MetaNoAlcanzada = 10,
    MetaAlcanzada = 11,
    SinApoyo = 12,
    YaReservado = 13,
    SinReserva = 14,
    MismaCuenta = 15,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subasta {
    pub vendedor: Address,
    pub base: i128,
    /// Precio para ganarla de inmediato (0 si no hay).
    pub compra_ya: i128,
    pub puja: i128,
    pub ganador: Option<Address>,
    pub abierta: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rifa {
    pub organizador: Address,
    pub precio: i128,
    pub numeros: Vec<Address>,
    pub ganador: Option<Address>,
    pub abierta: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Campana {
    pub organizador: Address,
    pub meta: i128,
    pub recaudado: i128,
    /// Marca de tiempo en que termina la campaña.
    pub cierre: u64,
    pub cobrada: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventoGratis {
    pub organizador: Address,
    pub garantia: i128,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Token,
    Total(Symbol),
    Subasta(u32),
    Rifa(u32),
    Campana(u32),
    Apoyo(u32, Address),
    Evento(u32),
    Reserva(u32, Address),
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
pub struct Subastas;

#[contractimpl]
impl Subastas {
    pub fn __constructor(env: Env, token: Address) {
        env.storage().instance().set(&Clave::Token, &token);
    }

    // ---------- Subastas ----------

    /// Publica una subasta con precio base y, si se quiere, precio de
    /// compra inmediata (0 para no tenerlo).
    pub fn crear_subasta(
        env: Env,
        vendedor: Address,
        base: i128,
        compra_ya: i128,
    ) -> Result<u32, Error> {
        vendedor.require_auth();
        if base <= 0 || compra_ya < 0 || (compra_ya > 0 && compra_ya < base) {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "subasta");
        let subasta = Subasta {
            vendedor,
            base,
            compra_ya,
            puja: 0,
            ganador: None,
            abierta: true,
        };
        guardar(&env, &Clave::Subasta(id), &subasta);
        Ok(id)
    }

    /// Puja por encima de la base y de la mejor puja. El dinero queda en
    /// garantía y la puja anterior regresa a su dueño.
    pub fn pujar(env: Env, postor: Address, subasta_id: u32, monto: i128) -> Result<(), Error> {
        postor.require_auth();
        let mut subasta = subasta_abierta(&env, subasta_id)?;
        if postor == subasta.vendedor {
            return Err(Error::MismaCuenta);
        }
        if monto < subasta.base || monto <= subasta.puja {
            return Err(Error::PujaBaja);
        }
        token(&env).transfer(&postor, env.current_contract_address(), &monto);
        devolver_puja(&env, &subasta);
        subasta.puja = monto;
        subasta.ganador = Some(postor.clone());
        guardar(&env, &Clave::Subasta(subasta_id), &subasta);
        publicar(&env, "puja", &postor, &subasta.vendedor, monto);
        Ok(())
    }

    /// El vendedor cierra la subasta y cobra la mejor puja. Devuelve el monto.
    pub fn cerrar_subasta(env: Env, subasta_id: u32) -> Result<i128, Error> {
        let mut subasta = subasta_abierta(&env, subasta_id)?;
        subasta.vendedor.require_auth();
        if let Some(ganador) = subasta.ganador.clone() {
            enviar(&env, &subasta.vendedor, subasta.puja);
            publicar(&env, "venta", &ganador, &subasta.vendedor, subasta.puja);
        }
        subasta.abierta = false;
        guardar(&env, &Clave::Subasta(subasta_id), &subasta);
        Ok(subasta.puja)
    }

    /// El vendedor cancela la subasta y la mejor puja regresa a su dueño.
    pub fn cancelar_subasta(env: Env, subasta_id: u32) -> Result<i128, Error> {
        let mut subasta = subasta_abierta(&env, subasta_id)?;
        subasta.vendedor.require_auth();
        let devuelto = subasta.puja;
        devolver_puja(&env, &subasta);
        subasta.puja = 0;
        subasta.ganador = None;
        subasta.abierta = false;
        guardar(&env, &Clave::Subasta(subasta_id), &subasta);
        Ok(devuelto)
    }

    /// Gana la subasta de inmediato pagando el precio de compra al vendedor;
    /// la mejor puja regresa a su dueño.
    pub fn comprar_ya(env: Env, comprador: Address, subasta_id: u32) -> Result<i128, Error> {
        comprador.require_auth();
        let mut subasta = subasta_abierta(&env, subasta_id)?;
        if subasta.compra_ya == 0 {
            return Err(Error::SinCompraYa);
        }
        if comprador == subasta.vendedor {
            return Err(Error::MismaCuenta);
        }
        token(&env).transfer(&comprador, &subasta.vendedor, &subasta.compra_ya);
        devolver_puja(&env, &subasta);
        subasta.puja = subasta.compra_ya;
        subasta.ganador = Some(comprador.clone());
        subasta.abierta = false;
        guardar(&env, &Clave::Subasta(subasta_id), &subasta);
        publicar(
            &env,
            "venta",
            &comprador,
            &subasta.vendedor,
            subasta.compra_ya,
        );
        Ok(subasta.compra_ya)
    }

    // ---------- Rifas ----------

    /// El organizador abre una rifa con el precio de cada número.
    pub fn crear_rifa(env: Env, organizador: Address, precio: i128) -> Result<u32, Error> {
        organizador.require_auth();
        if precio <= 0 {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "rifa");
        let rifa = Rifa {
            organizador,
            precio,
            numeros: Vec::new(&env),
            ganador: None,
            abierta: true,
        };
        guardar(&env, &Clave::Rifa(id), &rifa);
        Ok(id)
    }

    /// Compra un número de la rifa. Devuelve el número (empieza en 0).
    pub fn comprar_numero(env: Env, cuenta: Address, rifa_id: u32) -> Result<u32, Error> {
        cuenta.require_auth();
        let mut rifa = rifa_abierta(&env, rifa_id)?;
        if rifa.numeros.len() >= NUMEROS_MAXIMOS {
            return Err(Error::RifaLlena);
        }
        token(&env).transfer(&cuenta, env.current_contract_address(), &rifa.precio);
        rifa.numeros.push_back(cuenta.clone());
        guardar(&env, &Clave::Rifa(rifa_id), &rifa);
        publicar(&env, "numero", &cuenta, &rifa.organizador, rifa.precio);
        Ok(rifa.numeros.len() - 1)
    }

    /// El organizador sortea la rifa con el generador aleatorio de la red: el
    /// ganador recibe la mitad de lo recaudado y el organizador el resto.
    /// Devuelve al ganador.
    pub fn sortear_rifa(env: Env, rifa_id: u32) -> Result<Address, Error> {
        let mut rifa = rifa_abierta(&env, rifa_id)?;
        rifa.organizador.require_auth();
        let total = rifa.numeros.len();
        if total == 0 {
            return Err(Error::SinParticipantes);
        }
        let indice: u64 = env.prng().gen_range(0..total as u64);
        let ganador = rifa.numeros.get(indice as u32).unwrap();
        let bolsa = rifa.precio * total as i128;
        let premio = bolsa / 2;
        enviar(&env, &ganador, premio);
        enviar(&env, &rifa.organizador, bolsa - premio);
        publicar(&env, "premio", &rifa.organizador, &ganador, premio);
        rifa.ganador = Some(ganador.clone());
        rifa.abierta = false;
        guardar(&env, &Clave::Rifa(rifa_id), &rifa);
        Ok(ganador)
    }

    /// El organizador cancela la rifa y cada número se reembolsa. Devuelve
    /// el total reembolsado.
    pub fn cancelar_rifa(env: Env, rifa_id: u32) -> Result<i128, Error> {
        let mut rifa = rifa_abierta(&env, rifa_id)?;
        rifa.organizador.require_auth();
        for cuenta in rifa.numeros.iter() {
            enviar(&env, &cuenta, rifa.precio);
        }
        let total = rifa.precio * rifa.numeros.len() as i128;
        rifa.abierta = false;
        guardar(&env, &Clave::Rifa(rifa_id), &rifa);
        Ok(total)
    }

    // ---------- Fondeo colectivo ----------

    /// El organizador abre una campaña para fondear su evento: meta y
    /// duración en segundos.
    pub fn crear_campana(
        env: Env,
        organizador: Address,
        meta: i128,
        duracion: u64,
    ) -> Result<u32, Error> {
        organizador.require_auth();
        if meta <= 0 || duracion == 0 {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "campana");
        let campana = Campana {
            organizador,
            meta,
            recaudado: 0,
            cierre: env.ledger().timestamp() + duracion,
            cobrada: false,
        };
        guardar(&env, &Clave::Campana(id), &campana);
        Ok(id)
    }

    /// Apoya una campaña vigente; el dinero queda en garantía.
    pub fn apoyar_campana(
        env: Env,
        cuenta: Address,
        campana_id: u32,
        monto: i128,
    ) -> Result<i128, Error> {
        cuenta.require_auth();
        let mut campana = leer_campana(&env, campana_id)?;
        if env.ledger().timestamp() > campana.cierre {
            return Err(Error::CampanaTerminada);
        }
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        token(&env).transfer(&cuenta, env.current_contract_address(), &monto);
        let clave = Clave::Apoyo(campana_id, cuenta.clone());
        let previo: i128 = env.storage().persistent().get(&clave).unwrap_or(0);
        guardar(&env, &clave, &(previo + monto));
        campana.recaudado += monto;
        guardar(&env, &Clave::Campana(campana_id), &campana);
        publicar(&env, "apoyo", &cuenta, &campana.organizador, monto);
        Ok(campana.recaudado)
    }

    /// Si la campaña llegó a su meta, el organizador cobra lo recaudado.
    pub fn cobrar_campana(env: Env, campana_id: u32) -> Result<i128, Error> {
        let mut campana = leer_campana(&env, campana_id)?;
        campana.organizador.require_auth();
        if campana.cobrada {
            return Err(Error::Cerrada);
        }
        if campana.recaudado < campana.meta {
            return Err(Error::MetaNoAlcanzada);
        }
        enviar(&env, &campana.organizador, campana.recaudado);
        campana.cobrada = true;
        guardar(&env, &Clave::Campana(campana_id), &campana);
        Ok(campana.recaudado)
    }

    /// Si la campaña terminó sin llegar a su meta, quien apoyó recupera su
    /// dinero.
    pub fn recuperar_apoyo(env: Env, cuenta: Address, campana_id: u32) -> Result<i128, Error> {
        cuenta.require_auth();
        let mut campana = leer_campana(&env, campana_id)?;
        if env.ledger().timestamp() <= campana.cierre {
            return Err(Error::CampanaVigente);
        }
        if campana.recaudado >= campana.meta {
            return Err(Error::MetaAlcanzada);
        }
        let clave = Clave::Apoyo(campana_id, cuenta.clone());
        let apoyo: i128 = env
            .storage()
            .persistent()
            .get(&clave)
            .ok_or(Error::SinApoyo)?;
        env.storage().persistent().remove(&clave);
        enviar(&env, &cuenta, apoyo);
        campana.recaudado -= apoyo;
        guardar(&env, &Clave::Campana(campana_id), &campana);
        publicar(
            &env,
            "devolucion",
            &env.current_contract_address(),
            &cuenta,
            apoyo,
        );
        Ok(apoyo)
    }

    // ---------- Garantía de asistencia ----------

    /// El organizador de un evento gratis fija la garantía para reservar.
    pub fn crear_evento_gratis(
        env: Env,
        organizador: Address,
        garantia: i128,
    ) -> Result<u32, Error> {
        organizador.require_auth();
        if garantia <= 0 {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "evento");
        guardar(
            &env,
            &Clave::Evento(id),
            &EventoGratis {
                organizador,
                garantia,
            },
        );
        Ok(id)
    }

    /// Reserva un lugar en un evento gratis dejando la garantía en el contrato.
    pub fn reservar_con_garantia(env: Env, cuenta: Address, evento_id: u32) -> Result<i128, Error> {
        cuenta.require_auth();
        let evento = leer_evento(&env, evento_id)?;
        let clave = Clave::Reserva(evento_id, cuenta.clone());
        if env.storage().persistent().has(&clave) {
            return Err(Error::YaReservado);
        }
        token(&env).transfer(&cuenta, env.current_contract_address(), &evento.garantia);
        guardar(&env, &clave, &evento.garantia);
        publicar(
            &env,
            "garantia",
            &cuenta,
            &evento.organizador,
            evento.garantia,
        );
        Ok(evento.garantia)
    }

    /// El organizador confirma que la persona asistió y su garantía regresa.
    pub fn confirmar_asistencia(env: Env, evento_id: u32, cuenta: Address) -> Result<i128, Error> {
        let evento = leer_evento(&env, evento_id)?;
        evento.organizador.require_auth();
        let garantia = quitar_reserva(&env, evento_id, &cuenta)?;
        enviar(&env, &cuenta, garantia);
        publicar(&env, "devolucion", &evento.organizador, &cuenta, garantia);
        Ok(garantia)
    }

    /// La persona no se presentó: su garantía es para el organizador.
    pub fn penalizar_inasistencia(
        env: Env,
        evento_id: u32,
        cuenta: Address,
    ) -> Result<i128, Error> {
        let evento = leer_evento(&env, evento_id)?;
        evento.organizador.require_auth();
        let garantia = quitar_reserva(&env, evento_id, &cuenta)?;
        enviar(&env, &evento.organizador, garantia);
        publicar(&env, "penalizacion", &cuenta, &evento.organizador, garantia);
        Ok(garantia)
    }

    // ---------- Consultas ----------

    pub fn consultar_subasta(env: Env, subasta_id: u32) -> Result<Subasta, Error> {
        env.storage()
            .persistent()
            .get(&Clave::Subasta(subasta_id))
            .ok_or(Error::NoExiste)
    }

    pub fn consultar_rifa(env: Env, rifa_id: u32) -> Result<Rifa, Error> {
        env.storage()
            .persistent()
            .get(&Clave::Rifa(rifa_id))
            .ok_or(Error::NoExiste)
    }

    pub fn consultar_campana(env: Env, campana_id: u32) -> Result<Campana, Error> {
        leer_campana(&env, campana_id)
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

fn subasta_abierta(env: &Env, id: u32) -> Result<Subasta, Error> {
    let subasta: Subasta = env
        .storage()
        .persistent()
        .get(&Clave::Subasta(id))
        .ok_or(Error::NoExiste)?;
    if !subasta.abierta {
        return Err(Error::Cerrada);
    }
    Ok(subasta)
}

/// Regresa la mejor puja actual a su dueño, si la hay.
fn devolver_puja(env: &Env, subasta: &Subasta) {
    if let Some(anterior) = subasta.ganador.clone() {
        if subasta.puja > 0 {
            enviar(env, &anterior, subasta.puja);
        }
    }
}

fn rifa_abierta(env: &Env, id: u32) -> Result<Rifa, Error> {
    let rifa: Rifa = env
        .storage()
        .persistent()
        .get(&Clave::Rifa(id))
        .ok_or(Error::NoExiste)?;
    if !rifa.abierta {
        return Err(Error::Cerrada);
    }
    Ok(rifa)
}

fn leer_campana(env: &Env, id: u32) -> Result<Campana, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Campana(id))
        .ok_or(Error::NoExiste)
}

fn leer_evento(env: &Env, id: u32) -> Result<EventoGratis, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Evento(id))
        .ok_or(Error::NoExiste)
}

fn quitar_reserva(env: &Env, evento_id: u32, cuenta: &Address) -> Result<i128, Error> {
    let clave = Clave::Reserva(evento_id, cuenta.clone());
    let garantia: i128 = env
        .storage()
        .persistent()
        .get(&clave)
        .ok_or(Error::SinReserva)?;
    env.storage().persistent().remove(&clave);
    Ok(garantia)
}

mod test;
