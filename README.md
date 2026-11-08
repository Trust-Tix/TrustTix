# TrustTix

## Requisitos

**Para el front:**

- [Node.js](https://nodejs.org/) 20.19 o superior (incluye npm).
- Git.
- La extensión [Freighter](https://www.freighter.app) en Chrome, Edge o Brave, configurada en **Testnet** y con saldo de prueba (botón **Fund with Friendbot** dentro de la extensión).

**Para el contrato (opcional; ya está desplegado en Testnet):**

- [Rust](https://www.rust-lang.org/tools/install) 1.84 o superior y el target `wasm32v1-none`.
- [Stellar CLI](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup).
- En Windows, las herramientas de compilación de C++ de Visual Studio (Build Tools).

## Instalación

```bash
git clone https://github.com/DanielRosalesAlanis/TrustTix.git
cd TrustTix/frontend
npm install
```

Para el contrato:

```bash
rustup target add wasm32v1-none
```

## Cómo correrlo

**Front:**

```bash
cd frontend
npm run dev
```

Antes de la primera vez, copia `frontend/.env.example` como `frontend/.env.local` y pon el ID de cliente de Google en `VITE_GOOGLE_CLIENT_ID` (ver [Inicio de sesión con Google](#inicio-de-sesión-con-google)).

Abre <http://localhost:5173/?portal=app>, mira los eventos, inicia sesión con Google y conecta Freighter para comprar. El front ya apunta al contrato desplegado en Testnet (`CCWWMLZANZV54WMJQ5JQ6XP2AAUE5BGKK3Q5CPQZEJMREJ4OYRVN36GD`).

**Portales:** la misma app se publica en cuatro direcciones.

| Portal | En producción | En local |
| --- | --- | --- |
| Landing: quiénes somos y qué ofrecemos | trusttix.xyz | <http://localhost:5173> |
| Compradores: ver eventos y lugares sin cuenta; comprar y guardar boletos con cuenta | app.trusttix.xyz | <http://localhost:5173/?portal=app> |
| Organizadores: crear eventos y asignar personal | organizer.trusttix.xyz | <http://localhost:5173/?portal=organizer> |
| Personal de acceso: escanear boletos en fila | staff.trusttix.xyz | <http://localhost:5173/?portal=staff> |

En local el portal elegido se recuerda mientras la pestaña siga abierta; para cambiarlo, abre otra dirección con `?portal=`. Los enlaces viejos siguen funcionando: admin.trusttix.xyz lleva a organizer, y las rutas de la app en trusttix.xyz (por ejemplo `/eventos/2`) llevan a app.

El personal abre su portal, conecta su billetera y muestra su código QR; el organizador lo escanea desde el panel del evento y firma para autorizarlo en el contrato. En la puerta, la cámara del escáner se queda abierta y lee un boleto tras otro.

**Pruebas del contrato:**

```bash
cd contracts
cargo test
```

**Compilar el contrato:**

```bash
cd contracts
stellar contract build
```

**Desplegar una instancia propia en Testnet (opcional):**

```bash
stellar keys generate mi-cuenta --network testnet --fund
stellar contract deploy --wasm target/wasm32v1-none/release/trusttix.wasm \
  --source-account mi-cuenta --network testnet \
  -- --token $(stellar contract id asset --asset native --network testnet)
```

Para usar esa instancia en el front, agrega `VITE_CONTRATO_ID=<id del contrato>` a `frontend/.env.local` y vuelve a correr `npm run dev`. Para crear los eventos de ejemplo en ella: `.\scripts\sembrar.ps1 -Contrato <id del contrato>` desde la carpeta `contracts`.

## Inicio de sesión con Google

1. En [Google Cloud Console](https://console.cloud.google.com/), crea un proyecto y abre **Google Auth Platform**.
2. En **Branding**, llena el nombre de la app y el correo de soporte. En **Audience**, elige **External** y publica la app.
3. En **Clients**, crea un cliente de tipo **Web application** y agrega estos orígenes de JavaScript autorizados:
   `http://localhost`, `http://localhost:5173`, `https://trust-tix.vercel.app`, `https://trusttix.xyz`, `https://www.trusttix.xyz`, `https://app.trusttix.xyz`, `https://organizer.trusttix.xyz` y `https://staff.trusttix.xyz`.
4. Copia el ID de cliente en `VITE_GOOGLE_CLIENT_ID` (en `frontend/.env.local` y en las variables de entorno de Vercel).

La cuenta de Google solo identifica la sesión; los permisos dentro de TrustTix los da la firma de la billetera.

## Base de datos (Supabase)

El contrato guarda lo que define un boleto. La portada, la categoría, la dirección, las coordenadas y la descripción de cada evento viven en Supabase: una tabla `eventos` y un bucket público `eventos` para las imágenes.

1. Crea un proyecto en [Supabase](https://supabase.com/dashboard).
2. En **SQL Editor**, corre [supabase/migrations/20261103000000_eventos.sql](supabase/migrations/20261103000000_eventos.sql). Crea la tabla, sus reglas de acceso y el bucket.
3. En **Authentication > Sign In / Providers > Google**, activa Google y pega el mismo ID de cliente de `VITE_GOOGLE_CLIENT_ID` en **Client IDs**. Así la sesión de Google también abre la de Supabase.
4. En **Project Settings > API**, copia la URL y la llave pública en `VITE_SUPABASE_URL` y `VITE_SUPABASE_ANON_KEY` (en `frontend/.env.local` y en Vercel).
5. Carga el catálogo de ejemplo. Toma la llave `service_role` de la misma pantalla y úsala solo en la terminal:

   ```powershell
   cd frontend
   $env:SUPABASE_SERVICE_ROLE_KEY = "<service_role key>"
   npm run sembrar
   ```

   Sube las imágenes de [supabase/semilla/imagenes](supabase/semilla/imagenes) y los datos de [supabase/semilla/catalogo.json](supabase/semilla/catalogo.json). Se puede volver a correr; actualiza lo que ya existe.

Cualquiera puede leer los datos. Para registrar los de un evento nuevo hace falta sesión de Google, y solo quien los registró puede cambiarlos. Las portadas nuevas deben medir al menos 1600 px de ancho; la app las guarda en WebP a un máximo de 2400 px.

## Ubicación

La app pide la ubicación solo cuando la persona pulsa **Usar mi ubicación**. Con ella ordena los eventos por distancia y muestra el mapa del recinto. Las direcciones se convierten en coordenadas con [Nominatim](https://nominatim.org/) de OpenStreetMap, sin llave y con un límite de una consulta por segundo.
