# errserve — AGENTS.md

> Las directivas globales de este entorno (OpenSpec SDD + TDD, tags de estado, tabla de subagentes)
> se inyectan ya en cada sesión desde `~/.config/opencode/AGENTS.md`. No se repiten aquí: lo de abajo
> es lo específico de este repositorio.

## Estado real del repositorio (verificado)

Implementado, verificado y versionado. Ocho changes están **archivados** en
`openspec/changes/archive/` —`error-pages-parity`, `error-page-coverage`, `release-hardening`,
`security-hardening`, `retry-after-configurable`, `embedded-error-pages`, `fix-image-build` y
`publish-crate-on-tag`, los cinco primeros con fecha `2026-10-24` y los tres últimos con
`2026-10-25`— y sus requisitos viven fusionados en los cuatro specs de `openspec/specs/`:
`error-page`, `http-server`, `observability` y `response-extras`. Las deltas de
`embedded-error-pages` entraron en `error-page` y `http-server`; las de `fix-image-build` y
`publish-crate-on-tag`, en `http-server`.

- `src/main.rs` — arranque, router, middleware de páginas de error, la lista `SERVICE_ENDPOINTS` que acota ese middleware, el *fallback* de assets embebidos del servidor de ficheros, y los tests de router.
- `src/config.rs` — configuración desde el entorno (`Config`, `ConfigError`, `from_env_iter`).
- `src/path.rs` — normalización del path (percent-decode + plegado de `.`/`..`).
- `src/resolve.rs` — cascadas de código y formato, rango `400..=599`, tabla de razones.
- `src/render.rs` — cuerpo por formato y cabeceras; la cascada del HTML tiene **dos capas** (`CONTENT_DIR` entera y, después, la copia embebida, D1).
- `src/embedded.rs` — la tabla embebida: las once páginas (`PAGES`, con `PAGE_CODES` y `page(code)` derivados), la hoja `css/error.css` y `asset(path)`, la consulta de assets por path exacto. `PAGES` es la única fuente de verdad de qué códigos tienen página, y aquí vive la prueba de paridad (D5).
- `src/metrics.rs` — contador de páginas servidas por código **y por origen del código**, y su serialización en texto Prometheus.
- `content/` — **once** páginas HTML (`400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`, `502`, `503`, `504`), todas con la clase `c<código>` en el `body`, y la hoja única `content/css/error.css`; esas mismas once páginas y esa misma hoja son las que van **embebidas** en el binario.
- `Dockerfile`, `.dockerignore`, `.justfile`, `compose.yml` — empaquetado (imagen ~13 MB, no-root uid 10001); el `compose.yml` va endurecido: `read_only: true`, `cap_drop: [ALL]`, `no-new-privileges:true` e `init: true`.
- `README.md` — el contrato con Traefik y los ejemplos.

Puertas en verde: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y **143 tests**
(140 unitarios + 3 de integración sobre el contenido de `content/`).
Verificado además contra el servidor real y contra la imagen.

## Qué es

Clon en Rust, deliberadamente **supersencillo**, de [tarampampam/error-pages](https://github.com/tarampampam/error-pages),
pensado para usarse detrás de **Traefik** como servicio de páginas de error.

**La diferencia funcional que justifica el proyecto:** `error-pages` responde **siempre HTTP 200**,
porque asume que el proxy reescribe solo el cuerpo y conserva el código. Este servidor devuelve el
**código de error real**. Si una respuesta de página de error sale con `200`, el proyecto está roto.

La paridad con `error-pages` se mantiene en **dos ejes independientes**: el **código** que se
devuelve y la **negociación de formato** (HTML / JSON / XML / texto plano).

## Garantías que no se pueden romper

1. **El estado de una página de error es el código resuelto.** Nunca `2xx`, ni `1xx`, ni `3xx`.
   **Ninguna** respuesta del servidor de ficheros con estado de error se escapa ya —antes solo su
   `404`: el `405` de un método que no sirve estáticos, el `416` de un `Range` múltiple y el `500`
   de un segmento de path que no cabe como nombre de fichero salían crudos, sin cuerpo y sin
   `X-Robots-Tag`—, y el estado de esa respuesta **no** es una fuente de la cascada: la página lleva
   el código resuelto. Conservarlo obligaba a propagar cabeceras que describen *su* respuesta —el
   `Allow` del `405`, el `Content-Range` del `416`— so pena de incumplir un `MUST` de RFC 9110, y
   dejaba el estado y el cuerpo en desacuerdo (`404` con el cuerpo y el `Retry-After` del `503`). La
   regla se revisó por eso: conservar el estado del servidor de ficheros —lo que hacía la primera
   versión de este change— sale peor que sustituirlo, así que no la «arregles» de vuelta.
2. **El código resuelto está en `400..=599`.** Una fuente (path, `X-Code`, o `DEFAULT_ERROR_PAGE`)
   que aporte un código fuera de ese rango se considera **vacía** y se continúa la cascada; si
   ninguna aporta uno utilizable, se usa el defecto. `DEFAULT_ERROR_PAGE` fuera de rango es **error
   de arranque**. El rango es `400..=599` porque es el que Traefik intercepta (`errors.status:
   400-599`), y porque `1xx` no es representable como respuesta final, `204`/`304` no admiten cuerpo
   y un `2xx` contradice el punto 1.
3. **El código real solo manda en el acceso directo.** Con el middleware `errors` de Traefik
   delante de una aplicación, **Traefik conserva el código del upstream y el nuestro se ignora**
   (medido: backend `503` con el middleware pidiendo `/404` → el cliente recibió `503`). Nuestro
   código decide en el acceso **directo** —router *catch-all*, *default backend*—, que es el caso
   que justifica el proyecto. No prometas lo contrario en la documentación.
4. **Un asset existente gana a la página de error.** La garantía del código aplica a las *páginas de
   error*, no a los assets: `GET /css/error.css` es un `200 text/css`. Solo se sustituye una respuesta
   del servidor de ficheros **con estado de error**; su `200` —y su cuerpo— se sirve tal cual. No
   conviertas un asset en página de error por la cabecera `X-Code`.
5. **Los endpoints de servicio quedan fuera de la sustitución.** `SERVICE_ENDPOINTS` —`/healthz`,
   `/health`, `/health/live`, `/live`, `/version`, `/metrics`— se comprueba antes de interceptar: un
   método que el endpoint no admite es un hecho sobre *esa ruta*, y axum lo dice con su `405` y su
   `Allow`. Convertirlo en nuestra página daría un `404` que a la vez declara qué métodos admite la
   ruta: incoherente, y una mentira sobre qué es esa ruta. Esa lista tiene que **seguir a las rutas
   que declara `build_router`**: si añades una ruta de servicio, añádela también aquí. Límite
   conocido y aceptado: un `5xx` de un endpoint de servicio no pasaría por esta vía —hoy no puede,
   porque solo devuelven `200` o el `405` del router—.
6. **El `304` de una petición condicional se deja pasar a propósito.** `ServeDir` **sí** emite `3xx`:
   un `GET` de un asset con `If-Modified-Since` responde `304 Not Modified`, sin cuerpo y sin
   `X-Robots-Tag`, que es justo la respuesta que el cliente pidió. `append_index_html_on_directories(false)`
   suprime la redirección de directorio, **no** los `3xx`. No «endurezcas» esto leyendo lo contrario.
7. **El binario sirve las páginas y su hoja sin ficheros de datos.** Las once páginas y
   `css/error.css` van **dentro** del ejecutable (`include_str!`/`include_bytes!`), así que un
   `cargo install errserve` —un binario y ningún fichero de datos, con `CONTENT_DIR` por defecto
   apuntando a un `content` que no existe— sirve las páginas de verdad, no el respaldo de texto.
   Y **está medido**: desde un directorio vacío, el binario instalado desde crates.io con
   `cargo install errserve --locked` responde `404 text/html` a `GET /404` con `Accept: text/html`
   —byte a byte la misma página que `content/404.html`— y `200 text/css` a `GET /css/error.css`.
   `CONTENT_DIR` es solo el *override*: la **capa del operador se agota entera** —el fichero del
   código pedido y, si falta, el del código por defecto— antes de entrar en la copia embebida
   (D1), que repite esa misma cascada. Y lo que gana es **un fichero que se pueda leer**, no
   «siempre»: una página que existe y no se puede abrir responde el respaldo de texto con el
   código resuelto —el `warn` la delata—, en vez de que la copia embebida tape en silencio un
   directorio del operador roto (Opción A de `design.md`). Las dos copias **no pueden divergir**:
   una prueba de paridad compara sus bytes en los **dos sentidos** (D5). Nada de esto toca la
   garantía 1: el estado sigue siendo el código resuelto, nunca `200`.

## Configuración (solo entorno)

| Variable | Defecto | Para qué |
|---|---|---|
| `PORT` | `8080` | Puerto de escucha (bind `0.0.0.0`). |
| `LOG_LEVEL` | `info` | Filtro de registro. |
| `CONTENT_DIR` | `content` | *Override* de las páginas HTML y los assets **embebidos** en el binario. |
| `DEFAULT_ERROR_PAGE` | `404` | Código por defecto; fuera de `400..=599` es error de arranque. |
| `LOG_FORMAT` | `text` | `text` o `json`; cualquier otro valor, o vacía, es error de arranque. |
| `RETRY_AFTER` | `120` | Segundos de la cabecera `Retry-After` (solo `429` y `503`); no numérica, o vacía, es error de arranque. |

`GET /metrics` es un endpoint de servicio (siempre `200`, fuera de la resolución de páginas de
error) que expone el contador Prometheus de páginas servidas **por código y por el origen de ese
código**: `errors_pages_total{code="404",source="path"} 3`. `source` vale `path`, `header` (`X-Code`)
o `default` (`DEFAULT_ERROR_PAGE`), porque la cabecera la elige quien llama: sin esa etiqueta,
cualquiera puede inflar a voluntad el contador de un código y fabricar la apariencia de una
avalancha justo durante una incidencia. El contador vive en el estado del router, no en un `static`:
por eso dos routers no se contaminan entre sí. **No se publica en el router de respaldo** —no es una
página de error y delata el perfil de fallos de la infraestructura—: se deja fuera del *catch-all* o
se protege con un middleware de Traefik, que es donde va la autenticación. Dejarlo fuera es una
conjunción en la regla —``HostRegexp(`.+`) && !Path(`/metrics`)``—, y la llevan los tres ficheros que
entregan un respaldo desplegable (`compose.yml`, `examples/traefik/compose.yml` y el fragmento de
*provider de fichero* de `examples/traefik/README.md`). La guarda es `tests/traefik_catchall.rs`,
porque el CI no puede levantar contenedores y comprobarlo de extremo a extremo: la prueba lee esos
tres ficheros versionados y falla en cuanto un respaldo pierde la conjunción. Quitarla vuelve a
publicar el contador a cualquiera —una petición a `/metrics` en un Host sin router propio casa con el
respaldo y devuelve el perfil de fallos de la infraestructura—; con ella, Traefik responde su propio
`404` y el servicio ni se entera. `/version` se mantiene a la vista por paridad deliberada con
`error-pages`.

## Cómo llega el código de error (contrato con Traefik)

El middleware `errors` de Traefik reenvía la petición con la plantilla `errors.query: /{status}`. Por
tanto **el código viaja en el path**: una página 404 llega como `GET /404`. `X-Code` es el fallback.

Orden de resolución (el **primero** que haga match): **path** (`/404`, `/404.html`, `/404.json`,
`/503.xml`…) → cabecera **`X-Code`** → **`DEFAULT_ERROR_PAGE`** (defecto `404`).

## Normalización del path (no romper)

`ServeDir` sirve el path **percent-decodificado y con los `.` plegados**, así que clasificar sobre el
path crudo deja pasar la propia página de error como si fuera un asset —servida con `200`, justo el
fallo que este proyecto corrige—. Por eso `src/path.rs` normaliza **una sola vez** y el middleware
reescribe `*request.uri_mut()` antes de clasificar **y** de servir: clasificador y servidor de
ficheros no pueden discrepar. El re-encode deja `%` codificado para que `ServeDir` no decodifique dos
veces. El plegado de `..` hace `pop`, nunca sale de la raíz. No sustituyas esto por
`request.uri().path()` sin volver a leer `src/path.rs`.

## Formatos de respuesta (paridad con error-pages)

El **código** y el **formato** se resuelven por separado, cada uno con su cascada y el mismo
principio: gana el **primer** origen que haga match. Formatos: **HTML, JSON, XML, texto plano**.

Orden del formato: extensión del path (`.html`/`.htm`, `.json`, `.xml`, `.txt`) → `Content-Type` de
la petición → `X-Format` → `Accept` → **texto plano** por defecto.

Ojo con el defecto: **texto plano**, no HTML. Un navegador detrás de Traefik manda `Accept: text/html…`
y cae en el cuarto caso, pero una petición sin `Accept` (curl, health-checks) recibe texto plano.
Sigue la cascada tal cual para no divergir de `error-pages`; no «arregles» el defecto a HTML.

Cabeceras de toda página de error: `Content-Type` según el formato, `Content-Length`,
`X-Robots-Tag: noindex, nofollow, nosnippet, noarchive`, y `Retry-After` con el valor de
`RETRY_AFTER` (`120` por defecto) **solo** en `429` y `503` (el `Retry-After` describe el estado que
se sirve, no el de la respuesta sustituida).

Toda respuesta declara en `Vary` `Accept-Encoding` y los nombres de las cuatro cabeceras de
correlación —`X-Request-Id`, `X-Trace-Id`, `X-Correlation-Id`, `X-Amzn-Trace-Id`—, **siempre**, las
traiga la petición o no: la respuesta copia esas cabeceras, así que una caché intermedia tiene que
saber que varía por ellas. Una lista que cambiase según la petición no le serviría a una caché.

## Integración Traefik (lo mínimo que debe aceptar el servicio)

```yaml
traefik.http.middlewares.error-pages-middleware.errors.status: 400-599
traefik.http.middlewares.error-pages-middleware.errors.service: error-pages-service
traefik.http.middlewares.error-pages-middleware.errors.query: /{status}
traefik.http.services.error-pages-service.loadbalancer.server.port: 8080
```

Hay un **ejemplo ejecutable** en `examples/traefik/` (Traefik v3 + este servicio + un backend que
falla, con `check.sh`) y un `compose.yml` de producción en la raíz: no hace falta inventarse los
labels. ⚠️ ``HostRegexp(`{host:.+}`)`` es sintaxis de Traefik **v2** y en v3 **no casa con nada** —el
router sale `enabled` y Traefik responde su propio 404 sin llegar al servicio—; en v3 es
``HostRegexp(`.+`)``.

`/{status}` es lo que produce la petición `/404`; no lo cambies a un header sin cambiar también la
resolución del código. El puerto por defecto del servicio es **8080**, el mismo que publica el
`Dockerfile` y la receta `run` del `.justfile`.

## Comandos

```sh
cargo run                                   # servidor
cargo test                                  # 143 tests
cargo test <substring>                      # un test por nombre
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo check --offline                       # el repo compila SIN RED (caché local de Cargo)
cargo deny check -D warnings                # dependencias: avisos, licencias, duplicaciones y fuentes
just changelog [v0.3.0]                     # antepone al CHANGELOG la sección de lo no etiquetado
just build | run | dev                      # empaquetado (docker por defecto; just engine=podman ...)
```

Orden obligatorio antes de commit: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

## CHANGELOG

Se mantiene con [`git-cliff`](https://git-cliff.org) (`cliff.toml`) y la receta `just changelog`, que
**antepone** (`--prepend`) la sección de lo que todavía no está etiquetado:

- `just changelog` escribe la sección «Sin publicar» (los commits posteriores al último tag).
- `just changelog v0.3.0` escribe esa misma sección ya con el nombre del tag que se va a crear: es lo
  que se hace justo antes de `git tag`.
- La entrada de `0.1.0` está **escrita a mano** y es más rica que lo que saldría de los commits: el
  fichero **no se regenera**, así que se conserva.
- git-cliff no detecta una sección repetida y la escribiría dos veces, así que la receta **se niega**
  a anteponer una sección que ya esté en el fichero: para rehacerla, se borra antes a mano.

## Entrega

- `main` y `development` están **protegidas**: PR obligatorio, los checks `fmt, clippy y tests`,
  `cargo-deny` e `imagen` en verde y `strict` (la rama ha de estar al día). `enforce_admins` está
  activo, así que **tampoco el dueño empuja directo**. El flujo es: rama `feature/...` desde
  `development` → PR a `development` → PR de `development` a `main`.
- El CI (`.github/workflows/ci.yml`) tiene **tres jobs**: `fmt, clippy y tests` (las tres puertas),
  `cargo-deny` (la puerta de dependencias, con `deny.toml`) e `imagen`, que **construye la imagen**
  —la puerta del empaquetado, y la que faltaba: hasta el change `fix-image-build` la imagen solo se
  construía en `release.yml`, al etiquetar, así que un `Dockerfile` que dejara de compilar se
  descubría publicando—. **Los tres están marcados como requeridos en `main` y en `development`**, así
  que el rojo de cualquiera de ellos bloquea el merge (medido con la API: los `contexts` de las dos
  ramas son `["fmt, clippy y tests", "cargo-deny", "imagen"]`). Para volver a medirlo:
  `gh api repos/atareao/errserve/branches/main/protection --jq '.required_status_checks.contexts'`.
  Y el job `puertas` —el de las tres puertas— verifica **además** que el **paquete del crate es
  publicable**: `cargo publish --dry-run --locked`, que empaqueta, compila el paquete **extraído** y
  valida sus metadatos sin subir nada. Es la misma lección que la puerta de la imagen, ahora sobre
  el crate: sin ella, el empaquetado del crate solo se descubría al publicar. No hace falta un check
  requerido nuevo: va dentro de `puertas`, que ya lo es.
- La **etiqueta** `v*` publica **tres cosas**: la imagen, el **crate en crates.io** y el Release. El
  workflow `release.yml` tiene **tres jobs** —`version` → `imagen` → `crate`—, y el orden es
  deliberado: **la comprobación va la primera** y **lo irreversible, al final** —`cargo publish`;
  una versión publicada solo se puede *yank*—. `version` resuelve etiqueta, versión e imagen **una
  sola vez** y comprueba que la versión del `Cargo.toml` **es la de la etiqueta** **antes** de
  publicar nada: un desajuste (`Cargo.toml` en `0.4.0` y etiqueta `v0.4.1`) para el release entero
  sin dejar **ni** imagen **ni** Release **ni** crate. `imagen` construye, **prueba la imagen con
  docker real** (el runner sí lo tiene; aquí solo hay podman, que además descarta el `HEALTHCHECK`
  al construir en formato OCI), publica en GHCR —`latest` solo se mueve en versiones estables— y
  crea el Release, reutilizándolo si ya existe. `crate` publica con `cargo publish --locked` usando
  el secreto **`CARGO_REGISTRY_TOKEN`** (un token de crates.io; **sin él el job falla nombrando el
  secreto**) y es **idempotente**: si la versión ya está en el registro, no republica y sigue en
  verde —el `workflow_dispatch` existe justo para republicar una etiqueta—. Los **dos
  disparadores** siguen siendo el `push` de una etiqueta y el `workflow_dispatch` con el input `tag`
  obligatorio. Ojo a la asimetría: el `release.yml` construye la imagen **desde la rama**, no desde
  el árbol de la etiqueta; para una etiqueta ya publicada eso es lo que se quiere (republicar), pero
  una etiqueta que **nunca publicó** nada se puede mover al commit correcto en vez de
  reconstruirla desde la rama. **El crate, en cambio, sale del árbol de la etiqueta**, porque
  `cargo publish` publica lo que hay en el árbol de trabajo y en un `workflow_dispatch` el
  `github.sha` por defecto es la rama: esa asimetría está declarada.
- La visibilidad del paquete en GHCR **no se puede cambiar por API** (verificado: `PATCH /user/packages/...`
  da 404; no existe ni en el OpenAPI ni en GraphQL): solo a mano en los ajustes del paquete. La primera
  medición fue del paquete **viejo**, `ghcr.io/atareao/errors`, publicado antes del renombrado:
  **medido** entonces, quedó **público** —descarga anónima y `tags/list` responden `200`—, así que no
  hubo que tocar nada. El paquete **nuevo** ha seguido el mismo camino: `ghcr.io/atareao/errserve` se
  publicó con `0.3.0` (y `latest`), y la descarga anónima funciona —comprobado con un `podman pull` sin
  credenciales (`--authfile` con `{"auths":{}}`) y con `tags/list` anónimo—. Si algún día un
  `docker pull ghcr.io/atareao/errserve` responde `denied`, eso es un cambio de visibilidad y ahí está
  el arreglo a mano.
- El paquete de GHCR `ghcr.io/atareao/errors` queda **congelado en `0.2.0`**: renombrar el repositorio
  **no** renombra ni redirige el paquete, así que sigue publicado y descargable, pero **no recibe nada
  nuevo** —lo que se publique a partir de ahora va a `ghcr.io/atareao/errserve`—. Y el nombre `errors`
  **no debe reutilizarse nunca** para otro repositorio: GitHub deja redirecciones desde la ruta
  antigua del repositorio renombrado, y un repositorio nuevo con ese nombre las rompe.

## Quirks

- **Edition 2024** exige `rustc` ≥ 1.85; no bajes la edición a 2021.
- El repositorio **compila sin conexión**: usa `--offline` con Cargo y no añadas dependencias que no
  estén en la caché local.
- **El `on_failure` del `TraceLayer` va a `DEBUG` a propósito.** El clasificador por defecto marca
  todo 5xx como fallo y lo registra en `ERROR`; como este servicio *sirve* páginas 5xx, cada `503`
  normal inundaría el log de errores falsos. No lo subas de vuelta.
- `ServeDir` se construye con `append_index_html_on_directories(false)`: un servicio de errores no
  debe emitir `307`. Ojo: eso suprime **la redirección de directorio**, no todos los `3xx` — el `304`
  de una petición condicional sí aparece, y se deja pasar (garantía 6).
- La ruta de estáticos es **una sola**: `CONTENT_DIR` (defecto `content`), coherente entre el código,
  el `Dockerfile` y el `.dockerignore`. No la dupliques.
- Los tests de router usan `tower::ServiceExt::oneshot`; los de configuración no mutan el entorno
  (`Config::from_env_iter` existe para eso). No introduzcas `std::env::set_var` en pruebas.
- **La puerta de dependencias es `cargo deny check -D warnings`** (`deny.toml`, job `cargo-deny` del
  CI). Cada permiso del fichero lleva escrito su motivo: las licencias permitidas son las que hay de
  verdad en el árbol —MIT y Apache-2.0 cubren casi todo (`BSD-3-Clause` viene de `matchit`,
  `Unicode-3.0` de `unicode-ident`, y `0BSD`/`Zlib`/`BSL-1.0`/`Unlicense` son alternativas que
  nunca se usan porque la expresión las satisface por otra rama)—, y el único `skip` es la
  duplicación de `syn` 2/3, un proc-macro que no entra en el binario. El workflow **fija la versión
  de cargo-deny** (`0.20.2`, la misma en la que está verificado el fichero): el formato de `deny.toml`
  ha cambiado entre versiones —campos que se eliminan y pasan a ser error—, así que un binario nuevo
  no debe romper el CI por su cuenta. `-D warnings` hace fallar también la higiene del fichero (una
  licencia permitida que ya no use nadie).
- **El registro de peticiones existe, pero está a `DEBUG`.** `TraceLayer` emite `on_request` y
  `on_response` —método, ruta, latencia y estado—, y con el `LOG_LEVEL` por defecto (`info`) no se
  ven: solo sale la línea de arranque. Se encienden **sin tocar código**:
  `LOG_LEVEL="tower_http=debug,info"`; y para no traer de regalo el `on_failure` que marca todo `5xx`
  como fallo, `LOG_LEVEL="tower_http::trace::on_failure=off,tower_http=debug,info"`. Medido: dos
  líneas por petición —entrada y salida—, sin el falso «response failed» de cada `503`.
- **`include_str!` exige que el fichero exista al compilar, y `content/` viaja dentro del `.crate`.**
  Borrar o renombrar un `content/<código>.html` ya no degrada en silencio al respaldo de texto: es un
  **error de compilación** (D6). Y como el embedding se resuelve al compilar, `content/` tiene que
  seguir entrando en el paquete —comprobable con `cargo package --list --offline`, que lista las once
  páginas y `content/css/error.css`—: sin ese hecho el binario compila aquí y falla en la máquina de
  quien instala (D3).
  Y por eso el **stage de compilación de la imagen** también los necesita: no basta con que `content/`
  esté en el árbol, tiene que estar **en el `builder`**. Un `Dockerfile` que solo copie `content/` al
  stage de runtime **no compila** —medido: `exit code 101` y doce `couldn't read` al etiquetar la
  `0.4.0`—: es justo lo que caza el job `imagen` del CI en cada propuesta de cambio.
- **La prueba de paridad de las dos copias vive en el `#[cfg(test)]` de `src/embedded.rs`, y no en
  `tests/`.** El crate no tiene *lib target*, así que una prueba de integración no ve las constantes
  del binario; para verlas desde `tests/` habría que añadir un `lib` y cambiar la forma del crate
  entera (D5).
- **El respaldo de assets embebidos usa `ServeDir::fallback`, nunca `not_found_service`.**
  `not_found_service` es literalmente `fallback(SetStatus::new(f, NOT_FOUND))`, y `SetStatus` escribe
  el estado «regardless of what the inner service returns»: el `200 text/css` de la hoja saldría como
  un `404` **con cuerpo de CSS** y el middleware lo sustituiría por la página. Con `fallback` nadie
  corrige el estado del respaldo, así que **todo** path fuera de la tabla de assets tiene que
  responder `404` a mano (D2).
- **Ese respaldo corre en el hilo *async*** —a diferencia del render, que va en `spawn_blocking` por
  la auditoría I-1—, así que **no puede hacer E/S de disco**: los bytes son una tabla en memoria. Es
  también la razón de que sea un `Router` de axum y no un `struct` con `impl Service`:
  `tower::Service` solo es dependencia de desarrollo y no se puede nombrar en producción.
- **El `Dockerfile` copia `content/` dos veces, y la primera no es un «por si acaso»: es una entrada de
  la compilación.** El stage `builder` necesita las páginas y la hoja **para compilar** —las macros
  resuelven la ruta en tiempo de compilación—, así que su `COPY content ./content` es **obligatorio**:
  sin él, el `cargo build` del crate real muere y no hay imagen. La segunda copia, la del stage
  `runtime`, es la decisión que sigue abierta: `CONTENT_DIR` es la única ruta de estáticos y, como la
  capa del disco gana (D1), la imagen sirve esos ficheros y no los del binario, de modo que quitarla
  cambiaría qué bytes se sirven en producción. Es la pregunta abierta de `design.md`, y ahí sigue.
- **La publicación del crate tiene su propia puerta y su propio aviso, y los dos son deliberados.**
  `cargo publish --dry-run --locked` es la puerta del paquete en `puertas`, y es real: compila el
  paquete **extraído** (`target/package/errserve-<versión>`), así que un `content/` que no viaje en
  el `.crate` rompe el CI. Y no da falsos rojos por las dos causas que parecen probables: **no
  necesita token** (un PR de fork no rompe) y sobre una versión **ya publicada** solo avisa
  («already exists on crates.io index») y termina en verde. En `release.yml`, ni republicar falla ni
  falla avisar de que el `.crate` publicado venga de otro commit: el job `crate` compara el sha1 que
  `cargo package` deja dentro del `.crate` (`.cargo_vcs_info.json`) con el commit del árbol, y si no
  coincide **avisa** —`::warning::` y una línea en el resumen— y **sigue**. Los dos datos que lo
  justifican: en esa rama ya no hay nada que reparar (la publicación es irreversible y está hecha, y
  un rojo solo bloquearía republicar la imagen y reutilizar el Release, que es el camino que el
  `workflow_dispatch` existe para hacer), y el `.crate` publicado de la `0.3.0` **a mano** registra
  `bdb76fa…`, que **no** es el commit de su etiqueta `v0.3.0` (`d0032a2…`): una puerta dura daría
  rojo en la primera republicación legítima. El `ref` del `checkout` va **cualificado a
  `refs/tags/…`**, porque un `ref` sin cualificar se resuelve prefiriendo una **rama** del mismo
  nombre a la etiqueta.

## Referencia de implementación (qué no repetir)

`/data/rust/simpleserver` — proyecto hermano del mismo autor (atareao), de donde sale el stack
(`axum` + `tokio` + `tower-http` + `tracing`). Dos incoherencias suyas que aquí **no** se copian:
(a) declara `const STATIC_DIR = "static"` pero su `Dockerfile` copia `content/` —aquí la ruta es una
sola—; y (b) lee `LOG_LEVEL`/`PORT` del entorno **y** declara un `config.yml` que nunca lee —aquí la
configuración entra solo por el entorno—.
