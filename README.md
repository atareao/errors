# errors

Servidor de páginas de error para **Traefik**, escrito en Rust. Es un clon
deliberadamente mínimo de [tarampampam/error-pages][ep] con **una diferencia de
fondo**:

> `error-pages` responde **siempre HTTP 200**. Este servidor devuelve el
> **código de error real**.

[ep]: https://github.com/tarampampam/error-pages

## Por qué existe

La suposición de `error-pages` (el proxy reescribe solo el cuerpo y conserva el
código del upstream) se cumple con Nginx o ingress-nginx. **No** se cumple con el
patrón de Traefik que documenta su propia guía: un router de respaldo
`HostRegexp(.+)` con prioridad baja, sin upstream cuyo código preservar. Ahí el
`200` del servicio de errores llega tal cual al cliente y una página «404» se
sirve como `200 OK`. Este servicio cierra ese hueco.

## Contrato con Traefik

El middleware `errors` reenvía la petición con la plantilla `errors.query`, así
que **el código viaja en el path** (`/404`, `/503`):

```yaml
traefik.http.middlewares.error-pages-middleware.errors.status: 400-599
traefik.http.middlewares.error-pages-middleware.errors.service: error-pages-service
traefik.http.middlewares.error-pages-middleware.errors.query: /{status}
traefik.http.services.error-pages-service.loadbalancer.server.port: 8080
```

La respuesta lleva el **código resuelto** como estado HTTP. Nunca 200.

## Ejemplos

```sh
curl -i  localhost:8080/404                  # 404 Not Found (no 200), con X-Robots-Tag
curl -i  localhost:8080/503                  # 503 Service Unavailable, con Retry-After: 120
curl -i  -H 'X-Code: 503' localhost:8080/    # 503 cuando el path no trae código
curl -s  localhost:8080/404.json             # {"error":true,"code":404,...}
curl -s  -H 'X-Format: application/xml' localhost:8080/404
curl -i  localhost:8080/healthz              # 200
curl -s  localhost:8080/version              # {"version":"..."}
```

## Configuración (solo variables de entorno)

| Variable | Defecto | Para qué |
|---|---|---|
| `PORT` | `8080` | Puerto de escucha (bind `0.0.0.0`). El que asume la guía de Traefik. |
| `LOG_LEVEL` | `info` | Filtro de `tracing-subscriber`. |
| `CONTENT_DIR` | `content` | Directorio de páginas HTML y assets. |
| `DEFAULT_ERROR_PAGE` | `404` | Código cuando el path y `X-Code` no aportan uno utilizable. |

No se lee ningún fichero de configuración. Una variable **presente pero vacía**
es un error de arranque, no un valor por defecto. `DEFAULT_ERROR_PAGE` fuera del
rango `400..=599` también lo es.

## Cómo se resuelve

**Código** (primero que haga match): path (`/404`, `/404.json`, `/503.xml`…) →
cabecera `X-Code` → `DEFAULT_ERROR_PAGE`. Una fuente con un código fuera del
rango `400..=599` se ignora y se pasa a la siguiente.

**Formato** (primero que haga match): extensión del path (`.html`/`.htm`,
`.json`, `.xml`, `.txt`) → `Content-Type` de la petición → `X-Format` →
`Accept` → **texto plano** por defecto.

Código y formato son ortogonales: `/503.xml` es un 503 en XML.

Cada página de error lleva `Content-Type`, `Content-Length`,
`X-Robots-Tag: noindex, nofollow, nosnippet, noarchive` y, en `429` y `503`,
`Retry-After: 120`.

El HTML se sirve de `CONTENT_DIR/<código>.html` (con caída al fichero del código
por defecto); JSON, XML y texto se generan del código, su razón canónica y su
descripción.

## Códigos cubiertos

Once códigos tienen página propia: `400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`, `502`,
`503` y `504`. Cualquier otro cae al fichero del código por defecto, pero **el estado sigue siendo
el código resuelto**: nunca verás un `504` servido como `404`.

Todas las páginas comparten una sola hoja, `css/error.css`, y cada una se identifica con la clase
de su código (`<body class="c504">`), que es lo que permite diferenciar su aspecto sin duplicar la
hoja once veces.

## Quién decide el código HTTP

Depende de cómo esté montado el servicio detrás de Traefik, y conviene saberlo antes de confiar en
él:

| Montaje | Quién decide el estado | Nuestro código |
|---|---|---|
| Middleware `errors` sobre una aplicación | **Traefik**, que conserva el código del upstream | se ignora |
| Acceso **directo** (router *catch-all*, *default backend*) | **nosotros** | es el que llega al cliente |

Medido, no deducido: con el middleware interceptando, una configuración divergente —backend `503`
con el middleware pidiendo `/404`— hizo llegar al cliente un `503`. El caso que justifica este
proyecto es el segundo: ahí `error-pages` devuelve `200` y nosotros, el código real. El cuerpo y las
cabeceras (`Retry-After`, `X-Robots-Tag`, `Content-Type`, negociación por `Accept`) sí son nuestros
en los dos casos.

## Empaquetado

```sh
just build          # imagen (docker por defecto; `just engine=podman build` para podman)
just run            # contenedor en 8080
just dev            # cargo run en el host, leyendo content/
```

La imagen es multi-stage, con enlace estático a musl, usuario no-root (uid 10001)
y ~12 MB.

## Desarrollo

```sh
cargo run
cargo test
cargo fmt --check && cargo clippy -- -D warnings && cargo test   # antes de commitear
```

El repositorio compila **sin conexión**: las dependencias se resuelven contra la
caché local de Cargo (`cargo check --offline`).

## No incluye (a propósito)

Sin motor de plantillas, sin localización, sin gzip, sin copia de
`--proxy-headers`, sin generador estático y sin TLS (lo termina Traefik). Son
extras de `error-pages` que no hacen falta detrás de un proxy.

## Licencia

MIT.
