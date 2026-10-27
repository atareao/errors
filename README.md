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
hubiera producido; `PUT /minimal.html` respondía `405` y `Range: bytes=0-1,3-4` sobre un asset
respondía `416`, los dos sin cuerpo y sin pasar por la cascada.

Y hay **tres razones medidas** para no conservar su estado:

- el `405` saldría **sin el `Allow`** que RFC 9110 exige: la página la construimos nosotros y esa
  cabecera describe *su* respuesta, no la nuestra;
- el estado y el cuerpo quedarían en desacuerdo —un `404` con `Retry-After: 120` y el cuerpo del
  `503`—, contra la regla de que el `404` no lleva `Retry-After`;
- con `DEFAULT_ERROR_PAGE=503`, cualquier asset inexistente saldría `404`, dejando sin efecto a
  `X-Code`, que existe justo para los despliegues que no reescriben el path.

Un asset **existente** sigue ganando, que es la otra mitad de la regla: `GET /minimal.html` —una de
las páginas de tema que **no** son el alias— es un `200 text/html` crudo, con sus marcadores a la
vista, y no pasa por la página de error. La excepción es `/default.html`: es el **alias del código por
defecto**, así que se intercepta y responde el código resuelto, **nunca `200`** (ver «El tema es una página»).

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
trampa de copia-pega que la del *catch-all*: si copias esa regla, mira «Operación y observabilidad»,
porque además lleva **dos** exclusiones —la de `/metrics` y la del **leak de las páginas de tema**—.

## Ejemplos

```sh
curl -i  localhost:8080/404                  # 404 Not Found (no 200), con X-Robots-Tag
curl -i  localhost:8080/503                  # 503 Service Unavailable, con Retry-After: 120
curl -i  -H 'X-Code: 503' localhost:8080/    # 503 cuando el path no trae código
curl -s  localhost:8080/404.json             # {"error":true,"code":404,...}
curl -s  -H 'X-Format: application/xml' localhost:8080/404
curl -i  localhost:8080/healthz              # 200
curl -s  localhost:8080/version              # {"version":"..."}
curl -s  -H 'Accept: text/html' -H 'X-Theme: terminal' localhost:8080/404   # con la página del tema `terminal` dentro
```

## Configuración (solo variables de entorno)

| Variable | Defecto | Para qué |
|---|---|---|
| `PORT` | `8080` | Puerto de escucha (bind `0.0.0.0`). El que asume la guía de Traefik. |
| `LOG_LEVEL` | `info` | Filtro de `tracing-subscriber`. |
| `CONTENT_DIR` | `content` | El directorio del operador **y la fuente de temas**: *override* de la página de un tema **embebido**, y los `*.html` de su **raíz** **añaden** temas al registro. |
| `DEFAULT_ERROR_PAGE` | `404` | Código cuando el path y `X-Code` no aportan uno utilizable. |
| `LOG_FORMAT` | `text` | Formato del registro: `text` o `json`. |
| `RETRY_AFTER` | `120` | Segundos de la cabecera `Retry-After`. La llevan solo el `429` y el `503`. |
| `THEME` | `default` | Tema de las páginas **HTML**: uno de los **embebidos** (ver «Los temas») **o** un tema descubierto en `CONTENT_DIR`. |

No se lee ningún fichero de configuración. Una variable **presente pero vacía**
es un error de arranque, no un valor por defecto. `DEFAULT_ERROR_PAGE` fuera del
rango `400..=599` también lo es, y un `THEME` que no sea un nombre del **registro
de ese arranque** —o que venga vacío— aborta el arranque igual, con un error que
**nombra la variable** y **enumera los nombres del registro**: quien escribe
`THEME` cree que está configurando algo, así que el error tiene que decirle qué
puede escribir en su lugar. Y ojo con la contrapartida: como la lista es la de
**ese** arranque, un `THEME` **correcto** cuyo fichero **todavía** no esté en
`CONTENT_DIR` —un volumen que aún no se ha montado cuando el contenedor arranca—
**aborta** el arranque; la mitigación es del operador: declarar la dependencia del
volumen, o arrancar sin `THEME` y con el defecto.

`RETRY_AFTER` es el único valor de la respuesta que depende de tu infraestructura: la cabecera
`Retry-After` es lo que se le promete al cliente sobre cuándo volver, así que si tu backend tarda
cinco minutos en recuperarse, `120` es una mentira que el cliente obedece. Admite cualquier entero de
segundos —`0` significa «reintenta ya»— y no tiene techo. Una variable vacía, o que no sea un entero
de segundos, es un error de arranque. Lo que **no** se configura es qué códigos la llevan: son el
`429` y el `503`, y eso lo decide el código.

`CONTENT_DIR` es el directorio del operador: el ***override*** de la página de un tema **embebido**
**y** la fuente de los temas nuevos. El binario lleva **dentro** las páginas **embebidas** de tema, así que
un `cargo install errserve` —que deja un ejecutable y ningún fichero de datos, con `CONTENT_DIR`
apuntando por defecto a un `content` que no existe— sirve las páginas **embebidas**, y no el respaldo de texto. Lo
que haya en `CONTENT_DIR` gana **cuando se pueda leer**; lo que falte se sirve desde la copia
embebida. Y los `*.html` de su **raíz** que no sean la página de un **embebido** **añaden** temas al
registro: un fichero de más **es** un tema, y su tallo es su nombre (ver «El tema es una página»).

## Cómo se resuelve

**Código** (primero que haga match): path (`/404`, `/404.json`, `/503.xml`…) →
cabecera `X-Code` → `DEFAULT_ERROR_PAGE`. Una fuente con un código fuera del
rango `400..=599` se ignora y se pasa a la siguiente.

**Formato** (primero que haga match): extensión del path (`.html`/`.htm`,
`.json`, `.xml`, `.txt`) → `Content-Type` de la petición → `X-Format` →
`Accept` → **texto plano** por defecto.

Código y formato son ortogonales: `/503.xml` es un 503 en XML.

Y hay un **tercer eje**, que no reparte códigos ni formatos sino aspecto, y que solo toca al HTML: **la
página del tema** que se sirve. La elige `THEME` —el tema de todo el servicio— y, por petición, la
cabecera `X-Theme`, que gana a `THEME` cuando nombra un tema del **registro** —los **embebidos**
más los descubiertos en `CONTENT_DIR`—; un `X-Theme` ausente, desconocido o vacío **no** es un error y
cae al tema configurado. En JSON, XML y texto no hay página que incrustar, así que ahí el tema se
ignora (ver «Páginas y temas»).

Cada página de error lleva `Content-Type`, `Content-Length`,
`X-Robots-Tag: noindex, nofollow, nosnippet, noarchive` y, en `429` y `503`,
`Retry-After` con el valor de `RETRY_AFTER` (`120` por defecto).

El HTML es **la página del tema** elegido: la de `CONTENT_DIR/<nombre>.html` si se puede leer y, si no,
su copia embebida. **No** hay capa por código —`CONTENT_DIR/<código>.html` ya no se consulta—; el
diseño por código vive en la clase `body.c<código>`, que la plantilla rellena al servir. Además,
`/default.html` y `/default.htm` son el **alias del código por defecto** (`DEFAULT_ERROR_PAGE`): se
interceptan y responden ese código, nunca `200`. JSON, XML y texto se generan del código, su razón
canónica y su descripción. Las cuatro formas de un mismo código llevan la **misma** razón y la
**misma** descripción: las tablas de `resolve` son su fuente única.

## Códigos cubiertos

**Cualquier** código de `400..=599` tiene página, no solo las que hay en `content/`: la página del tema es una **plantilla**
(por defecto `content/default.html`) que el servidor rellena al vuelo, así que un `451`, un `418` o un
`599` salen con **su propio** código, su razón y su descripción, y con **su propio** estado —nunca
verás un `504` servido como `404`, ni un cuerpo que diga `404` para otro código—. Quince códigos traen
su descripción escrita en el código (`400`, `401`, `403`, `404`, `405`, `408`, `410`, `418`, `422`,
`429`, `500`, `501`, `502`, `503` y `504`); el resto usa una genérica, siempre con la razón canónica
del código en el título.

La plantilla lleva tres marcadores que el servidor sustituye **al servir**:

| Marcador | Se sustituye por |
|---|---|
| `{{code}}` | El código resuelto (`404`), allí donde aparezca. |
| `{{reason}}` | La razón canónica del código (`Not Found`). |
| `{{description}}` | La descripción del código (la tabla de `resolve`). |

Dónde va cada uno lo decide la página. Las **canónicas** los colocan en un orden fijo: `{{code}}` en
el `<title>`, en el `<h1>` y en la clase del `body` (`class="c{{code}}"`), `{{reason}}` en el `<h2>`
y `{{description}}` en el `<p>`; y es esa clase del `body` (`c504`, `c503`…) lo que permite
diferenciar el aspecto de cada código sin duplicar la página. Las **ilustradas** los reparten a su
manera —`matrix` en un `data-text`, `terminal` en su banner—, y ninguna forma es más correcta que la
otra: el servidor solo ve marcadores y los rellena todos.

### Cómo cambiar el texto de una página de error

Hay tres vías, de la más global a la más local:

1. **El texto de todos los códigos**: edita la tabla de `resolve::description` en `src/resolve.rs` y
   recompila. Es la **fuente única** del texto de los cuatro formatos (HTML, JSON, XML y texto), así
   que el cambio vale para todos ellos a la vez.
2. **La página del tema**: deja tu fichero en `CONTENT_DIR/<nombre>.html` (por ejemplo,
   `content/default.html` para el tema por defecto). Gana a la copia embebida y se lee **al servir**,
   así que la ves cambiar sin reiniciar. Si **no** lleva marcadores, se sirve **tal cual**, byte a
   byte: su texto fijo es tuyo. Es el **punto de personalización** del operador.
3. **Una página del tema que se adapte al código**: escribe los marcadores `{{code}}`, `{{reason}}` y
   `{{description}}` en tu página del tema y el servidor la rellenará con el código resuelto, igual
   que hace con la del proyecto. Así una sola página cubre cualquier código `400..=599`.

**Qué pierde el operador, escrito sin maquillarlo**: la capa por **código**
(`CONTENT_DIR/<código>.html`) y la del código por defecto (`CONTENT_DIR/404.html`) **ya no se
consultan**. El punto de personalización es la página del **tema**, así que un `content/404.html` que
tuvieras **deja de servir**: con `THEME=matrix` verás la página de `matrix`, no la tuya. Lo que
conservas: tu página del tema es tuya, gana a la embebida y se sirve byte a byte si no lleva
marcadores. Si quieres granularidad por código, el camino es el `<style>` de tu página, no un fichero
por código.

## Páginas y temas

Una página de tema es **autocontenida**, y aquí eso significa algo un poco más laxo que en otras
casas: todo lo que la página necesita para verse —su CSS, su JavaScript, sus tipografías, sus
imágenes— vive **dentro de la propia página** o es un **recurso de internet**; lo que **nunca** puede
ser es un recurso **servido por este servicio**. El errserve no publica ningún asset suyo que las
páginas necesiten.

Y no es una preferencia estética: es el arreglo de un fallo medido, cuyo motivo sigue a la vista en el
path que lo causaba. Con el middleware `errors` de Traefik —el montaje de cualquier aplicación con
dominio propio—, la página se sirve **en el host de la aplicación**, donde este servicio **no sirve
assets**. La página enlazaba `/css/error.css`, y ese path lo contestaba el upstream, no este servicio:
el navegador pedía la hoja, recibía su `404` con `Content-Type: text/plain`, el middleware interceptaba
ese `404` y lo volvía a pedir como página de error con un `Accept` que no negocia HTML
—`text/css,*/*;q=0.1`—, así que la cascada de formato caía al **texto plano** y el navegador
rechazaba la hoja: *«MIME type ('text/plain') is not a supported stylesheet MIME type»*. El resultado
era un `503` correcto, con el código correcto, y la página **sin estilos**. **Ese enlace ya no
existe**: `content/css/error.css` y el asset `/css/error.css` se retiraron en la `0.8.0` —un tema es
una página con su `<style>` dentro—, así que no hay `<link>` que servir.

Las dos formas legítimas conviven en `content/`, y ninguna es un apaño:

- **Dentro de la página**: el CSS y el JavaScript van incrustados. Las **once** llevan su `<style>`, y
  **siete** su `<script>`.
- **En internet**: `terminal`, `antique` y `starwars` importan su tipografía de Google Fonts con
  `@import` —Fira Code; Cinzel Decorative y Marcellus; Orbitron y Rajdhani—. Ese recurso no lo sirve
  este servicio ni el upstream de la aplicación, así que no hay nadie que pueda contestarle un `404`
  con el que reescribir el formato; y si Google no responde —sin red, o bajo una CSP que no lo
  permita—, la página **mantiene el estilo** y usa la tipografía de reserva que cada una declara
  (`monospace`, `serif`, `sans-serif`).

La paridad con `error-pages` —cuyas páginas son «fully self-contained — all styles and images are
embedded directly in the HTML, without loading any external resources»— se mantiene en el eje que
importa: **el servicio de errores no sirve ningún asset que las páginas necesiten**. No es literal en
«sin recursos externos», porque tres de las nuestras importan una tipografía; sí lo es en que nada de
lo que una página necesita depende de este binario.

### El tema es una página

Un tema **es** su página, y el nombre del tema **es** el de su fichero: viven en la **raíz** de
`CONTENT_DIR` —`content/<nombre>.html`—. Las páginas **embebidas** van dentro del binario y la raíz
del directorio **añade** temas: un `*.html` de más **es** un tema, y su **tallo** es su nombre
(`neon.html` → `neon`). El barrido del directorio es **al arrancar**, así que **añadir** un tema exige
**reiniciar**: un fichero añadido con el servicio en marcha **no** es un tema hasta el siguiente
arranque —se sigue sirviendo como **asset crudo**—. Los **bytes** de la página, en cambio, se leen **al
servir**, así que **editar** un tema que ya estaba se ve **sin** reiniciar. Elegir un tema no duplica
nada: elige **qué página** se sirve. Tu página gana a la embebida; una página **sin** marcadores se
sirve **byte a byte**; y una que existe y no se puede leer responde el **respaldo de texto** con el
código resuelto —el `warn` la delata—, sin cambiar el estado. Un tema **descubierto** no tiene copia
embebida: su fichero es su **única** copia, así que si se esfuma responde el respaldo de texto con el
estado intacto.

Las reglas del barrido, para no esperar lo que no es: solo la **raíz** (un `temas/neon.html` no es un
tema); la extensión tiene que ser **exactamente** `html` (`neon.htm`, `neon.HTML` y `neon.css` **no**
son temas); el nombre es el **tallo**; el tallo de un **embebido** no añade un tema
(`content/default.html` es la página del `default` **embebido**, no un `default` de más); y un tallo
que **reclame la resolución de códigos** (`404`, `999`, `200`…) **no** es un tema, porque ese path
responde **otra** página de error. Los descubiertos van **ordenados por nombre** detrás de los embebidos.

### Los temas

Los temas **embebidos** al binario son la **base** del registro, y un directorio puede **añadir**
más (ver «El tema es una página»). Hay **once**, y vienen en dos familias: las **canónicas** siguen
al pie de la letra el contrato de la casa —`<main>`, `<h1>{{code}}</h1>`, `<h2>{{reason}}</h2>`,
`<hr>`, `<p>{{description}}</p>` y `<footer>errserve</footer>`, con la clase `c{{code}}` en el
`body`, sin `<script>` y sin ninguna petición—; las **ilustradas** llevan su propio marcado, su `<script>` en
línea y sus controles. Lo que las une es lo único que el servidor necesita: los tres marcadores. La
lista completa sale de `content/*.html`.

| Tema | Página en `CONTENT_DIR` | Aspecto |
|---|---|---|
| `default` | `default.html` | canónica. El tema por defecto: claro u oscuro según decida el sistema, con un acento por código. Su `<main>` entra con un fundido de 180 ms que se neutraliza en `prefers-reduced-motion: reduce`. Sus reglas pueden cambiar de una versión a otra; lo que no cambia es que sigue siendo el tema por defecto. |
| `compact` | `compact.html` | canónica. Todo lo corto posible, con el título y el subtítulo en una línea, y claro u oscuro según el sistema: para móvil y CLI. |
| `amber` | `amber.html` | canónica. Monocromo ámbar (`#ffb000`) sobre casi negro, tipo CRT. |
| `aurora` | `aurora.html` | canónica. Oscuro con degradados y brillo: fondo azul→violeta, tarjeta de cristal y un halo tras el código, con un acento luminoso distinto por código. |
| `minimal` | `minimal.html` | ilustrada. Oscuro, con tarjeta de cristal y acento índigo-violeta; botones «Ir al Inicio» y «Volver Atrás». |
| `paper` | `paper.html` | ilustrada. Serif (Georgia), hoja crema sobre gris claro, aire de página impresa; acento ámbar; botones «Volver al inicio» y «Regresar». |
| `matrix` | `matrix.html` | ilustrada. Un **documento propio**: canvas de lluvia, glitch, scanlines, terminal, pastillas roja y azul, minijuego de teclado y toast. Su `<script>` consulta `prefers-reduced-motion` y no arranca la lluvia con movimiento reducido. |
| `terminal` | `terminal.html` | ilustrada. Un **emulador de terminal interactivo** (VT100): banner ASCII, reloj, logs de arranque y un **CLI** donde escribir `help` o `home`, con conmutadores de fósforo (verde, ámbar, cian, monocromo). Importa Fira Code de Google Fonts. |
| `antique` | `antique.html` | ilustrada. Oro sobre casi negro, aire de club señorial; botón «Entrar al Club». Importa Cinzel Decorative y Marcellus de Google Fonts. |
| `oasis` | `oasis.html` | ilustrada. Degradado radial nocturno en azul con tarjeta de cristal y acento cian; botón «Volver al Oasis». |
| `starwars` | `starwars.html` | ilustrada. HUD retro-futurista en cian («sable»), con scanlines de CRT; botón «Volver a la Base». Importa Orbitron y Rajdhani de Google Fonts. |

`THEME` (entorno, al arrancar) elige el tema de **todo el servicio**, y `X-Theme` (cabecera de la
petición) elige el de **esa** petición, con el primer-origen-que-haga-match de siempre: un `X-Theme`
**conocido** —un tema del registro— gana a `THEME`. Un `X-Theme` desconocido **no** es un error —cae al
tema configurado—, porque esa cabecera la elige quien llama y un cliente no puede romper el servicio;
la comparación es **exacta**, así que `X-Theme: Terminal` no es el tema `terminal` y cae al
configurado. `X-Theme` solo afecta al **HTML**: un JSON o un XML no tienen página que incrustar, así
que ahí el tema se ignora.

Las páginas de tema son **opacas**: el servicio no valida su contenido. Una página puede llevar
`@import`, animación, redirecciones, los marcadores `{{code}}`/`{{reason}}`/`{{description}}` o
ninguno; se sirve tal cual y el render solo rellena los marcadores cuando están. Añadir un tema es
dejar su `.html` en `content/` y reconstruir: `build.rs` lo descubre al compilar y lo embebe. El
registro en marcha es `EMBEBIDOS ∪ CONTENT_DIR`, y el fichero del directorio gana por nombre. No hay
una lista cerrada de temas ni una guarda que inspeccione la página: la decisión de qué lleva cada una
es de quien la escribe. El servicio las sirve también como assets crudos si se piden; la regla del
respaldo que bloquea ese leak está en «Operación y observabilidad».

### El endurecimiento y la CSP

Las páginas son **incompatibles con una CSP estricta** (`default-src 'none'`): cada una lleva su
`<style>` **en línea** y **siete** de ellas —todas las ilustradas— su `<script>` **en línea**. Si el
proxy pone una CSP sin `style-src 'unsafe-inline'` —o sin *nonce*—, las páginas pierden el estilo; sin
`script-src`, las ilustradas pierden su `<script>` —`matrix`, el teclado del minijuego y las pastillas;
`terminal`, su CLI—; y `terminal`, `antique` y `starwars` necesitan además `style-src` y `font-src`
para `fonts.googleapis.com`, o caen a la tipografía de reserva. El servicio **no emite CSP**: la pone
el proxy, que es donde va el endurecimiento. Es una **decisión de diseño** escrita, no un fallo.

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
es un detalle de la prosa: son **dos** conjunciones en la regla —la de `/metrics` y la que cierra el
**leak de las páginas de tema**—, y las llevan las **cinco** definiciones de respaldo desplegable que
entrega este repositorio, en **cuatro** ficheros: el ejemplo de `examples/traefik/` (sus labels), la
plantilla de producción de `compose.yml`, este mismo `README.md` y las **dos** del README del ejemplo
—el bloque de prosa y la variante de *provider de fichero*—. La segunda no es cosmética: el nombre de
cada tema **es** la ruta de su página, así que sin ella el respaldo publicaba los paths de las
páginas de tema que **no** son el alias del código por defecto, con `200` y los marcadores
`{{code}}`/`{{reason}}`/`{{description}}` a la vista, a quien preguntara por un Host sin router
propio.

```yaml
traefik.http.routers.errserve-catchall.rule: HostRegexp(`.+`) && !Path(`/metrics`) && (!PathRegexp(`\.html?((%2[fF])|/|$)`) || Path(`/default.html`) || Path(`/default.htm`))
```

`tests/traefik_catchall.rs` las guarda, porque la comprobación de extremo a extremo necesita un motor
de contenedores y no corre en el CI: sin esa prueba, la fuga solo se descubriría volviendo a
desplegar. Y quitar la de `/metrics` no rompe el respaldo: una petición a `/metrics` no casa con
ningún router, Traefik responde su propio `404` —19 B, texto plano, sin `X-Robots-Tag`— y el servicio
ni se entera, mientras que para cualquier otro path el respaldo sigue sirviendo el código real
(`/` → `404` con la firma del servicio, `/503` → `503` con su `Retry-After`).

`/version` sí se queda a la vista **a propósito**, por paridad con `error-pages`: publica la versión
y nada más, y hoy no hay ningún aviso que lo haga explotable.

La respuesta se comprime cuando el cliente envía `Accept-Encoding: gzip` —y entonces responde
con codificación por trozos, sin `Content-Length`—, y declara `Vary: Accept-Encoding, X-Request-Id,
X-Trace-Id, X-Correlation-Id, X-Amzn-Trace-Id, X-Theme`: todas, siempre, las traiga la petición o
no. Las cabeceras de correlación se copian de la petición a la respuesta cuando vienen —no se
inventan—, así que una caché intermedia tiene que saber que la respuesta varía por ellas; y `X-Theme`
cambia **el cuerpo** —la página del tema que va incrustada en el HTML—, así que sin declararla una caché le
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
just smoke          # construye y corre el smoke test contra la imagen, con el motor de esta máquina
just login          # entra en GHCR (una vez por máquina)
just push           # publica a mano la versión y `latest`
```

`just smoke` invoca el mismo script que los dos workflows (`.github/scripts/smoke.sh`), con el motor
de esta máquina; `PORT=… just smoke` cambia el puerto de loopback si el `8080` está ocupado.

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
que construye la imagen, la **prueba** y la empuja a GHCR con su versión y `latest` y crea el Release
con las notas generadas. Las aserciones sobre la imagen arrancada viven en **un solo artefacto
versionado**,
[`.github/scripts/smoke.sh`](https://github.com/atareao/errserve/blob/main/.github/scripts/smoke.sh),
y lo invocan **las dos** puertas: el CI en cada propuesta de cambio y el release **antes** de publicar
nada. El motivo es una lección medida: en la `v0.8.0` la aserción `GET /css/error.css -> 200` vivía en
un único `run:` del `release.yml`, y **dos copias de la misma lista divergen** —el cambio del contrato
no pasó por ahí y el fallo apareció al etiquetar—. El `HEALTHCHECK` y el `USER` ya **no** se
ejercitan solo aquí, y el **`502`** entra en la lista —el caso por el que existe el proyecto, que
hasta ahora **ninguna** puerta nombraba—. El `HEALTHCHECK` solo se puede **exigir** cuando la imagen
lo declara: `podman` construye en formato OCI y lo descarta, así que el script lo dice en voz alta y
**no** lo cuenta como fallo. Un `workflow_dispatch` republica una etiqueta ya existente, pero exige
indicarla en el input `tag`: en un dispatch no hay etiqueta y `github.ref_name` es la rama.

`main` y `development` están **protegidas**: todo entra por **PR**, los **tres** checks requeridos
—**`fmt, clippy y tests`**, **`cargo-deny`** e **`imagen`**— están marcados como requeridos en las
**dos** ramas con `strict` (la rama ha de estar al día) y `enforce_admins` está activo, así que
**tampoco el dueño** empuja directo: ningún PR se fusiona hasta que los tres están en verde. Y el job
`imagen` **construye la imagen y la prueba** (`bash .github/scripts/smoke.sh`), porque construir no es
probar. El release no sustituye a esos checks.

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
just smoke                                                       # smoke test de la imagen, antes de commitear
```

El repositorio compila **sin conexión**: las dependencias se resuelven contra la
caché local de Cargo (`cargo check --offline`).

## No incluye (a propósito)

Sin motor de plantillas —solo la sustitución literal de los tres marcadores de la
página—, sin localización, sin generador estático y sin TLS (lo termina Traefik).
Se copian **solo** las cuatro cabeceras de correlación, no el conjunto completo de
`--proxy-headers`, y la compresión se limita a gzip.

Tampoco hay rotación de temas: `error-pages` tiene `--rotation-mode` (aleatorio
al arrancar, horario, diario) y aquí no existe —un tema, y ya—. Los temas
siguen **embebidos** al binario —un `cargo install` sirve los temas sin ficheros de datos—, **y**
el directorio `CONTENT_DIR` **añade** nombres: un `*.html` de más en su raíz **es** un tema nuevo (ver
«El tema es una página»). `THEME` **sí** depende del sistema de ficheros al arrancar: se valida contra
el registro de **ese** arranque, y un `THEME` correcto cuyo fichero todavía no esté montado **aborta**.
Es una decisión declarada, con su precio: el barrido es al arrancar, así que **añadir** un tema exige
**reiniciar**.

## Licencia

MIT.
