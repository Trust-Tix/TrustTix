#![no_std]

//! Seguros TrustTix: seguro de cancelación para boletos y depósitos de
//! garantía (renta del recinto, equipo prestado al personal).
//!
//! Reglas que hace cumplir la red:
//! 1. Cada póliza está respaldada por el fondo del organizador: no se vende
//!    una cobertura que el fondo no pueda pagar, y el organizador no retira
//!    lo que está comprometido.
//! 2. La prima es el 10 % de la cobertura y entra al fondo.
//! 3. Un depósito de garantía regresa completo salvo los daños que cobre
//!    quien lo recibió; si no responde en 7 días, quien depositó lo recupera.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env,
    Symbol, Vec,
};

const DIA: u32 = 17_280;
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;
/// Prima del seguro: 10 % de la cobertura, en puntos base.
const PRIMA: i128 = 1_000;
const PUNTOS_BASE: i128 = 10_000;
/// Plazo para que quien recibió un depósito lo devuelva o cobre daños.
const PLAZO_DEPOSITO: u64 = 7 * 24 * 60 * 60;
const GRUPO_MAXIMO: u32 = 10;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NoExiste = 1,
    MontoInvalido = 2,
    SinCobertura = 3,
    Inactiva = 4,
    SinFondos = 5,
    Comprometido = 6,
    MismaCuenta = 7,
    NoVencido = 8,
    Cerrado = 9,
    CantidadInvalida = 10,
}

/// Fondo que respalda las pólizas de un organizador.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fondo {
    pub organizador: Address,
    pub saldo: i128,
    /// Suma de las coberturas de las pólizas activas.
    pub comprometido: i128,
    pub cerrado: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Poliza {
    pub asegurado: Address,
    pub fondo: u32,
    pub cobertura: i128,
    pub prima: i128,
    pub activa: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deposito {
    pub depositante: Address,
    pub beneficiario: Address,
    pub monto: i128,
    pub vence: u64,
    pub cerrado: bool,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Token,
    Total(Symbol),
    Fondo(u32),
    Poliza(u32),
    Deposito(u32),
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
pub struct Seguros;

#[contractimpl]
impl Seguros {
    pub fn __constructor(env: Env, token: Address) {
        env.storage().instance().set(&Clave::Token, &token);
    }

    // ---------- Fondo del seguro ----------

    /// El organizador deposita la reserva que respaldará las pólizas.
    pub fn fondear_seguro(env: Env, organizador: Address, monto: i128) -> Result<u32, Error> {
        organizador.require_auth();
        recibir(&env, &organizador, monto)?;
        let id = siguiente(&env, "fondo");
        let fondo = Fondo {
            organizador: organizador.clone(),
            saldo: monto,
            comprometido: 0,
            cerrado: false,
        };
        guardar(&env, &Clave::Fondo(id), &fondo);
        publicar(
            &env,
            "fondo",
            &organizador,
            &env.current_contract_address(),
            monto,
        );
        Ok(id)
    }

    /// El organizador suma reserva al fondo.
    pub fn aumentar_fondo(env: Env, fondo_id: u32, monto: i128) -> Result<i128, Error> {
        let mut fondo = fondo_abierto(&env, fondo_id)?;
        fondo.organizador.require_auth();
        recibir(&env, &fondo.organizador, monto)?;
        fondo.saldo += monto;
        guardar(&env, &Clave::Fondo(fondo_id), &fondo);
        Ok(fondo.saldo)
    }

    /// El organizador retira reserva que no esté comprometida en pólizas.
    pub fn retirar_fondo(env: Env, fondo_id: u32, monto: i128) -> Result<i128, Error> {
        let mut fondo = fondo_abierto(&env, fondo_id)?;
        fondo.organizador.require_auth();
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        if monto > fondo.saldo - fondo.comprometido {
            return Err(Error::Comprometido);
        }
        enviar(&env, &fondo.organizador, monto);
        fondo.saldo -= monto;
        guardar(&env, &Clave::Fondo(fondo_id), &fondo);
        Ok(fondo.saldo)
    }

    /// Sin pólizas activas, el organizador cierra el fondo y recupera todo.
    pub fn cerrar_fondo(env: Env, fondo_id: u32) -> Result<i128, Error> {
        let mut fondo = fondo_abierto(&env, fondo_id)?;
        fondo.organizador.require_auth();
        if fondo.comprometido > 0 {
            return Err(Error::Comprometido);
        }
        let monto = fondo.saldo;
        if monto > 0 {
            enviar(&env, &fondo.organizador, monto);
        }
        fondo.saldo = 0;
        fondo.cerrado = true;
        guardar(&env, &Clave::Fondo(fondo_id), &fondo);
        Ok(monto)
    }

    // ---------- Pólizas ----------

    /// Contrata un seguro de cancelación: paga el 10 % de la cobertura al
    /// fondo. Devuelve el número de póliza.
    pub fn contratar_seguro(
        env: Env,
        cuenta: Address,
        fondo_id: u32,
        cobertura: i128,
    ) -> Result<u32, Error> {
        cuenta.require_auth();
        emitir_poliza(&env, &cuenta, &cuenta, fondo_id, cobertura)
    }

    /// Contrata el seguro para otra persona: paga `pagador` y la póliza es
    /// de `cuenta`.
    pub fn contratar_seguro_para(
        env: Env,
        pagador: Address,
        cuenta: Address,
        fondo_id: u32,
        cobertura: i128,
    ) -> Result<u32, Error> {
        pagador.require_auth();
        if pagador == cuenta {
            return Err(Error::MismaCuenta);
        }
        emitir_poliza(&env, &pagador, &cuenta, fondo_id, cobertura)
    }

    /// Asegura a todo un grupo (hasta 10) con la misma cobertura en una sola
    /// transacción. Devuelve los números de póliza.
    pub fn contratar_seguro_varios(
        env: Env,
        pagador: Address,
        cuentas: Vec<Address>,
        fondo_id: u32,
        cobertura: i128,
    ) -> Result<Vec<u32>, Error> {
        pagador.require_auth();
        if cuentas.is_empty() || cuentas.len() > GRUPO_MAXIMO {
            return Err(Error::CantidadInvalida);
        }
        let mut polizas = Vec::new(&env);
        for cuenta in cuentas.iter() {
            polizas.push_back(emitir_poliza(&env, &pagador, &cuenta, fondo_id, cobertura)?);
        }
        Ok(polizas)
    }

    /// Si el evento se canceló, el organizador paga la cobertura completa al
    /// asegurado desde el fondo.
    pub fn pagar_siniestro(env: Env, poliza_id: u32) -> Result<i128, Error> {
        let poliza = leer_poliza(&env, poliza_id)?;
        let fondo = leer_fondo(&env, poliza.fondo)?;
        fondo.organizador.require_auth();
        pagar_poliza(&env, poliza_id)
    }

    /// Paga varias pólizas del mismo fondo (hasta 10) en una transacción.
    /// Devuelve el total pagado.
    pub fn pagar_siniestros(env: Env, fondo_id: u32, polizas: Vec<u32>) -> Result<i128, Error> {
        let fondo = leer_fondo(&env, fondo_id)?;
        fondo.organizador.require_auth();
        if polizas.is_empty() || polizas.len() > GRUPO_MAXIMO {
            return Err(Error::CantidadInvalida);
        }
        let mut total = 0;
        for poliza_id in polizas.iter() {
            if leer_poliza(&env, poliza_id)?.fondo != fondo_id {
                return Err(Error::NoExiste);
            }
            total += pagar_poliza(&env, poliza_id)?;
        }
        Ok(total)
    }

    /// El asegurado cancela su póliza antes de un siniestro y recupera la
    /// mitad de la prima.
    pub fn cancelar_poliza(env: Env, poliza_id: u32) -> Result<i128, Error> {
        let mut poliza = poliza_activa(&env, poliza_id)?;
        poliza.asegurado.require_auth();
        let mut fondo = leer_fondo(&env, poliza.fondo)?;
        let devolucion = poliza.prima / 2;
        if devolucion > 0 {
            enviar(&env, &poliza.asegurado, devolucion);
        }
        fondo.saldo -= devolucion;
        fondo.comprometido -= poliza.cobertura;
        guardar(&env, &Clave::Fondo(poliza.fondo), &fondo);
        poliza.activa = false;
        guardar(&env, &Clave::Poliza(poliza_id), &poliza);
        publicar(
            &env,
            "devolucion",
            &env.current_contract_address(),
            &poliza.asegurado,
            devolucion,
        );
        Ok(devolucion)
    }

    /// El asegurado pasa su póliza a otra persona (por ejemplo, junto con su
    /// boleto).
    pub fn transferir_poliza(env: Env, poliza_id: u32, a: Address) -> Result<(), Error> {
        let mut poliza = poliza_activa(&env, poliza_id)?;
        poliza.asegurado.require_auth();
        if poliza.asegurado == a {
            return Err(Error::MismaCuenta);
        }
        publicar(&env, "traspaso", &poliza.asegurado, &a, poliza.cobertura);
        poliza.asegurado = a;
        guardar(&env, &Clave::Poliza(poliza_id), &poliza);
        Ok(())
    }

    // ---------- Depósitos de garantía ----------

    /// Deja un depósito de garantía para `beneficiario` (el recinto, quien
    /// presta el equipo). Devuelve su número.
    pub fn dejar_deposito(
        env: Env,
        depositante: Address,
        beneficiario: Address,
        monto: i128,
    ) -> Result<u32, Error> {
        depositante.require_auth();
        if depositante == beneficiario {
            return Err(Error::MismaCuenta);
        }
        recibir(&env, &depositante, monto)?;
        let id = siguiente(&env, "deposito");
        let deposito = Deposito {
            depositante: depositante.clone(),
            beneficiario,
            monto,
            vence: env.ledger().timestamp() + PLAZO_DEPOSITO,
            cerrado: false,
        };
        guardar(&env, &Clave::Deposito(id), &deposito);
        publicar(
            &env,
            "deposito",
            &depositante,
            &env.current_contract_address(),
            monto,
        );
        Ok(id)
    }

    /// Quien depositó suma dinero a su garantía.
    pub fn aumentar_deposito(env: Env, deposito_id: u32, monto: i128) -> Result<i128, Error> {
        let mut deposito = deposito_abierto(&env, deposito_id)?;
        deposito.depositante.require_auth();
        recibir(&env, &deposito.depositante, monto)?;
        deposito.monto += monto;
        guardar(&env, &Clave::Deposito(deposito_id), &deposito);
        Ok(deposito.monto)
    }

    /// Quien recibió el depósito lo regresa completo: no hubo daños.
    pub fn devolver_deposito(env: Env, deposito_id: u32) -> Result<i128, Error> {
        let deposito = deposito_abierto(&env, deposito_id)?;
        deposito.beneficiario.require_auth();
        cerrar_deposito(&env, deposito_id, deposito, 0)
    }

    /// Devuelve varios depósitos (hasta 10) del mismo beneficiario en una
    /// transacción. Devuelve el total regresado.
    pub fn devolver_depositos(
        env: Env,
        beneficiario: Address,
        depositos: Vec<u32>,
    ) -> Result<i128, Error> {
        beneficiario.require_auth();
        if depositos.is_empty() || depositos.len() > GRUPO_MAXIMO {
            return Err(Error::CantidadInvalida);
        }
        let mut total = 0;
        for deposito_id in depositos.iter() {
            let deposito = deposito_abierto(&env, deposito_id)?;
            if deposito.beneficiario != beneficiario {
                return Err(Error::NoExiste);
            }
            total += cerrar_deposito(&env, deposito_id, deposito, 0)?;
        }
        Ok(total)
    }

    /// Quien recibió el depósito cobra los daños y el resto regresa a quien
    /// depositó. Devuelve lo regresado.
    pub fn cobrar_danos(env: Env, deposito_id: u32, danos: i128) -> Result<i128, Error> {
        let deposito = deposito_abierto(&env, deposito_id)?;
        deposito.beneficiario.require_auth();
        if danos <= 0 || danos > deposito.monto {
            return Err(Error::MontoInvalido);
        }
        cerrar_deposito(&env, deposito_id, deposito, danos)
    }

    /// Si quien recibió el depósito no lo resolvió en 7 días, quien depositó
    /// lo recupera completo.
    pub fn reclamar_deposito(env: Env, deposito_id: u32) -> Result<i128, Error> {
        let deposito = deposito_abierto(&env, deposito_id)?;
        deposito.depositante.require_auth();
        if env.ledger().timestamp() <= deposito.vence {
            return Err(Error::NoVencido);
        }
        cerrar_deposito(&env, deposito_id, deposito, 0)
    }

    // ---------- Consultas ----------

    pub fn consultar_fondo(env: Env, fondo_id: u32) -> Result<Fondo, Error> {
        leer_fondo(&env, fondo_id)
    }

    pub fn consultar_poliza(env: Env, poliza_id: u32) -> Result<Poliza, Error> {
        leer_poliza(&env, poliza_id)
    }

    pub fn consultar_deposito(env: Env, deposito_id: u32) -> Result<Deposito, Error> {
        env.storage()
            .persistent()
            .get(&Clave::Deposito(deposito_id))
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

fn recibir(env: &Env, de: &Address, monto: i128) -> Result<(), Error> {
    if monto <= 0 {
        return Err(Error::MontoInvalido);
    }
    token(env).transfer(de, env.current_contract_address(), &monto);
    Ok(())
}

fn enviar(env: &Env, para: &Address, monto: i128) {
    token(env).transfer(&env.current_contract_address(), para, &monto);
}

fn leer_fondo(env: &Env, id: u32) -> Result<Fondo, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Fondo(id))
        .ok_or(Error::NoExiste)
}

fn fondo_abierto(env: &Env, id: u32) -> Result<Fondo, Error> {
    let fondo = leer_fondo(env, id)?;
    if fondo.cerrado {
        return Err(Error::Cerrado);
    }
    Ok(fondo)
}

fn leer_poliza(env: &Env, id: u32) -> Result<Poliza, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Poliza(id))
        .ok_or(Error::NoExiste)
}

fn poliza_activa(env: &Env, id: u32) -> Result<Poliza, Error> {
    let poliza = leer_poliza(env, id)?;
    if !poliza.activa {
        return Err(Error::Inactiva);
    }
    Ok(poliza)
}

/// Cobra la prima a `pagador` y emite una póliza para `asegurado`, siempre
/// que el fondo pueda respaldar la cobertura.
fn emitir_poliza(
    env: &Env,
    pagador: &Address,
    asegurado: &Address,
    fondo_id: u32,
    cobertura: i128,
) -> Result<u32, Error> {
    let mut fondo = fondo_abierto(env, fondo_id)?;
    if cobertura <= 0 {
        return Err(Error::MontoInvalido);
    }
    let prima = cobertura * PRIMA / PUNTOS_BASE;
    if prima <= 0 {
        return Err(Error::MontoInvalido);
    }
    if fondo.saldo + prima - fondo.comprometido < cobertura {
        return Err(Error::SinCobertura);
    }
    recibir(env, pagador, prima)?;
    fondo.saldo += prima;
    fondo.comprometido += cobertura;
    guardar(env, &Clave::Fondo(fondo_id), &fondo);
    let id = siguiente(env, "poliza");
    let poliza = Poliza {
        asegurado: asegurado.clone(),
        fondo: fondo_id,
        cobertura,
        prima,
        activa: true,
    };
    guardar(env, &Clave::Poliza(id), &poliza);
    publicar(env, "prima", pagador, &fondo.organizador, prima);
    Ok(id)
}

/// Paga la cobertura de una póliza activa desde su fondo.
fn pagar_poliza(env: &Env, poliza_id: u32) -> Result<i128, Error> {
    let mut poliza = poliza_activa(env, poliza_id)?;
    let mut fondo = leer_fondo(env, poliza.fondo)?;
    if fondo.saldo < poliza.cobertura {
        return Err(Error::SinFondos);
    }
    enviar(env, &poliza.asegurado, poliza.cobertura);
    fondo.saldo -= poliza.cobertura;
    fondo.comprometido -= poliza.cobertura;
    guardar(env, &Clave::Fondo(poliza.fondo), &fondo);
    poliza.activa = false;
    guardar(env, &Clave::Poliza(poliza_id), &poliza);
    publicar(
        env,
        "siniestro",
        &fondo.organizador,
        &poliza.asegurado,
        poliza.cobertura,
    );
    Ok(poliza.cobertura)
}

fn deposito_abierto(env: &Env, id: u32) -> Result<Deposito, Error> {
    let deposito: Deposito = env
        .storage()
        .persistent()
        .get(&Clave::Deposito(id))
        .ok_or(Error::NoExiste)?;
    if deposito.cerrado {
        return Err(Error::Cerrado);
    }
    Ok(deposito)
}

/// Cierra un depósito: `danos` van al beneficiario y el resto a quien
/// depositó. Devuelve lo regresado a quien depositó.
fn cerrar_deposito(env: &Env, id: u32, mut deposito: Deposito, danos: i128) -> Result<i128, Error> {
    if danos > 0 {
        enviar(env, &deposito.beneficiario, danos);
        publicar(
            env,
            "danos",
            &deposito.depositante,
            &deposito.beneficiario,
            danos,
        );
    }
    let regreso = deposito.monto - danos;
    if regreso > 0 {
        enviar(env, &deposito.depositante, regreso);
        publicar(
            env,
            "devolucion",
            &env.current_contract_address(),
            &deposito.depositante,
            regreso,
        );
    }
    deposito.cerrado = true;
    guardar(env, &Clave::Deposito(id), &deposito);
    Ok(regreso)
}

mod test;
