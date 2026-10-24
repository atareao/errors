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

## Integración con Traefik

Hay un **ejemplo ejecutable** en [`examples/traefik/`](examples/traefik/) —Traefik v3 + este servicio
+ un backend que devuelve 4xx/5xx— con un `check.sh` que comprueba los cuatro casos, y un
[`compose.yml`](compose.yml) en la raíz listo para producción (solo el servicio, con los labels de
Traefik para descomentar). El ejemplo explica además el matiz de quién decide el código HTTP.

Los dos usan la **imagen publicada** (`ghcr.io/atareao/errors`), así que se levantan sin construir
nada; `ERRORS_IMAGE` apunta a otra etiqueta o a una imagen local si hace falta.

Una trampa que conviene conocer antes de copiar una regla de internet: ``HostRegexp(`{host:.+}`)`` es
sintaxis de Traefik **v2** y en v3 **no casa con nada** —el router aparece `enabled` y Traefik
responde su propio 404, sin llegar al servicio—. En v3 es ``HostRegexp(`.+`)``.

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
| `LOG_FORMAT` | `text` | Formato del registro: `text` o `json`. |

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

## Operación y observabilidad

`GET /metrics` expone en texto Prometheus el contador de páginas servidas por código
(`errors_pages_total{code="404"}`). Es un endpoint de servicio: siempre `200`, fuera de la
resolución de páginas de error.

La respuesta se comprime cuando el cliente envía `Accept-Encoding: gzip` —y entonces responde
con codificación por trozos, sin `Content-Length`—, y declara `Vary: Accept-Encoding`. Las
cabeceras de correlación `X-Request-Id`, `X-Trace-Id`, `X-Correlation-Id` y `X-Amzn-Trace-Id`
se copian de la petición a la respuesta cuando vienen; no se inventan.

## Imagen

La imagen publicada está en **GHCR**, no en Docker Hub:

```sh
docker pull ghcr.io/atareao/errors:latest     # o una versión fija: :0.1.0
```

El [`compose.yml`](compose.yml) de la raíz y el ejemplo de [`examples/traefik/`](examples/traefik/)
la usan por defecto (`ERRORS_IMAGE` cambia la referencia), así que **no hay que construir nada** para
probarlos. Si el `pull` responde `denied` sin credenciales, el paquete sigue privado: ver
[Publicación](#publicación).

Es multi-stage, con enlace estático a musl, usuario no-root (uid 10001) y ~12 MB.

Para construirla en local (docker por defecto; `just engine=podman` para podman):

```sh
just build          # imagen, con la versión del último tag de git y `latest`
just run            # contenedor en 8080
just dev            # cargo run en el host, leyendo content/
just login          # entra en GHCR (una vez por máquina)
just push           # publica a mano la versión y `latest`
```

## CHANGELOG

Se mantiene con [`git-cliff`](https://git-cliff.org) a partir de los commits convencionales:

```sh
just changelog              # antepone la sección de lo que aún no está etiquetado
just changelog v0.2.0       # lo mismo, ya con el nombre del tag que se va a crear
```

Antepone, no regenera: la entrada de `0.1.0` está escrita a mano y el fichero no se pierde.

## Dependencias

`cargo deny check -D warnings` (con `deny.toml`) revisa avisos de seguridad, licencias, duplicaciones
y fuentes. Corre en el CI, en el job `cargo-deny`:

```sh
cargo deny check -D warnings
```

## Publicación

Se publica **con una etiqueta**:

```sh
git tag v0.1.0 && git push origin v0.1.0
```

Eso dispara [`.github/workflows/release.yml`](.github/workflows/release.yml), que construye la
imagen, la **prueba con docker de verdad** —que `/404` devuelva 404, el `HEALTHCHECK` del
`Dockerfile` y el `USER 10001:10001`; en la máquina de desarrollo no hay docker, solo podman, que
además descarta el healthcheck al construir en formato OCI—, la empuja a GHCR con su versión y
`latest` y crea el Release con las notas generadas. Un `workflow_dispatch` republica una etiqueta ya
existente, pero exige indicarla en el input `tag`: en un dispatch no hay etiqueta y
`github.ref_name` es la rama.

`main` y `development` están **protegidas**: todo entra por **PR** y ningún PR se fusiona hasta que
el check **`fmt, clippy y tests`** está en verde. El release no sustituye a ese check.

### La visibilidad del paquete

En GitHub **no se puede cambiar la visibilidad de un paquete por API**: el
`PATCH /user/packages/{type}/{name}` que circula por ahí ya no existe (medido: responde `404`, no está
en el OpenAPI de GitHub y en GraphQL no hay mutación de paquetes), así que solo se toca a mano en los
ajustes del paquete.

En este repositorio, **medido tras la primera publicación**, el paquete quedó **público**: la
descarga anónima funciona. Si algún día `docker pull ghcr.io/atareao/errors` responde `denied`, es
que el paquete quedó privado: se arregla una sola vez en
<https://github.com/users/atareao/packages/container/errors/settings> » *Danger Zone* » *Change
visibility* » **Public**.

## Desarrollo

```sh
cargo run
cargo test
cargo fmt --check && cargo clippy -- -D warnings && cargo test   # antes de commitear
```

El repositorio compila **sin conexión**: las dependencias se resuelven contra la
caché local de Cargo (`cargo check --offline`).

## No incluye (a propósito)

Sin motor de plantillas, sin localización, sin generador estático y sin TLS (lo
termina Traefik). Se copian **solo** las cuatro cabeceras de correlación, no el
conjunto completo de `--proxy-headers`, y la compresión se limita a gzip.

## Licencia

MIT.
