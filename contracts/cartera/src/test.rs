#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env, Vec};

const XLM: i128 = 10_000_000;

struct Prueba<'a> {
    env: Env,
    cartera: CarteraClient<'a>,
    token: TokenClient<'a>,
    barra: Address,
    comida: Address,
}

fn preparar<'a>() -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let emisor = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(emisor).address();
    let admin = Address::generate(&env);
    let id = env.register(Cartera, (token_id.clone(), admin));
    let cartera = CarteraClient::new(&env, &id);
    let barra = Address::generate(&env);
    let comida = Address::generate(&env);
    cartera.registrar_comercio(&barra);
    cartera.registrar_comercio(&comida);
    Prueba {
        token: TokenClient::new(&env, &token_id),
        env,
        cartera,
        barra,
        comida,
    }
}

fn con_xlm(p: &Prueba, monto: i128) -> Address {
    let cuenta = Address::generate(&p.env);
    StellarAssetClient::new(&p.env, &p.token.address).mint(&cuenta, &monto);
    cuenta
}

/// Una cuenta con `monto` ya recargado en la cartera.
fn con_saldo(p: &Prueba, monto: i128) -> Address {
    let cuenta = con_xlm(p, monto);
    p.cartera.recargar(&cuenta, &monto);
    cuenta
}

#[test]
fn recargar_pasa_xlm_al_saldo_del_evento() {
    let p = preparar();
    let fan = con_xlm(&p, 10 * XLM);
    assert_eq!(p.cartera.recargar(&fan, &(4 * XLM)), 4 * XLM);
    assert_eq!(p.env.auths()[0].0, fan);
    assert_eq!(p.token.balance(&fan), 6 * XLM);
    assert_eq!(p.token.balance(&p.cartera.address), 4 * XLM);
    assert_eq!(
        p.cartera.try_recargar(&fan, &0),
        Err(Ok(Error::MontoInvalido))
    );
}

#[test]
fn recargar_para_cobra_a_quien_paga_y_suma_al_otro() {
    let p = preparar();
    let mama = con_xlm(&p, 5 * XLM);
    let fan = Address::generate(&p.env);
    p.cartera.recargar_para(&mama, &fan, &(5 * XLM));
    assert_eq!(p.env.auths()[0].0, mama);
    assert_eq!(p.cartera.saldo(&fan), 5 * XLM);
    assert_eq!(p.token.balance(&mama), 0);
    assert_eq!(
        p.cartera.try_recargar_para(&mama, &mama, &XLM),
        Err(Ok(Error::MismaCuenta))
    );
}

#[test]
fn recargar_varios_carga_a_todo_el_grupo_en_una_transaccion() {
    let p = preparar();
    let quien_paga = con_xlm(&p, 6 * XLM);
    let a = Address::generate(&p.env);
    let b = Address::generate(&p.env);
    let c = Address::generate(&p.env);
    let grupo = Vec::from_array(&p.env, [a.clone(), b.clone(), c.clone()]);
    assert_eq!(
        p.cartera.recargar_varios(&quien_paga, &grupo, &(2 * XLM)),
        6 * XLM
    );
    assert_eq!(p.cartera.saldo(&a), 2 * XLM);
    assert_eq!(p.cartera.saldo(&c), 2 * XLM);
    assert_eq!(p.token.balance(&quien_paga), 0);
    assert_eq!(
        p.cartera
            .try_recargar_varios(&quien_paga, &Vec::new(&p.env), &XLM),
        Err(Ok(Error::CantidadInvalida))
    );
}

#[test]
fn retirar_regresa_parte_del_saldo_a_la_cuenta() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    assert_eq!(p.cartera.retirar(&fan, &(2 * XLM)), 3 * XLM);
    assert_eq!(p.token.balance(&fan), 2 * XLM);
    assert_eq!(
        p.cartera.try_retirar(&fan, &(4 * XLM)),
        Err(Ok(Error::SinSaldo))
    );
}

#[test]
fn retirar_todo_deja_el_saldo_en_cero() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    assert_eq!(p.cartera.retirar_todo(&fan), 5 * XLM);
    assert_eq!(p.cartera.saldo(&fan), 0);
    assert_eq!(p.token.balance(&fan), 5 * XLM);
    assert_eq!(
        p.cartera.try_retirar_todo(&fan),
        Err(Ok(Error::MontoInvalido))
    );
}

#[test]
fn transferir_saldo_mueve_saldo_entre_asistentes() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    let amiga = Address::generate(&p.env);
    p.cartera.transferir_saldo(&fan, &amiga, &(2 * XLM));
    assert_eq!(p.cartera.saldo(&fan), 3 * XLM);
    assert_eq!(p.cartera.saldo(&amiga), 2 * XLM);
    assert_eq!(
        p.cartera.try_transferir_saldo(&fan, &fan, &XLM),
        Err(Ok(Error::MismaCuenta))
    );
}

#[test]
fn pagar_solo_en_comercios_registrados() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    p.cartera.pagar(&fan, &p.barra, &(2 * XLM));
    assert_eq!(p.cartera.saldo(&fan), 3 * XLM);
    assert_eq!(p.cartera.ganancias(&p.barra), 2 * XLM);
    let pirata = Address::generate(&p.env);
    assert_eq!(
        p.cartera.try_pagar(&fan, &pirata, &XLM),
        Err(Ok(Error::NoEsComercio))
    );
    assert_eq!(
        p.cartera.try_pagar(&fan, &p.barra, &(4 * XLM)),
        Err(Ok(Error::SinSaldo))
    );
}

#[test]
fn pagar_con_propina_lleva_todo_al_comercio() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    assert_eq!(
        p.cartera
            .pagar_con_propina(&fan, &p.barra, &(3 * XLM), &XLM),
        4 * XLM
    );
    assert_eq!(p.cartera.ganancias(&p.barra), 4 * XLM);
    assert_eq!(p.cartera.saldo(&fan), XLM);
    assert_eq!(
        p.cartera.try_pagar_con_propina(&fan, &p.barra, &XLM, &0),
        Err(Ok(Error::MontoInvalido))
    );
}

#[test]
fn pagar_varios_paga_toda_la_cuenta_o_nada() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    let comercios = Vec::from_array(&p.env, [p.barra.clone(), p.comida.clone()]);
    let montos = Vec::from_array(&p.env, [2 * XLM, XLM]);
    assert_eq!(p.cartera.pagar_varios(&fan, &comercios, &montos), 3 * XLM);
    assert_eq!(p.cartera.ganancias(&p.comida), XLM);
    let demasiado = Vec::from_array(&p.env, [XLM, 5 * XLM]);
    assert_eq!(
        p.cartera.try_pagar_varios(&fan, &comercios, &demasiado),
        Err(Ok(Error::SinSaldo))
    );
    assert_eq!(p.cartera.saldo(&fan), 2 * XLM);
}

#[test]
fn recargar_y_pagar_cobra_directo_de_la_cuenta() {
    let p = preparar();
    let fan = con_xlm(&p, 3 * XLM);
    p.cartera.recargar_y_pagar(&fan, &p.comida, &(3 * XLM));
    assert_eq!(p.token.balance(&fan), 0);
    assert_eq!(p.cartera.saldo(&fan), 0);
    assert_eq!(p.cartera.ganancias(&p.comida), 3 * XLM);
}

#[test]
fn el_comercio_reembolsa_un_pago_al_saldo() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    p.cartera.pagar(&fan, &p.barra, &(3 * XLM));
    p.cartera.reembolsar_pago(&p.barra, &fan, &XLM);
    assert_eq!(p.env.auths()[0].0, p.barra);
    assert_eq!(p.cartera.saldo(&fan), 3 * XLM);
    assert_eq!(p.cartera.ganancias(&p.barra), 2 * XLM);
    assert_eq!(
        p.cartera.try_reembolsar_pago(&p.barra, &fan, &(3 * XLM)),
        Err(Ok(Error::SinSaldo))
    );
}

#[test]
fn preautorizar_aparta_saldo_para_el_comercio() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    p.cartera.preautorizar(&fan, &p.barra, &(4 * XLM));
    assert_eq!(p.cartera.saldo(&fan), XLM);
    assert_eq!(p.cartera.retenido(&fan, &p.barra), 4 * XLM);
    assert_eq!(
        p.cartera.try_pagar(&fan, &p.comida, &(2 * XLM)),
        Err(Ok(Error::SinSaldo))
    );
}

#[test]
fn capturar_cobra_lo_consumido_y_regresa_el_resto() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    p.cartera.preautorizar(&fan, &p.barra, &(4 * XLM));
    assert_eq!(p.cartera.capturar(&p.barra, &fan, &(3 * XLM)), XLM);
    assert_eq!(p.cartera.ganancias(&p.barra), 3 * XLM);
    assert_eq!(p.cartera.saldo(&fan), 2 * XLM);
    assert_eq!(
        p.cartera.try_capturar(&p.barra, &fan, &XLM),
        Err(Ok(Error::SinRetencion))
    );
}

#[test]
fn cancelar_preautorizacion_regresa_todo() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    p.cartera.preautorizar(&fan, &p.barra, &(4 * XLM));
    assert_eq!(p.cartera.cancelar_preautorizacion(&p.barra, &fan), 4 * XLM);
    assert_eq!(p.cartera.saldo(&fan), 5 * XLM);
    assert_eq!(p.cartera.retenido(&fan, &p.barra), 0);
}

#[test]
fn cobrar_paga_al_comercio_todo_lo_cobrado() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    p.cartera.pagar(&fan, &p.barra, &(3 * XLM));
    assert_eq!(p.cartera.cobrar(&p.barra), 3 * XLM);
    assert_eq!(p.token.balance(&p.barra), 3 * XLM);
    assert_eq!(
        p.cartera.try_cobrar(&p.barra),
        Err(Ok(Error::MontoInvalido))
    );
}

#[test]
fn cobrar_parcial_deja_el_resto_en_la_cartera() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    p.cartera.pagar(&fan, &p.barra, &(3 * XLM));
    assert_eq!(p.cartera.cobrar_parcial(&p.barra, &XLM), 2 * XLM);
    assert_eq!(p.token.balance(&p.barra), XLM);
}

#[test]
fn liquidar_comercios_paga_a_todos_al_cierre() {
    let p = preparar();
    let fan = con_saldo(&p, 5 * XLM);
    p.cartera.pagar(&fan, &p.barra, &(3 * XLM));
    p.cartera.pagar(&fan, &p.comida, &XLM);
    let comercios = Vec::from_array(&p.env, [p.barra.clone(), p.comida.clone()]);
    assert_eq!(p.cartera.liquidar_comercios(&comercios), 4 * XLM);
    assert_eq!(p.token.balance(&p.barra), 3 * XLM);
    assert_eq!(p.token.balance(&p.comida), XLM);
    assert_eq!(p.cartera.ganancias(&p.barra), 0);
}

#[test]
fn devolver_saldos_regresa_lo_que_no_se_gasto() {
    let p = preparar();
    let a = con_saldo(&p, 2 * XLM);
    let b = con_saldo(&p, 3 * XLM);
    p.cartera.pagar(&b, &p.barra, &XLM);
    let cuentas = Vec::from_array(&p.env, [a.clone(), b.clone()]);
    assert_eq!(p.cartera.devolver_saldos(&cuentas), 4 * XLM);
    assert_eq!(p.token.balance(&a), 2 * XLM);
    assert_eq!(p.token.balance(&b), 2 * XLM);
    assert_eq!(p.cartera.saldo(&b), 0);
}

#[test]
fn crear_vaca_abre_el_fondo_con_el_primer_aporte() {
    let p = preparar();
    let ana = con_saldo(&p, 5 * XLM);
    let vaca = p.cartera.crear_vaca(&ana, &(2 * XLM));
    let datos = p.cartera.consultar_vaca(&vaca);
    assert_eq!(datos.saldo, 2 * XLM);
    assert_eq!(datos.creador, ana);
    assert_eq!(p.cartera.saldo(&ana), 3 * XLM);
}

#[test]
fn aportar_vaca_suma_saldo_de_otros() {
    let p = preparar();
    let ana = con_saldo(&p, 5 * XLM);
    let luis = con_saldo(&p, 5 * XLM);
    let vaca = p.cartera.crear_vaca(&ana, &(2 * XLM));
    p.cartera.aportar_vaca(&luis, &vaca, &(3 * XLM));
    assert_eq!(p.cartera.consultar_vaca(&vaca).saldo, 5 * XLM);
    assert_eq!(p.cartera.saldo(&luis), 2 * XLM);
}

#[test]
fn retirar_aporte_solo_antes_de_gastar() {
    let p = preparar();
    let ana = con_saldo(&p, 5 * XLM);
    let luis = con_saldo(&p, 5 * XLM);
    let vaca = p.cartera.crear_vaca(&ana, &(2 * XLM));
    p.cartera.aportar_vaca(&luis, &vaca, &(3 * XLM));
    assert_eq!(p.cartera.retirar_aporte(&luis, &vaca), 3 * XLM);
    assert_eq!(p.cartera.saldo(&luis), 5 * XLM);
    p.cartera.pagar_con_vaca(&vaca, &p.barra, &XLM);
    assert_eq!(
        p.cartera.try_retirar_aporte(&ana, &vaca),
        Err(Ok(Error::VacaUsada))
    );
}

#[test]
fn pagar_con_vaca_lo_firma_quien_la_creo() {
    let p = preparar();
    let ana = con_saldo(&p, 5 * XLM);
    let vaca = p.cartera.crear_vaca(&ana, &(4 * XLM));
    assert_eq!(p.cartera.pagar_con_vaca(&vaca, &p.barra, &(3 * XLM)), XLM);
    assert_eq!(p.env.auths()[0].0, ana);
    assert_eq!(p.cartera.ganancias(&p.barra), 3 * XLM);
    assert_eq!(
        p.cartera.try_pagar_con_vaca(&vaca, &p.barra, &(2 * XLM)),
        Err(Ok(Error::SinSaldo))
    );
}

#[test]
fn disolver_vaca_reparte_lo_que_queda_en_proporcion() {
    let p = preparar();
    let ana = con_saldo(&p, 5 * XLM);
    let luis = con_saldo(&p, 5 * XLM);
    let vaca = p.cartera.crear_vaca(&ana, &(2 * XLM));
    p.cartera.aportar_vaca(&luis, &vaca, &(2 * XLM));
    p.cartera.pagar_con_vaca(&vaca, &p.barra, &(2 * XLM));
    assert_eq!(p.cartera.disolver_vaca(&vaca), 2 * XLM);
    assert_eq!(p.cartera.saldo(&ana), 4 * XLM);
    assert_eq!(p.cartera.saldo(&luis), 4 * XLM);
    assert!(!p.cartera.consultar_vaca(&vaca).abierta);
    assert_eq!(
        p.cartera.try_disolver_vaca(&vaca),
        Err(Ok(Error::VacaCerrada))
    );
}
