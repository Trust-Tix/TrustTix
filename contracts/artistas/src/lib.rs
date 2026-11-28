#![no_std]

//! Artistas TrustTix: pagos a artistas, patrocinios y bolsas de premios de un
//! evento, con el dinero en garantía en el contrato hasta que se libera.
//!
//! Reglas que hace cumplir la red:
//! 1. El pago de un artista queda depositado antes del evento: el organizador
//!    no puede quedarse con él, solo liberarlo al artista o recuperarlo si el
//!    artista renuncia.
//! 2. Un patrocinio queda en garantía y solo quien patrocina lo libera al
//!    organizador; mientras no lo libere, lo puede retirar.
//! 3. Los repartos se calculan en puntos base y siempre suman el 100 %.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env,
    Symbol, Vec,
};

const DIA: u32 = 17_280;
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;
const PUNTOS_BASE: u32 = 10_000;
/// Cuentas que se pagan como máximo en una transacción.
const GRUPO_MAXIMO: u32 = 10;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NoExiste = 1,
    Cerrado = 2,
    MontoInvalido = 3,
    SinFondos = 4,
    CantidadInvalida = 5,
    PartesInvalidas = 6,
    MismaCuenta = 7,
}

/// Pago de un artista en garantía: lo que sigue depositado y lo ya pagado.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trato {
    pub organizador: Address,
    pub artista: Address,
    pub deposito: i128,
    pub pagado: i128,
    pub cerrado: bool,
}

/// Patrocinio en garantía para un organizador.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Patrocinio {
    pub patrocinador: Address,
    pub organizador: Address,
    pub saldo: i128,
    pub cerrado: bool,
}

/// Bolsa de premios o de apoyo a artistas que administra un organizador.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bolsa {
    pub organizador: Address,
    pub saldo: i128,
    pub cerrada: bool,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Token,
    Total(Symbol),
    Trato(u32),
    Patrocinio(u32),
    Bolsa(u32),
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
pub struct Artistas;

#[contractimpl]
impl Artistas {
    pub fn __constructor(env: Env, token: Address) {
        env.storage().instance().set(&Clave::Token, &token);
    }

    // ---------- Pago a artistas ----------

    /// El organizador contrata a un artista y deposita su pago en garantía.
    /// Devuelve el número de trato.
    pub fn contratar_artista(
        env: Env,
        organizador: Address,
        artista: Address,
        monto: i128,
    ) -> Result<u32, Error> {
        organizador.require_auth();
        if organizador == artista {
            return Err(Error::MismaCuenta);
        }
        recibir(&env, &organizador, monto)?;
        let id = siguiente(&env, "trato");
        let trato = Trato {
            organizador: organizador.clone(),
            artista,
            deposito: monto,
            pagado: 0,
            cerrado: false,
        };
        guardar(&env, &Clave::Trato(id), &trato);
        publicar(
            &env,
            "deposito",
            &organizador,
            &env.current_contract_address(),
            monto,
        );
        Ok(id)
    }

    /// El organizador suma dinero al pago en garantía (por ejemplo, otra
    /// función o un set más largo).
    pub fn aumentar_pago(env: Env, trato_id: u32, monto: i128) -> Result<i128, Error> {
        let mut trato = trato_abierto(&env, trato_id)?;
        trato.organizador.require_auth();
        recibir(&env, &trato.organizador, monto)?;
        trato.deposito += monto;
        guardar(&env, &Clave::Trato(trato_id), &trato);
        publicar(
            &env,
            "deposito",
            &trato.organizador,
            &env.current_contract_address(),
            monto,
        );
        Ok(trato.deposito)
    }

    /// El organizador libera un anticipo al artista. Devuelve lo que sigue
    /// en garantía.
    pub fn pagar_anticipo(env: Env, trato_id: u32, monto: i128) -> Result<i128, Error> {
        let mut trato = trato_abierto(&env, trato_id)?;
        trato.organizador.require_auth();
        if monto <= 0 || monto > trato.deposito {
            return Err(Error::SinFondos);
        }
        pagar_trato(&env, trato_id, &mut trato, monto);
        Ok(trato.deposito)
    }

    /// Después de la presentación, el organizador libera todo lo que queda al
    /// artista y cierra el trato. Devuelve lo pagado en esta liquidación.
    pub fn liquidar_artista(env: Env, trato_id: u32) -> Result<i128, Error> {
        let mut trato = trato_abierto(&env, trato_id)?;
        trato.organizador.require_auth();
        let monto = trato.deposito;
        if monto > 0 {
            pagar_trato(&env, trato_id, &mut trato, monto);
        }
        trato.cerrado = true;
        guardar(&env, &Clave::Trato(trato_id), &trato);
        Ok(monto)
    }

    /// Liquida varios tratos del mismo organizador (hasta 10) en una sola
    /// transacción. Devuelve el total pagado.
    pub fn liquidar_varios(
        env: Env,
        organizador: Address,
        tratos: Vec<u32>,
    ) -> Result<i128, Error> {
        organizador.require_auth();
        validar_grupo(tratos.len())?;
        let mut total = 0;
        for trato_id in tratos.iter() {
            let mut trato = trato_abierto(&env, trato_id)?;
            if trato.organizador != organizador {
                return Err(Error::NoExiste);
            }
            let monto = trato.deposito;
            if monto > 0 {
                pagar_trato(&env, trato_id, &mut trato, monto);
            }
            trato.cerrado = true;
            guardar(&env, &Clave::Trato(trato_id), &trato);
            total += monto;
        }
        Ok(total)
    }

    /// El artista renuncia al trato: lo que siga en garantía regresa al
    /// organizador. Devuelve lo regresado.
    pub fn renunciar_artista(env: Env, trato_id: u32) -> Result<i128, Error> {
        let mut trato = trato_abierto(&env, trato_id)?;
        trato.artista.require_auth();
        let monto = trato.deposito;
        if monto > 0 {
            enviar(&env, &trato.organizador, monto);
            publicar(
                &env,
                "devolucion",
                &trato.artista,
                &trato.organizador,
                monto,
            );
        }
        trato.deposito = 0;
        trato.cerrado = true;
        guardar(&env, &Clave::Trato(trato_id), &trato);
        Ok(monto)
    }

    /// Bono directo del organizador a un artista con trato, fuera de la
    /// garantía. Queda sumado a lo pagado.
    pub fn dar_bono(env: Env, trato_id: u32, monto: i128) -> Result<i128, Error> {
        let mut trato: Trato = leer(&env, &Clave::Trato(trato_id)).ok_or(Error::NoExiste)?;
        trato.organizador.require_auth();
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        token(&env).transfer(&trato.organizador, &trato.artista, &monto);
        trato.pagado += monto;
        guardar(&env, &Clave::Trato(trato_id), &trato);
        publicar(&env, "bono", &trato.organizador, &trato.artista, monto);
        Ok(trato.pagado)
    }

    /// El organizador paga directo a varios artistas (hasta 10), cada uno su
    /// monto, en una sola transacción. Devuelve el total.
    pub fn pagar_artistas(
        env: Env,
        organizador: Address,
        artistas: Vec<Address>,
        montos: Vec<i128>,
    ) -> Result<i128, Error> {
        organizador.require_auth();
        validar_grupo(artistas.len())?;
        if artistas.len() != montos.len() {
            return Err(Error::CantidadInvalida);
        }
        let mut total = 0;
        for (artista, monto) in artistas.iter().zip(montos.iter()) {
            if monto <= 0 {
                return Err(Error::MontoInvalido);
            }
            token(&env).transfer(&organizador, &artista, &monto);
            publicar(&env, "pago", &organizador, &artista, monto);
            total += monto;
        }
        Ok(total)
    }

    // ---------- Patrocinios ----------

    /// Una marca deposita un patrocinio en garantía para un organizador.
    /// Devuelve el número de patrocinio.
    pub fn patrocinar(
        env: Env,
        patrocinador: Address,
        organizador: Address,
        monto: i128,
    ) -> Result<u32, Error> {
        patrocinador.require_auth();
        if patrocinador == organizador {
            return Err(Error::MismaCuenta);
        }
        recibir(&env, &patrocinador, monto)?;
        let id = siguiente(&env, "patrocinio");
        let patrocinio = Patrocinio {
            patrocinador: patrocinador.clone(),
            organizador,
            saldo: monto,
            cerrado: false,
        };
        guardar(&env, &Clave::Patrocinio(id), &patrocinio);
        publicar(
            &env,
            "patrocinio",
            &patrocinador,
            &env.current_contract_address(),
            monto,
        );
        Ok(id)
    }

    /// La marca suma dinero a su patrocinio.
    pub fn aumentar_patrocinio(env: Env, id: u32, monto: i128) -> Result<i128, Error> {
        let mut patrocinio = patrocinio_abierto(&env, id)?;
        patrocinio.patrocinador.require_auth();
        recibir(&env, &patrocinio.patrocinador, monto)?;
        patrocinio.saldo += monto;
        guardar(&env, &Clave::Patrocinio(id), &patrocinio);
        Ok(patrocinio.saldo)
    }

    /// La marca libera una parte del patrocinio al organizador (por ejemplo,
    /// al confirmar el anuncio). Devuelve lo que sigue en garantía.
    pub fn liberar_patrocinio_parcial(env: Env, id: u32, monto: i128) -> Result<i128, Error> {
        let mut patrocinio = patrocinio_abierto(&env, id)?;
        patrocinio.patrocinador.require_auth();
        if monto <= 0 || monto > patrocinio.saldo {
            return Err(Error::SinFondos);
        }
        enviar(&env, &patrocinio.organizador, monto);
        patrocinio.saldo -= monto;
        guardar(&env, &Clave::Patrocinio(id), &patrocinio);
        publicar(
            &env,
            "patrocinio",
            &patrocinio.patrocinador,
            &patrocinio.organizador,
            monto,
        );
        Ok(patrocinio.saldo)
    }

    /// La marca libera todo el patrocinio al organizador y lo cierra.
    pub fn liberar_patrocinio(env: Env, id: u32) -> Result<i128, Error> {
        let mut patrocinio = patrocinio_abierto(&env, id)?;
        patrocinio.patrocinador.require_auth();
        let monto = patrocinio.saldo;
        if monto > 0 {
            enviar(&env, &patrocinio.organizador, monto);
            publicar(
                &env,
                "patrocinio",
                &patrocinio.patrocinador,
                &patrocinio.organizador,
                monto,
            );
        }
        patrocinio.saldo = 0;
        patrocinio.cerrado = true;
        guardar(&env, &Clave::Patrocinio(id), &patrocinio);
        Ok(monto)
    }

    /// Si el evento no se hizo, la marca recupera lo que no liberó.
    pub fn retirar_patrocinio(env: Env, id: u32) -> Result<i128, Error> {
        let mut patrocinio = patrocinio_abierto(&env, id)?;
        patrocinio.patrocinador.require_auth();
        let monto = patrocinio.saldo;
        if monto > 0 {
            enviar(&env, &patrocinio.patrocinador, monto);
            publicar(
                &env,
                "devolucion",
                &env.current_contract_address(),
                &patrocinio.patrocinador,
                monto,
            );
        }
        patrocinio.saldo = 0;
        patrocinio.cerrado = true;
        guardar(&env, &Clave::Patrocinio(id), &patrocinio);
        Ok(monto)
    }

    // ---------- Repartos ----------

    /// Reparte un pago (la taquilla de una noche, por ejemplo) entre varias
    /// cuentas según sus partes en puntos base, que deben sumar 10,000. Lo
    /// que sobre por redondeo va a la primera cuenta.
    pub fn repartir(
        env: Env,
        pagador: Address,
        cuentas: Vec<Address>,
        partes: Vec<u32>,
        monto: i128,
    ) -> Result<i128, Error> {
        pagador.require_auth();
        validar_grupo(cuentas.len())?;
        if cuentas.len() != partes.len() {
            return Err(Error::CantidadInvalida);
        }
        if partes.iter().sum::<u32>() != PUNTOS_BASE {
            return Err(Error::PartesInvalidas);
        }
        repartir_desde(&env, &pagador, &cuentas, &partes, monto)
    }

    /// Reparte un pago en partes iguales entre varias cuentas.
    pub fn repartir_igual(
        env: Env,
        pagador: Address,
        cuentas: Vec<Address>,
        monto: i128,
    ) -> Result<i128, Error> {
        pagador.require_auth();
        validar_grupo(cuentas.len())?;
        let mut partes = Vec::new(&env);
        let base = PUNTOS_BASE / cuentas.len();
        for i in 0..cuentas.len() {
            let extra = if i == 0 {
                PUNTOS_BASE - base * cuentas.len()
            } else {
                0
            };
            partes.push_back(base + extra);
        }
        repartir_desde(&env, &pagador, &cuentas, &partes, monto)
    }

    // ---------- Bolsas de premios ----------

    /// El organizador abre una bolsa de premios con un primer depósito.
    pub fn crear_bolsa(env: Env, organizador: Address, monto: i128) -> Result<u32, Error> {
        organizador.require_auth();
        recibir(&env, &organizador, monto)?;
        let id = siguiente(&env, "bolsa");
        let bolsa = Bolsa {
            organizador: organizador.clone(),
            saldo: monto,
            cerrada: false,
        };
        guardar(&env, &Clave::Bolsa(id), &bolsa);
        publicar(
            &env,
            "bolsa",
            &organizador,
            &env.current_contract_address(),
            monto,
        );
        Ok(id)
    }

    /// Cualquiera (fans, marcas) aporta a una bolsa abierta.
    pub fn aportar_bolsa(
        env: Env,
        cuenta: Address,
        bolsa_id: u32,
        monto: i128,
    ) -> Result<i128, Error> {
        cuenta.require_auth();
        let mut bolsa = bolsa_abierta(&env, bolsa_id)?;
        recibir(&env, &cuenta, monto)?;
        bolsa.saldo += monto;
        guardar(&env, &Clave::Bolsa(bolsa_id), &bolsa);
        publicar(
            &env,
            "bolsa",
            &cuenta,
            &env.current_contract_address(),
            monto,
        );
        Ok(bolsa.saldo)
    }

    /// El organizador paga un premio desde la bolsa.
    pub fn pagar_desde_bolsa(
        env: Env,
        bolsa_id: u32,
        artista: Address,
        monto: i128,
    ) -> Result<i128, Error> {
        let mut bolsa = bolsa_abierta(&env, bolsa_id)?;
        bolsa.organizador.require_auth();
        if monto <= 0 || monto > bolsa.saldo {
            return Err(Error::SinFondos);
        }
        enviar(&env, &artista, monto);
        bolsa.saldo -= monto;
        guardar(&env, &Clave::Bolsa(bolsa_id), &bolsa);
        publicar(&env, "premio", &bolsa.organizador, &artista, monto);
        Ok(bolsa.saldo)
    }

    /// El organizador reparte la bolsa completa en partes iguales entre los
    /// ganadores (hasta 10) y la cierra. Devuelve lo repartido.
    pub fn repartir_bolsa(env: Env, bolsa_id: u32, ganadores: Vec<Address>) -> Result<i128, Error> {
        let mut bolsa = bolsa_abierta(&env, bolsa_id)?;
        bolsa.organizador.require_auth();
        validar_grupo(ganadores.len())?;
        let parte = bolsa.saldo / ganadores.len() as i128;
        if parte <= 0 {
            return Err(Error::SinFondos);
        }
        for ganador in ganadores.iter() {
            enviar(&env, &ganador, parte);
            publicar(&env, "premio", &bolsa.organizador, &ganador, parte);
        }
        let repartido = parte * ganadores.len() as i128;
        // Lo que sobre por redondeo regresa al organizador.
        let resto = bolsa.saldo - repartido;
        if resto > 0 {
            enviar(&env, &bolsa.organizador, resto);
        }
        bolsa.saldo = 0;
        bolsa.cerrada = true;
        guardar(&env, &Clave::Bolsa(bolsa_id), &bolsa);
        Ok(repartido)
    }

    /// El organizador cierra la bolsa y recupera lo que no se repartió.
    pub fn cerrar_bolsa(env: Env, bolsa_id: u32) -> Result<i128, Error> {
        let mut bolsa = bolsa_abierta(&env, bolsa_id)?;
        bolsa.organizador.require_auth();
        let monto = bolsa.saldo;
        if monto > 0 {
            enviar(&env, &bolsa.organizador, monto);
            publicar(
                &env,
                "devolucion",
                &env.current_contract_address(),
                &bolsa.organizador,
                monto,
            );
        }
        bolsa.saldo = 0;
        bolsa.cerrada = true;
        guardar(&env, &Clave::Bolsa(bolsa_id), &bolsa);
        Ok(monto)
    }

    // ---------- Consultas ----------

    pub fn consultar_trato(env: Env, trato_id: u32) -> Result<Trato, Error> {
        leer(&env, &Clave::Trato(trato_id)).ok_or(Error::NoExiste)
    }

    pub fn consultar_patrocinio(env: Env, id: u32) -> Result<Patrocinio, Error> {
        leer(&env, &Clave::Patrocinio(id)).ok_or(Error::NoExiste)
    }

    pub fn consultar_bolsa(env: Env, bolsa_id: u32) -> Result<Bolsa, Error> {
        leer(&env, &Clave::Bolsa(bolsa_id)).ok_or(Error::NoExiste)
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

fn leer<V: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(env: &Env, clave: &Clave) -> Option<V> {
    env.storage().persistent().get(clave)
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

fn validar_grupo(largo: u32) -> Result<(), Error> {
    if largo == 0 || largo > GRUPO_MAXIMO {
        return Err(Error::CantidadInvalida);
    }
    Ok(())
}

/// Recibe `monto` de `de` en el contrato (garantía).
fn recibir(env: &Env, de: &Address, monto: i128) -> Result<(), Error> {
    if monto <= 0 {
        return Err(Error::MontoInvalido);
    }
    token(env).transfer(de, env.current_contract_address(), &monto);
    Ok(())
}

/// Envía `monto` desde la garantía del contrato a `para`.
fn enviar(env: &Env, para: &Address, monto: i128) {
    token(env).transfer(&env.current_contract_address(), para, &monto);
}

fn trato_abierto(env: &Env, id: u32) -> Result<Trato, Error> {
    let trato: Trato = leer(env, &Clave::Trato(id)).ok_or(Error::NoExiste)?;
    if trato.cerrado {
        return Err(Error::Cerrado);
    }
    Ok(trato)
}

fn patrocinio_abierto(env: &Env, id: u32) -> Result<Patrocinio, Error> {
    let patrocinio: Patrocinio = leer(env, &Clave::Patrocinio(id)).ok_or(Error::NoExiste)?;
    if patrocinio.cerrado {
        return Err(Error::Cerrado);
    }
    Ok(patrocinio)
}

fn bolsa_abierta(env: &Env, id: u32) -> Result<Bolsa, Error> {
    let bolsa: Bolsa = leer(env, &Clave::Bolsa(id)).ok_or(Error::NoExiste)?;
    if bolsa.cerrada {
        return Err(Error::Cerrado);
    }
    Ok(bolsa)
}

/// Libera `monto` de la garantía de un trato al artista.
fn pagar_trato(env: &Env, trato_id: u32, trato: &mut Trato, monto: i128) {
    enviar(env, &trato.artista, monto);
    trato.deposito -= monto;
    trato.pagado += monto;
    guardar(env, &Clave::Trato(trato_id), trato);
    publicar(env, "pago", &trato.organizador, &trato.artista, monto);
}

fn repartir_desde(
    env: &Env,
    pagador: &Address,
    cuentas: &Vec<Address>,
    partes: &Vec<u32>,
    monto: i128,
) -> Result<i128, Error> {
    if monto <= 0 {
        return Err(Error::MontoInvalido);
    }
    let mut repartido = 0;
    let mut montos = Vec::new(env);
    for parte in partes.iter() {
        let cantidad = monto * parte as i128 / PUNTOS_BASE as i128;
        montos.push_back(cantidad);
        repartido += cantidad;
    }
    for (i, cuenta) in cuentas.iter().enumerate() {
        let mut cantidad = montos.get(i as u32).unwrap();
        if i == 0 {
            cantidad += monto - repartido;
        }
        if cantidad > 0 {
            token(env).transfer(pagador, &cuenta, &cantidad);
            publicar(env, "reparto", pagador, &cuenta, cantidad);
        }
    }
    Ok(monto)
}

mod test;
