#![no_std]

//! Abonos TrustTix: abonos de temporada, pagos en mensualidades, alcancía
//! para el próximo evento y club de fans con membresía.
//!
//! Reglas que hace cumplir la red:
//! 1. Un abono da un número fijo de entradas; solo el organizador las marca
//!    como usadas y, si lo reembolsa, devuelve la parte no usada.
//! 2. En un plan de pagos, cada cuota va directo al organizador y el plan se
//!    completa cuando se pagó el total.
//! 3. El dinero de una alcancía es de su dueño: solo él lo retira o lo gasta.
//! 4. La membresía del club dura 30 días por cada cuota pagada.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env, Symbol,
};

const DIA: u32 = 17_280;
const UMBRAL_TTL: u32 = 7 * DIA;
const EXTENDER_TTL: u32 = 30 * DIA;
/// Duración de la membresía por cada cuota: 30 días.
const MES: u64 = 30 * 24 * 60 * 60;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NoExiste = 1,
    MontoInvalido = 2,
    SinAbono = 3,
    SinUsos = 4,
    PlanCompleto = 5,
    SinFondos = 6,
    MismaCuenta = 7,
    CuotasInvalidas = 8,
}

/// Abono que ofrece un organizador: precio y entradas que incluye.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Abono {
    pub organizador: Address,
    pub precio: i128,
    pub usos: u32,
}

/// Plan de pagos de una compra en mensualidades.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    pub comprador: Address,
    pub organizador: Address,
    pub total: i128,
    pub pagado: i128,
    pub cuota: i128,
    pub cancelado: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alcancia {
    pub dueno: Address,
    pub saldo: i128,
    pub meta: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Club {
    pub organizador: Address,
    pub cuota: i128,
}

#[contracttype]
#[derive(Clone)]
enum Clave {
    Token,
    Total(Symbol),
    Abono(u32),
    Usos(u32, Address),
    Plan(u32),
    Alcancia(u32),
    Club(u32),
    Miembro(u32, Address),
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
pub struct Abonos;

#[contractimpl]
impl Abonos {
    pub fn __constructor(env: Env, token: Address) {
        env.storage().instance().set(&Clave::Token, &token);
    }

    // ---------- Abonos de temporada ----------

    /// El organizador ofrece un abono: precio y número de entradas.
    pub fn crear_abono(
        env: Env,
        organizador: Address,
        precio: i128,
        usos: u32,
    ) -> Result<u32, Error> {
        organizador.require_auth();
        if precio <= 0 || usos == 0 {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "abono");
        guardar(
            &env,
            &Clave::Abono(id),
            &Abono {
                organizador,
                precio,
                usos,
            },
        );
        Ok(id)
    }

    /// Compra un abono: paga al organizador y recibe sus entradas.
    /// Devuelve las entradas disponibles.
    pub fn comprar_abono(env: Env, cuenta: Address, abono_id: u32) -> Result<u32, Error> {
        cuenta.require_auth();
        vender_abono(&env, &cuenta, &cuenta, abono_id)
    }

    /// Compra un abono de regalo: paga `pagador` y las entradas son de `cuenta`.
    pub fn comprar_abono_para(
        env: Env,
        pagador: Address,
        cuenta: Address,
        abono_id: u32,
    ) -> Result<u32, Error> {
        pagador.require_auth();
        if pagador == cuenta {
            return Err(Error::MismaCuenta);
        }
        vender_abono(&env, &pagador, &cuenta, abono_id)
    }

    /// Quien ya tiene el abono paga otra vez y suma sus entradas.
    pub fn renovar_abono(env: Env, cuenta: Address, abono_id: u32) -> Result<u32, Error> {
        cuenta.require_auth();
        if usos(&env, abono_id, &cuenta) == 0 {
            return Err(Error::SinAbono);
        }
        vender_abono(&env, &cuenta, &cuenta, abono_id)
    }

    /// El organizador marca una entrada del abono como usada en la puerta.
    /// Devuelve las que quedan.
    pub fn usar_abono(env: Env, abono_id: u32, cuenta: Address) -> Result<u32, Error> {
        let abono = leer_abono(&env, abono_id)?;
        abono.organizador.require_auth();
        let quedan = usos(&env, abono_id, &cuenta);
        if quedan == 0 {
            return Err(Error::SinUsos);
        }
        guardar(&env, &Clave::Usos(abono_id, cuenta.clone()), &(quedan - 1));
        publicar(&env, "entrada", &abono.organizador, &cuenta, 0);
        Ok(quedan - 1)
    }

    /// Pasa las entradas que quedan de un abono a otra cuenta.
    pub fn transferir_abono(
        env: Env,
        de: Address,
        a: Address,
        abono_id: u32,
    ) -> Result<u32, Error> {
        de.require_auth();
        if de == a {
            return Err(Error::MismaCuenta);
        }
        let quedan = usos(&env, abono_id, &de);
        if quedan == 0 {
            return Err(Error::SinAbono);
        }
        env.storage()
            .persistent()
            .remove(&Clave::Usos(abono_id, de.clone()));
        let total = usos(&env, abono_id, &a) + quedan;
        guardar(&env, &Clave::Usos(abono_id, a.clone()), &total);
        publicar(&env, "traspaso", &de, &a, quedan as i128);
        Ok(total)
    }

    /// El organizador devuelve la parte proporcional de las entradas que no
    /// se usaron y el abono queda sin entradas. Devuelve el monto.
    pub fn reembolsar_abono(env: Env, abono_id: u32, cuenta: Address) -> Result<i128, Error> {
        let abono = leer_abono(&env, abono_id)?;
        abono.organizador.require_auth();
        let quedan = usos(&env, abono_id, &cuenta);
        if quedan == 0 {
            return Err(Error::SinAbono);
        }
        let monto = abono.precio * quedan as i128 / abono.usos as i128;
        token(&env).transfer(&abono.organizador, &cuenta, &monto);
        env.storage()
            .persistent()
            .remove(&Clave::Usos(abono_id, cuenta.clone()));
        publicar(&env, "reembolso", &abono.organizador, &cuenta, monto);
        Ok(monto)
    }

    // ---------- Pagos en mensualidades ----------

    /// Empieza a pagar una compra en `cuotas` partes iguales; la primera se
    /// paga en este momento. Devuelve el número de plan.
    pub fn iniciar_plan(
        env: Env,
        comprador: Address,
        organizador: Address,
        total: i128,
        cuotas: u32,
    ) -> Result<u32, Error> {
        comprador.require_auth();
        if total <= 0 {
            return Err(Error::MontoInvalido);
        }
        if !(2..=12).contains(&cuotas) {
            return Err(Error::CuotasInvalidas);
        }
        if comprador == organizador {
            return Err(Error::MismaCuenta);
        }
        let id = siguiente(&env, "plan");
        let mut plan = Plan {
            comprador: comprador.clone(),
            organizador,
            total,
            pagado: 0,
            cuota: total / cuotas as i128,
            cancelado: false,
        };
        let primera = plan.cuota;
        abonar(&env, id, &mut plan, &comprador, primera)?;
        Ok(id)
    }

    /// El comprador paga la siguiente cuota. Devuelve lo que falta.
    pub fn pagar_cuota(env: Env, plan_id: u32) -> Result<i128, Error> {
        let mut plan = leer_plan(&env, plan_id)?;
        plan.comprador.require_auth();
        let comprador = plan.comprador.clone();
        let monto = plan.cuota.min(plan.total - plan.pagado);
        abonar(&env, plan_id, &mut plan, &comprador, monto)?;
        Ok(plan.total - plan.pagado)
    }

    /// Otra persona paga la siguiente cuota del plan de alguien más.
    pub fn pagar_cuota_por(env: Env, pagador: Address, plan_id: u32) -> Result<i128, Error> {
        pagador.require_auth();
        let mut plan = leer_plan(&env, plan_id)?;
        let monto = plan.cuota.min(plan.total - plan.pagado);
        abonar(&env, plan_id, &mut plan, &pagador, monto)?;
        Ok(plan.total - plan.pagado)
    }

    /// El comprador liquida de una vez todo lo que falta.
    pub fn pagar_plan_completo(env: Env, plan_id: u32) -> Result<i128, Error> {
        let mut plan = leer_plan(&env, plan_id)?;
        plan.comprador.require_auth();
        let comprador = plan.comprador.clone();
        let monto = plan.total - plan.pagado;
        abonar(&env, plan_id, &mut plan, &comprador, monto)?;
        Ok(monto)
    }

    /// El organizador cancela el plan y devuelve todo lo que se había pagado.
    pub fn cancelar_plan(env: Env, plan_id: u32) -> Result<i128, Error> {
        let mut plan = leer_plan(&env, plan_id)?;
        plan.organizador.require_auth();
        if plan.cancelado || plan.pagado >= plan.total {
            return Err(Error::PlanCompleto);
        }
        if plan.pagado > 0 {
            token(&env).transfer(&plan.organizador, &plan.comprador, &plan.pagado);
            publicar(
                &env,
                "reembolso",
                &plan.organizador,
                &plan.comprador,
                plan.pagado,
            );
        }
        let devuelto = plan.pagado;
        plan.pagado = 0;
        plan.cancelado = true;
        guardar(&env, &Clave::Plan(plan_id), &plan);
        Ok(devuelto)
    }

    // ---------- Alcancía ----------

    /// Abre una alcancía para el próximo evento con un primer depósito y una
    /// meta. El dinero queda en el contrato.
    pub fn abrir_alcancia(env: Env, dueno: Address, meta: i128, monto: i128) -> Result<u32, Error> {
        dueno.require_auth();
        if meta <= 0 {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "alcancia");
        let mut alcancia = Alcancia {
            dueno: dueno.clone(),
            saldo: 0,
            meta,
        };
        depositar(&env, id, &mut alcancia, &dueno, monto)?;
        Ok(id)
    }

    /// El dueño ahorra más en su alcancía. Devuelve el saldo.
    pub fn ahorrar(env: Env, alcancia_id: u32, monto: i128) -> Result<i128, Error> {
        let mut alcancia = leer_alcancia(&env, alcancia_id)?;
        alcancia.dueno.require_auth();
        let dueno = alcancia.dueno.clone();
        depositar(&env, alcancia_id, &mut alcancia, &dueno, monto)?;
        Ok(alcancia.saldo)
    }

    /// Alguien más deposita en la alcancía de otra persona.
    pub fn ahorrar_para(
        env: Env,
        pagador: Address,
        alcancia_id: u32,
        monto: i128,
    ) -> Result<i128, Error> {
        pagador.require_auth();
        let mut alcancia = leer_alcancia(&env, alcancia_id)?;
        depositar(&env, alcancia_id, &mut alcancia, &pagador, monto)?;
        Ok(alcancia.saldo)
    }

    /// El dueño saca todo el dinero de su alcancía.
    pub fn retirar_alcancia(env: Env, alcancia_id: u32) -> Result<i128, Error> {
        let mut alcancia = leer_alcancia(&env, alcancia_id)?;
        alcancia.dueno.require_auth();
        let monto = alcancia.saldo;
        if monto <= 0 {
            return Err(Error::SinFondos);
        }
        enviar(&env, &alcancia.dueno, monto);
        alcancia.saldo = 0;
        guardar(&env, &Clave::Alcancia(alcancia_id), &alcancia);
        publicar(
            &env,
            "retiro",
            &env.current_contract_address(),
            &alcancia.dueno,
            monto,
        );
        Ok(monto)
    }

    /// El dueño paga a un organizador directo desde su alcancía.
    pub fn pagar_con_alcancia(
        env: Env,
        alcancia_id: u32,
        organizador: Address,
        monto: i128,
    ) -> Result<i128, Error> {
        let mut alcancia = leer_alcancia(&env, alcancia_id)?;
        alcancia.dueno.require_auth();
        if monto <= 0 || monto > alcancia.saldo {
            return Err(Error::SinFondos);
        }
        enviar(&env, &organizador, monto);
        alcancia.saldo -= monto;
        guardar(&env, &Clave::Alcancia(alcancia_id), &alcancia);
        publicar(&env, "pago", &alcancia.dueno, &organizador, monto);
        Ok(alcancia.saldo)
    }

    // ---------- Club de fans ----------

    /// El organizador abre un club de fans con una cuota mensual.
    pub fn crear_club(env: Env, organizador: Address, cuota: i128) -> Result<u32, Error> {
        organizador.require_auth();
        if cuota <= 0 {
            return Err(Error::MontoInvalido);
        }
        let id = siguiente(&env, "club");
        guardar(&env, &Clave::Club(id), &Club { organizador, cuota });
        Ok(id)
    }

    /// Paga la cuota y entra al club por 30 días. Devuelve cuándo vence.
    pub fn unirse_club(env: Env, cuenta: Address, club_id: u32) -> Result<u64, Error> {
        cuenta.require_auth();
        cobrar_membresia(&env, club_id, &cuenta, &cuenta)
    }

    /// Paga otra cuota y suma 30 días a una membresía. Si ya venció, cuenta
    /// desde hoy.
    pub fn renovar_membresia(env: Env, cuenta: Address, club_id: u32) -> Result<u64, Error> {
        cuenta.require_auth();
        if vence(&env, club_id, &cuenta) == 0 {
            return Err(Error::NoExiste);
        }
        cobrar_membresia(&env, club_id, &cuenta, &cuenta)
    }

    /// Regala un mes de membresía: paga `pagador` y el mes es de `cuenta`.
    pub fn regalar_membresia(
        env: Env,
        pagador: Address,
        cuenta: Address,
        club_id: u32,
    ) -> Result<u64, Error> {
        pagador.require_auth();
        if pagador == cuenta {
            return Err(Error::MismaCuenta);
        }
        cobrar_membresia(&env, club_id, &pagador, &cuenta)
    }

    // ---------- Consultas ----------

    pub fn entradas(env: Env, abono_id: u32, cuenta: Address) -> u32 {
        usos(&env, abono_id, &cuenta)
    }

    pub fn consultar_plan(env: Env, plan_id: u32) -> Result<Plan, Error> {
        leer_plan(&env, plan_id)
    }

    pub fn consultar_alcancia(env: Env, alcancia_id: u32) -> Result<Alcancia, Error> {
        leer_alcancia(&env, alcancia_id)
    }

    pub fn membresia_vence(env: Env, club_id: u32, cuenta: Address) -> u64 {
        vence(&env, club_id, &cuenta)
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

fn leer_abono(env: &Env, id: u32) -> Result<Abono, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Abono(id))
        .ok_or(Error::NoExiste)
}

fn usos(env: &Env, abono_id: u32, cuenta: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&Clave::Usos(abono_id, cuenta.clone()))
        .unwrap_or(0)
}

/// Cobra el abono a `pagador`, lo paga al organizador y suma las entradas a `cuenta`.
fn vender_abono(
    env: &Env,
    pagador: &Address,
    cuenta: &Address,
    abono_id: u32,
) -> Result<u32, Error> {
    let abono = leer_abono(env, abono_id)?;
    token(env).transfer(pagador, &abono.organizador, &abono.precio);
    let total = usos(env, abono_id, cuenta) + abono.usos;
    guardar(env, &Clave::Usos(abono_id, cuenta.clone()), &total);
    publicar(env, "abono", pagador, &abono.organizador, abono.precio);
    Ok(total)
}

fn leer_plan(env: &Env, id: u32) -> Result<Plan, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Plan(id))
        .ok_or(Error::NoExiste)
}

/// Paga `monto` de un plan al organizador desde `pagador`.
fn abonar(
    env: &Env,
    plan_id: u32,
    plan: &mut Plan,
    pagador: &Address,
    monto: i128,
) -> Result<(), Error> {
    if plan.cancelado || plan.pagado >= plan.total {
        return Err(Error::PlanCompleto);
    }
    if monto <= 0 {
        return Err(Error::MontoInvalido);
    }
    token(env).transfer(pagador, &plan.organizador, &monto);
    plan.pagado += monto;
    guardar(env, &Clave::Plan(plan_id), plan);
    publicar(env, "cuota", pagador, &plan.organizador, monto);
    Ok(())
}

fn leer_alcancia(env: &Env, id: u32) -> Result<Alcancia, Error> {
    env.storage()
        .persistent()
        .get(&Clave::Alcancia(id))
        .ok_or(Error::NoExiste)
}

fn depositar(
    env: &Env,
    id: u32,
    alcancia: &mut Alcancia,
    de: &Address,
    monto: i128,
) -> Result<(), Error> {
    if monto <= 0 {
        return Err(Error::MontoInvalido);
    }
    token(env).transfer(de, env.current_contract_address(), &monto);
    alcancia.saldo += monto;
    guardar(env, &Clave::Alcancia(id), alcancia);
    publicar(env, "ahorro", de, &alcancia.dueno, monto);
    Ok(())
}

fn vence(env: &Env, club_id: u32, cuenta: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&Clave::Miembro(club_id, cuenta.clone()))
        .unwrap_or(0)
}

/// Cobra una cuota del club a `pagador` y suma 30 días a la membresía de `cuenta`.
fn cobrar_membresia(
    env: &Env,
    club_id: u32,
    pagador: &Address,
    cuenta: &Address,
) -> Result<u64, Error> {
    let club: Club = env
        .storage()
        .persistent()
        .get(&Clave::Club(club_id))
        .ok_or(Error::NoExiste)?;
    token(env).transfer(pagador, &club.organizador, &club.cuota);
    let ahora = env.ledger().timestamp();
    let desde = vence(env, club_id, cuenta).max(ahora);
    let nuevo = desde + MES;
    guardar(env, &Clave::Miembro(club_id, cuenta.clone()), &nuevo);
    publicar(env, "club", pagador, &club.organizador, club.cuota);
    Ok(nuevo)
}

mod test;
