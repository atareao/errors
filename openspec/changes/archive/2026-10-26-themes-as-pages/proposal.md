# Proposal: themes-as-pages

## Why

Hoy un tema es **una sola hoja de estilos**, y está escrito como contrato en el spec vivo de
`error-page` («El tema del HTML es configurable»): «Un tema SHALL ser **una sola hoja de estilos**:
añadir un tema es añadir un fichero, no duplicar la página». El eje del tema solo toca el **aspecto**:
el marcado lo pone el operador —o, agotado el operador, la plantilla del proyecto— y el tema se limita a
rellenar el `<style>` que sustituye al marcador `<link rel="stylesheet" href="/css/error.css" />`.

Ese modelo se queda corto para un tema que quiere entregar una **experiencia**, no un color. El tema
`matrix` —lluvia digital en un `<canvas>`, terminal, pastillas roja y azul, un minijuego y un toast—
necesita su propio marcado, su propio script y su propio `<canvas>`; nada de eso cabe en una hoja. Con
el modelo actual, la única forma de entregarlo sería meter su HTML y su JS en la **página del
proyecto**, que es la de todo el mundo: un tema rico obligaría a los demás a cargar su canvas y su
script. El usuario **rechazó** que un tema sea solo CSS y aprobó que **todos los temas sean páginas**.

Y el segundo hecho es de **inventario**: el conjunto de nombres es **seis** y está escrito en prosa y en
constantes por medio repositorio —`compose.yml`, `README.md`, `AGENTS.md`, el spec de `error-page`,
`src/theme.rs` y `src/config.rs`—. Añadir un tema no es añadir una fila a una tabla: es un barrido. El
conjunto pasa a **siete** nombres.

Y el tercero es de **precedencia**: el repositorio **entrega** la página del tema por defecto. Si la
capa por código del operador ganase siempre —la regla de hoy—, la página del tema elegido no se vería
**nunca** cuando el operador tuviera su propio `content/404.html`. La decisión de qué gana no es
cosmética: decide si los temas existen o no.

## La decisión que se somete a aprobación

**Un tema ES una página, y el nombre del tema es la ruta del fichero.**

1. **Siete nombres**, los mismos de hoy más uno: `default`, `minimal`, `terminal`, `paper`, `amber`,
   `compact` y `matrix`. La página de cada tema vive en la **raíz** de `CONTENT_DIR`, con el tallo
   derivado del nombre: `CONTENT_DIR/<nombre>.html` (`default.html`, `matrix.html`, …). Siete ficheros
   en `content/`.
2. **El nombre es la ruta.** `Theme` SHALL ser `{ name, embedded }` y su `page_path()` SHALL ser
   `format!("{name}.html")`: no hay tabla de rutas que pueda divergir del nombre. Las siete páginas van
   **embebidas** con `include_str!`, así que un binario instalado sin `content/` sirve los siete
   diseños, y borrar una página es un **error de compilación**, no un respaldo silencioso.
3. **`Theme::page(content_dir)`.** Un fichero **legible** en disco gana; **ausente** cae a la embebida
   **sin** `warn`; **presente pero ilegible** es un error de E/S (`RenderError::Io`) que responde el
   **respaldo de texto** con el estado intacto, como hoy para las páginas: la página es **contenido**,
   no adorno, y no se tapa con la copia embebida.
4. **La cascada del cuerpo HTML queda en un solo paso: la página del tema.** Se retiran la capa por
   código (`CONTENT_DIR/<código>.html`) y la capa del código por defecto
   (`CONTENT_DIR/<código por defecto>.html`), con sus pruebas. `DEFAULT_ERROR_PAGE` **conserva** su
   significado —el código por defecto, `404` de fábrica— pero ya **no nombra ningún fichero**.
5. **`content/404.html` pasa a ser `content/default.html`** —la página del tema `default`, la del
   proyecto y el punto de personalización del operador—. `content/css/error.css` y las cinco
   `content/themes/*.css` **se borran**: las reglas de cada una pasan dentro del `<style>` de la página
   de su tema.
6. **Cada página es autocontenida**: su `<style>` dentro, y su `<script>` también dentro si lo lleva.
   `matrix.html` es un **documento propio completamente distinto** —canvas de lluvia, glitch,
   scanlines, pastillas, terminal, minijuego, toast—; su marcado puede diferir del de las seis sobrias,
   que comparten el esqueleto `main > h1 / h2 / hr / p` más `footer` y el `body class="c{{code}}"`.
7. **Desaparecen** el marcador `<link rel="stylesheet" href="/css/error.css" />`;
   `src/render.rs::autocontenido()`, `marcador()`, `atributos_de_etiqueta()`, `es_el_marcador()`,
   `MARCADOR_ETIQUETA` y `MARCADOR_HREF`; `Theme::sheet` y `sheet_path`; `embedded::PAGE`,
   `DEFAULT_SHEET_BYTES`, `DEFAULT_SHEET`, `ASSETS` y `asset()`; el asset público `/css/error.css` y su
   requisito en `http-server`; y el respaldo de assets embebidos de `main.rs` (`embedded_assets()`) al
   quedarse sin tabla. La fase RED lo confirma antes de retirarlo.
8. **`/default.html` y `/default.htm` son alias del código por defecto** (`DEFAULT_ERROR_PAGE`),
   resueltos **después** de la normalización del path y tratados como el código, no como asset; el
   origen del código para las métricas sigue siendo `Path`. Sin esto, ese fichero se serviría crudo con
   los marcadores a la vista. Las otras seis páginas (`/minimal.html`, `/matrix.html`, …) se sirven como
   **assets crudos** si alguien las pide: leak cosmético, documentado y mitigado en el proxy (bloquear
   `*.html` salvo `/default.html`).
9. **No cambia nada de esto**: el estado es siempre el código resuelto y nunca `200`; el formato
   (JSON/XML/texto ignoran el tema); `X-Robots-Tag`; `Retry-After` solo en `429`/`503`; `Vary` con las
   cuatro cabeceras de correlación y `X-Theme` **siempre**; la normalización del path; `SERVICE_ENDPOINTS`;
   el respaldo de texto para una página ilegible; que un `X-Theme` ilegible no impida servir la página;
   y la semántica de `THEME` (lista **cerrada** de siete nombres, comparación exacta, valor desconocido
   o vacío **aborta el arranque** nombrando los siete) y de `X-Theme` (el conocido gana al configurado;
   desconocido, ausente o vacío cae al configurado).
10. **Los marcadores siguen siendo tres**: `{{code}}`, `{{reason}}`, `{{description}}`, rellenados al
    servir, escapados de `&<>`, escaneo por bytes, se sustituye cada aparición, y una página **sin**
    marcadores se sirve **byte a byte** sin tocar el disco. Las siete páginas los llevan.
11. **Guardas y pruebas** (se *retargetizan* de hojas a páginas): red (`@import`, `url(` cuyo destino no
    sea el propio documento, `<script src=`, `fetch(`, `XMLHttpRequest`, `WebSocket`, `sendBeacon`,
    listas cerradas y **límite declarado por escrito**); movimiento (el bloque
    `@media (prefers-reduced-motion: reduce)` **existente y no vacío** neutraliza cada animación
    declarada; y la página de `matrix` **menciona** `prefers-reduced-motion` en su script —la guarda
    solo puede ver la mención, y lo dice—); cobertura de marcado **solo sobre las seis páginas sobrias**
    (`matrix` es libre: no tiene por qué llevar `h2` ni `hr`). Nuevas: cada nombre tiene su fichero y el
    tallo coincide con el nombre; **quitando el bloque `<style>` de cada una, las seis páginas sobrias
    son byte a byte idénticas**; cada página lleva los tres marcadores; ninguna página referencia nada
    externo; el cuerpo servido no lleva ningún `<link>`.
12. **Rupturas del release 0.8.0** (minor con rupturas, permitido en `0.x`): `/css/error.css` y
    `themes/*.css` dejan de existir; `content/404.html` → `content/default.html`; la capa por código
    desaparece (el diseño por código pasa a `body.c{{code}}` y al CSS de la página). **El release —tag,
    imagen, crate y Release— queda pendiente de OK explícito del usuario**: en `tasks.md` va como tarea
    final bloqueada, no como algo a hacer.

## What Changes

1. **`src/theme.rs`** — `Theme` pasa a `{ name, embedded }`; `page_path()` deriva `"<nombre>.html"`;
   `Theme::page(content_dir)` resuelve con la regla de asset (legible > embebida; ausente > embebida sin
   `warn`; ilegible > `Io`); `THEMES` pasa a siete entradas con la página embebida de cada tema;
   `THEME_NAMES` enumera siete; `Theme::sheet` y `sheet_path` se retiran.
2. **`src/render.rs`** — la cascada del HTML se queda en **un paso** (la página del tema); se retiran
   `autocontenido()`, `marcador()`, `atributos_de_etiqueta()`, `es_el_marcador()`, `MARCADOR_ETIQUETA`,
   `MARCADOR_HREF` y `read_html()`; `plantilla()`, `rellena()`, `reemplaza_todos()`, `busca()` y
   `escape_html()` no cambian; el respaldo de texto sigue en `render_text_fallback`.
3. **`src/embedded.rs`** — embebe **las siete páginas**; se retiran `PAGE`, `DEFAULT_SHEET_BYTES`,
   `DEFAULT_SHEET`, `ASSETS` y `asset()`; la paridad byte a byte se comprueba página a página.
4. **`src/config.rs`** — `THEME` valida contra los **siete** nombres; el comentario y el test que dicen
   «seis» pasan a «siete».
5. **`src/main.rs` / `src/resolve.rs`** — el alias `/default.html`/`/default.htm` se clasifica como el
   código por defecto, después de la normalización del path y antes de `ServeDir`; se retira
   `embedded_assets()` y su `fallback` de `ServeDir` (no queda tabla de assets embebidos).
6. **`content/`** — `default.html` (antes `404.html`), `minimal.html`, `terminal.html`, `paper.html`,
   `amber.html`, `compact.html` y `matrix.html`, cada una con su `<style>` dentro; se borran
   `content/css/error.css` y las cinco `content/themes/*.css`.
7. **`tests/content_pages.rs`** — un test por propiedad nueva: siete ficheros con el tallo del nombre,
   tres marcadores por página, las seis sobrias idénticas quitando el `<style>`, ningún `href` externo,
   el cuerpo servido sin `<link>`.
8. **Configuración y prosa** — `compose.yml`, `README.md`, `AGENTS.md` y `CHANGELOG.md`: de seis a
   siete nombres y del modelo «página + hoja» al modelo «el tema es una página».

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `error-page`: **modificado** «El código se resuelve en cascada» (alias `/default.html`/`/default.htm`),
  «El path se normaliza antes de clasificarse» (el alias después de normalizar), «Un asset existente
  gana a la página de error» (la excepción del alias; las seis páginas de tema son assets crudos), «Un
  fallo al leer el HTML no cambia el estado» (la página del tema: legible/ausente/ilegible), «Los
  códigos que un proxy emite tienen su propia página» (siete páginas embebidas, sin *override* por
  código) y «Cada página se identifica con la clase de su código» (las siete páginas). **Retirados** «El
  cuerpo depende del formato y el código es ortogonal», «Las páginas no referencian ficheros que no
  existen», «El HTML servido es autocontenido» y «El tema del HTML es configurable», cuyos artefactos
  —la cascada por código, la hoja, el marcador `<link>`— desaparecen. **Añadidos** «El cuerpo HTML es la
  página del tema», «El tema del HTML elige una página», «Las siete páginas del proyecto», «Ninguna
  página pide nada por la red» y «Una página que anima puede apagarse».
- `http-server`: **modificado** «La configuración se lee solo del entorno» (siete nombres) y «La imagen
  se construye desde el árbol y trae las páginas dentro» (las siete páginas; la asimetría del stage de
  runtime, decisión abierta). **Retirado** «Los assets estáticos se sirven desde CONTENT_DIR» (la tabla
  de assets embebidos desaparece). **Añadido** «Los ficheros de CONTENT_DIR se sirven como assets».

## Non-goals

- **No se añade rotación de temas** (`--rotation-mode` de `error-pages`): sigue siendo un no-goal
  declarado del proyecto.
- **El tema no toca el formato.** JSON, XML y texto plano no tienen página que incrustar y el tema se
  ignora en ellos.
- **Las guardas son textuales y declaran su límite.** La de red no ve una URL construida en tiempo de
  ejecución, ni `window["fetch"]`, ni una librería que pida por su cuenta; la de movimiento sobre el
  script solo comprueba la **mención** de `prefers-reduced-motion`, no que la rama sea correcta; la del
  CSS garantiza el patrón canónico, no que todos los navegadores apaguen el movimiento. Los tres límites
  se escriben en la documentación y en la cabecera de cada página.
- **No se ocultan las seis páginas planas.** `GET /minimal.html`, `/terminal.html`, `/paper.html`,
  `/amber.html`, `/compact.html` y `/matrix.html` responden `200` con el fichero crudo y los marcadores
  a la vista: es coherente con cómo se servían antes las hojas de `content/themes/`. El leak es
  cosmético y, con las páginas en la raíz de `CONTENT_DIR`, es **inevitable**; el lugar donde se bloquea
  —`*.html` salvo `/default.html`— es **el proxy**, y el servicio no promete ocultarlas.
- **No se valida HTML al servir.** El servicio no parsea la página (es contenido) ni el CSS (es adorno):
  validar en el arranque haría que el arranque dependiera del sistema de ficheros.
- **No se promete que `matrix` sea igual que las sobrias.** Es un documento propio; la cobertura de
  marcado solo cubre las seis sobrias, y la paridad byte a byte de esas seis se vigila con una prueba,
  no con una guarda.
- **No se implementa el release.** Etiqueta, imagen, crate y Release quedan tras el OK explícito del
  usuario (`tasks.md`, fase 5).

## Impact

- `src/theme.rs` — `Theme` gana `page_path()` y pierde `sheet_path`/`sheet`; `THEMES` a siete;
  `THEME_NAMES` a siete; guardas nuevas de red, movimiento y cobertura sobre las páginas; tests
  `la_tabla_declara_los_seis_temas`, `theme_by_name_solo_acepta_los_seis` y
  `las_hojas_embebidas_son_byte_a_byte_las_de_content` pasan a páginas y a siete.
- `src/render.rs` — `html_body`/`raw_html` a un solo paso; retiradas las funciones y constantes del
  marcador y de la hoja; pruebas del fixture adaptadas a la página del tema.
- `src/embedded.rs` — las siete páginas embebidas; retirados `PAGE`, `DEFAULT_SHEET_BYTES`,
  `DEFAULT_SHEET`, `ASSETS` y `asset()`; paridad byte a byte página a página.
- `src/config.rs` — comentarios y tests de `THEME` a siete.
- `src/main.rs` / `src/resolve.rs` — alias `/default.html`/`/default.htm` como el código por defecto;
  retirado `embedded_assets()` y su `fallback`.
- `content/default.html`, `minimal.html`, `terminal.html`, `paper.html`, `amber.html`, `compact.html`,
  `matrix.html` — las siete páginas, cada una autocontenida; borradas `content/css/error.css` y las cinco
  `content/themes/*.css`.
- `tests/content_pages.rs` y `tests/fixtures/` — las pruebas de las siete páginas y los fixtures del
  operador adaptados (`tests/fixtures/content/404.html` → `default.html`, sin los fixtures de hoja).
- `compose.yml`, `README.md`, `AGENTS.md`, `CHANGELOG.md` — seis → siete y el modelo nuevo.
- `openspec/specs/error-page/spec.md` y `openspec/specs/http-server/spec.md` — vía las deltas de este
  change.
- `Cargo.toml` — versión `0.8.0` en la fase de release, con la ruptura documentada.
