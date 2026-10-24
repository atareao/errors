# errors — AGENTS.md

> Las directivas globales de este entorno (OpenSpec SDD + TDD, tags de estado, tabla de subagentes)
> se inyectan ya en cada sesión desde `~/.config/opencode/AGENTS.md`. No se repiten aquí: lo de abajo
> es lo específico de este repositorio.

## Estado real del repositorio (verificado)

Implementado, verificado y versionado. Cinco changes están **archivados** en
`openspec/changes/archive/` —`error-pages-parity`, `error-page-coverage`, `release-hardening`,
`security-hardening` y `retry-after-configurable`, los cinco con fecha `2026-10-24`— y sus
requisitos viven fusionados en los cuatro specs de `openspec/specs/`: `error-page`, `http-server`,
`observability` y `response-extras`.

- `src/main.rs` — arranque, router, middleware de páginas de error, la lista `SERVICE_ENDPOINTS` que acota ese middleware, y los tests de router.
- `src/config.rs` — configuración desde el entorno (`Config`, `ConfigError`, `from_env_iter`).
- `src/path.rs` — normalización del path (percent-decode + plegado de `.`/`..`).
- `src/resolve.rs` — cascadas de código y formato, rango `400..=599`, tabla de razones.
- `src/render.rs` — cuerpo por formato y cabeceras.
- `src/metrics.rs` — contador de páginas servidas por código **y por origen del código**, y su serialización en texto Prometheus.
- `content/` — **once** páginas HTML (`400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`, `502`, `503`, `504`), todas con la clase `c<código>` en el `body`, y la hoja única `content/css/error.css`.
- `Dockerfile`, `.dockerignore`, `.justfile`, `compose.yml` — empaquetado (imagen ~12 MB, no-root uid 10001); el `compose.yml` va endurecido: `read_only: true`, `cap_drop: [ALL]`, `no-new-privileges:true` e `init: true`.
- `README.md` — el contrato con Traefik y los ejemplos.

Puertas en verde: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y **124 tests**
(121 unitarios + 3 de integración sobre el contenido de `content/`).
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

## Configuración (solo entorno)

| Variable | Defecto | Para qué |
|---|---|---|
| `PORT` | `8080` | Puerto de escucha (bind `0.0.0.0`). |
| `LOG_LEVEL` | `info` | Filtro de registro. |
| `CONTENT_DIR` | `content` | Páginas HTML y assets. |
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
se protege con un middleware de Traefik, que es donde va la autenticación. `/version` se mantiene a
la vista por paridad deliberada con `error-pages`.

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
cargo test                                  # 124 tests
cargo test <substring>                      # un test por nombre
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo check --offline                       # el repo compila SIN RED (caché local de Cargo)
cargo deny check -D warnings                # dependencias: avisos, licencias, duplicaciones y fuentes
just changelog [v0.2.0]                     # antepone al CHANGELOG la sección de lo no etiquetado
just build | run | dev                      # empaquetado (docker por defecto; just engine=podman ...)
```

Orden obligatorio antes de commit: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

## CHANGELOG

Se mantiene con [`git-cliff`](https://git-cliff.org) (`cliff.toml`) y la receta `just changelog`, que
**antepone** (`--prepend`) la sección de lo que todavía no está etiquetado:

- `just changelog` escribe la sección «Sin publicar» (los commits posteriores al último tag).
- `just changelog v0.2.0` escribe esa misma sección ya con el nombre del tag que se va a crear: es lo
  que se hace justo antes de `git tag`.
- La entrada de `0.1.0` está **escrita a mano** y es más rica que lo que saldría de los commits: el
  fichero **no se regenera**, así que se conserva.
- git-cliff no detecta una sección repetida y la escribiría dos veces, así que la receta **se niega**
  a anteponer una sección que ya esté en el fichero: para rehacerla, se borra antes a mano.

## Entrega

- `main` y `development` están **protegidas**: PR obligatorio, el check `fmt, clippy y tests` en
  verde y `strict` (la rama ha de estar al día). `enforce_admins` está activo, así que **tampoco el
  dueño empuja directo**. El flujo es: rama `feature/...` desde `development` → PR a `development` →
  PR de `development` a `main`.
- El CI (`.github/workflows/ci.yml`) tiene **dos jobs**: `fmt, clippy y tests` (las tres puertas) y
  `cargo-deny` (la puerta de dependencias, con `deny.toml`). En la protección de rama **solo está
  marcado como requerido el primero**; el de `cargo-deny` corre en cada PR pero su rojo no bloquea el
  merge hasta que se añada a `required_status_checks` (medido con la API: los `contexts` de `main` y
  de `development` son `["fmt, clippy y tests"]`).
- La imagen y el Release se publican empujando una **etiqueta** `v*`: el workflow `release.yml`
  construye, **prueba la imagen con docker real** (el runner sí lo tiene; aquí solo hay podman, que
  además descarta el `HEALTHCHECK` al construir en formato OCI), publica en GHCR y crea el Release.
  La etiqueta `latest` solo se mueve en versiones estables.
- La visibilidad del paquete en GHCR **no se puede cambiar por API** (verificado: `PATCH /user/packages/...`
  da 404; no existe ni en el OpenAPI ni en GraphQL): solo a mano en los ajustes del paquete. En este
  repositorio, **medido** tras la primera publicación, el paquete quedó **público** —descarga anónima
  y `tags/list` responden `200`—, así que no hubo que tocar nada; pero si algún día un
  `docker pull ghcr.io/atareao/errors` responde `denied`, ese es el motivo y ahí está el arreglo.

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

## Referencia de implementación (qué no repetir)

`/data/rust/simpleserver` — proyecto hermano del mismo autor (atareao), de donde sale el stack
(`axum` + `tokio` + `tower-http` + `tracing`). Dos incoherencias suyas que aquí **no** se copian:
(a) declara `const STATIC_DIR = "static"` pero su `Dockerfile` copia `content/` —aquí la ruta es una
sola—; y (b) lee `LOG_LEVEL`/`PORT` del entorno **y** declara un `config.yml` que nunca lee —aquí la
configuración entra solo por el entorno—.
