-- Cuántas billeteras TrustTix existen, para mostrar la tracción en la landing.
-- La tabla billeteras solo la lee cada dueño; esta función devuelve el total y
-- nada más, así que cualquiera puede consultarla sin ver datos de nadie.

create function public.total_billeteras() returns bigint
  language sql
  stable
  security definer
  set search_path = ''
as $$
  select count(*) from public.billeteras;
$$;

revoke all on function public.total_billeteras() from public;
grant execute on function public.total_billeteras() to anon, authenticated;
