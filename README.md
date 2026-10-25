# errserve

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

La respuesta lleva el **código resuelto** como estado HTTP. Nunca 200. Y eso vale también para lo
que sirve el servidor de ficheros: cualquier respuesta suya **con estado de error** —no solo su
`404`: también el `405` de un método que no sirve estáticos, el `416` de un `Range` múltiple y el
`500` de un segmento de path que no cabe como nombre de fichero— se sustituye por la página de
error, con **el código resuelto** (path → `X-Code` → `DEFAULT_ERROR_PAGE`), **nunca** con su estado:
el estado del servidor de ficheros no es una fuente de la cascada.

Con el binario anterior se veía el hueco: `GET /<300 caracteres>` respondía `500` **con el cuerpo
vacío** (`content-length: 0`), sin `Content-Type`, sin `X-Robots-Tag` y sin que ninguna página lo
hubiera producido; `PUT /css/error.css` respondía `405` y `Range: bytes=0-1,3-4` sobre un asset
respondía `416`, los dos sin cuerpo y sin pasar por la cascada.

Y hay **tres razones medidas** para no conservar su estado:

- el `405` saldría **sin el `Allow`** que RFC 9110 exige: la página la construimos nosotros y esa
  cabecera describe *su* respuesta, no la nuestra;
- el estado y el cuerpo quedarían en desacuerdo —un `404` con `Retry-After: 120` y el cuerpo del
  `503`—, contra la regla de que el `404` no lleva `Retry-After`;
- con `DEFAULT_ERROR_PAGE=503`, cualquier asset inexistente saldría `404`, dejando sin efecto a
  `X-Code`, que existe justo para los despliegues que no reescriben el path.

Un asset **existente** sigue ganando, que es la otra mitad de la regla: `GET /css/error.css` es un
`200 text/css` y no pasa por la página de error.

## Integración con Traefik

Hay un **ejemplo ejecutable** en
[`examples/traefik/`](https://github.com/atareao/errserve/tree/main/examples/traefik/) —Traefik v3 +
este servicio + un backend que devuelve 4xx/5xx— con un `check.sh` que comprueba los cuatro casos, y
un [`compose.yml`](https://github.com/atareao/errserve/blob/main/compose.yml) en la raíz listo para
producción (solo el servicio, con los labels de Traefik para descomentar). El ejemplo explica además
el matiz de quién decide el código HTTP.

Los dos usan la **imagen publicada** (`ghcr.io/atareao/errserve`), así que se levantan sin construir
nada; `ERRSERVE_IMAGE` apunta a otra etiqueta o a una imagen local si hace falta.

Una trampa que conviene conocer antes de copiar una regla de internet: ``HostRegexp(`{host:.+}`)`` es
sintaxis de Traefik **v2** y en v3 **no casa con nada** —el router aparece `enabled` y Traefik
responde su propio 404, sin llegar al servicio—. En v3 es ``HostRegexp(`.+`)``. Es el mismo tipo de
trampa de copia-pega que la del *catch-all*: si copias esa regla, mira el párrafo de `/metrics` en
«Operación y observabilidad», porque además lleva una exclusión.

## Ejemplos

```sh
curl -i  localhost:8080/404                  # 404 Not Found (no 200), con X-Robots-Tag
curl -i  localhost:8080/503                  # 503 Service Unavailable, con Retry-After: 120
curl -i  -H 'X-Code: 503' localhost:8080/    # 503 cuando el path no trae código
curl -s  localhost:8080/404.json             # {"error":true,"code":404,...}
curl -s  -H 'X-Format: application/xml' localhost:8080/404
curl -i  localhost:8080/healthz              # 200
curl -s  localhost:8080/version              # {"version":"..."}
curl -s  -H 'Accept: text/html' -H 'X-Theme: terminal' localhost:8080/404   # con la hoja del tema `terminal` dentro
```

## Configuración (solo variables de entorno)

| Variable | Defecto | Para qué |
|---|---|---|
| `PORT` | `8080` | Puerto de escucha (bind `0.0.0.0`). El que asume la guía de Traefik. |
| `LOG_LEVEL` | `info` | Filtro de `tracing-subscriber`. |
| `CONTENT_DIR` | `content` | *Override* de las páginas HTML y los assets **embebidos** en el binario. |
| `DEFAULT_ERROR_PAGE` | `404` | Código cuando el path y `X-Code` no aportan uno utilizable. |
| `LOG_FORMAT` | `text` | Formato del registro: `text` o `json`. |
| `RETRY_AFTER` | `120` | Segundos de la cabecera `Retry-After`. La llevan solo el `429` y el `503`. |
| `THEME` | `default` | Tema de las páginas **HTML**: `default`, `minimal`, `terminal`, `paper`, `amber` o `compact`. |

No se lee ningún fichero de configuración. Una variable **presente pero vacía**
es un error de arranque, no un valor por defecto. `DEFAULT_ERROR_PAGE` fuera del
rango `400..=599` también lo es, y un `THEME` que no sea uno de los seis temas
—o que venga vacío— aborta el arranque igual, con un error que **nombra la
variable** y **enumera los seis nombres**: quien escribe `THEME` cree que está
configurando algo, así que el error tiene que decirle qué puede escribir en su
lugar.

`RETRY_AFTER` es el único valor de la respuesta que depende de tu infraestructura: la cabecera
`Retry-After` es lo que se le promete al cliente sobre cuándo volver, así que si tu backend tarda
cinco minutos en recuperarse, `120` es una mentira que el cliente obedece. Admite cualquier entero de
segundos —`0` significa «reintenta ya»— y no tiene techo. Una variable vacía, o que no sea un entero
de segundos, es un error de arranque. Lo que **no** se configura es qué códigos la llevan: son el
`429` y el `503`, y eso lo decide el código.

`CONTENT_DIR` es solo un ***override***. El binario lleva **dentro** la página —una plantilla— y su
hoja, así que un `cargo install errserve` —que deja un ejecutable y ningún fichero de datos, con
`CONTENT_DIR` apuntando por defecto a un `content` que no existe— sirve la página de verdad y su hoja,
y no el respaldo de texto. Lo que haya en `CONTENT_DIR` gana **cuando se pueda leer**; lo que falte se
sirve desde la copia embebida.

## Cómo se resuelve

**Código** (primero que haga match): path (`/404`, `/404.json`, `/503.xml`…) →
cabecera `X-Code` → `DEFAULT_ERROR_PAGE`. Una fuente con un código fuera del
rango `400..=599` se ignora y se pasa a la siguiente.

**Formato** (primero que haga match): extensión del path (`.html`/`.htm`,
`.json`, `.xml`, `.txt`) → `Content-Type` de la petición → `X-Format` →
`Accept` → **texto plano** por defecto.

Código y formato son ortogonales: `/503.xml` es un 503 en XML.

Y hay un **tercer eje**, que no reparte códigos ni formatos sino aspecto, y que solo toca al HTML: la
hoja que va incrustada. La elige `THEME` —el tema de todo el servicio— y, por petición, la cabecera
`X-Theme`, que gana a `THEME` cuando nombra uno de los seis temas; un `X-Theme` ausente, desconocido o
vacío **no** es un error y cae al tema configurado. En JSON, XML y texto no hay hoja que incrustar, así
que ahí el tema se ignora (ver «Páginas autocontenidas y temas»).

Cada página de error lleva `Content-Type`, `Content-Length`,
`X-Robots-Tag: noindex, nofollow, nosnippet, noarchive` y, en `429` y `503`,
`Retry-After` con el valor de `RETRY_AFTER` (`120` por defecto).

El HTML se sirve de `CONTENT_DIR/<código>.html` (con caída al fichero del código
por defecto) y, agotadas esas dos lecturas, de la **plantilla embebida** en el
binario, rellenada con el código resuelto; JSON, XML y texto se generan del
código, su razón canónica y su descripción. Las cuatro formas de un mismo código
llevan la **misma** razón y la **misma** descripción: las tablas de `resolve` son
su fuente única.

## Códigos cubiertos

**Cualquier** código de `400..=599` tiene página, no solo once: la página es una **plantilla**
(`content/404.html`) que el servidor rellena al vuelo, así que un `451`, un `418` o un `599` salen
con **su propio** código, su razón y su descripción, y con **su propio** estado —nunca verás un `504`
servido como `404`, ni un cuerpo que diga `404` para otro código—. Quince códigos traen su
descripción escrita en el código (`400`, `401`, `403`, `404`, `405`, `408`, `410`, `418`, `422`,
`429`, `500`, `501`, `502`, `503` y `504`); el resto usa una genérica, siempre con la razón canónica
del código en el título.

La plantilla lleva tres marcadores que el servidor sustituye **al servir**:

| Marcador | Se sustituye por |
|---|---|
| `{{code}}` | El código resuelto (`404`), allí donde aparezca: el `<title>`, el `<h1>` y la clase del `body` (`class="c{{code}}"`). |
| `{{reason}}` | La razón canónica del código (`Not Found`). |
| `{{description}}` | La descripción del código (la tabla de `resolve`). |

Todo comparte una sola hoja, `css/error.css` —que **también va embebida**, y es tan *override* como
la página—, y la clase del `body` (`c504`, `c503`…) es lo que permite diferenciar el aspecto de cada
código sin duplicar la hoja. Esa hoja es el tema `default`, y el `<link>` que la referencia en el
**fichero** es el marcador que el servidor sustituye al servir (ver «Páginas autocontenidas y
temas»).

### Cómo cambiar el texto de una página de error

Hay tres vías, de la más global a la más local:

1. **El texto de todos los códigos**: edita la tabla de `resolve::description` en `src/resolve.rs` y
   recompila. Es la **fuente única** del texto de los cuatro formatos (HTML, JSON, XML y texto), así
   que el cambio vale para todos ellos a la vez.
2. **Una página propia para un código**: deja tu fichero en `CONTENT_DIR/<código>.html` (por ejemplo
   `content/451.html`). Gana a la plantilla del proyecto y, si **no** lleva marcadores, se sirve **tal
   cual**, byte a byte: su texto fijo es tuyo. Un `CONTENT_DIR/<código por defecto>.html` (por
   defecto, `content/404.html`) sirve de respaldo para cualquier código que no tenga el suyo.
3. **Una página propia que se adapte al código**: escribe los marcadores `{{code}}`, `{{reason}}` y
   `{{description}}` en tu propia página y el servidor la rellenará con el código resuelto, igual que
   hace con la del proyecto. Un `CONTENT_DIR/<código>.html` con marcadores cubre así cualquier código
   sin que tengas que escribir un fichero por código.

## Páginas autocontenidas y temas

El HTML que sirve este servicio es **autocontenido**: lleva la hoja de estilos **dentro**, así que la
página no pide ningún fichero para verse. No es una preferencia estética, es el arreglo de un fallo
medido. Con el middleware `errors` de Traefik —el montaje de cualquier aplicación con dominio
propio—, la página se sirve **en el host de la aplicación**, donde `/css/error.css` lo contesta el
upstream, no este servicio: el navegador pide la hoja, recibe su `404` con `Content-Type: text/plain`,
el middleware intercepta ese `404` y lo vuelve a pedir como página de error con un `Accept` que no
negocia HTML —`text/css,*/*;q=0.1`—, así que la cascada de formato cae al **texto plano** y el
navegador rechaza la hoja: *«MIME type ('text/plain') is not a supported stylesheet MIME type»*. El
resultado era un `503` correcto, con el código correcto, y la página **sin estilos**. En el host del
respaldo el mismo enlace sí resolvía —un asset existente gana a la página de error—, y esa asimetría
escondía el fallo.

`error-pages` no lo sufre porque sus páginas son «fully self-contained — all styles and images are
embedded directly in the HTML, without loading any external resources»: esto es **paridad**, no un
adorno.

Cómo se hace, sin romper nada de lo de arriba:

- Las **páginas** de `CONTENT_DIR` conservan su `<link rel="stylesheet" href="/css/error.css" />` y
  el servidor lo sustituye por un bloque `<style>` **al servir**. El fichero conserva el enlace a
  propósito: sigue siendo legible y previsualizable, y así no se duplica la página por tema.
- Una página **sin** el marcador —una página propia, ya autocontenida— se sirve **byte a byte**, sin
  añadir ni sustituir nada; y en ese caso el servicio no lee ninguna hoja, así que un CSS roto no
  puede afectar a una página que no lo necesita.
- Un `<link>` que **no** sea el marcador no se toca: es tuyo.
- `/css/error.css` **sigue sirviéndose** como asset, con su `200 text/css`: no es una página de error
  y hay instalaciones que lo enlazan. Y la hoja que se incrusta y la que se sirve son **la misma**,
  resuelta con la misma regla —tu fichero, si se puede leer; si no, la copia embebida—, así que no
  pueden discrepar. Un fallo al leerla **no** tumba la página: deja un `warn` en el registro y se usa
  la copia embebida.

### Los seis temas

Un tema es **una sola hoja**: elegirlo no duplica la página.

| Tema | Hoja en `CONTENT_DIR` | Aspecto |
|---|---|---|
| `default` | `css/error.css` | El tema sobrio de la casa, y el **único** que anima: un fundido de 180 ms sobre el contenido, que se apaga con `prefers-reduced-motion: reduce`. Sus reglas pueden cambiar de una versión a otra; lo que no cambia es que sigue siendo el tema por defecto y que su hoja sigue en `css/error.css` —y por tanto como asset público `/css/error.css`—. |
| `minimal` | `themes/minimal.css` | Solo tipografía, sin acento de color; claro u oscuro según decida el sistema. |
| `terminal` | `themes/terminal.css` | Monoespaciado, fósforo verde sobre negro. No se mueve: un terminal apagado, sin parpadeo ni cursor. |
| `paper` | `themes/paper.css` | Serif, fondo crema, aire de página impresa. |
| `amber` | `themes/amber.css` | Monocromo ámbar, tipo CRT. |
| `compact` | `themes/compact.css` | Todo lo corto posible, con el título y el subtítulo en una línea: para móvil y CLI. |

`THEME` (entorno, al arrancar) elige el tema de **todo el servicio**, y `X-Theme` (cabecera de la
petición) elige el de **esa** petición, con el primer-origen-que-haga-match de siempre: un `X-Theme`
conocido gana a `THEME`. Un `X-Theme` desconocido **no** es un error —cae al tema configurado—,
porque esa cabecera la elige quien llama y un cliente no puede romper el servicio; la comparación es
**exacta**, así que `X-Theme: Terminal` no es el tema `terminal` y cae al configurado. `X-Theme` solo
afecta al **HTML**: un JSON o un XML no tienen hoja, así que ahí el tema se ignora.

Ninguno de los seis referencia recursos externos —ni `@import`, ni `url(...)` remotos, ni fuentes
descargadas—: una página que necesita la red para verse es exactamente el defecto que este apartado
corrige. Los cinco temas nuevos viven en `CONTENT_DIR` (`content/themes/`), así que además se sirven
como assets —`GET /themes/terminal.css` es un `200 text/css`— y puedes verlos sin incrustar nada.

Una hoja de tema **puede animar**: el movimiento es una decisión de aspecto, no un defecto de
autocontención. Ahora bien, lo que anima tiene que poder apagarse: una hoja que declare una animación
—`animation`, `animation-name`, `transition` o `transition-property` con un valor **distinto de
`none`**— tiene que **neutralizarla** dentro de un bloque `@media (prefers-reduced-motion: reduce)` que
**exista**, **no esté vacío** y apague **cada** una con la forma canónica de su familia —`animation:
none` o `animation-name: none` para una animación, `transition: none` o `transition-property: none`
para una transición; `animation: none` **no** apaga una transición—. `@keyframes` por sí solo no obliga
a nada: define fotogramas y no anima mientras ninguna declaración los use. Y el motivo sigue siendo el
mismo: hay gente a la que el movimiento le marea, y una página de error que se mueve mientras intentas
leer qué ha pasado es peor página de error.

Las hojas **incorporadas** cumplen dos contratos —no pedir nada por la red y poder apagar el
movimiento— y las pruebas del proyecto los comprueban. Ese segundo contrato es una comprobación
**textual sobre los bytes** de la hoja: garantiza que el patrón canónico está, **no** que el CSS apague
el movimiento en todos los navegadores —eso exigiría un motor de CSS y una matriz de navegadores—. Y esa
comprobación cubre las **seis hojas que el proyecto entrega**, no la tuya: una hoja propia en
`CONTENT_DIR/themes/<nombre>.css` es tuya y el servicio no la inspecciona. Y `url(#…)` —un filtro o
degradado SVG— y `@font-face` con `src: local(...)` están **permitidos**, porque no piden nada a la
red.

`THEME` es el equivalente del `--template-name`/`$TEMPLATE_NAME` de `error-pages` —que por defecto usa
`app-down`—, con una diferencia que se declara: allí la plantilla puede **rotar**
(`--rotation-mode`) y aquí no (ver «No incluye»).

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

`GET /metrics` expone en texto Prometheus el contador de páginas servidas, con **dos** etiquetas:

```
errors_pages_total{code="404",source="path"} 3
errors_pages_total{code="503",source="header"} 1
```

`code` es el código servido y `source` dice de dónde salió ese código: `path`, `header` (`X-Code`)
o `default` (`DEFAULT_ERROR_PAGE`). La etiqueta hace falta porque `X-Code` la elige quien llama: sin
ella, cualquiera puede inflar a voluntad el contador de un código y fabricar la apariencia de una
avalancha justo durante una incidencia; con ella, ese inflado se ve al leer el scrape.

**No publiques `/metrics` en el router de respaldo.** Es un endpoint de servicio —siempre `200`,
fuera de la resolución de páginas de error— y no una página de error: expone la actividad del propio
servicio (qué códigos ha servido, que es el perfil de fallos de tu infraestructura), así que déjalo
fuera del *catch-all* o ponle delante un middleware de Traefik (*basicAuth*, una `IPAllowList`). La
aplicación **no** autentica nada: el sitio para eso es el proxy. Y dejarlo fuera del *catch-all* no
es un detalle de la prosa: es una conjunción en la regla, y es lo que llevan **los tres** ficheros
que entregan un respaldo desplegable —el ejemplo de `examples/traefik/`, la plantilla de producción
de `compose.yml` y la variante de *provider de fichero* del README del ejemplo—:

```yaml
traefik.http.routers.errserve-catchall.rule: HostRegexp(`.+`) && !Path(`/metrics`)
```

`tests/traefik_catchall.rs` la guarda, porque la comprobación de extremo a extremo necesita un motor
de contenedores y no corre en el CI: sin esa prueba, la fuga solo se descubriría volviendo a
desplegar. Y quitarla no rompe el respaldo: una petición a `/metrics` no casa con ningún router,
Traefik responde su propio `404` —19 B, texto plano, sin `X-Robots-Tag`— y el servicio ni se entera,
mientras que para cualquier otro path el respaldo sigue sirviendo el código real (`/` → `404` con la
firma del servicio, `/503` → `503` con su `Retry-After`).

`/version` sí se queda a la vista **a propósito**, por paridad con `error-pages`: publica la versión
y nada más, y hoy no hay ningún aviso que lo haga explotable.

La respuesta se comprime cuando el cliente envía `Accept-Encoding: gzip` —y entonces responde
con codificación por trozos, sin `Content-Length`—, y declara `Vary: Accept-Encoding, X-Request-Id,
X-Trace-Id, X-Correlation-Id, X-Amzn-Trace-Id, X-Theme`: las seis, siempre, las traiga la petición o
no. Las cabeceras de correlación se copian de la petición a la respuesta cuando vienen —no se
inventan—, así que una caché intermedia tiene que saber que la respuesta varía por ellas; y `X-Theme`
cambia **el cuerpo** —la hoja que va incrustada en el HTML—, así que sin declararla una caché le
entregaría a un cliente el tema que pidió otro, o el cuerpo entero de otro. Sin caché por medio el
impacto es nulo (Traefik no cachea por defecto), pero con un CDN delante no.

### El registro de peticiones

El servicio ya traza cada petición —método, ruta, latencia y estado—, pero con el `LOG_LEVEL` por
defecto (`info`) no se ve: lo único que sale es la línea de arranque. Para encenderlo:

```sh
LOG_LEVEL="tower_http=debug,info"                                   # dos líneas por petición: entrada y salida
LOG_LEVEL="tower_http::trace::on_failure=off,tower_http=debug,info" # sin el ruido de los 5xx
```

La segunda forma es la recomendable, y el motivo está en el propio diseño del servicio: el
clasificador de `tower-http` marca **todo `5xx` como fallo**, y como este servicio *sirve* páginas
`5xx`, cada `503` normal fingiría un error. Por eso su nivel bajó a `DEBUG`; al encender el acceso
conviene silenciarlo, o volverá el ruido por la puerta de atrás. Con `LOG_FORMAT=json` cada suceso
sale en una línea de JSON —las de salida, con `latency` y `status`—, listo para un colector sin
expresiones regulares.

Lo que el registro **no** trae: la dirección del cliente ni el `X-Code` recibido. Para saber quién
está inflando el contador de un código, esa información ya está en `/metrics`, en la etiqueta
`source`.

## Imagen

La imagen publicada está en **GHCR**, no en Docker Hub:

```sh
docker pull ghcr.io/atareao/errserve:latest     # o una versión fija: :0.3.0
```

El [`compose.yml`](https://github.com/atareao/errserve/blob/main/compose.yml) de la raíz y el
ejemplo de [`examples/traefik/`](https://github.com/atareao/errserve/tree/main/examples/traefik/) la
usan por defecto (`ERRSERVE_IMAGE` cambia la referencia), así que **no hay que construir nada** para
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
git tag v0.2.0 && git push origin v0.2.0
```

Eso dispara
[`.github/workflows/release.yml`](https://github.com/atareao/errserve/blob/main/.github/workflows/release.yml),
que construye la imagen, la **prueba con docker de verdad** —que `/404` devuelva 404, el
`HEALTHCHECK` del `Dockerfile` y el `USER 10001:10001`; en la máquina de desarrollo no hay docker,
solo podman, que además descarta el healthcheck al construir en formato OCI—, la empuja a GHCR con
su versión y `latest` y crea el Release con las notas generadas. Un `workflow_dispatch` republica
una etiqueta ya existente, pero exige indicarla en el input `tag`: en un dispatch no hay etiqueta y
`github.ref_name` es la rama.

`main` y `development` están **protegidas**: todo entra por **PR** y ningún PR se fusiona hasta que
el check **`fmt, clippy y tests`** está en verde. El release no sustituye a ese check.

### La visibilidad del paquete

En GitHub **no se puede cambiar la visibilidad de un paquete por API**: el
`PATCH /user/packages/{type}/{name}` que circula por ahí ya no existe (medido: responde `404`, no está
en el OpenAPI de GitHub y en GraphQL no hay mutación de paquetes), así que solo se toca a mano en los
ajustes del paquete.

En este repositorio, **medido tras la primera publicación**, el paquete quedó **público**: la
descarga anónima funciona. Si algún día `docker pull ghcr.io/atareao/errserve` responde `denied`, es
que el paquete quedó privado: se arregla una sola vez en
<https://github.com/users/atareao/packages/container/errserve/settings> » *Danger Zone* » *Change
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

Sin motor de plantillas —solo la sustitución literal de los tres marcadores de la
página—, sin localización, sin generador estático y sin TLS (lo termina Traefik).
Se copian **solo** las cuatro cabeceras de correlación, no el conjunto completo de
`--proxy-headers`, y la compresión se limita a gzip.

Tampoco hay rotación de temas: `error-pages` tiene `--rotation-mode` (aleatorio
al arrancar, horario, diario) y aquí no existe —un tema, y ya—. Y los seis temas
están **incorporados al binario**, así que `THEME` no admite un nombre que no sea
uno de ellos: un `content/themes/<nombre>.css` propio **reemplaza** la hoja de un
tema que ya existe, pero no **añade** un séptimo nombre, porque eso haría que
`THEME` dependiera del sistema de ficheros al arrancar.

## Licencia

MIT.
