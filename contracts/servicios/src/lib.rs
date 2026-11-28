#![no_std]

//! Servicios TrustTix: mercancía en preventa, estacionamiento por horas,
//! viaje compartido al evento y votación del setlist con XLM.
//!
//! Reglas que hace cumplir la red:
//! 1. El pago de un pedido queda en garantía hasta que quien compró confirma
//!    que lo recibió; antes de eso lo puede cancelar.
//! 2. En el estacionamiento se deja el máximo al entrar y al salir solo se
//!    cobran las horas usadas.
//! 3. Cada pasajero paga al conductor al confirmar su llegada; si el viaje
//!    se cancela, quien no ha llegado recupera su pago.
//! 4. Lo votado en el setlist es para el artista con más XLM.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env,
    Symbol, Vec,
};

const DIA: u32 = 17_280;
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;
const HORA: u64 = 60 * 60;
const GRUPO_MAXIMO: u32 = 10;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NoExiste = 1,
    MontoInvalido = 2,
    Agotado = 3,
    NoPendiente = 4,
    YaDentro = 5,
    NoDentro = 6,
    SinAsiento = 7,
    YaPagado = 8,
    Cerrado = 9,
    OpcionInvalida = 10,
    SinVotos = 11,
    CantidadInvalida = 12,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Producto {
    pub vendedor: Address,
    pub precio: i128,
    pub existencias: u32,
}

/// Estado de un pedido: 0 pendiente, 1 entregado, 2 cancelado.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pedido {
    pub comprador: Address,
    pub producto: u32,
    pub monto: i128,
    pub estado: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Estacionamiento {
    pub operador: Address,
    pub tarifa_hora: i128,
    pub maximo_horas: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Estancia {
    pub entrada: u64,
    pub deposito: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Viaje {
    pub conductor: Address,
    pub precio: i128,
    pub asientos: u32,
    pub pasajeros: Vec<Address>,
    pub cancelado: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Votacion {
    pub organizador: Address,
    pub artistas: Vec<Address>,
    pub votos: Vec<i128>,
    pub cerrada: bool,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Token,
    Total(Symbol),
    Producto(u32),
    Pedido(u32),
    Estacionamiento(u32),
    Estancia(u32, Address),
    Viaje(u32),
    Pagado(u32, Address),
    Votacion(u32),
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
pub struct Servicios;

#[contractimpl]
impl Servicios {
    pub fn __constructor(env: Env, token: Address) {
        env.storage().instance().set(&Clave::Token, &token);
    }

    // ---------- Mercancía en preventa ----------

    /// El vendedor publica un producto con su precio y existencias.
    pub fn crear_producto(
        env: Env,
        vendedor: Address,
        precio: i128,
        existencias: u32,
    ) -> Result<u32, Error> {
        vendedor.require_auth();
        if precio <= 0 || existencias == 0 {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "producto");
        guardar(
            &env,
            &Clave::Producto(id),
            &Producto {
                vendedor,
                precio,
                existencias,
            },
        );
        Ok(id)
    }

    /// Aparta una pieza pagando su precio; el dinero queda en garantía hasta
    /// la entrega. Devuelve el número de pedido.
    pub fn preordenar(env: Env, comprador: Address, producto_id: u32) -> Result<u32, Error> {
        comprador.require_auth();
        let mut producto = leer_producto(&env, producto_id)?;
        if producto.existencias == 0 {
            return Err(Error::Agotado);
        }
        token(&env).transfer(&comprador, env.current_contract_address(), &producto.precio);
        producto.existencias -= 1;
        guardar(&env, &Clave::Producto(producto_id), &producto);
        let id = siguiente(&env, "pedido");
        let pedido = Pedido {
            comprador: comprador.clone(),
            producto: producto_id,
            monto: producto.precio,
            estado: 0,
        };
        guardar(&env, &Clave::Pedido(id), &pedido);
        publicar(
            &env,
            "pedido",
            &comprador,
            &producto.vendedor,
            producto.precio,
        );
        Ok(id)
    }

    /// Quien compró confirma que recibió su pieza y el pago se libera al vendedor.
    pub fn confirmar_entrega(env: Env, pedido_id: u32) -> Result<i128, Error> {
        let mut pedido = pedido_pendiente(&env, pedido_id)?;
        pedido.comprador.require_auth();
        let producto = leer_producto(&env, pedido.producto)?;
        enviar(&env, &producto.vendedor, pedido.monto);
        pedido.estado = 1;
        guardar(&env, &Clave::Pedido(pedido_id), &pedido);
        publicar(
            &env,
            "entrega",
            &pedido.comprador,
            &producto.vendedor,
            pedido.monto,
        );
        Ok(pedido.monto)
    }

    /// Antes de la entrega, quien compró cancela, recupera su dinero y la
    /// pieza vuelve a estar disponible.
    pub fn cancelar_pedido(env: Env, pedido_id: u32) -> Result<i128, Error> {
        let mut pedido = pedido_pendiente(&env, pedido_id)?;
        pedido.comprador.require_auth();
        let mut producto = leer_producto(&env, pedido.producto)?;
        enviar(&env, &pedido.comprador, pedido.monto);
        producto.existencias += 1;
        guardar(&env, &Clave::Producto(pedido.producto), &producto);
        pedido.estado = 2;
        guardar(&env, &Clave::Pedido(pedido_id), &pedido);
        publicar(
            &env,
            "devolucion",
            &env.current_contract_address(),
            &pedido.comprador,
            pedido.monto,
        );
        Ok(pedido.monto)
    }

    // ---------- Estacionamiento ----------

    /// El operador abre un estacionamiento con tarifa por hora y un máximo
    /// de horas.
    pub fn crear_estacionamiento(
        env: Env,
        operador: Address,
        tarifa_hora: i128,
        maximo_horas: u32,
    ) -> Result<u32, Error> {
        operador.require_auth();
        if tarifa_hora <= 0 || maximo_horas == 0 {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "estacionamiento");
        let est = Estacionamiento {
            operador,
            tarifa_hora,
            maximo_horas,
        };
        guardar(&env, &Clave::Estacionamiento(id), &est);
        Ok(id)
    }

    /// Entra al estacionamiento dejando en garantía el cobro máximo.
    pub fn entrar_estacionamiento(env: Env, cuenta: Address, est_id: u32) -> Result<i128, Error> {
        cuenta.require_auth();
        let est = leer_estacionamiento(&env, est_id)?;
        let clave = Clave::Estancia(est_id, cuenta.clone());
        if env.storage().persistent().has(&clave) {
            return Err(Error::YaDentro);
        }
        let deposito = est.tarifa_hora * est.maximo_horas as i128;
        token(&env).transfer(&cuenta, env.current_contract_address(), &deposito);
        let estancia = Estancia {
            entrada: env.ledger().timestamp(),
            deposito,
        };
        guardar(&env, &clave, &estancia);
        publicar(&env, "entrada", &cuenta, &est.operador, deposito);
        Ok(deposito)
    }

    /// Sale del estacionamiento: se cobran las horas usadas (mínimo una) y
    /// el resto de la garantía regresa. Devuelve lo cobrado.
    pub fn salir_estacionamiento(env: Env, cuenta: Address, est_id: u32) -> Result<i128, Error> {
        cuenta.require_auth();
        let est = leer_estacionamiento(&env, est_id)?;
        let clave = Clave::Estancia(est_id, cuenta.clone());
        let estancia: Estancia = env
            .storage()
            .persistent()
            .get(&clave)
            .ok_or(Error::NoDentro)?;
        env.storage().persistent().remove(&clave);
        let segundos = env.ledger().timestamp().saturating_sub(estancia.entrada);
        let horas = segundos.div_ceil(HORA).clamp(1, est.maximo_horas as u64);
        let cobro = est.tarifa_hora * horas as i128;
        enviar(&env, &est.operador, cobro);
        if estancia.deposito > cobro {
            enviar(&env, &cuenta, estancia.deposito - cobro);
        }
        publicar(&env, "salida", &cuenta, &est.operador, cobro);
        Ok(cobro)
    }

    // ---------- Viaje compartido ----------

    /// Un conductor ofrece lugares en su auto para llegar al evento.
    pub fn ofrecer_viaje(
        env: Env,
        conductor: Address,
        precio: i128,
        asientos: u32,
    ) -> Result<u32, Error> {
        conductor.require_auth();
        if precio <= 0 || asientos == 0 || asientos > GRUPO_MAXIMO {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "viaje");
        let viaje = Viaje {
            conductor,
            precio,
            asientos,
            pasajeros: Vec::new(&env),
            cancelado: false,
        };
        guardar(&env, &Clave::Viaje(id), &viaje);
        Ok(id)
    }

    /// Aparta un asiento pagando el precio; el dinero queda en garantía.
    pub fn apartar_asiento(env: Env, pasajero: Address, viaje_id: u32) -> Result<u32, Error> {
        pasajero.require_auth();
        let mut viaje = viaje_abierto(&env, viaje_id)?;
        if viaje.pasajeros.len() >= viaje.asientos {
            return Err(Error::SinAsiento);
        }
        token(&env).transfer(&pasajero, env.current_contract_address(), &viaje.precio);
        viaje.pasajeros.push_back(pasajero.clone());
        guardar(&env, &Clave::Viaje(viaje_id), &viaje);
        publicar(&env, "asiento", &pasajero, &viaje.conductor, viaje.precio);
        Ok(viaje.asientos - viaje.pasajeros.len())
    }

    /// El pasajero confirma que llegó y su pago se libera al conductor.
    pub fn confirmar_llegada(env: Env, pasajero: Address, viaje_id: u32) -> Result<i128, Error> {
        pasajero.require_auth();
        let viaje = viaje_abierto(&env, viaje_id)?;
        if !viaje.pasajeros.contains(&pasajero) {
            return Err(Error::SinAsiento);
        }
        let clave = Clave::Pagado(viaje_id, pasajero.clone());
        if env.storage().persistent().has(&clave) {
            return Err(Error::YaPagado);
        }
        guardar(&env, &clave, &true);
        enviar(&env, &viaje.conductor, viaje.precio);
        publicar(&env, "llegada", &pasajero, &viaje.conductor, viaje.precio);
        Ok(viaje.precio)
    }

    /// El conductor cancela el viaje y quien no ha confirmado su llegada
    /// recupera su pago. Devuelve el total reembolsado.
    pub fn cancelar_viaje(env: Env, viaje_id: u32) -> Result<i128, Error> {
        let mut viaje = viaje_abierto(&env, viaje_id)?;
        viaje.conductor.require_auth();
        let mut total = 0;
        for pasajero in viaje.pasajeros.iter() {
            if !env
                .storage()
                .persistent()
                .has(&Clave::Pagado(viaje_id, pasajero.clone()))
            {
                enviar(&env, &pasajero, viaje.precio);
                total += viaje.precio;
            }
        }
        viaje.cancelado = true;
        guardar(&env, &Clave::Viaje(viaje_id), &viaje);
        Ok(total)
    }

    // ---------- Votación del setlist ----------

    /// El organizador abre una votación entre artistas o canciones (hasta 10).
    pub fn crear_votacion(
        env: Env,
        organizador: Address,
        artistas: Vec<Address>,
    ) -> Result<u32, Error> {
        organizador.require_auth();
        if artistas.len() < 2 || artistas.len() > GRUPO_MAXIMO {
            return Err(Error::CantidadInvalida);
        }
        let mut votos = Vec::new(&env);
        for _ in 0..artistas.len() {
            votos.push_back(0i128);
        }
        let id = siguiente(&env, "votacion");
        let votacion = Votacion {
            organizador,
            artistas,
            votos,
            cerrada: false,
        };
        guardar(&env, &Clave::Votacion(id), &votacion);
        Ok(id)
    }

    /// Vota por una opción poniendo XLM: cada XLM es un voto y queda en la
    /// bolsa del ganador. Devuelve los votos de esa opción.
    pub fn votar_con_xlm(
        env: Env,
        cuenta: Address,
        votacion_id: u32,
        opcion: u32,
        monto: i128,
    ) -> Result<i128, Error> {
        cuenta.require_auth();
        let mut votacion = votacion_abierta(&env, votacion_id)?;
        if opcion >= votacion.artistas.len() {
            return Err(Error::OpcionInvalida);
        }
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        token(&env).transfer(&cuenta, env.current_contract_address(), &monto);
        let nuevos = votacion.votos.get(opcion).unwrap() + monto;
        votacion.votos.set(opcion, nuevos);
        guardar(&env, &Clave::Votacion(votacion_id), &votacion);
        publicar(
            &env,
            "voto",
            &cuenta,
            &votacion.artistas.get(opcion).unwrap(),
            monto,
        );
        Ok(nuevos)
    }

    /// El organizador cierra la votación y toda la bolsa va al artista con
    /// más votos. Devuelve al ganador.
    pub fn premiar_ganador(env: Env, votacion_id: u32) -> Result<Address, Error> {
        let mut votacion = votacion_abierta(&env, votacion_id)?;
        votacion.organizador.require_auth();
        let mut mejor = 0;
        let mut bolsa = 0;
        for (i, votos) in votacion.votos.iter().enumerate() {
            bolsa += votos;
            if votos > votacion.votos.get(mejor).unwrap() {
                mejor = i as u32;
            }
        }
        if bolsa == 0 {
            return Err(Error::SinVotos);
        }
        let ganador = votacion.artistas.get(mejor).unwrap();
        enviar(&env, &ganador, bolsa);
        votacion.cerrada = true;
        guardar(&env, &Clave::Votacion(votacion_id), &votacion);
        publicar(&env, "premio", &votacion.organizador, &ganador, bolsa);
        Ok(ganador)
    }

    // ---------- Consultas ----------

    pub fn consultar_pedido(env: Env, pedido_id: u32) -> Result<Pedido, Error> {
        env.storage()
            .persistent()
            .get(&Clave::Pedido(pedido_id))
            .ok_or(Error::NoExiste)
    }

    pub fn consultar_votacion(env: Env, votacion_id: u32) -> Result<Votacion, Error> {
        env.storage()
            .persistent()
            .get(&Clave::Votacion(votacion_id))
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

fn leer_producto(env: &Env, id: u32) -> Result<Producto, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Producto(id))
        .ok_or(Error::NoExiste)
}

fn pedido_pendiente(env: &Env, id: u32) -> Result<Pedido, Error> {
    let pedido: Pedido = env
        .storage()
        .persistent()
        .get(&Clave::Pedido(id))
        .ok_or(Error::NoExiste)?;
    if pedido.estado != 0 {
        return Err(Error::NoPendiente);
    }
    Ok(pedido)
}

fn leer_estacionamiento(env: &Env, id: u32) -> Result<Estacionamiento, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Estacionamiento(id))
        .ok_or(Error::NoExiste)
}

fn viaje_abierto(env: &Env, id: u32) -> Result<Viaje, Error> {
    let viaje: Viaje = env
        .storage()
        .persistent()
        .get(&Clave::Viaje(id))
        .ok_or(Error::NoExiste)?;
    if viaje.cancelado {
        return Err(Error::Cerrado);
    }
    Ok(viaje)
}

fn votacion_abierta(env: &Env, id: u32) -> Result<Votacion, Error> {
    let votacion: Votacion = env
        .storage()
        .persistent()
        .get(&Clave::Votacion(id))
        .ok_or(Error::NoExiste)?;
    if votacion.cerrada {
        return Err(Error::Cerrado);
    }
    Ok(votacion)
}

mod test;
