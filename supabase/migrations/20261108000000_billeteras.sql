-- Billetera TrustTix de cada cuenta.
--
-- La billetera integrada se crea en el navegador. Guardarla aquí, ligada a la
-- cuenta de Google (auth.uid()), permite abrir la misma billetera desde otro
-- dispositivo, otro subdominio o después de borrar los datos del navegador.
--
-- Es una custodia simple pensada para Stellar Testnet: la clave secreta queda
-- en la base de datos y solo su dueño puede leerla, pero quien administre el
-- proyecto de Supabase también podría. Para Mainnet se reemplaza por passkeys
-- o por un custodio.

create table public.billeteras (
  usuario        uuid        primary key default auth.uid() references auth.users (id) on delete cascade,
  direccion      text        not null check (direccion ~ '^G[A-Z2-7]{55}$'),
  secreta        text        not null check (secreta ~ '^S[A-Z2-7]{55}$'),
  creada_en      timestamptz not null default now(),
  actualizada_en timestamptz not null default now()
);

alter table public.billeteras enable row level security;

-- Nadie más que el dueño la ve: ni las visitas sin cuenta ni otras cuentas.
create policy "ver mi billetera"
  on public.billeteras for select
  to authenticated
  using (usuario = auth.uid());

create policy "guardar mi billetera"
  on public.billeteras for insert
  to authenticated
  with check (usuario = auth.uid());

-- Importar una clave en otro dispositivo reemplaza la billetera de la cuenta.
create policy "reemplazar mi billetera"
  on public.billeteras for update
  to authenticated
  using (usuario = auth.uid())
  with check (usuario = auth.uid());

create trigger billeteras_actualizada
  before update on public.billeteras
  for each row execute function public.marcar_actualizado();
