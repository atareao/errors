# errserve — AGENTS.md

> Las directivas globales de este entorno (OpenSpec SDD + TDD, tags de estado, tabla de subagentes)
> se inyectan ya en cada sesión desde `~/.config/opencode/AGENTS.md`. No se repiten aquí: lo de abajo
> es lo específico de este repositorio.

## Estado real del repositorio (verificado)

Implementado, verificado y versionado. **Diecisiete** changes están **archivados** en
`openspec/changes/archive/` —cinco con fecha `2026-10-24` (`error-pages-parity`, `error-page-coverage`,
`release-hardening`, `security-hardening` y `retry-after-configurable`), ocho con `2026-10-25`
(`embedded-error-pages`, `error-page-template`, `self-contained-themes`, `theme-sheet-contracts`,
`theme-visual-review`, `catchall-metrics-exclusion`, `fix-image-build` y `publish-crate-on-tag`) y tres
con `2026-10-26` (`themes-as-pages`, `smoke-test-and-catchall-html-exclusion` y
`themes-from-content-dir`) y uno con `2026-10-27` (`themes-discovered-at-build`)— y sus requisitos viven
fusionados en los cuatro specs de `openspec/specs/`: `error-page`, `http-server`, `observability` y
`response-extras`. Los **cuatro últimos** changes archivados: `themes-as-pages` —el tema deja de ser
**una hoja** y pasa a ser **una página**, con la cascada del cuerpo HTML en un solo paso—, que entró en
`error-page` y `http-server` (+6 requisitos añadidos, ~8 modificados, -5 retirados);
`smoke-test-and-catchall-html-exclusion` —el smoke test pasa a un artefacto único que corren los dos
workflows, el respaldo gana la exclusión del leak de las páginas de tema y el `502` entra en una
puerta—, que entró **solo** en `http-server` (+3 añadidos, ~1 modificado, 0 retirados); y
`themes-from-content-dir` —la lista de temas deja de ser una tabla cerrada en el binario y
`CONTENT_DIR` **añade** temas al **registro** construido al arrancar, con la trampa del volumen sin
montar—, que entró en `error-page` y `http-server` (+1 añadido, ~3 modificados, 0 retirados); y
`themes-discovered-at-build` —el descubrimiento de las páginas embebidas se mueve del arranque al
**compilador** (`build.rs` lee `content/*.html` y genera el array `EMBEBIDOS`, que `src/embedded.rs`
incluye con `include!`), las páginas pasan a ser **opacas** (se retiran las tres guardas de contenido
—red, movimiento y cobertura de marcado— y la prueba de paridad con la copia embebida) y la lista de
temas del proyecto deja de estar escrita a mano—, que entró en `error-page` y `http-server`
(+4 añadidos, ~4 modificados, -6 retirados). **No hay ningún
change en curso**: `openspec/changes/` solo tiene `archive/`, y `openspec validate --all
--strict` da **4 passed, 0 failed**.

- `src/main.rs` — arranque, router, middleware de páginas de error, la lista `SERVICE_ENDPOINTS` que acota ese middleware, el clasificador `is_error_page_path` (con el alias `/default.html`/`/default.htm`) y los tests de router; el tema se resuelve **contra el registro** antes del `spawn_blocking`, y `X-Theme` **nunca** compone una ruta.
- `src/config.rs` — configuración desde el entorno (`Config`, `ConfigError`, `from_env_iter`); `Config` **posee** el registro (`themes: Themes`) y el tema elegido **por valor** (`theme: Theme`, ya no `&'static Theme`), y `ConfigError` gana `UnknownTheme { value, validos }`.
- `src/path.rs` — normalización del path (percent-decode + plegado de `.`/`..`).
- `src/resolve.rs` — cascadas de código y formato, rango `400..=599`, y las tablas de razón canónica (`canonical_reason`) y descripción (`description`) —**fuente única** del texto de los cuatro formatos, que el HTML incrusta al servir—.
- `src/render.rs` — cuerpo por formato y cabeceras; la cascada del HTML tiene **un solo paso** (la página del tema, `Theme::page`), ya **sin capa por código**, y el HTML se sirve **relleno** (`plantilla`: sustituye `{{code}}`/`{{reason}}`/`{{description}}` por el código resuelto y las tablas de `resolve`, escapando lo insertado).
- `src/theme.rs` — la **gestión de temas**: el registro parte de `EMBEBIDOS` —las páginas que `build.rs` descubrió en `content/` al compilar— y se completa al arrancar con los `*.html` descubiertos en la raíz de `CONTENT_DIR`; el fichero del directorio gana por nombre. Las páginas son **opacas**: el servicio no valida su contenido, sus marcadores ni su autocontención, y no las inspecciona al servir. La ruta de cada página sigue **derivada del nombre** (`Theme::page_path` → `"<nombre>.html"`) y `Theme::page` resuelve la página **como un asset** (disco si se puede leer; la copia embebida si no, y **solo** un tema embebido la tiene: uno descubierto sin fichero responde el respaldo de texto).
- `src/embedded.rs` — incluye con `include!` el array `EMBEBIDOS` que genera `build.rs` a partir de `content/*.html`; una entrada por página y nombre. El binario sirve las páginas de tema sin ficheros de datos. El array se regenera al compilar: añadir un tema es dejar el `.html` en `content/` y reconstruir. Las páginas son **opacas** y no hay prueba de paridad ni guarda de contenido.
- `src/metrics.rs` — contador de páginas servidas por código **y por origen del código**, y su serialización en texto Prometheus.
- `content/` — **las páginas de tema del proyecto**, una por nombre en la **raíz**; `build.rs` las descubre al compilar y las embebe. Son **opacas**: cada una puede llevar su `<style>`, su `<script>`, los marcadores `{{code}}`/`{{reason}}`/`{{description}}` o ninguno; el servicio no las valida. Ya no hay hoja ni `content/css/`. Son **once**, en dos familias: las **canónicas** (`default`, `amber`, `compact`, `aurora`) siguen el contrato de la casa —`main`/`h1`/`h2`/`hr`/`p` y `<footer>errserve</footer>`, con `class="c{{code}}"`—, sin `<script>` ni red y con el contraste medido en la cabecera; las **ilustradas** (`antique`, `matrix`, `minimal`, `oasis`, `paper`, `starwars`, `terminal`) llevan su propio marcado, su `<script>` en línea y sus controles. **Tres** importan una tipografía de Google Fonts (`terminal`, `antique`, `starwars`): el contrato es que el **estilo vive dentro** —no hay hoja que servir—, no que ninguna página haga una petición.
- `Dockerfile`, `.dockerignore`, `.justfile`, `compose.yml` — empaquetado (imagen ~13 MB, no-root uid 10001); el `compose.yml` va endurecido: `read_only: true`, `cap_drop: [ALL]`, `no-new-privileges:true` e `init: true`.
- `README.md` — el contrato con Traefik y los ejemplos.
- `.github/scripts/smoke.sh` — el **único** sitio donde viven las aserciones sobre la **imagen
  arrancada** (estado, cabeceras, cuerpo del HTML, `USER` y `HEALTHCHECK`). Lo invocan por su ruta
  literal los **dos** workflows que necesitan probar la imagen: `ci.yml` (job `imagen`, tras
  construirla) y `release.yml` (job `imagen`, antes de publicar). No construye ni publica nada.
  Arranca **dos** contenedores: el de siempre —la imagen tal cual, que prueba el leak
  (`/minimal.html` → `200`)— y un **segundo** con un `CONTENT_DIR` **montado** y un tema de más, en su
  propio puerto de loopback (`PORT_VOLUMEN`, por defecto `PORT` + 1), porque el barrido del directorio
  del operador depende de que el montaje sea legible por el uid **10001** con el que corre la imagen.
- `tests/` — las guardas sobre ficheros versionados: `traefik_catchall.rs` (las definiciones de
  respaldo de los cuatro ficheros, con la regresión de la forma anclada al fin de cadena) y
  `puertas.rs` (los dos workflows invocan el script, ninguno duplica sus aserciones y `check.sh`
  conserva el `502`); y los fixtures del descubrimiento, en `tests/fixtures/temas-extras/` y
  `tests/fixtures/temas-extension/`.

Puertas en verde: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y la suite de
pruebas (unitaria, `puertas` y `traefik_catchall`). Verificado además contra el servidor real y
contra la imagen.

## Qué es

Clon en Rust, deliberadamente **supersencillo**, de [tarampampam/error-pages](https://github.com/tarampampam/error-pages),
pensado para usarse detrás de **Traefik** como servicio de páginas de error.

**La diferencia funcional que justifica el proyecto:** `error-pages` responde **siempre HTTP 200**,
porque asume que el proxy reescribe solo el cuerpo y conserva el código. Este servidor devuelve el
**código de error real**. Si una respuesta de página de error sale con `200`, el proyecto está roto.

La paridad con `error-pages` se mantiene en **dos ejes independientes**: el **código** que se
devuelve y la **negociación de formato** (HTML / JSON / XML / texto plano). Y el change
`self-contained-themes` cerró un **tercer** hueco de paridad: sus páginas son «fully self-contained»
—incrustan estilos e imágenes en el HTML—, y aquí el HTML servido **es la página del tema**, con su
hoja **dentro**, con `THEME`/`X-Theme` como equivalente de su `--template-name`/`$TEMPLATE_NAME` (por
defecto `app-down`); su rotación (`--rotation-mode`) **no** existe aquí, y es un no-goal declarado.

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
   error*, no a los assets: `GET /minimal.html` —una de las páginas de tema que **no** son el
   alias— es un `200 text/html` crudo, con sus marcadores a la vista. Solo se sustituye una respuesta
   del servidor de ficheros **con estado de error**; su `200` —y su cuerpo— se sirve tal cual. La
   excepción es `/default.html`: es el **alias del código por defecto**, se intercepta y responde el
   código resuelto, nunca `200`. No conviertas un asset en página de error por la cabecera `X-Code`.
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
7. **El binario sirve las páginas sin ficheros de datos, y la hoja ya no existe.** Los temas
   van **dentro** del ejecutable: `build.rs` lee `content/*.html` al compilar y genera `EMBEBIDOS`
   con `include_str!`, así que un `cargo install errserve` —un binario y ningún fichero de datos,
   con `CONTENT_DIR` por defecto apuntando a un `content` que no existe— sirve las páginas de tema,
   no el respaldo de texto. La página del tema se resuelve **al servir** (`Theme::page`: el fichero
   legible en `CONTENT_DIR/<nombre>.html` o, si no, su copia embebida). **La hoja y su asset ya no
   existen**: `content/css/error.css`, `themes/*.css` y `GET /css/error.css` se retiraron en la
   `0.8.0`; la hoja de cada tema vive en el `<style>` de su propia página.
   `CONTENT_DIR` es el *override* de la página de un tema **embebido** **y la fuente de temas nuevos**:
   los embebidos se quedan y su **raíz** **añade** —un `*.html` de más **es** un tema, y su tallo es su
   nombre— a un **registro** que se construye **al arrancar** (los embebidos **más** los descubiertos;
   las reglas del barrido y la trampa del montaje están en el *quirk* del barrido). El precio, escrito:
   **añadir** un tema exige **reiniciar**, mientras que los **bytes** de la página se leen al servir,
   así que **editar** uno que ya estaba se ve **sin** reiniciar. Y lo que gana es **un fichero que se
   pueda leer**, no «siempre»: una página que existe y no se puede abrir responde el respaldo de texto
   con el código resuelto —el `warn` la delata—, en vez de que la copia embebida tape en silencio un
   directorio del operador roto (Opción A de `design.md`). La copia embebida sale del mismo fichero de
   `content/`, así que no hay dos copias que puedan divergir ni una prueba de paridad que las vigile.
   Nada de esto toca la garantía 1: el estado sigue siendo el código resuelto, nunca `200`.
8. **El respaldo que este repositorio entrega no publica las páginas de tema crudas.** El nombre
   del tema **es** la ruta de su página (`content/<nombre>.html`), y las páginas que no
   son el alias se sirven como assets **crudos**, con los marcadores
   `{{code}}`/`{{reason}}`/`{{description}}` a la vista: un Host sin router propio las publicaba a
   cualquiera con `200`. La conjunción que lo cierra es idéntica en los **cinco** sitios
   desplegables —**cuatro** ficheros: los labels del ejemplo, los de la plantilla de producción,
   el bloque del `README.md` raíz y las **dos** definiciones de `examples/traefik/README.md`— y
   cubre las **tres** formas con las que un cliente alcanza esa página cruda, las tres **medidas**
   con `200` y los marcadores a la vista contra Traefik v3.6.25: el path terminado en `.html`/`.htm`,
   el mismo con **barra final** o un segmento detrás (`/minimal.html/`, `/matrix.html/`) y el mismo
   con la barra **percent-encoded**, en las dos cajas (`/minimal.html%2F`, `/matrix.html%2f`). Una
   exclusión anclada al fin de cadena —``!PathRegexp(`\.html?$`)``— **no** la cierra: medida, dejaba
   **más de una decena de** formas sirviendo el fichero crudo. La rama `%2[fF]` no es adorno: Traefik
   normaliza los escapes de los caracteres **no reservados** (`/minimal%2Ehtml` queda bloqueado
   porque lo lee como `minimal.html`) y **no** el de la barra, que sobrevive como `%2F`; y la regla
   **no depende de `sanitizePath`** —la tabla de 41 paths es **idéntica** con el *entrypoint* en su
   defecto y con `sanitizePath=false`—, mientras que la forma que solo hace opcional la barra final
   **sí** filtra con `sanitizePath=false`. Los **dos alias** `/default.html` y `/default.htm` quedan
   **fuera** de la exclusión **a propósito**: su path se clasifica como el código por defecto, el
   servicio lo intercepta y responde el código resuelto, **nunca `200`**. El **precio**, medido: la
   conjunción manda a la `404` de Traefik —sin pasar por el servicio— todo path cuyo `.html`/`.htm`
   vaya seguido de fin de cadena, de `/` o de `%2F` (`/404.html`, `/503.html`, `/sub/x.html`,
   `/x.html/y`, `/default.html%2F`); siguen siendo `404` y no rompe el flujo real, porque el
   middleware `errors` pide `/404` **sin** extensión (`errors.query: /{status}`). Y **dos límites**
   declarados y medidos: `PathRegexp` distingue mayúsculas, así que `/MINIMAL.HTML` y sus variantes
   llegan al servicio —pero reciben **su** `404` con `X-Robots-Tag`, nunca el fichero crudo: es una
   dependencia del sistema de ficheros, no una propiedad del servicio—; y las formas que sobreviven
   a la normalización de Traefik y no llevan barra (`/minimal.html%3F`, `/minimal.html%23`,
   `/minimal.html%3B`, `/minimal.html%20`, `/minimal.html%2E`, `/minimal.html.`,
   `/minimal%252Ehtml`, `/minimal%2Fhtml`) llegan al servicio y reciben **su** `404`, sin
   marcadores. **Ninguna prueba textual puede demostrar que la regla cierre todas las
   codificaciones** que un cliente pueda inventar: eso lo mide `examples/traefik/check.sh` con el
   ejemplo levantado, forma a forma. Lo guarda `tests/traefik_catchall.rs` **sobre los ficheros
   versionados** —la comprobación de extremo a extremo necesita un motor de contenedores y no corre
   en el CI—, con la regresión de la forma anclada al fin de cadena entre sus casos.

## Configuración (solo entorno)

| Variable | Defecto | Para qué |
|---|---|---|
| `PORT` | `8080` | Puerto de escucha (bind `0.0.0.0`). |
| `LOG_LEVEL` | `info` | Filtro de registro. |
| `CONTENT_DIR` | `content` | El directorio del operador: *override* de la página de un tema embebido **y** fuente de temas nuevos (los `*.html` de su **raíz** **añaden** al registro). |
| `DEFAULT_ERROR_PAGE` | `404` | Código por defecto; fuera de `400..=599` es error de arranque. |
| `LOG_FORMAT` | `text` | `text` o `json`; cualquier otro valor, o vacía, es error de arranque. |
| `RETRY_AFTER` | `120` | Segundos de la cabecera `Retry-After` (solo `429` y `503`); no numérica, o vacía, es error de arranque. |
| `THEME` | `default` | Tema de las páginas **HTML**: uno de los **embebidos** **o** un tema descubierto en `CONTENT_DIR`; un nombre que no esté en el **registro** de ese arranque, o vacía, es error de arranque. |

`GET /metrics` es un endpoint de servicio (siempre `200`, fuera de la resolución de páginas de
error) que expone el contador Prometheus de páginas servidas **por código y por el origen de ese
código**: `errors_pages_total{code="404",source="path"} 3`. `source` vale `path`, `header` (`X-Code`)
o `default` (`DEFAULT_ERROR_PAGE`), porque la cabecera la elige quien llama: sin esa etiqueta,
cualquiera puede inflar a voluntad el contador de un código y fabricar la apariencia de una
avalancha justo durante una incidencia. El contador vive en el estado del router, no en un `static`:
por eso dos routers no se contaminan entre sí. **No se publica en el router de respaldo** —no es una
página de error y delata el perfil de fallos de la infraestructura—: se deja fuera del *catch-all* o
se protege con un middleware de Traefik, que es donde va la autenticación. Dejarlo fuera es una
conjunción en la regla —``HostRegexp(`.+`) && !Path(`/metrics`)``—, y la llevan las **cinco**
definiciones de respaldo de los **cuatro** ficheros que lo entregan (`compose.yml`,
`examples/traefik/compose.yml`, el bloque del `README.md` raíz y las **dos** de
`examples/traefik/README.md`). La guarda es `tests/traefik_catchall.rs` —**nueve** pruebas—,
porque el CI no puede levantar contenedores y comprobarlo de extremo a extremo: la prueba lee esos
cuatro ficheros versionados y falla en cuanto un respaldo pierde la conjunción. Quitarla vuelve a
publicar el contador a cualquiera —una petición a `/metrics` en un Host sin router propio casa con el
respaldo y devuelve el perfil de fallos de la infraestructura—; con ella, Traefik responde su propio
`404` y el servicio ni se entera. `/version` se mantiene a la vista por paridad deliberada con
`error-pages`.

## Cómo llega el código de error (contrato con Traefik)

El middleware `errors` de Traefik reenvía la petición con la plantilla `errors.query: /{status}`. Por
tanto **el código viaja en el path**: una página 404 llega como `GET /404`. `X-Code` es el fallback.

Orden de resolución (el **primero** que haga match): **path** (`/404`, `/404.html`, `/404.json`,
`/503.xml`…) → cabecera **`X-Code`** → **`DEFAULT_ERROR_PAGE`** (defecto `404`). `/default.html` y
`/default.htm` son el **alias del código por defecto**: se resuelven **después** de la normalización
del path, responden `DEFAULT_ERROR_PAGE` y **nunca `200`** (las otras páginas de tema se sirven
como assets crudos; ver la garantía 4).

El **tema** es un eje aparte, y solo toca al HTML: lo elige `THEME` (entorno, al arrancar) y, por
petición, la cabecera **`X-Theme`**, con el mismo principio —el **primer** origen que haga match, así
que un `X-Theme` conocido gana a `THEME`—. Un `X-Theme` ausente, desconocido o vacío **no** es un
error: cae al configurado, porque esa cabecera la elige quien llama y un cliente no puede romper el
servicio. Comparación **exacta** (`X-Theme: Terminal` no es `terminal`). `THEME`, en cambio, la
escribe el operador: un valor que no sea uno de los nombres —o la cadena vacía— **aborta el
arranque** con un error que nombra la variable y enumera los nombres válidos, como `LOG_FORMAT`. En
JSON, XML y texto plano no hay página que incrustar, así que el tema se ignora.

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

Toda respuesta declara en `Vary` `Accept-Encoding`, los nombres de las cuatro cabeceras de
correlación —`X-Request-Id`, `X-Trace-Id`, `X-Correlation-Id`, `X-Amzn-Trace-Id`— y **`X-Theme`**,
**siempre**, las traiga la petición o no: la respuesta copia las cabeceras de correlación, así que una
caché intermedia tiene que saber que varía por ellas; y **el cuerpo** del HTML depende de `X-Theme` —la
página del tema que va incrustada—, así que sin declararla una caché le entregaría a un cliente el tema que pidió
otro, o el cuerpo entero de otro. Sin caché por medio el impacto es nulo (Traefik no cachea por
defecto), pero con un CDN delante no. Una lista que cambiase según la petición no le serviría a una
caché: por eso es fija.

## Integración Traefik (lo mínimo que debe aceptar el servicio)

```yaml
traefik.http.middlewares.error-pages-middleware.errors.status: 400-599
traefik.http.middlewares.error-pages-middleware.errors.service: error-pages-service
traefik.http.middlewares.error-pages-middleware.errors.query: /{status}
traefik.http.services.error-pages-service.loadbalancer.server.port: 8080
```

Y el respaldo que el repositorio entrega —los tres labels y las dos definiciones del README del
ejemplo— lleva la conjunción completa, con las **dos** exclusiones (la de `/metrics` y la del leak
de las páginas de tema, garantía 8):

```yaml
traefik.http.routers.errserve-catchall.rule: HostRegexp(`.+`) && !Path(`/metrics`) && (!PathRegexp(`\.html?((%2[fF])|/|$)`) || Path(`/default.html`) || Path(`/default.htm`))
```

⚠️ La regla lleva una barra invertida (`\.html?…`), y en un escalar YAML **entrecomillado** esa
barra va **doblada** (`\\.`): escribirla simple ahí **no** vale —el parser falla con
`found unknown escape character`, el *provider* **entero** deja de cargar y el contenedor sigue
arriba **sin ningún router**, en silencio—. Los **cinco** sitios de este repositorio la escriben
como escalar **plano**, que es donde la barra va simple y donde los **dos** `$` de la regla llegan
a Traefik **verbatim** (medido). El **precio** de la conjunción y sus **dos límites** están escritos
en la garantía 8.

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
cargo test                                  # 213 tests
cargo test <substring>                      # un test por nombre
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo check --offline                       # el repo compila SIN RED (caché local de Cargo)
cargo deny check -D warnings                # dependencias: avisos, licencias, duplicaciones y fuentes
just changelog [v0.3.0]                     # antepone al CHANGELOG la sección de lo no etiquetado
just build | run | dev                      # empaquetado (docker por defecto; just engine=podman ...)
just smoke                                  # build + smoke test; PORT=… y PORT_VOLUMEN=… cambian los dos loopback
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
  `cargo-deny` (la puerta de dependencias, con `deny.toml`) e `imagen`, que **construye la imagen y
  la prueba** (`bash .github/scripts/smoke.sh`, el mismo script que corre el release) —la puerta del
  empaquetado, y la que faltaba: hasta el change `fix-image-build` la imagen solo se construía en
  `release.yml`, al etiquetar, así que un `Dockerfile` que dejara de compilar se descubría
  publicando—. **Los tres están marcados como requeridos en `main` y en `development`**, así
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
  sin dejar **ni** imagen **ni** Release **ni** crate. `imagen` construye, **prueba** la imagen
  (`.github/scripts/smoke.sh`, el mismo script que corre el CI; el runner sí tiene `docker` real
  —aquí solo hay `podman`, que además descarta el `HEALTHCHECK` al construir en formato OCI—),
  publica en GHCR —`latest` solo se mueve en versiones estables— y
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

Y un aviso para quien compare imágenes entre versiones: **el mismo código no da el mismo digest**.
Medido entre la `0.8.0` y la `0.8.1`, cuyo árbol del servicio es **idéntico** —`git diff v0.8.0 v0.8.1
-- src/ content/ Dockerfile` sale vacío—: de las **cinco** capas de la `0.8.1` (`sha256:5751ff2f…`)
solo coincide con la `0.8.0` (`sha256:44f88a26…`) la **primera**, la base `alpine`. Y el build local
tampoco reproduce el del runner: la imagen etiquetada `v0.8.0` construida en esta máquina tiene
`sha256:e35ab086…`. Lo único que el servicio dice distinto entre las dos versiones es `GET /version`
—el binario embebe `CARGO_PKG_VERSION`—: `{"version":"0.8.1"}` frente a `{"version":"0.8.0"}`.

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
- **`build.rs` lee `content/*.html` y genera `EMBEBIDOS`, y `src/embedded.rs` lo incluye con `include!`.**
  El descubrimiento se resuelve al compilar: borrar o renombrar **cualquiera** de las páginas del proyecto
  —`content/*.html`— ya no degrada en silencio al respaldo de texto: es un
  **error de compilación** (D6). Y como el embedding se resuelve al compilar, `content/` tiene que seguir
  entrando en el paquete —comprobable con `cargo package --list --offline`, que lista las páginas—: sin
  ese hecho el binario compila aquí y falla en la máquina de quien instala (D3). Y por eso el **stage de
  compilación de la imagen** también necesita `content/` **y `build.rs`**: no basta con que `content/`
  esté en el árbol, tienen que estar **en el `builder`** —sin `build.rs` la compilación muere con
  `OUT_DIR`/`EMBEBIDOS` ausentes (medido: `exit code 101`)—. Un `Dockerfile` que solo copie `content/` al
  stage de runtime **no compila**: es justo lo que caza el job `imagen` del CI en cada propuesta de
  cambio. La medición histórica del modelo anterior —el `exit code 101` y los doce `couldn't read` de la
  `0.4.0`— queda como referencia de lo que se juega el build en ese stage.
- **Las páginas son opacas: el servicio no valida su contenido.** No hay guardas de red, movimiento
  ni cobertura de marcado, ni prueba de paridad de las páginas canónicas. Una página de tema —embebida
  o descubierta— puede llevar `@import`, animación, redirecciones, marcadores o ninguno; el servicio
  la sirve tal cual y solo rellena `{{code}}`/`{{reason}}`/`{{description}}` cuando están. El modelo
  anterior intentaba imponer tres comprobaciones textuales (red, movimiento y marcado) a las páginas
  **embebidas**; se retiraron con el descubrimiento al compilar: una página es contenido del
  operador y el servicio no la inspecciona. La decisión de qué lleva cada página es del autor del
  tema, no una puerta del binario.
- **El `<hr>` de las páginas canónicas es decorativo, y su contraste bajo es una decisión medida.**
  Lo llevan **cuatro** páginas: `default` y `compact` (1,43 sobre el fondo claro y 1,55 sobre el
  oscuro), `amber` (1,48) y `aurora` (1,49). Va por debajo del 3:1 de WCAG 1.4.11 y queda
  fuera de ese criterio porque cubre el **contenido no textual necesario para entender el contenido** y
  los componentes de interfaz, no el adorno. Se decide **no** subirlo —llevarlo a 3:1 pediría un trazo
  mucho más marcado y rompería el aire de las páginas canónicas—, y es una **decisión explícita del
  usuario**, no el resultado de una guarda: cada `<hr>` queda con su ratio en la cabecera de su página
  para que quien vuelva a medirlo no lo cuente como hallazgo. **Ninguna otra** página lleva `<hr>`. Lo
  que **sí** importa es el contraste del **texto** —el que se lee—, que se mide **a mano** (objetivo
  **AA**) y se deja escrito en la cabecera de cada canónica: **ninguna guarda mide el aspecto**
  —paleta, escala, jerarquía y espaciado se revisan a ojo—.
- **La página del proyecto es una plantilla, y ya no son ficheros sueltos: son las páginas.** Cada tema
  **es** su página —`content/*.html`— y el servidor la **rellena al
  servir** (`src/render.rs::plantilla`): `{{code}}` con el código resuelto, `{{reason}}` con
  `resolve::canonical_reason(code)` y `{{description}}` con `resolve::description(code)`, los tres —el
  código también en la clase del `body`, `class="c{{code}}"`, y en `data-text="{{code}}"` de `matrix`—.
  Las páginas **canónicas** envuelven el contenido en un `<main>` y cierran con un
  `<footer>errserve</footer>` —**sin `href`**, y con el nombre como **texto del fichero**, no como
  marcador: siguen siendo tres—. Las **ilustradas** llevan su propio marcado —`matrix`, un documento
  entero con canvas y pastillas; `terminal`, un CLI; otras, un botón que vuelve a `/`—, así que ni el
  `<main>` ni el `<h2>` ni el `<footer>` son obligatorios en todas: lo único que el servidor necesita
  de una página son los **tres marcadores**. Las
  reglas de esa sustitución: una página **sin** marcadores se sirve **byte a byte** —sin tocar el
  disco—, se sustituye **cada** aparición y no solo la primera, el escaneo es **por bytes** (una página
  del operador puede no estar en UTF-8) y se avanza por el documento de entrada, así que el valor recién
  insertado no se vuelve a escanear. Lo que se inserta se **escapa** (`&`, `<`, `>`): la razón y la
  descripción las escribe el servidor, no un humano dentro del HTML, y escapar de más convertiría en
  `&quot;` una comilla que habría que leer tal cual.

  El único valor que cae dentro de un **atributo** es el **código** (`class="c{{code}}"` en las páginas,
  `data-text="{{code}}"` en `matrix`), que es un `u16` y solo produce dígitos: `escape_html` no escapa
  la comilla, así que la seguridad la sostiene **esa invariante**, no el escaper —la tabla de razones ya
  trae una comilla, `canonical_reason(418) == "I'm a teapot"`, y una prueba la fija—.

  `resolve.rs` es la **fuente única** del texto de los cuatro formatos: su tabla de descripciones manda
  para HTML, JSON, XML y texto. Antes el `<p>` de las páginas HTML llevaba su prosa copiada a mano y
  **cinco** códigos (`400`, `401`, `405`, `408`, `504`) decían una cosa en HTML y otra en JSON; ahora no
  pueden discrepar.

  La **cascada del cuerpo HTML tiene un solo paso**: la página del tema (`CONTENT_DIR/<nombre>.html` o
  su copia embebida). Se **retiraron** la capa por código y la del código por defecto
  (`CONTENT_DIR/<código>.html`, `CONTENT_DIR/<código por defecto>.html`): con las páginas en el
  repositorio, esa capa quedaría **siempre sombreada**. `DEFAULT_ERROR_PAGE` **conserva** su significado
  —el código que se sirve cuando ninguna fuente aporta uno utilizable— pero **ya no nombra ningún
  fichero**; el diseño por código se expresa con la clase `c{{code}}` del `body`, que no puede discrepar
  del estado. Se **retiraron** `autocontenido()`, `marcador()`, `MARCADOR_ETIQUETA`, `MARCADOR_HREF` y
  `read_html()` de `render.rs`, y `embedded::{PAGE, DEFAULT_SHEET_BYTES, DEFAULT_SHEET, ASSETS,
  asset()}`; el **respaldo de texto** queda reservado a un fallo de **E/S** —una página del operador que
  existe y no se puede leer responde el respaldo con el código resuelto, sin cambiar el estado—.

  El **operador conserva y gana**: su página del tema gana a la embebida y su página **sin** marcadores
  se sirve byte a byte (su texto fijo es suyo). **Pierde** la capa por código: un `content/404.html`
  suyo **ya no se consulta** (con `THEME=matrix` verá la página de `matrix`, no la suya), así que el
  camino para granularidad por código es el `<style>` de su página del tema.
- **La prueba de paridad de las dos copias ya no existe.** El modelo anterior comparaba la copia
  embebida con el fichero de `content/`; ahora la copia embebida **sale del mismo fichero** que
  `build.rs` incluye, así que no hay dos copias que puedan divergir. El crate no tiene *lib target*,
  así que una prueba de integración tampoco vería las constantes del binario (D5).
- **Lo que se retiró con la hoja, y por qué no queda andamiaje.** El modelo anterior incrustaba al
  servir una **hoja** dentro de la página, sustituyendo el marcador
  `<link rel="stylesheet" href="/css/error.css" />`, y publicaba esa hoja como asset en
  `/css/error.css`. Todo eso se fue de una pieza en `themes-as-pages`: `render.rs::autocontenido()`,
  `marcador()`, `MARCADOR_ETIQUETA`, `MARCADOR_HREF` y `read_html()`; `Theme::sheet` y `sheet_path`;
  `embedded::{PAGE, DEFAULT_SHEET_BYTES, DEFAULT_SHEET, ASSETS, asset()}`; el asset público
  `/css/error.css` y su requisito; y `main.rs::embedded_assets()` con su `fallback` de `ServeDir`, al
  quedarse **sin tabla** —ese respaldo usaba `fallback` y **no** `not_found_service`, que reescribe el
  estado «regardless of what the inner service returns» y habría servido un `404` con cuerpo de CSS
  (D2); una trampa que ya no tiene objeto al no haber tabla de assets—. El motivo de retirarlo **todo
  junto** es el que enseñó `self-contained-themes`: dejar la mitad de un mecanismo invita a que alguien
  lo «reactive» sin leer por qué se fue. Hoy cada tema es **una página** con su `<style>` dentro; **no
  hay `<link>` que sustituir ni hoja que servir**, así que ninguna de esas tres reglas del marcador
  —sin marcador no se lee hoja, un `<link>` que no es el marcador se respeta, se sustituye cada
  aparición— tiene ya nada que gobernar.

  Detalle de **historia con su motivo**, para que nadie lo «simplifique» de vuelta sin leerlo: la hoja
  `default` se embebía **una sola vez** a través de un `static` **de tipo array** —
  `DEFAULT_SHEET_BYTES` era el array, `DEFAULT_SHEET` un `&[u8]` que apuntaba a él— porque el asset
  `/css/error.css` y el tema `default` tenían que apuntar a la **misma** dirección y `std::ptr::eq` se
  usaba como comprobación. Un `static X: &[u8] = include_bytes!(…)` **no bastaba**: al leerlo en el
  inicializador de otro `static` el compilador re-inlinea el `include_bytes!` y con varias *codegen
  units* las direcciones diferían (medido: `ptr::eq` da `false` con `cargo test`). Con la hoja retirada
  **ese andamiaje ya no existe**: las páginas se comparan por **bytes**, no por punteros, así que
  cada una es un `&'static str` directo con su `include_str!` y no hace falta ningún array intermedio.
- **La página del operador gana a la embebida.** `Theme::page` aplica a la página la misma regla que a
  cualquier asset: un fichero **legible** en `CONTENT_DIR/<nombre>.html` gana a la copia embebida. Como
  se resuelve **al servir**, un operador que edite su página la ve cambiar sin reiniciar. Un fichero
  **ausente** cae a la embebida **sin** `warn` (es lo normal en un binario instalado sin ficheros de
  datos); uno **presente pero ilegible** deja un `warn!` que **nombra el fichero y su causa** y el
  render lo convierte en el **respaldo de texto** con el estado intacto —la página es *contenido*, no
  adorno: tapar en silencio con la copia embebida un directorio del operador roto lo escondería—. Es la
  Opción A de `design.md` (garantía 7), sin cambios para las páginas.
- **El registro de temas se construye al arrancar, y su barrido tiene reglas exactas.** El registro
  efectivo es los **embebidos** **más** los temas descubiertos en la **raíz** de
  `CONTENT_DIR`: un `*.html` de más **es** un tema, y su **tallo** es su nombre (`neon.html` → `neon`).
  Las reglas, una a una: **solo la raíz** —un `temas/neon.html` no es un tema—; la extensión tiene que
  ser **exactamente** `html` (`neon.htm`, `neon.HTML` y `neon.css` **no** lo son); el **tallo de un
  tema embebido** no añade un tema (`content/default.html` es la página del `default` embebido, no un
  `default` de más); y un **tallo que reclama la resolución de códigos** queda **fuera** —el criterio
  **no** es el rango utilizable y está medido: `code_from_path` **parsea** el número y
  `is_error_page_path` decide **sin** filtrar `400..=599`, así que **cualquier** tallo que se lea como
  un `u16` (`404`, `999`, `200`…) reclama el path, que responde **otra** página de error—. Los
  descubiertos van **ordenados por nombre** detrás de los embebidos (`read_dir` no promete ningún orden).
  El barrido es **al arrancar**: **añadir** un tema exige **reiniciar**, mientras que los **bytes** de
  la página se leen **al servir**, así que **editar** uno ya descubierto se ve **sin** reiniciar. Un
  directorio que **no existe** deja los embebidos **sin** `warn` (es lo normal en un binario sin ficheros
  de datos); uno que existe y **no se puede listar** deja los embebidos **con** un `warn` que **nombra el
  directorio y su causa**. Y la **trampa del montaje** (D7): un `THEME` **correcto** cuyo fichero
  **todavía** no esté en `CONTENT_DIR` —un volumen que aún no se ha montado cuando el contenedor
  arranca— **aborta** el arranque, porque `THEME` se valida contra el registro de **ese** arranque; la
  mitigación es del operador —declarar la dependencia del volumen, o arrancar sin `THEME`—. Y el
  servicio **no inspecciona** el contenido de ninguna página: las páginas son opacas, tanto las
  embebidas como las descubiertas, y no hay guardas que las validen.
- **El `Dockerfile` copia `content/` dos veces, y la primera no es un «por si acaso»: es una entrada de
  la compilación.** El stage `builder` necesita `content/` **y `build.rs`** para compilar —`build.rs`
  resuelve la ruta de las páginas en tiempo de compilación—, así que sus copias de `content/` y
  `build.rs` son **obligatorias**: sin `build.rs` la compilación muere con `OUT_DIR`/`EMBEBIDOS`
  ausentes (medido: `exit code 101`); sin `content/`, el `cargo build` del crate real muere y no hay
  imagen. La segunda copia, la del stage `runtime`, es la decisión que sigue abierta: `CONTENT_DIR` es
  la única ruta de estáticos y, como la capa del disco gana (D1), la imagen sirve esos ficheros y no los
  del binario, de modo que quitarla cambiaría qué bytes se sirven en producción. Es la pregunta abierta
  de `design.md`, y ahí sigue.
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
- **El smoke test es un artefacto único, y su guarda tiene un límite escrito.** La guarda es
  `tests/puertas.rs` (**nueve** pruebas): que `ci.yml` y `release.yml` invoquen el script por su ruta
  literal, que **ningún** workflow lleve los ayudantes de aserción (`comprobar(`, `cabecera(`,
  `fallo `) ni su propio bucle de arranque contra el contenedor, y que `examples/traefik/check.sh`
  conserve el caso del `502`; **no demuestra** que el script lo compruebe todo —eso lo mide correrlo
  contra una imagen de verdad—, y la lista de ayudantes es **cerrada** a propósito (un `curl` suelto,
  como el que el release usa contra la API de GHCR, no es incumplimiento). El script recibe `IMAGE`
  (obligatoria), el motor por `ENGINE` (`docker` por defecto; aquí `podman`) y el puerto de loopback
  por `PORT`; arranca los contenedores solo en loopback, espera a `/healthz` y los retira **siempre**,
  también al fallar. Arranca **dos**: el de siempre —la imagen tal cual, el que prueba el leak
  (`/minimal.html -> 200`, la garantía 4)— y un **segundo** con un `CONTENT_DIR` **montado** y un tema
  de más, en su propio puerto de loopback (`PORT_VOLUMEN`, por defecto `PORT` + 1). Ese segundo existe
  porque el barrido del directorio del operador (change `themes-from-content-dir`) depende de que el
  montaje sea legible por el uid **10001** con el que corre la imagen: medido, un directorio en `0700`
  responde `404` y con `chmod 755` responde `200`, algo que `cargo test` **no puede ver** porque lee el
  directorio con el uid de quien compila. El `HEALTHCHECK` solo se puede **exigir** cuando la imagen lo
  declara: `podman build` **sin** `--format docker` construye en formato OCI, que lo descarta —esta
  casa sí pasa ese flag (`.justfile`), así que aquí sí se declara y sí se comprueba—, y eso es un hecho
  sobre la construcción de quien lo corre, no sobre el servicio; si la imagen no lo declara, el script
  lo dice en voz alta y **no** lo cuenta como fallo. En esta máquina el paso de smoke añade **~20,6 s**
  con los **dos** contenedores, con un `docker build` en frío (`--no-cache`) de **29,7 s**; en el
  runner de Actions ese número solo se mide en el CI. Para
  correrlo a mano aquí: `just smoke` (construye y lo corre con el motor de esta máquina), `PORT=9090
  just smoke` cambia el puerto de loopback y `PORT_VOLUMEN=9091` el del segundo contenedor. Sigue sin
  construir ni publicar nada y sigue siendo el **único** sitio con las aserciones.
- **La conjunción del leak no depende de `sanitizePath`, y la trampa del escalar entrecomillado es
  del despliegue.** Medido: la tabla de 41 paths con la regla final es **idéntica** con el
  *entrypoint* en su defecto y con `sanitizePath=false`, mientras que la forma que solo hace opcional
  la barra final (`\.html?/?$`) **sí** filtra con `sanitizePath=false` —ataba el leak a una opción del
  operador, y por eso no se usa—. Y la regla lleva una barra invertida, que en un escalar YAML
  **entrecomillado** va doblada (`\\.`): esa trampa y los dos `$` de la regla están en «Integración
  Traefik», y el **precio** de la conjunción y sus **dos límites** en la garantía 8.

## Referencia de implementación (qué no repetir)

`/data/rust/simpleserver` — proyecto hermano del mismo autor (atareao), de donde sale el stack
(`axum` + `tokio` + `tower-http` + `tracing`). Dos incoherencias suyas que aquí **no** se copian:
(a) declara `const STATIC_DIR = "static"` pero su `Dockerfile` copia `content/` —aquí la ruta es una
sola—; y (b) lee `LOG_LEVEL`/`PORT` del entorno **y** declara un `config.yml` que nunca lee —aquí la
configuración entra solo por el entorno—.
