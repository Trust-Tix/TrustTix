#![no_std]

//! Bóveda TrustTix: guarda XLM y los suelta a quien presente un código
//! promocional, una vez por cuenta. Sirve para premios, regalos o un bono de
//! bienvenida para el primer boleto.
//!
//! Reglas que hace cumplir la red:
//! 1. El código no se guarda en texto: solo su huella SHA-256. Se compara sin
//!    importar mayúsculas ni espacios al inicio o al final.
//! 2. Cada cuenta reclama una sola vez y firma su reclamo.
//! 3. Cada reclamo paga el mismo monto; si la bóveda no alcanza, no paga nada.
//! 4. Solo el administrador recupera lo que sobre.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Bytes,
    BytesN, Env, String,
};

/// Ledgers aproximados en un día (un ledger cada ~5 segundos).
const DIA: u32 = 17_280;
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;
/// Largo máximo del código, en bytes.
const LARGO_MAXIMO: u32 = 64;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    CodigoIncorrecto = 1,
    YaReclamo = 2,
    SinFondos = 3,
    MontoInvalido = 4,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Admin,
    Token,
    /// Huella SHA-256 del código normalizado.
    Codigo,
    /// Lo que recibe cada cuenta al reclamar.
    Monto,
    Reclamos,
    Reclamo(Address),
}

#[contractevent]
pub struct Deposito {
    #[topic]
    pub de: Address,
    pub cantidad: i128,
}

#[contractevent]
pub struct Reclamo {
    #[topic]
    pub cuenta: Address,
    pub monto: i128,
}

#[contractevent]
pub struct Retiro {
    pub admin: Address,
    pub cantidad: i128,
}

#[contract]
pub struct Boveda;

#[contractimpl]
impl Boveda {
    /// `token`: contrato del activo que se guarda (en Testnet, el de XLM).
    /// `codigo_hash`: SHA-256 del código en minúsculas y sin espacios a los lados.
    /// `monto`: lo que recibe cada cuenta que reclama.
    pub fn __constructor(
        env: Env,
        admin: Address,
        token: Address,
        codigo_hash: BytesN<32>,
        monto: i128,
    ) -> Result<(), Error> {
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        let datos = env.storage().instance();
        datos.set(&Clave::Admin, &admin);
        datos.set(&Clave::Token, &token);
        datos.set(&Clave::Codigo, &codigo_hash);
        datos.set(&Clave::Monto, &monto);
        datos.set(&Clave::Reclamos, &0u32);
        Ok(())
    }

    /// Cualquier cuenta mete XLM a la bóveda. Devuelve lo que queda disponible.
    pub fn depositar(env: Env, de: Address, cantidad: i128) -> Result<i128, Error> {
        de.require_auth();
        if cantidad <= 0 {
            return Err(Error::MontoInvalido);
        }
        token_de(&env).transfer(&de, env.current_contract_address(), &cantidad);
        extender(&env);
        Deposito { de, cantidad }.publish(&env);
        Ok(disponible(&env))
    }

    /// Paga el monto de la bóveda a `cuenta` si el código es correcto y esa
    /// cuenta no ha reclamado antes. Devuelve lo que recibió.
    pub fn reclamar(env: Env, cuenta: Address, codigo: String) -> Result<i128, Error> {
        cuenta.require_auth();
        let clave = Clave::Reclamo(cuenta.clone());
        if env.storage().persistent().has(&clave) {
            return Err(Error::YaReclamo);
        }
        let esperado: BytesN<32> = env.storage().instance().get(&Clave::Codigo).unwrap();
        if huella(&env, &codigo) != Some(esperado) {
            return Err(Error::CodigoIncorrecto);
        }
        let monto = monto(&env);
        if disponible(&env) < monto {
            return Err(Error::SinFondos);
        }

        token_de(&env).transfer(&env.current_contract_address(), &cuenta, &monto);
        env.storage().persistent().set(&clave, &monto);
        env.storage()
            .persistent()
            .extend_ttl(&clave, UMBRAL_TTL, EXTENDER_TTL);
        let reclamos: u32 = env.storage().instance().get(&Clave::Reclamos).unwrap_or(0);
        env.storage()
            .instance()
            .set(&Clave::Reclamos, &(reclamos + 1));
        extender(&env);

        Reclamo { cuenta, monto }.publish(&env);
        Ok(monto)
    }

    /// El administrador se lleva lo que quede en la bóveda. Devuelve cuánto retiró.
    pub fn retirar_sobrante(env: Env) -> i128 {
        let admin: Address = env.storage().instance().get(&Clave::Admin).unwrap();
        admin.require_auth();
        let cantidad = disponible(&env);
        if cantidad > 0 {
            token_de(&env).transfer(&env.current_contract_address(), &admin, &cantidad);
        }
        Retiro { admin, cantidad }.publish(&env);
        cantidad
    }

    // ---------- Consultas públicas ----------

    pub fn disponible(env: Env) -> i128 {
        disponible(&env)
    }

    pub fn monto(env: Env) -> i128 {
        monto(&env)
    }

    pub fn reclamos(env: Env) -> u32 {
        env.storage().instance().get(&Clave::Reclamos).unwrap_or(0)
    }

    pub fn ya_reclamo(env: Env, cuenta: Address) -> bool {
        env.storage().persistent().has(&Clave::Reclamo(cuenta))
    }

    pub fn admin(env: Env) -> Address {
        env.storage().instance().get(&Clave::Admin).unwrap()
    }
}

// ---------- Funciones internas ----------

fn token_de(env: &Env) -> token::Client<'_> {
    let id: Address = env.storage().instance().get(&Clave::Token).unwrap();
    token::Client::new(env, &id)
}

fn disponible(env: &Env) -> i128 {
    token_de(env).balance(&env.current_contract_address())
}

fn monto(env: &Env) -> i128 {
    env.storage().instance().get(&Clave::Monto).unwrap()
}

fn extender(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(UMBRAL_TTL, EXTENDER_TTL);
}

/// SHA-256 del código en minúsculas y sin espacios al inicio ni al final.
/// Un código vacío o demasiado largo no tiene huella.
fn huella(env: &Env, codigo: &String) -> Option<BytesN<32>> {
    let largo = codigo.len();
    if largo == 0 || largo > LARGO_MAXIMO {
        return None;
    }
    let mut texto = [0u8; LARGO_MAXIMO as usize];
    let texto = &mut texto[..largo as usize];
    codigo.copy_into_slice(texto);
    texto.make_ascii_lowercase();
    let inicio = texto.iter().position(|b| *b != b' ')?;
    let fin = texto.iter().rposition(|b| *b != b' ')? + 1;
    let limpio = Bytes::from_slice(env, &texto[inicio..fin]);
    Some(env.crypto().sha256(&limpio).to_bytes())
}

mod test;
