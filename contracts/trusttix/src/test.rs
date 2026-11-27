#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Bytes, Env, String};

const PRECIO: i128 = 250_000_000; // 25 XLM
/// Comisiones de TrustTix en puntos base: 3 % por compra y 2 % por reventa.
const COMPRA: u32 = 300;
const REVENTA: u32 = 200;

struct Prueba<'a> {
    env: Env,
    tix: TrustTixClient<'a>,
    token: TokenClient<'a>,
    organizador: Address,
    admin: Address,
}

fn preparar<'a>() -> Prueba<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let emisor = Address::generate(&env);
    let activo = env.register_stellar_asset_contract_v2(emisor);
    let token_id = activo.address();

    let admin = Address::generate(&env);
    let tix_id = env.register(TrustTix, (token_id.clone(), admin.clone(), COMPRA, REVENTA));
    let tix = TrustTixClient::new(&env, &tix_id);
    let token = TokenClient::new(&env, &token_id);
    let organizador = Address::generate(&env);

    Prueba {
        env,
        tix,
        token,
        organizador,
        admin,
    }
}

fn texto(env: &Env, t: &str) -> String {
    String::from_str(env, t)
}

fn crear(p: &Prueba, capacidad: u32, precio: i128) -> u32 {
    p.tix.crear_evento(
        &p.organizador,
        &texto(&p.env, "El Rey León"),
        &texto(&p.env, "Teatro Telcel"),
        &texto(&p.env, "2026-12-12T20:00"),
        &capacidad,
        &precio,
    )
}

fn con_saldo(p: &Prueba, cantidad: i128) -> Address {
    let cuenta = Address::generate(&p.env);
    StellarAssetClient::new(&p.env, &p.token.address).mint(&cuenta, &cantidad);
    cuenta
}

#[test]
fn comprar_cobra_paga_al_organizador_y_emite_el_boleto() {
    let p = preparar();
    let evento = crear(&p, 10, PRECIO);
    let comprador = con_saldo(&p, 1_000_000_000);

    let boleto = p.tix.comprar(&comprador, &evento);

    assert_eq!(boleto, 1);
    // 3 % para TrustTix y el resto para el organizador.
    assert_eq!(p.token.balance(&comprador), 1_000_000_000 - PRECIO);
    assert_eq!(p.token.balance(&p.admin), 7_500_000);
    assert_eq!(p.token.balance(&p.organizador), PRECIO - 7_500_000);

    let b = p.tix.consultar_boleto(&boleto);
    assert_eq!(b.dueno, comprador);
    assert_eq!(b.evento, evento);
    assert!(!b.usado);
    assert_eq!(p.tix.consultar_evento(&evento).vendidos, 1);
    assert_eq!(p.tix.boletos_de(&comprador).len(), 1);
}

#[test]
fn no_se_vende_mas_que_la_capacidad() {
    let p = preparar();
    let evento = crear(&p, 2, 0);
    let a = Address::generate(&p.env);
    let b = Address::generate(&p.env);
    let c = Address::generate(&p.env);

    p.tix.comprar(&a, &evento);
    p.tix.comprar(&b, &evento);
    let tercera = p.tix.try_comprar(&c, &evento);

    assert_eq!(tercera, Err(Ok(Error::Agotado)));
    assert_eq!(p.tix.consultar_evento(&evento).vendidos, 2);
}

#[test]
fn sin_saldo_no_hay_boleto() {
    let p = preparar();
    let evento = crear(&p, 5, PRECIO);
    let sin_dinero = Address::generate(&p.env);

    assert!(p.tix.try_comprar(&sin_dinero, &evento).is_err());
    assert_eq!(p.tix.consultar_evento(&evento).vendidos, 0);
    assert_eq!(p.tix.total_boletos(), 0);
}

#[test]
fn solo_el_dueno_transfiere() {
    let p = preparar();
    let evento = crear(&p, 5, 0);
    let dueno = Address::generate(&p.env);
    let amigo = Address::generate(&p.env);
    let extrano = Address::generate(&p.env);
    let boleto = p.tix.comprar(&dueno, &evento);

    let intento = p.tix.try_transferir(&boleto, &extrano, &amigo);
    assert_eq!(intento, Err(Ok(Error::NoEsDueno)));

    p.tix.transferir(&boleto, &dueno, &amigo);
    assert_eq!(p.tix.consultar_boleto(&boleto).dueno, amigo);
    assert_eq!(p.tix.boletos_de(&dueno).len(), 0);
    assert_eq!(p.tix.boletos_de(&amigo).len(), 1);

    // El dueño anterior ya no puede moverlo.
    let otra_vez = p.tix.try_transferir(&boleto, &dueno, &extrano);
    assert_eq!(otra_vez, Err(Ok(Error::NoEsDueno)));
}

#[test]
fn una_sola_entrada_y_solo_personal_autorizado() {
    let p = preparar();
    let evento = crear(&p, 5, 0);
    let asistente = Address::generate(&p.env);
    let personal = Address::generate(&p.env);
    let colado = Address::generate(&p.env);
    let boleto = p.tix.comprar(&asistente, &evento);

    let sin_permiso = p.tix.try_registrar_entrada(&boleto, &colado);
    assert_eq!(sin_permiso, Err(Ok(Error::NoAutorizado)));

    p.tix.autorizar_personal(&evento, &personal);
    assert!(p.tix.es_personal(&evento, &personal));
    p.tix.registrar_entrada(&boleto, &personal);
    assert!(p.tix.consultar_boleto(&boleto).usado);

    let segunda = p.tix.try_registrar_entrada(&boleto, &personal);
    assert_eq!(segunda, Err(Ok(Error::YaUsado)));

    // Un boleto usado ya no se puede transferir.
    let amigo = Address::generate(&p.env);
    let transferir = p.tix.try_transferir(&boleto, &asistente, &amigo);
    assert_eq!(transferir, Err(Ok(Error::YaUsado)));
}

#[test]
fn el_organizador_puede_registrar_entradas_y_quitar_personal() {
    let p = preparar();
    let evento = crear(&p, 5, 0);
    let personal = Address::generate(&p.env);
    p.tix.autorizar_personal(&evento, &personal);
    p.tix.quitar_personal(&evento, &personal);
    assert!(!p.tix.es_personal(&evento, &personal));

    let asistente = Address::generate(&p.env);
    let boleto = p.tix.comprar(&asistente, &evento);
    p.tix.registrar_entrada(&boleto, &p.organizador);
    assert!(p.tix.consultar_boleto(&boleto).usado);
}

#[test]
fn la_comision_se_redondea_a_favor_del_organizador() {
    let p = preparar();
    // 3 % de 1,001 stroops son 30.03: TrustTix cobra 30 y el organizador recibe 971.
    let evento = crear(&p, 5, 1_001);
    let comprador = con_saldo(&p, 10_000);

    p.tix.comprar(&comprador, &evento);

    assert_eq!(p.token.balance(&p.admin), 30);
    assert_eq!(p.token.balance(&p.organizador), 971);
}

#[test]
fn un_evento_gratis_no_cobra_comision() {
    let p = preparar();
    let evento = crear(&p, 5, 0);
    let asistente = Address::generate(&p.env);

    p.tix.comprar(&asistente, &evento);

    assert_eq!(p.token.balance(&p.admin), 0);
    assert_eq!(p.tix.consultar_evento(&evento).vendidos, 1);
}

#[test]
fn el_administrador_cambia_la_comision_y_firma_el_cambio() {
    let p = preparar();
    assert_eq!(
        p.tix.comisiones(),
        Comisiones {
            compra: COMPRA,
            reventa: REVENTA
        }
    );
    assert_eq!(p.tix.admin(), p.admin);

    p.tix.configurar_comision(&0, &150);

    // El cambio quedó firmado por la cuenta de TrustTix, no por cualquiera.
    let firmas = p.env.auths();
    assert_eq!(firmas.len(), 1);
    assert_eq!(firmas[0].0, p.admin);
    assert_eq!(
        p.tix.comisiones(),
        Comisiones {
            compra: 0,
            reventa: 150
        }
    );

    // Con comisión 0, el organizador recibe el precio completo.
    let evento = crear(&p, 5, PRECIO);
    let comprador = con_saldo(&p, PRECIO);
    p.tix.comprar(&comprador, &evento);
    assert_eq!(p.token.balance(&p.organizador), PRECIO);
    assert_eq!(p.token.balance(&p.admin), 0);
}

#[test]
fn ninguna_comision_pasa_del_diez_por_ciento() {
    let p = preparar();
    assert_eq!(
        p.tix.try_configurar_comision(&1_001, &200),
        Err(Ok(Error::ComisionInvalida))
    );
    assert_eq!(
        p.tix.try_configurar_comision(&300, &1_001),
        Err(Ok(Error::ComisionInvalida))
    );
    assert!(p.tix.try_configurar_comision(&1_000, &1_000).is_ok());
}

#[test]
#[should_panic]
fn no_se_puede_desplegar_con_una_comision_excesiva() {
    let env = Env::default();
    let token = Address::generate(&env);
    let admin = Address::generate(&env);
    env.register(TrustTix, (token, admin, 1_500u32, 200u32));
}

// ---------- Reventa ----------

/// Un evento de 25 XLM y un boleto comprado por `vendedor`.
fn boleto_para_revender(p: &Prueba) -> (u32, Address) {
    let evento = crear(p, 10, PRECIO);
    let vendedor = con_saldo(p, PRECIO);
    let boleto = p.tix.comprar(&vendedor, &evento);
    (boleto, vendedor)
}

#[test]
fn la_reventa_paga_al_vendedor_cobra_2_por_ciento_y_cambia_el_dueno() {
    let p = preparar();
    let (boleto, vendedor) = boleto_para_revender(&p);
    let comision_compra = p.token.balance(&p.admin);
    let reventa = 270_000_000; // 27 XLM, dentro del tope de 27.5
    let comprador = con_saldo(&p, reventa);

    p.tix.publicar_reventa(&boleto, &vendedor, &reventa);
    assert_eq!(p.tix.consultar_reventa(&boleto), reventa);
    p.tix.comprar_reventa(&boleto, &comprador);

    // 2 % de 27 XLM para TrustTix y el resto para quien revendió.
    assert_eq!(p.token.balance(&p.admin) - comision_compra, 5_400_000);
    assert_eq!(p.token.balance(&vendedor), reventa - 5_400_000);
    assert_eq!(p.token.balance(&comprador), 0);

    assert_eq!(p.tix.consultar_boleto(&boleto).dueno, comprador);
    assert_eq!(p.tix.boletos_de(&vendedor).len(), 0);
    assert_eq!(p.tix.boletos_de(&comprador).len(), 1);
    // La oferta desaparece: no se puede comprar dos veces.
    assert_eq!(
        p.tix.try_consultar_reventa(&boleto),
        Err(Ok(Error::NoEnReventa))
    );
    let otro = con_saldo(&p, reventa);
    assert_eq!(
        p.tix.try_comprar_reventa(&boleto, &otro),
        Err(Ok(Error::NoEnReventa))
    );
}

#[test]
fn nadie_revende_arriba_del_tope() {
    let p = preparar();
    let (boleto, vendedor) = boleto_para_revender(&p);

    // El tope inicial es 110 % del precio: 27.5 XLM.
    let caro = p.tix.try_publicar_reventa(&boleto, &vendedor, &275_000_001);
    assert_eq!(caro, Err(Ok(Error::SobreTope)));
    assert!(p
        .tix
        .try_publicar_reventa(&boleto, &vendedor, &275_000_000)
        .is_ok());

    let negativo = p.tix.try_publicar_reventa(&boleto, &vendedor, &-1);
    assert_eq!(negativo, Err(Ok(Error::PrecioInvalido)));
}

#[test]
fn solo_el_dueno_publica_o_cancela_la_reventa() {
    let p = preparar();
    let (boleto, vendedor) = boleto_para_revender(&p);
    let extrano = Address::generate(&p.env);

    let ajeno = p.tix.try_publicar_reventa(&boleto, &extrano, &PRECIO);
    assert_eq!(ajeno, Err(Ok(Error::NoEsDueno)));

    p.tix.publicar_reventa(&boleto, &vendedor, &PRECIO);
    let cancelar_ajeno = p.tix.try_cancelar_reventa(&boleto, &extrano);
    assert_eq!(cancelar_ajeno, Err(Ok(Error::NoEsDueno)));

    p.tix.cancelar_reventa(&boleto, &vendedor);
    assert_eq!(
        p.tix.try_consultar_reventa(&boleto),
        Err(Ok(Error::NoEnReventa))
    );
    let dos_veces = p.tix.try_cancelar_reventa(&boleto, &vendedor);
    assert_eq!(dos_veces, Err(Ok(Error::NoEnReventa)));
}

#[test]
fn transferir_o_entrar_retira_el_boleto_de_la_reventa() {
    let p = preparar();
    let (boleto, vendedor) = boleto_para_revender(&p);
    let amigo = Address::generate(&p.env);

    // Al regalarlo, la oferta del dueño anterior deja de valer.
    p.tix.publicar_reventa(&boleto, &vendedor, &PRECIO);
    p.tix.transferir(&boleto, &vendedor, &amigo);
    assert_eq!(
        p.tix.try_consultar_reventa(&boleto),
        Err(Ok(Error::NoEnReventa))
    );

    // Un boleto que ya entró no se puede publicar ni comprar.
    p.tix.publicar_reventa(&boleto, &amigo, &PRECIO);
    p.tix.registrar_entrada(&boleto, &p.organizador);
    assert_eq!(
        p.tix.try_consultar_reventa(&boleto),
        Err(Ok(Error::NoEnReventa))
    );
    let usado = p.tix.try_publicar_reventa(&boleto, &amigo, &PRECIO);
    assert_eq!(usado, Err(Ok(Error::YaUsado)));
}

#[test]
fn nadie_se_compra_su_propio_boleto_y_sin_saldo_no_hay_reventa() {
    let p = preparar();
    let (boleto, vendedor) = boleto_para_revender(&p);
    p.tix.publicar_reventa(&boleto, &vendedor, &PRECIO);

    let a_si_mismo = p.tix.try_comprar_reventa(&boleto, &vendedor);
    assert_eq!(a_si_mismo, Err(Ok(Error::MismoDueno)));

    let sin_dinero = Address::generate(&p.env);
    assert!(p.tix.try_comprar_reventa(&boleto, &sin_dinero).is_err());
    assert_eq!(p.tix.consultar_boleto(&boleto).dueno, vendedor);
    assert_eq!(p.tix.consultar_reventa(&boleto), PRECIO);
}

#[test]
fn el_organizador_fija_el_tope_de_reventa() {
    let p = preparar();
    let (boleto, vendedor) = boleto_para_revender(&p);
    let evento = p.tix.consultar_boleto(&boleto).evento;
    assert_eq!(p.tix.consultar_evento(&evento).tope_reventa, 11_000);

    // Sube el tope a 130 %: 32.5 XLM. Lo firma el organizador, nadie más.
    p.tix.fijar_tope_reventa(&evento, &13_000);
    let firmas = p.env.auths();
    assert_eq!(firmas.len(), 1);
    assert_eq!(firmas[0].0, p.organizador);
    assert_eq!(p.tix.consultar_evento(&evento).tope_reventa, 13_000);
    assert!(p
        .tix
        .try_publicar_reventa(&boleto, &vendedor, &325_000_000)
        .is_ok());

    // Si lo baja a 100 %, la oferta de 32.5 XLM ya no se puede comprar.
    p.tix.fijar_tope_reventa(&evento, &10_000);
    let comprador = con_saldo(&p, 325_000_000);
    assert_eq!(
        p.tix.try_comprar_reventa(&boleto, &comprador),
        Err(Ok(Error::SobreTope))
    );
    assert_eq!(p.tix.consultar_boleto(&boleto).dueno, vendedor);

    // Fuera del rango de 100 % a 150 % se rechaza.
    assert_eq!(
        p.tix.try_fijar_tope_reventa(&evento, &9_999),
        Err(Ok(Error::TopeInvalido))
    );
    assert_eq!(
        p.tix.try_fijar_tope_reventa(&evento, &15_001),
        Err(Ok(Error::TopeInvalido))
    );
    assert_eq!(
        p.tix.try_fijar_tope_reventa(&99, &12_000),
        Err(Ok(Error::EventoNoExiste))
    );
}

// ---------- Cancelación y reembolso ----------

#[test]
fn un_evento_cancelado_no_vende_no_revende_ni_registra_entradas() {
    let p = preparar();
    let (boleto, vendedor) = boleto_para_revender(&p);
    let evento = p.tix.consultar_boleto(&boleto).evento;
    p.tix.publicar_reventa(&boleto, &vendedor, &PRECIO);

    p.tix.cancelar_evento(&evento);
    // Lo firma el organizador.
    assert_eq!(p.env.auths()[0].0, p.organizador);
    assert!(p.tix.consultar_evento(&evento).cancelado);

    let comprador = con_saldo(&p, PRECIO);
    assert_eq!(
        p.tix.try_comprar(&comprador, &evento),
        Err(Ok(Error::EventoCancelado))
    );
    assert_eq!(
        p.tix.try_comprar_reventa(&boleto, &comprador),
        Err(Ok(Error::EventoCancelado))
    );
    assert_eq!(
        p.tix.try_publicar_reventa(&boleto, &vendedor, &PRECIO),
        Err(Ok(Error::EventoCancelado))
    );
    assert_eq!(
        p.tix.try_registrar_entrada(&boleto, &p.organizador),
        Err(Ok(Error::EventoCancelado))
    );
    assert_eq!(
        p.tix.try_fijar_tope_reventa(&evento, &12_000),
        Err(Ok(Error::EventoCancelado))
    );
    // No se cobró nada a quien intentó comprar.
    assert_eq!(p.token.balance(&comprador), PRECIO);
    // Cancelar dos veces se rechaza.
    assert_eq!(
        p.tix.try_cancelar_evento(&evento),
        Err(Ok(Error::EventoCancelado))
    );
}

#[test]
fn el_reembolso_devuelve_el_precio_al_dueno_actual_una_sola_vez() {
    let p = preparar();
    let (boleto, comprador) = boleto_para_revender(&p);
    let evento = p.tix.consultar_boleto(&boleto).evento;
    // Lo regaló antes de la cancelación: el reembolso es para quien lo tiene.
    let amigo = Address::generate(&p.env);
    p.tix.transferir(&boleto, &comprador, &amigo);

    // Mientras el evento siga en pie no hay reembolsos.
    assert_eq!(p.tix.try_reembolsar(&boleto), Err(Ok(Error::EventoActivo)));

    p.tix.cancelar_evento(&evento);
    // El organizador recibió el 97 %; para devolver el 100 % pone la diferencia.
    StellarAssetClient::new(&p.env, &p.token.address).mint(&p.organizador, &7_500_000);
    let monto = p.tix.reembolsar(&boleto);
    // Lo paga y lo firma el organizador.
    assert_eq!(p.env.auths()[0].0, p.organizador);

    assert_eq!(monto, PRECIO);
    assert_eq!(p.token.balance(&amigo), PRECIO);
    assert_eq!(p.token.balance(&p.organizador), 0);
    assert!(p.tix.consultar_boleto(&boleto).reembolsado);

    // Un boleto reembolsado ya no se reembolsa, ni se transfiere, ni se revende.
    assert_eq!(p.tix.try_reembolsar(&boleto), Err(Ok(Error::YaReembolsado)));
    assert_eq!(
        p.tix.try_transferir(&boleto, &amigo, &comprador),
        Err(Ok(Error::YaReembolsado))
    );
    assert_eq!(
        p.tix.try_publicar_reventa(&boleto, &amigo, &PRECIO),
        Err(Ok(Error::YaReembolsado))
    );
}

#[test]
fn un_boleto_usado_o_gratis_no_mueve_dinero_al_reembolsar() {
    let p = preparar();
    let evento = crear(&p, 5, 0);
    let fue = Address::generate(&p.env);
    let no_fue = Address::generate(&p.env);
    let usado = p.tix.comprar(&fue, &evento);
    let gratis = p.tix.comprar(&no_fue, &evento);
    p.tix.registrar_entrada(&usado, &p.organizador);

    p.tix.cancelar_evento(&evento);

    // Quien ya entró no recibe reembolso.
    assert_eq!(p.tix.try_reembolsar(&usado), Err(Ok(Error::YaUsado)));
    // Un boleto gratis se marca como reembolsado sin transferir nada.
    assert_eq!(p.tix.reembolsar(&gratis), 0);
    assert!(p.tix.consultar_boleto(&gratis).reembolsado);
    assert_eq!(p.token.balance(&no_fue), 0);
}

// ---------- Compras de varios boletos y de regalo ----------

#[test]
fn comprar_varios_cobra_una_vez_y_emite_todos_los_boletos() {
    let p = preparar();
    let evento = crear(&p, 10, PRECIO);
    let comprador = con_saldo(&p, PRECIO * 3);

    let boletos = p.tix.comprar_varios(&comprador, &evento, &3);

    assert_eq!(boletos.len(), 3);
    assert_eq!(p.tix.boletos_de(&comprador), boletos);
    assert_eq!(p.tix.consultar_evento(&evento).vendidos, 3);
    // 3 % del total para TrustTix y el resto para el organizador.
    assert_eq!(p.token.balance(&comprador), 0);
    assert_eq!(p.token.balance(&p.admin), 22_500_000);
    assert_eq!(p.token.balance(&p.organizador), PRECIO * 3 - 22_500_000);
    for id in boletos.iter() {
        assert_eq!(p.tix.consultar_boleto(&id).dueno, comprador);
    }
}

#[test]
fn ninguna_cuenta_pasa_de_diez_boletos_por_evento() {
    let p = preparar();
    let evento = crear(&p, 50, 0);
    let fan = Address::generate(&p.env);

    p.tix.comprar_varios(&fan, &evento, &8);
    p.tix.comprar(&fan, &evento);
    // 9 ya comprados: 2 más pasarían de 10.
    assert_eq!(
        p.tix.try_comprar_varios(&fan, &evento, &2),
        Err(Ok(Error::LimitePorCuenta))
    );
    p.tix.comprar(&fan, &evento);
    assert_eq!(
        p.tix.try_comprar(&fan, &evento),
        Err(Ok(Error::LimitePorCuenta))
    );
    assert_eq!(p.tix.boletos_de(&fan).len(), 10);
    // El límite es por evento: en otro evento puede volver a comprar.
    let otro = crear(&p, 50, 0);
    p.tix.comprar(&fan, &otro);
}

#[test]
fn comprar_varios_respeta_la_capacidad_y_la_cantidad() {
    let p = preparar();
    let evento = crear(&p, 3, 0);
    let fan = Address::generate(&p.env);

    assert_eq!(
        p.tix.try_comprar_varios(&fan, &evento, &4),
        Err(Ok(Error::Agotado))
    );
    assert_eq!(
        p.tix.try_comprar_varios(&fan, &evento, &0),
        Err(Ok(Error::CantidadInvalida))
    );
    assert_eq!(p.tix.consultar_evento(&evento).vendidos, 0);
    p.tix.comprar_varios(&fan, &evento, &3);
    assert_eq!(p.tix.consultar_evento(&evento).vendidos, 3);
}

#[test]
fn comprar_para_regala_el_boleto_y_cobra_a_quien_paga() {
    let p = preparar();
    let evento = crear(&p, 10, PRECIO);
    let quien_paga = con_saldo(&p, PRECIO);
    let amiga = Address::generate(&p.env);

    let boleto = p.tix.comprar_para(&quien_paga, &amiga, &evento);
    // Lo firma quien paga.
    assert_eq!(p.env.auths()[0].0, quien_paga);

    assert_eq!(p.tix.consultar_boleto(&boleto).dueno, amiga);
    assert_eq!(p.tix.boletos_de(&amiga).len(), 1);
    assert_eq!(p.tix.boletos_de(&quien_paga).len(), 0);
    assert_eq!(p.token.balance(&quien_paga), 0);
    assert_eq!(p.token.balance(&p.organizador), PRECIO - 7_500_000);
    assert_eq!(
        p.tix.try_comprar_para(&amiga, &amiga, &evento),
        Err(Ok(Error::MismoDueno))
    );
}

// ---------- Reembolso por lote ----------

#[test]
fn reembolsar_lote_devuelve_el_precio_de_todo_el_evento() {
    let p = preparar();
    let evento = crear(&p, 10, PRECIO);
    let otro = crear(&p, 10, PRECIO);
    let a = con_saldo(&p, PRECIO * 2);
    let b = con_saldo(&p, PRECIO);
    let ajeno = con_saldo(&p, PRECIO);
    p.tix.comprar_varios(&a, &evento, &2); // boletos 1 y 2
    p.tix.comprar(&ajeno, &otro); // boleto 3, de otro evento
    let ultimo = p.tix.comprar(&b, &evento); // boleto 4
    p.tix.registrar_entrada(&ultimo, &p.organizador);
    p.tix.cancelar_evento(&evento);
    // El organizador recibió el 97 %; pone la diferencia para devolver el 100 %.
    StellarAssetClient::new(&p.env, &p.token.address).mint(&p.organizador, &PRECIO);

    assert_eq!(
        p.tix.try_reembolsar_lote(&otro, &1, &4),
        Err(Ok(Error::EventoActivo))
    );
    let reembolsados = p.tix.reembolsar_lote(&evento, &1, &4);

    // Se reembolsan 1 y 2; el 3 es de otro evento y el 4 ya entró.
    assert_eq!(reembolsados, 2);
    assert_eq!(p.token.balance(&a), PRECIO * 2);
    assert!(p.tix.consultar_boleto(&1).reembolsado);
    assert!(!p.tix.consultar_boleto(&3).reembolsado);
    assert!(!p.tix.consultar_boleto(&4).reembolsado);
    // Una segunda pasada ya no reembolsa nada.
    assert_eq!(p.tix.reembolsar_lote(&evento, &1, &4), 0);
}

#[test]
fn reembolsar_lote_acepta_hasta_diez_boletos() {
    let p = preparar();
    let evento = crear(&p, 5, 0);
    p.tix.cancelar_evento(&evento);
    assert_eq!(p.tix.try_reembolsar_lote(&evento, &1, &10), Ok(Ok(0)));
    for (desde, hasta) in [(1, 11), (0, 5), (5, 4)] {
        assert_eq!(
            p.tix.try_reembolsar_lote(&evento, &desde, &hasta),
            Err(Ok(Error::CantidadInvalida))
        );
    }
}

// ---------- Ofertas ----------

#[test]
fn aceptar_una_oferta_paga_al_dueno_y_le_pasa_el_boleto_a_quien_oferto() {
    let p = preparar();
    let (boleto, dueno) = boleto_para_revender(&p);
    let comision_compra = p.token.balance(&p.admin);
    let oferta = 260_000_000; // 26 XLM, dentro del tope de 27.5
    let interesado = con_saldo(&p, oferta);

    p.tix.ofertar(&interesado, &boleto, &oferta);
    // El dinero queda en garantía en el contrato.
    assert_eq!(p.token.balance(&interesado), 0);
    assert_eq!(p.token.balance(&p.tix.address), oferta);
    assert_eq!(p.tix.consultar_oferta(&boleto, &interesado), oferta);

    p.tix.aceptar_oferta(&boleto, &dueno, &interesado);
    assert_eq!(p.env.auths()[0].0, dueno);

    // 2 % para TrustTix y el resto para quien vendió.
    assert_eq!(p.token.balance(&p.admin) - comision_compra, 5_200_000);
    assert_eq!(p.token.balance(&dueno), oferta - 5_200_000);
    assert_eq!(p.token.balance(&p.tix.address), 0);
    assert_eq!(p.tix.consultar_boleto(&boleto).dueno, interesado);
    assert_eq!(
        p.tix.try_consultar_oferta(&boleto, &interesado),
        Err(Ok(Error::NoHayOferta))
    );
}

#[test]
fn retirar_una_oferta_devuelve_el_dinero_y_ofertar_otra_vez_la_reemplaza() {
    let p = preparar();
    let (boleto, dueno) = boleto_para_revender(&p);
    let interesado = con_saldo(&p, PRECIO * 2);

    p.tix.ofertar(&interesado, &boleto, &200_000_000);
    p.tix.ofertar(&interesado, &boleto, &PRECIO);
    // Solo queda en garantía la última oferta.
    assert_eq!(p.token.balance(&p.tix.address), PRECIO);
    assert_eq!(p.token.balance(&interesado), PRECIO);

    assert_eq!(p.tix.retirar_oferta(&boleto, &interesado), PRECIO);
    assert_eq!(p.token.balance(&interesado), PRECIO * 2);
    assert_eq!(p.token.balance(&p.tix.address), 0);
    assert_eq!(
        p.tix.try_aceptar_oferta(&boleto, &dueno, &interesado),
        Err(Ok(Error::NoHayOferta))
    );
    assert_eq!(
        p.tix.try_retirar_oferta(&boleto, &interesado),
        Err(Ok(Error::NoHayOferta))
    );
}

#[test]
fn las_ofertas_respetan_el_tope_y_al_dueno() {
    let p = preparar();
    let (boleto, dueno) = boleto_para_revender(&p);
    let interesado = con_saldo(&p, PRECIO * 2);
    let extrano = Address::generate(&p.env);

    assert_eq!(
        p.tix.try_ofertar(&interesado, &boleto, &275_000_001),
        Err(Ok(Error::SobreTope))
    );
    assert_eq!(
        p.tix.try_ofertar(&interesado, &boleto, &0),
        Err(Ok(Error::PrecioInvalido))
    );
    assert_eq!(
        p.tix.try_ofertar(&dueno, &boleto, &PRECIO),
        Err(Ok(Error::MismoDueno))
    );
    // Solo el dueño acepta, y el dinero sigue en garantía si falla.
    p.tix.ofertar(&interesado, &boleto, &PRECIO);
    assert_eq!(
        p.tix.try_aceptar_oferta(&boleto, &extrano, &interesado),
        Err(Ok(Error::NoEsDueno))
    );
    assert_eq!(p.token.balance(&p.tix.address), PRECIO);
}

// ---------- Devolución acordada ----------

#[test]
fn devolver_boleto_regresa_el_precio_y_libera_el_lugar() {
    let p = preparar();
    let evento = crear(&p, 1, PRECIO);
    let fan = con_saldo(&p, PRECIO);
    let boleto = p.tix.comprar(&fan, &evento);
    // El evento estaba agotado; el organizador pone la comisión que no recibió.
    StellarAssetClient::new(&p.env, &p.token.address).mint(&p.organizador, &7_500_000);

    assert_eq!(p.tix.devolver_boleto(&boleto), PRECIO);
    // Firman el dueño y el organizador.
    let firmas = p.env.auths();
    assert!(firmas.iter().any(|f| f.0 == fan));
    assert!(firmas.iter().any(|f| f.0 == p.organizador));

    assert_eq!(p.token.balance(&fan), PRECIO);
    assert!(p.tix.consultar_boleto(&boleto).reembolsado);
    assert_eq!(p.tix.consultar_evento(&evento).vendidos, 0);
    // El lugar volvió a la venta.
    let otra = con_saldo(&p, PRECIO);
    p.tix.comprar(&otra, &evento);
    assert_eq!(
        p.tix.try_devolver_boleto(&boleto),
        Err(Ok(Error::YaReembolsado))
    );
}

// ---------- Actualizar el código ----------

#[test]
fn el_admin_actualiza_el_codigo_sin_perder_los_datos() {
    let p = preparar();
    let evento = crear(&p, 10, PRECIO);
    let fan = con_saldo(&p, PRECIO);
    let boleto = p.tix.comprar(&fan, &evento);

    // Una versión anterior del contrato, ya compilada, como código nuevo.
    let wasm = p.env.deployer().upload_contract_wasm(Bytes::from_slice(
        &p.env,
        include_bytes!("../testdata/trusttix-v2.wasm"),
    ));
    p.tix.actualizar(&wasm);
    assert_eq!(p.env.auths()[0].0, p.admin);

    // Los datos siguen ahí y los lee el código nuevo.
    assert_eq!(p.tix.consultar_evento(&evento).vendidos, 1);
    assert_eq!(p.tix.consultar_boleto(&boleto).dueno, fan);
    assert_eq!(p.tix.admin(), p.admin);
}

#[test]
fn validaciones_al_crear_y_consultar() {
    let p = preparar();
    let vacio = p.tix.try_crear_evento(
        &p.organizador,
        &texto(&p.env, "Sin lugares"),
        &texto(&p.env, "Ninguno"),
        &texto(&p.env, "2026-12-01T20:00"),
        &0,
        &0,
    );
    assert_eq!(vacio, Err(Ok(Error::CapacidadInvalida)));

    let negativo = p.tix.try_crear_evento(
        &p.organizador,
        &texto(&p.env, "Precio raro"),
        &texto(&p.env, "Ninguno"),
        &texto(&p.env, "2026-12-01T20:00"),
        &10,
        &-1,
    );
    assert_eq!(negativo, Err(Ok(Error::PrecioInvalido)));

    assert_eq!(
        p.tix.try_consultar_evento(&99),
        Err(Ok(Error::EventoNoExiste))
    );
    assert_eq!(
        p.tix.try_consultar_boleto(&99),
        Err(Ok(Error::BoletoNoExiste))
    );
    assert_eq!(p.tix.total_eventos(), 0);
}
