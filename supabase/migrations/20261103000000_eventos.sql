-- Datos descriptivos de los eventos de TrustTix.
--
-- Lo que define la validez de un boleto (organizador, nombre, recinto, fecha,
-- capacidad, vendidos y precio) vive en el contrato de Soroban. Aquí solo va lo
-- que no hace falta en la red: imagen, categoría, dirección, coordenadas y texto.
-- Cada fila se enlaza con el contrato por su ID y el número de evento que asignó.

create table public.eventos (
  contrato           text        not null,
  evento_id          integer     not null check (evento_id > 0),
  categoria          text        not null default 'Eventos',
  ciudad             text,
  direccion          text,
  descripcion        text,
  organizador_nombre text,
  fecha_fin          date,
  lat                double precision,
  lng                double precision,
  -- Ruta del archivo dentro del bucket "eventos" de Storage.
  imagen             text,
  creado_por         uuid        default auth.uid() references auth.users (id) on delete set null,
  creado_en          timestamptz not null default now(),
  actualizado_en     timestamptz not null default now(),
  primary key (contrato, evento_id)
);

alter table public.eventos enable row level security;

-- Cualquiera puede leer: la tienda se ve sin cuenta.
create policy "eventos visibles para todos"
  on public.eventos for select
  to anon, authenticated
  using (true);

-- Con sesión de Google se registran los datos de un evento nuevo.
create policy "registrar datos con sesion"
  on public.eventos for insert
  to authenticated
  with check (creado_por = auth.uid());

-- Solo quien registró los datos los puede cambiar.
create policy "editar datos propios"
  on public.eventos for update
  to authenticated
  using (creado_por = auth.uid())
  with check (creado_por = auth.uid());

create function public.marcar_actualizado() returns trigger
  language plpgsql
  set search_path = ''
as $$
begin
  new.actualizado_en := now();
  return new;
end;
$$;

create trigger eventos_actualizado
  before update on public.eventos
  for each row execute function public.marcar_actualizado();

-- Imágenes de los eventos: lectura pública, y cada usuario sube solo a su carpeta
-- (eventos/<id de usuario>/...). Las del catálogo van en eventos/catalogo/.
-- Cada imagen nueva lleva un nombre único, así que no hace falta reemplazar archivos.
insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('eventos', 'eventos', true, 5242880, array['image/webp', 'image/jpeg', 'image/png', 'image/avif']);

create policy "subir imagenes a mi carpeta"
  on storage.objects for insert
  to authenticated
  with check (bucket_id = 'eventos' and (storage.foldername(name))[1] = auth.uid()::text);
