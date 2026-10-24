# errors — AGENTS.md

> Las directivas globales de este entorno (OpenSpec SDD + TDD, tags de estado, tabla de subagentes)
> se inyectan ya en cada sesión desde `~/.config/opencode/AGENTS.md`. No se repiten aquí: lo de abajo
> es lo específico de este repositorio.

## Estado real del repositorio (verificado)

Implementado, verificado y versionado. El change `error-pages-parity` está **archivado**
(`openspec/changes/archive/2026-10-24-error-pages-parity/`) y sus requisitos viven fusionados en
`openspec/specs/error-page/` y `openspec/specs/http-server/`.

- `src/main.rs` — arranque, router, middleware de páginas de error y los tests de router.
- `src/config.rs` — configuración desde el entorno (`Config`, `ConfigError`, `from_env_iter`).
- `src/path.rs` — normalización del path (percent-decode + plegado de `.`/`..`).
- `src/resolve.rs` — cascadas de código y formato, rango `400..=599`, tabla de razones.
- `src/render.rs` — cuerpo por formato y cabeceras.
- `content/` — **once** páginas HTML (`400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`, `502`, `503`, `504`), todas con la clase `c<código>` en el `body`, y la hoja única `content/css/error.css`.
- `Dockerfile`, `.dockerignore`, `.justfile` — empaquetado (imagen ~12 MB, no-root uid 10001).
- `README.md` — el contrato con Traefik y los ejemplos.

Puertas en verde: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y **101 tests**
(98 unitarios + 3 de integración sobre el contenido de `content/`).
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
   error*, no a los assets: `GET /css/error.css` es un `200 text/css`. No conviertas un asset en página
   de error por la cabecera `X-Code`.

## Configuración (solo entorno)

| Variable | Defecto | Para qué |
|---|---|---|
| `PORT` | `8080` | Puerto de escucha (bind `0.0.0.0`). |
| `LOG_LEVEL` | `info` | Filtro de registro. |
| `CONTENT_DIR` | `content` | Páginas HTML y assets. |
| `DEFAULT_ERROR_PAGE` | `404` | Código por defecto; fuera de `400..=599` es error de arranque. |
| `LOG_FORMAT` | `text` | `text` o `json`; cualquier otro valor, o vacía, es error de arranque. |

`GET /metrics` es un endpoint de servicio (siempre `200`, fuera de la resolución de páginas de
error) que expone el contador Prometheus de páginas servidas por código. El contador vive en el
estado del router, no en un `static`: por eso dos routers no se contaminan entre sí.

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
`X-Robots-Tag: noindex, nofollow, nosnippet, noarchive`, y `Retry-After: 120` **solo** en `429` y
`503`.

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
cargo test                                  # 101 tests
cargo test <substring>                      # un test por nombre
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo check --offline                       # el repo compila SIN RED (caché local de Cargo)
just build | run | dev                      # empaquetado (docker por defecto; just engine=podman ...)
```

Orden obligatorio antes de commit: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

## Quirks

- **Edition 2024** exige `rustc` ≥ 1.85; no bajes la edición a 2021.
- El repositorio **compila sin conexión**: usa `--offline` con Cargo y no añadas dependencias que no
  estén en la caché local.
- **El `on_failure` del `TraceLayer` va a `DEBUG` a propósito.** El clasificador por defecto marca
  todo 5xx como fallo y lo registra en `ERROR`; como este servicio *sirve* páginas 5xx, cada `503`
  normal inundaría el log de errores falsos. No lo subas de vuelta.
- `ServeDir` se construye con `append_index_html_on_directories(false)`: un servicio de errores no
  debe emitir `307`.
- La ruta de estáticos es **una sola**: `CONTENT_DIR` (defecto `content`), coherente entre el código,
  el `Dockerfile` y el `.dockerignore`. No la dupliques.
- Los tests de router usan `tower::ServiceExt::oneshot`; los de configuración no mutan el entorno
  (`Config::from_env_iter` existe para eso). No introduzcas `std::env::set_var` en pruebas.

## Referencia de implementación (qué no repetir)

`/data/rust/simpleserver` — proyecto hermano del mismo autor (atareao), de donde sale el stack
(`axum` + `tokio` + `tower-http` + `tracing`). Dos incoherencias suyas que aquí **no** se copian:
(a) declara `const STATIC_DIR = "static"` pero su `Dockerfile` copia `content/` —aquí la ruta es una
sola—; y (b) lee `LOG_LEVEL`/`PORT` del entorno **y** declara un `config.yml` que nunca lee —aquí la
configuración entra solo por el entorno—.
