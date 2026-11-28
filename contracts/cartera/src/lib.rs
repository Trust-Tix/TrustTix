#![no_std]

//! Cartera TrustTix: saldo cashless para pagar dentro del evento (barra,
//! comida, mercancía) con la misma cuenta del boleto.
//!
//! Reglas que hace cumplir la red:
//! 1. El saldo de cada asistente está respaldado por XLM guardado en el
//!    contrato, y solo su dueño lo gasta o lo retira.
//! 2. Solo los comercios que registró TrustTix cobran pagos.
//! 3. Una preautorización aparta saldo para un comercio: lo que no se cobra
//!    regresa al asistente.
//! 4. En una vaca (fondo de grupo), solo quien la creó paga con ella, y al
//!    disolverla cada quien recupera su parte de lo que quede.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env, Vec,
};

const DIA: u32 = 17_280;
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;
/// Cuentas o comercios que se atienden como máximo en una transacción.
const GRUPO_MAXIMO: u32 = 10;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    SinSaldo = 1,
    MontoInvalido = 2,
    NoEsComercio = 3,
    SinRetencion = 4,
    VacaNoExiste = 5,
    NoEsCreador = 6,
    VacaCerrada = 7,
    CantidadInvalida = 8,
    SinAporte = 9,
    VacaUsada = 10,
    MismaCuenta = 11,
}

/// Fondo común de un grupo: cuánto se aportó en total y cuánto queda.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vaca {
    pub creador: Address,
    pub aportado: i128,
    pub saldo: i128,
    pub abierta: bool,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Admin,
    Token,
    TotalVacas,
    Saldo(Address),
    Comercio(Address),
    Ganancias(Address),
    Retenido(Address, Address),
    Vaca(u32),
    Aporte(u32, Address),
    Aportantes(u32),
}

#[contractevent]
pub struct Movimiento {
    #[topic]
    pub tipo: soroban_sdk::Symbol,
    pub de: Address,
    pub para: Address,
    pub monto: i128,
}

#[contract]
pub struct Cartera;

#[contractimpl]
impl Cartera {
    /// `token`: activo con el que se recarga (en Testnet, el contrato de XLM).
    /// `admin`: cuenta de TrustTix que registra comercios.
    pub fn __constructor(env: Env, token: Address, admin: Address) {
        env.storage().instance().set(&Clave::Token, &token);
        env.storage().instance().set(&Clave::Admin, &admin);
    }

    /// TrustTix registra un comercio del evento para que pueda cobrar.
    pub fn registrar_comercio(env: Env, comercio: Address) {
        admin(&env).require_auth();
        guardar(&env, &Clave::Comercio(comercio), &true);
    }

    // ---------- Recargas y retiros ----------

    /// El asistente pasa XLM de su cuenta a su saldo del evento.
    pub fn recargar(env: Env, cuenta: Address, monto: i128) -> Result<i128, Error> {
        cuenta.require_auth();
        recargar_a(&env, &cuenta, &cuenta, monto)
    }

    /// Alguien más recarga el saldo de un asistente (papás, amigos).
    pub fn recargar_para(
        env: Env,
        pagador: Address,
        cuenta: Address,
        monto: i128,
    ) -> Result<i128, Error> {
        pagador.require_auth();
        if pagador == cuenta {
            return Err(Error::MismaCuenta);
        }
        recargar_a(&env, &pagador, &cuenta, monto)
    }

    /// Una persona recarga el mismo monto a todo su grupo (hasta 10) en una
    /// sola transacción. Devuelve el total cobrado.
    pub fn recargar_varios(
        env: Env,
        pagador: Address,
        cuentas: Vec<Address>,
        monto: i128,
    ) -> Result<i128, Error> {
        pagador.require_auth();
        validar_grupo(cuentas.len())?;
        for cuenta in cuentas.iter() {
            recargar_a(&env, &pagador, &cuenta, monto)?;
        }
        Ok(monto * cuentas.len() as i128)
    }

    /// El asistente regresa parte de su saldo a su cuenta.
    pub fn retirar(env: Env, cuenta: Address, monto: i128) -> Result<i128, Error> {
        cuenta.require_auth();
        quitar_saldo(&env, &cuenta, monto)?;
        pagar_token(&env, &cuenta, monto);
        publicar(
            &env,
            "retiro",
            &env.current_contract_address(),
            &cuenta,
            monto,
        );
        Ok(saldo(&env, &cuenta))
    }

    /// El asistente regresa todo su saldo a su cuenta. Devuelve lo retirado.
    pub fn retirar_todo(env: Env, cuenta: Address) -> Result<i128, Error> {
        cuenta.require_auth();
        let monto = saldo(&env, &cuenta);
        quitar_saldo(&env, &cuenta, monto)?;
        pagar_token(&env, &cuenta, monto);
        publicar(
            &env,
            "retiro",
            &env.current_contract_address(),
            &cuenta,
            monto,
        );
        Ok(monto)
    }

    /// Pasa saldo a otro asistente sin sacarlo del evento.
    pub fn transferir_saldo(env: Env, de: Address, a: Address, monto: i128) -> Result<(), Error> {
        de.require_auth();
        if de == a {
            return Err(Error::MismaCuenta);
        }
        quitar_saldo(&env, &de, monto)?;
        sumar_saldo(&env, &a, monto);
        publicar(&env, "saldo", &de, &a, monto);
        Ok(())
    }

    // ---------- Pagos en el evento ----------

    /// Paga en un comercio del evento con el saldo.
    pub fn pagar(env: Env, cuenta: Address, comercio: Address, monto: i128) -> Result<(), Error> {
        cuenta.require_auth();
        cobrar_a(&env, &cuenta, &comercio, monto)
    }

    /// Paga y deja propina en el mismo movimiento. Todo va al comercio.
    pub fn pagar_con_propina(
        env: Env,
        cuenta: Address,
        comercio: Address,
        monto: i128,
        propina: i128,
    ) -> Result<i128, Error> {
        cuenta.require_auth();
        if propina <= 0 {
            return Err(Error::MontoInvalido);
        }
        cobrar_a(&env, &cuenta, &comercio, monto)?;
        cobrar_a(&env, &cuenta, &comercio, propina)?;
        Ok(monto + propina)
    }

    /// Paga en varios comercios a la vez (la cuenta de la noche). Si un pago
    /// no alcanza, no se hace ninguno. Devuelve el total.
    pub fn pagar_varios(
        env: Env,
        cuenta: Address,
        comercios: Vec<Address>,
        montos: Vec<i128>,
    ) -> Result<i128, Error> {
        cuenta.require_auth();
        validar_grupo(comercios.len())?;
        if comercios.len() != montos.len() {
            return Err(Error::CantidadInvalida);
        }
        let mut total = 0;
        for (comercio, monto) in comercios.iter().zip(montos.iter()) {
            cobrar_a(&env, &cuenta, &comercio, monto)?;
            total += monto;
        }
        Ok(total)
    }

    /// Paga directo desde la cuenta, sin recargar antes: el XLM entra y queda
    /// como ganancia del comercio en la misma transacción.
    pub fn recargar_y_pagar(
        env: Env,
        cuenta: Address,
        comercio: Address,
        monto: i128,
    ) -> Result<(), Error> {
        cuenta.require_auth();
        recargar_a(&env, &cuenta, &cuenta, monto)?;
        cobrar_a(&env, &cuenta, &comercio, monto)
    }

    /// El comercio devuelve parte de lo cobrado al saldo de un asistente.
    pub fn reembolsar_pago(
        env: Env,
        comercio: Address,
        cuenta: Address,
        monto: i128,
    ) -> Result<(), Error> {
        comercio.require_auth();
        quitar_ganancia(&env, &comercio, monto)?;
        sumar_saldo(&env, &cuenta, monto);
        publicar(&env, "reembolso", &comercio, &cuenta, monto);
        Ok(())
    }

    // ---------- Preautorizaciones ----------

    /// Aparta saldo para un comercio (por ejemplo, la cuenta abierta de la
    /// barra). Mientras está apartado, el asistente no lo puede gastar.
    pub fn preautorizar(
        env: Env,
        cuenta: Address,
        comercio: Address,
        monto: i128,
    ) -> Result<(), Error> {
        cuenta.require_auth();
        es_comercio(&env, &comercio)?;
        quitar_saldo(&env, &cuenta, monto)?;
        let clave = Clave::Retenido(cuenta.clone(), comercio.clone());
        let previo: i128 = env.storage().persistent().get(&clave).unwrap_or(0);
        guardar(&env, &clave, &(previo + monto));
        publicar(&env, "retencion", &cuenta, &comercio, monto);
        Ok(())
    }

    /// El comercio cobra lo consumido de una preautorización y el resto
    /// regresa al saldo del asistente. Devuelve lo que regresó.
    pub fn capturar(
        env: Env,
        comercio: Address,
        cuenta: Address,
        monto: i128,
    ) -> Result<i128, Error> {
        comercio.require_auth();
        let retenido = liberar_retencion(&env, &cuenta, &comercio)?;
        if monto <= 0 || monto > retenido {
            return Err(Error::MontoInvalido);
        }
        sumar_ganancia(&env, &comercio, monto);
        sumar_saldo(&env, &cuenta, retenido - monto);
        publicar(&env, "pago", &cuenta, &comercio, monto);
        Ok(retenido - monto)
    }

    /// El comercio cancela la preautorización y todo regresa al asistente.
    pub fn cancelar_preautorizacion(
        env: Env,
        comercio: Address,
        cuenta: Address,
    ) -> Result<i128, Error> {
        comercio.require_auth();
        let retenido = liberar_retencion(&env, &cuenta, &comercio)?;
        sumar_saldo(&env, &cuenta, retenido);
        publicar(&env, "liberacion", &comercio, &cuenta, retenido);
        Ok(retenido)
    }

    // ---------- Cobros de los comercios ----------

    /// El comercio retira todo lo que ha cobrado. Devuelve el monto.
    pub fn cobrar(env: Env, comercio: Address) -> Result<i128, Error> {
        comercio.require_auth();
        let monto = ganancias(&env, &comercio);
        quitar_ganancia(&env, &comercio, monto)?;
        pagar_token(&env, &comercio, monto);
        publicar(
            &env,
            "cobro",
            &env.current_contract_address(),
            &comercio,
            monto,
        );
        Ok(monto)
    }

    /// El comercio retira solo una parte de lo cobrado.
    pub fn cobrar_parcial(env: Env, comercio: Address, monto: i128) -> Result<i128, Error> {
        comercio.require_auth();
        quitar_ganancia(&env, &comercio, monto)?;
        pagar_token(&env, &comercio, monto);
        publicar(
            &env,
            "cobro",
            &env.current_contract_address(),
            &comercio,
            monto,
        );
        Ok(ganancias(&env, &comercio))
    }

    /// Al cierre, TrustTix paga a varios comercios (hasta 10) lo que cobraron.
    /// Devuelve el total pagado.
    pub fn liquidar_comercios(env: Env, comercios: Vec<Address>) -> Result<i128, Error> {
        admin(&env).require_auth();
        validar_grupo(comercios.len())?;
        let mut total = 0;
        for comercio in comercios.iter() {
            let monto = ganancias(&env, &comercio);
            if monto > 0 {
                quitar_ganancia(&env, &comercio, monto)?;
                pagar_token(&env, &comercio, monto);
                publicar(
                    &env,
                    "cobro",
                    &env.current_contract_address(),
                    &comercio,
                    monto,
                );
                total += monto;
            }
        }
        Ok(total)
    }

    /// Al cierre, TrustTix regresa a varios asistentes (hasta 10) el saldo
    /// que no gastaron. Devuelve el total devuelto.
    pub fn devolver_saldos(env: Env, cuentas: Vec<Address>) -> Result<i128, Error> {
        admin(&env).require_auth();
        validar_grupo(cuentas.len())?;
        let mut total = 0;
        for cuenta in cuentas.iter() {
            let monto = saldo(&env, &cuenta);
            if monto > 0 {
                quitar_saldo(&env, &cuenta, monto)?;
                pagar_token(&env, &cuenta, monto);
                publicar(
                    &env,
                    "retiro",
                    &env.current_contract_address(),
                    &cuenta,
                    monto,
                );
                total += monto;
            }
        }
        Ok(total)
    }

    // ---------- Vaca (fondo de grupo) ----------

    /// Abre una vaca con un primer aporte del saldo de quien la crea.
    /// Devuelve su número.
    pub fn crear_vaca(env: Env, creador: Address, monto: i128) -> Result<u32, Error> {
        creador.require_auth();
        let id: u32 = env
            .storage()
            .instance()
            .get(&Clave::TotalVacas)
            .unwrap_or(0)
            + 1;
        env.storage().instance().set(&Clave::TotalVacas, &id);
        let vaca = Vaca {
            creador: creador.clone(),
            aportado: 0,
            saldo: 0,
            abierta: true,
        };
        guardar(&env, &Clave::Vaca(id), &vaca);
        aportar(&env, id, &creador, monto)?;
        Ok(id)
    }

    /// Cualquiera pone saldo en una vaca abierta.
    pub fn aportar_vaca(env: Env, cuenta: Address, vaca_id: u32, monto: i128) -> Result<(), Error> {
        cuenta.require_auth();
        aportar(&env, vaca_id, &cuenta, monto)
    }

    /// Mientras nadie haya gastado de la vaca, quien aportó recupera lo suyo.
    pub fn retirar_aporte(env: Env, cuenta: Address, vaca_id: u32) -> Result<i128, Error> {
        cuenta.require_auth();
        let mut vaca = leer_vaca(&env, vaca_id)?;
        if !vaca.abierta {
            return Err(Error::VacaCerrada);
        }
        if vaca.saldo != vaca.aportado {
            return Err(Error::VacaUsada);
        }
        let clave = Clave::Aporte(vaca_id, cuenta.clone());
        let aporte: i128 = env
            .storage()
            .persistent()
            .get(&clave)
            .ok_or(Error::SinAporte)?;
        env.storage().persistent().remove(&clave);
        vaca.aportado -= aporte;
        vaca.saldo -= aporte;
        guardar(&env, &Clave::Vaca(vaca_id), &vaca);
        sumar_saldo(&env, &cuenta, aporte);
        Ok(aporte)
    }

    /// Quien creó la vaca paga con ella en un comercio del evento.
    pub fn pagar_con_vaca(
        env: Env,
        vaca_id: u32,
        comercio: Address,
        monto: i128,
    ) -> Result<i128, Error> {
        let mut vaca = leer_vaca(&env, vaca_id)?;
        vaca.creador.require_auth();
        if !vaca.abierta {
            return Err(Error::VacaCerrada);
        }
        es_comercio(&env, &comercio)?;
        if monto <= 0 || monto > vaca.saldo {
            return Err(Error::SinSaldo);
        }
        vaca.saldo -= monto;
        guardar(&env, &Clave::Vaca(vaca_id), &vaca);
        sumar_ganancia(&env, &comercio, monto);
        publicar(&env, "pago", &vaca.creador, &comercio, monto);
        Ok(vaca.saldo)
    }

    /// Cierra la vaca y reparte lo que quede entre quienes aportaron, en
    /// proporción a su aporte. Devuelve lo repartido.
    pub fn disolver_vaca(env: Env, vaca_id: u32) -> Result<i128, Error> {
        let mut vaca = leer_vaca(&env, vaca_id)?;
        vaca.creador.require_auth();
        if !vaca.abierta {
            return Err(Error::VacaCerrada);
        }
        let aportantes: Vec<Address> = env
            .storage()
            .persistent()
            .get(&Clave::Aportantes(vaca_id))
            .unwrap_or(Vec::new(&env));
        let mut repartido = 0;
        for cuenta in aportantes.iter() {
            let aporte: i128 = env
                .storage()
                .persistent()
                .get(&Clave::Aporte(vaca_id, cuenta.clone()))
                .unwrap_or(0);
            if aporte > 0 && vaca.aportado > 0 {
                let parte = vaca.saldo * aporte / vaca.aportado;
                sumar_saldo(&env, &cuenta, parte);
                repartido += parte;
            }
        }
        // Lo que sobre por redondeo es para quien la creó.
        sumar_saldo(&env, &vaca.creador, vaca.saldo - repartido);
        vaca.saldo = 0;
        vaca.abierta = false;
        guardar(&env, &Clave::Vaca(vaca_id), &vaca);
        Ok(repartido)
    }

    // ---------- Consultas ----------

    pub fn saldo(env: Env, cuenta: Address) -> i128 {
        saldo(&env, &cuenta)
    }

    pub fn ganancias(env: Env, comercio: Address) -> i128 {
        ganancias(&env, &comercio)
    }

    pub fn retenido(env: Env, cuenta: Address, comercio: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&Clave::Retenido(cuenta, comercio))
            .unwrap_or(0)
    }

    pub fn consultar_vaca(env: Env, vaca_id: u32) -> Result<Vaca, Error> {
        leer_vaca(&env, vaca_id)
    }
}

// ---------- Funciones internas ----------

fn admin(env: &Env) -> Address {
    env.storage().instance().get(&Clave::Admin).unwrap()
}

fn guardar<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, clave: &Clave, valor: &V) {
    env.storage().persistent().set(clave, valor);
    env.storage()
        .persistent()
        .extend_ttl(clave, UMBRAL_TTL, EXTENDER_TTL);
}

fn publicar(env: &Env, tipo: &str, de: &Address, para: &Address, monto: i128) {
    Movimiento {
        tipo: soroban_sdk::Symbol::new(env, tipo),
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

fn saldo(env: &Env, cuenta: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&Clave::Saldo(cuenta.clone()))
        .unwrap_or(0)
}

fn sumar_saldo(env: &Env, cuenta: &Address, monto: i128) {
    let nuevo = saldo(env, cuenta) + monto;
    guardar(env, &Clave::Saldo(cuenta.clone()), &nuevo);
}

fn quitar_saldo(env: &Env, cuenta: &Address, monto: i128) -> Result<(), Error> {
    if monto <= 0 {
        return Err(Error::MontoInvalido);
    }
    let actual = saldo(env, cuenta);
    if monto > actual {
        return Err(Error::SinSaldo);
    }
    guardar(env, &Clave::Saldo(cuenta.clone()), &(actual - monto));
    Ok(())
}

fn ganancias(env: &Env, comercio: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&Clave::Ganancias(comercio.clone()))
        .unwrap_or(0)
}

fn sumar_ganancia(env: &Env, comercio: &Address, monto: i128) {
    let nuevo = ganancias(env, comercio) + monto;
    guardar(env, &Clave::Ganancias(comercio.clone()), &nuevo);
}

fn quitar_ganancia(env: &Env, comercio: &Address, monto: i128) -> Result<(), Error> {
    if monto <= 0 {
        return Err(Error::MontoInvalido);
    }
    let actual = ganancias(env, comercio);
    if monto > actual {
        return Err(Error::SinSaldo);
    }
    guardar(env, &Clave::Ganancias(comercio.clone()), &(actual - monto));
    Ok(())
}

fn es_comercio(env: &Env, comercio: &Address) -> Result<(), Error> {
    if env
        .storage()
        .persistent()
        .get::<_, bool>(&Clave::Comercio(comercio.clone()))
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(Error::NoEsComercio)
    }
}

/// Recibe XLM de `pagador` y lo suma al saldo de `cuenta`.
fn recargar_a(env: &Env, pagador: &Address, cuenta: &Address, monto: i128) -> Result<i128, Error> {
    if monto <= 0 {
        return Err(Error::MontoInvalido);
    }
    token::Client::new(env, &token_de_pago(env)).transfer(
        pagador,
        env.current_contract_address(),
        &monto,
    );
    sumar_saldo(env, cuenta, monto);
    publicar(env, "recarga", pagador, cuenta, monto);
    Ok(saldo(env, cuenta))
}

/// Pasa `monto` del saldo de `cuenta` a las ganancias de un comercio registrado.
fn cobrar_a(env: &Env, cuenta: &Address, comercio: &Address, monto: i128) -> Result<(), Error> {
    es_comercio(env, comercio)?;
    quitar_saldo(env, cuenta, monto)?;
    sumar_ganancia(env, comercio, monto);
    publicar(env, "pago", cuenta, comercio, monto);
    Ok(())
}

fn token_de_pago(env: &Env) -> Address {
    env.storage().instance().get(&Clave::Token).unwrap()
}

fn pagar_token(env: &Env, para: &Address, monto: i128) {
    token::Client::new(env, &token_de_pago(env)).transfer(
        &env.current_contract_address(),
        para,
        &monto,
    );
}

/// Quita la preautorización de `cuenta` con `comercio` y devuelve su monto.
fn liberar_retencion(env: &Env, cuenta: &Address, comercio: &Address) -> Result<i128, Error> {
    let clave = Clave::Retenido(cuenta.clone(), comercio.clone());
    let retenido: i128 = env
        .storage()
        .persistent()
        .get(&clave)
        .ok_or(Error::SinRetencion)?;
    env.storage().persistent().remove(&clave);
    Ok(retenido)
}

fn leer_vaca(env: &Env, id: u32) -> Result<Vaca, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Vaca(id))
        .ok_or(Error::VacaNoExiste)
}

/// Pasa saldo de `cuenta` a una vaca abierta y anota su aporte.
fn aportar(env: &Env, vaca_id: u32, cuenta: &Address, monto: i128) -> Result<(), Error> {
    let mut vaca = leer_vaca(env, vaca_id)?;
    if !vaca.abierta {
        return Err(Error::VacaCerrada);
    }
    quitar_saldo(env, cuenta, monto)?;
    vaca.aportado += monto;
    vaca.saldo += monto;
    guardar(env, &Clave::Vaca(vaca_id), &vaca);

    let clave = Clave::Aporte(vaca_id, cuenta.clone());
    let previo: i128 = env.storage().persistent().get(&clave).unwrap_or(0);
    if previo == 0 {
        let lista = Clave::Aportantes(vaca_id);
        let mut aportantes: Vec<Address> = env
            .storage()
            .persistent()
            .get(&lista)
            .unwrap_or(Vec::new(env));
        aportantes.push_back(cuenta.clone());
        guardar(env, &lista, &aportantes);
    }
    guardar(env, &clave, &(previo + monto));
    publicar(env, "vaca", cuenta, &vaca.creador, monto);
    Ok(())
}

mod test;
