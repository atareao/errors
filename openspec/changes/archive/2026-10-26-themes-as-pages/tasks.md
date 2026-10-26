# Tasks: themes-as-pages

> Un tema deja de ser **una hoja de estilos** y pasa a ser **una página**: el nombre del tema **es** la
> ruta del fichero (`CONTENT_DIR/<nombre>.html`). Siete páginas, una por nombre, en la raíz de
> `content/`, todas autocontenidas —su `<style>` dentro, su `<script>` también si lo lleva—. La cascada
> del cuerpo HTML se queda en un solo paso —la página del tema— y el diseño por código se muda a
> `body.c{{code}}`. Se retiran la hoja, el marcador `<link>`, `autocontenido()` y el respaldo de assets
> embebidos. Las siete páginas van embebidas con `include_str!`. Se añade el tema `matrix` —siete
> nombres— y las guardas se *retargetizan* de hojas a páginas. El change ya está **implementado** (fases
> 0–3 en verde y verificadas); las tareas de abajo se marcan a medida que se completan. El release
> (fase 5) queda pendiente de OK explícito del usuario.

## 0. Prerrequisito

- [x] 0.1 Confirmar contra el árbol que las mediciones del change siguen siendo ciertas: el conjunto de
      nombres es seis y está en `compose.yml`, `README.md`, `AGENTS.md`,
      `openspec/specs/error-page/spec.md`, `src/theme.rs` (`THEMES` `[Theme; 6]`, `THEME_NAMES`) y
      `src/config.rs`. Si alguna línea se ha movido, actualizar la lista en `proposal.md` y `design.md`
      **antes** de empezar la fase RED.
- [x] 0.2 Confirmar que `Theme` sigue teniendo `name`, `sheet_path` y `embedded`; que `THEMES` es
      `[Theme; 6]`; que `Theme::sheet` resuelve la hoja como asset; y que `raw_html` consulta
      `CONTENT_DIR/<código>.html` y `CONTENT_DIR/<código por defecto>.html` antes de `embedded::PAGE`
      (`src/render.rs`).
- [x] 0.3 Confirmar el inventario de `content/`: **una** página (`404.html`) y **seis** hojas
      (`css/error.css` y `themes/*.css`). El test `el_proyecto_entrega_una_sola_pagina` cuenta una
      página; tras el change contará **siete**.
- [x] 0.4 Confirmar que `src/main.rs` clasifica un path con código en su primer segmento
      (`is_error_page_path` + `code_from_path`) y que **no** trata `/default.html` como el código por
      defecto: hoy llegaría a `ServeDir` y se serviría crudo. La fase RED lo prueba.
- [x] 0.5 Confirmar que `openspec validate themes-as-pages --strict` acepta la delta de este change
      antes de tocar código; si rechaza algo, corregir la documentación, no la implementación.
- [x] 0.6 Confirmar que el fixture del operador es `tests/fixtures/content/404.html` y que las pruebas de
      `src/render.rs` lo usan como «la página propia del operador»; tras el change ese papel lo tiene
      `tests/fixtures/content/default.html`.

## 1. RED (pruebas que fallan primero)

- [x] 1.1 **`Theme::page_path()` y `Theme::page(content_dir)`** en `src/theme.rs`, con un directorio
      temporal: un fichero **legible** en `CONTENT_DIR/<nombre>.html` gana a la copia embebida; un
      fichero **ausente** devuelve la copia embebida **sin** dejar constancia; un fichero **presente
      pero ilegible** devuelve `std::io::Error`. Se prueba con las **siete** páginas.
- [x] 1.2 **Los siete nombres.** `THEMES.len() == 7`; `THEME_NAMES` enumera siete; `theme_by_name("matrix")`
      existe; `THEME=matrix` arranca; un `THEME=neon` enumera los siete en el mensaje (`src/theme.rs`,
      `src/config.rs`, `src/main.rs`).
- [x] 1.3 **El nombre es la ruta.** Para cada tema, `page_path()` es `"<nombre>.html"` y el fichero
      existe en `content/` con ese tallo.
- [x] 1.4 **La página del tema se sirve rellena.** `{{code}}`, `{{reason}}` y `{{description}}` se
      sustituyen con el código resuelto y las tablas de `resolve`, con escape (`&`, `<`, `>`), y una
      página **sin** marcadores se sirve byte a byte. El caso `451` nombra `451` y no `404`
      (`src/render.rs`).
- [x] 1.5 **El cuerpo servido no lleva ningún `<link>` y lleva `<style>`**: prueba de router en
      `src/main.rs` sobre `GET /404` con `Accept: text/html`.
- [x] 1.6 **El alias `/default.html` y `/default.htm`**: se clasifican como el código por defecto
      **después** de la normalización del path; no llegan a `ServeDir`; ganan a `X-Code`; el origen es
      `Path`; `/default.html` con `THEME=matrix` sirve la página de `matrix` y con `THEME=default` la de
      `default` (`src/main.rs`).
- [x] 1.7 **Las seis páginas planas como assets crudos.** `GET /minimal.html`, `/terminal.html`,
      `/paper.html`, `/amber.html`, `/compact.html` y `/matrix.html` responden `200` con el fichero tal
      cual y los marcadores a la vista; `GET /default.html` **no**: se intercepta (`src/main.rs`).
- [x] 1.8 **El respaldo de assets embebidos sin tabla.** Sin tabla de assets, un path fuera del disco
      sigue respondiendo `404` y el middleware lo sustituye por la página del tema: la prueba fija el
      `404` y el cuerpo de página de error **antes** de retirar `embedded_assets()` y volvió a pasar
      **después** de retirarlo (`src/main.rs`).
- [x] 1.9 **`tests/content_pages.rs`**: cada nombre tiene su fichero y el tallo coincide con el nombre
      (siete `*.html`); cada página lleva los tres marcadores; **quitado el bloque `<style>`, las seis
      páginas sobrias son byte a byte idénticas**; ningún `href` local apunta a un fichero inexistente.
- [x] 1.10 **Guarda de red sobre páginas** — `ninguna_pagina_puede_hacer_una_peticion` en el
      `#[cfg(test)]` de `src/theme.rs`, con fixtures en línea: `fetch(`, `XMLHttpRequest`, `WebSocket`,
      `sendBeacon`, `<script src=`, `<img src=` externo, `url(...)` externo en un `<style>` incrustado y
      `@import` incumplen; `url(#…)`, `data:` y `src: local(...)` cumplen; un comentario no cuenta; una
      etiqueta en mayúsculas (`<SCRIPT SRC=`) incumple. (El limpiador de comentarios es **por regiones**:
      ver la desviación de la fase 2.)
- [x] 1.11 **Guarda de movimiento sobre páginas** — `una_pagina_que_anima_puede_apagarse`: una
      declaración de `animation`/`animation-name`/`transition`/`transition-property` con valor distinto de
      `none` sin neutralizar en un bloque `@media (prefers-reduced-motion: reduce)` existente y no vacío
      incumple; neutralizada, cumple; y una página cuyo script usa `requestAnimationFrame` y **no**
      menciona `prefers-reduced-motion` incumple. El caso «menciona la preferencia y anima igual pasa» se
      prueba y se declara como límite. La guarda mide la neutralización **por familia, no por
      declaración**, y ese segundo límite («dos animaciones declaradas y una sola neutralizada pasan»)
      también se prueba y se declara (ver la desviación de la fase 3).
- [x] 1.12 **Cobertura de marcado solo de las sobrias** — `cada_pagina_sobria_cubre_el_marcado`: las seis
      sobrias estilan `body`, `h1`, `h2`, `hr`, `p` y `footer`; `matrix` **queda fuera** de esta guarda.
- [x] 1.13 Confirmar que las pruebas previas siguen en verde y que las que fallan son **solo** las
      nuevas que se espera que fallen.

## 2. GREEN (implementación mínima)

- [x] 2.1 `src/theme.rs`: `Theme` pasa a `{ name, embedded }`; `page_path()` deriva `"<nombre>.html"`;
      `page(content_dir)` resuelve legible/ausente/ilegible; `THEMES` a `[Theme; 7]` con la página
      embebida de cada tema (**referenciada** de `src/embedded.rs`, que es quien la embebe con
      `include_str!`); `THEME_NAMES` con los siete; retirados `sheet_path` y `Theme::sheet`.
- [x] 2.2 `src/render.rs`: `html_body`/`raw_html` quedan en **un solo paso** —la página del tema—;
      retirados `autocontenido()`, `marcador()`, `atributos_de_etiqueta()`, `es_el_marcador()`,
      `MARCADOR_ETIQUETA`, `MARCADOR_HREF` y `read_html()`; `plantilla()`, `rellena()`,
      `reemplaza_todos()`, `busca()`, `escape_html()` y `render_text_fallback()` no cambian.
- [x] 2.3 `src/embedded.rs`: embebe las **siete** páginas; retirados `PAGE`, `DEFAULT_SHEET_BYTES`,
      `DEFAULT_SHEET`, `ASSETS` y `asset()`; la paridad byte a byte se comprueba **página a página**.
- [x] 2.4 `src/main.rs`: retirados `embedded_assets()` y el `.fallback(embedded_assets())` de
      `ServeDir`, con la prueba 1.8 ya en verde; `is_error_page_path` sigue decidiendo qué es página de
      error.
- [x] 2.5 `src/resolve.rs`/`src/main.rs`: el alias `/default.html`/`/default.htm` se clasifica como el
      código por defecto después de la normalización del path y antes de `ServeDir`; origen `Path`.
- [x] 2.6 `src/config.rs`: `THEME` valida contra los siete; comentarios y tests que dicen «seis» pasan a
      «siete».
- [x] 2.7 `content/`: `404.html` → `default.html`; nuevas `minimal.html`, `terminal.html`, `paper.html`,
      `amber.html`, `compact.html` y `matrix.html`; **borradas** `content/css/error.css` y las cinco
      `content/themes/*.css`; las reglas de cada hoja pasan al `<style>` de su página. Las seis sobrias
      comparten el marcado; `matrix` trae canvas, terminal, pastillas, minijuego y toast, con
      `matchMedia('(prefers-reduced-motion: reduce)')` en su script.
- [x] 2.8 `tests/content_pages.rs` y `tests/fixtures/`: `tests/fixtures/content/404.html` →
      `default.html`; retirados los fixtures de hoja (`tests/fixtures/content/css/*`,
      `tests/fixtures/hoja-ilegible/`, `tests/fixtures/tema-del-operador/`); adaptados
      `tests/fixtures/pagina-ilegible/`, `tests/fixtures/pagina-sin-marcadores/` y
      `tests/fixtures/con-marcador/` a la página del tema.
- [x] 2.9 Todas las pruebas en verde; `git diff content/` con siete páginas nuevas, la hoja y las cinco
      hojas de tema borradas, y **ninguna** regla de las seis sobrias divergiendo entre sí.

### Desviaciones del plan (fase 2)

- **`render_body` perdió el parámetro `default_code`.** Con la capa por código retirada, ese parámetro
  se quedó sin uso; se eliminó de la firma y sus llamadores (`render_page` y los tests) concuerdan. El
  plan (2.2) solo hablaba de «un solo paso», no de la firma.
- **`Theme::embedded` es `&'static str`, no `&[u8]`.** El `design.md` (D1) mostraba `&[u8]`; la API de
  esta fase fija `&'static str`, así que la paridad de las dos copias se compara **por bytes** y el
  patrón del `static` de tipo array con `std::ptr::eq` —que existía para dar una dirección única a la
  **hoja**— **desaparece** con ella: ya no hay identidad de puntero que comprobar.
- **El limpiador de comentarios de las guardas es por regiones.** El `<!-- … -->` se limpia en toda la
  página; el `/* … */` **solo** dentro de `<style>` o `<script>` —es sintaxis de CSS y de JavaScript, no
  de HTML—; y el `//` **solo** dentro de `<script>` —fuera de un guion es texto visible, como en
  `<p class="boot">// errserve</p>` de `matrix`, y tratarlo como comentario taparía una petición que
  viniera detrás—. Se probó que un `//` en prosa HTML **no** tapa un `<img src=` que venga detrás.

## 3. REFACTOR / verificación

- [x] 3.1 `src/theme.rs`: la documentación de módulo —«un tema es una sola hoja»— pasa a «un tema es una
      página»; el contrato de red y el de movimiento se describen sobre la página.
- [x] 3.2 `src/render.rs`: la documentación de la cascada explica el **único** paso y que el diseño por
      código vive en la clase del `body`.
- [x] 3.3 `src/embedded.rs`: la documentación de las siete páginas y de la paridad página a página.
- [x] 3.4 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo check --offline`,
      `cargo test --offline` y `cargo deny check -D warnings` limpios.
- [x] 3.5 `cargo package --list --offline --allow-dirty`: las **siete** páginas viajan en el `.crate`; no
      queda ninguna hoja referenciada por `include_bytes!`.
- [x] 3.6 `openspec validate themes-as-pages --strict` limpio. Si rechaza un escenario o un nombre,
      corregir el texto —**no** inventar un nombre nuevo— y volver a validar.

### Desviaciones del plan (fase 3)

- **La guarda de movimiento se quedó por familia, no por declaración** (decisión del director, con su
  motivo): ligar cada declaración con *su* neutralización exigiría un parser de selectores y rechazaría
  neutralizaciones legítimas como `* { animation: none }` o una lista de selectores. El límite quedó
  escrito en el spec (escenario «dos animaciones declaradas y una sola neutralizada pasan la guarda»),
  en `design.md` (D6, guarda 2) y en `src/theme.rs` (doc de módulo, `css_que_anima` y
  `bloque_neutraliza`). La guarda **caza** el caso cercano —una declaración que anima **dentro** del
  bloque reduce—, y ese caso tiene su prueba y su escenario.
- **`HEAD` sobre una página de error responde sin cuerpo.** Antes el middleware servía cuerpo en un
  `HEAD`; lo destapó la prueba que repuso la cobertura de `HEAD` (que se había quedado sin una). Ahora
  `error_pages` captura `is_head` y `page_response` devuelve el cuerpo vacío, alineado con `ServeDir` y
  con las rutas de axum. Dos pruebas lo fijan (asset existente y path inexistente).
- **La revisión visual (5.3) destapó un defecto de legibilidad en `matrix` y se arregló.** La lluvia se
  dibuja con `ctx.fillStyle = '#39ff14'`, que es **exactamente** `--verde` —el color del `game-code`
  del minijuego— y **más brillante** que `--apagado` (`#7ddc9f`), el color de la línea de arranque, de la
  cabecera del minijuego y de su mensaje; y esos tres textos iban **sobre la lluvia sin fondo que los
  aislara** (el panel del juego era `rgba(0, 255, 65, 0.03)` y la línea de arranque no tenía fondo). Un
  glifo brillante detrás de una letra apagada se la comía, y detrás de un dígito del código se fundía con
  él: un incumplimiento del requisito «texto legible por encima del canvas» que **ninguna guarda puede
  medir** —ninguna mide el aspecto— y que solo se ve mirando. El arreglo: el panel del minijuego pasa a
  **opaco** con un tono propio (`--panel-2: #0b2214`, un pelo más claro que `--panel` para que `.key`
  siga leyéndose como su propia superficie) y la línea de arranque **sigue flotando sobre la lluvia** con
  un halo oscuro de cuatro desplazamientos. Los pares nuevos quedan **medidos y escritos** en la cabecera
  del fichero (10,06 / 12,36 / 14,46), y con ellos un límite declarado: la separación tecla/panel mide
  1,13 y es una frontera de **superficie** —como el `<hr>` decorativo—, no un par de texto; lo que
  sostiene el teclado es su borde y su texto a 17,44. No se tocó ni una declaración de animación: los
  cinco pares declaración/neutralización siguen casando con la guarda.

## 4. DOCS

- [x] 4.1 `compose.yml`: el comentario de `THEME` enumera los siete nombres.
- [x] 4.2 `README.md`: de seis a siete temas; el modelo «el tema es una página» con la estructura plana
      `content/<nombre>.html`; el alias `/default.html`; el leak de las seis páginas y la mitigación en
      el proxy; el punto de personalización del operador.
- [x] 4.3 `AGENTS.md`: los siete temas; el modelo nuevo; las garantías que mencionan la página y la hoja
      reescritas (el binario sirve las siete páginas sin ficheros de datos; el operador conserva su
      página de tema y pierde la capa por código); las tres guardas y sus límites.
- [x] 4.4 `CHANGELOG.md`: la entrada de `0.8.0` con las **rupturas** —`/css/error.css` y `themes/*.css`
      dejan de existir; `content/404.html` → `content/default.html`; la capa por código desaparece—. Se
      escribe en la fase de release, no antes.

### Desviaciones del plan (fase 4)

- **Lo que el auditor de seguridad de las páginas dejó escrito en la documentación**, además del modelo
  nuevo: la **invariante del atributo** —el único valor que cae dentro de un atributo es el código, un
  `u16` que solo produce dígitos; `escape_html` no cubre la comilla, y una prueba de
  `tests/content_pages.rs` la fija en las siete páginas— y la **incompatibilidad con una CSP estricta**
  —cada página lleva su `<style>` (y `matrix` su `<script>`) **en línea**, así que un despliegue que
  prohíba el estilo/script en línea sin `unsafe-inline`/nonce necesita ajustar su política—.

## 5. RELEASE — pendiente de OK explícito del usuario

> **No se hace commit, ni PR, ni etiqueta, ni publicación hasta que el usuario lo decida.** Esta fase
> se queda marcada y **no se ejecuta** en la fase de documentación.

- [x] 5.1 `Cargo.toml`: versión `0.8.0` (minor con rupturas, permitido en `0.x`). `CHANGELOG.md` con la
      entrada de `0.8.0` (tarea 4.4). Hecho: `Cargo.toml` y `Cargo.lock` están en `0.8.0` y la entrada
      está escrita (4.4).
- [x] 5.2 `cargo publish --dry-run --locked` y `cargo package --list --offline`: el paquete lleva las
      siete páginas. Medido: en el árbol **sin commitear** (el de ahora) `cargo publish` rechaza por los
      ficheros sucios, así que la puerta corrió con `--locked --allow-dirty` —empaqueta 39 ficheros y
      **compila el paquete extraído**—; el `--allow-dirty` **no** salta el `--locked` (el lock está
      coherente) y en el CI, sobre el árbol limpio de la etiqueta, no hace falta. `cargo package --list`
      lista las **siete** páginas de `content/` y ninguna hoja de tema.
- [x] 5.3 Revisión humana de las siete páginas servidas de verdad: claro y oscuro, con y sin
      `prefers-reduced-motion: reduce`, el canvas de `matrix` sin lluvia cuando se pide movimiento
      reducido, y las seis sobrias. Un número no distingue una jerarquía legible de una que no lo es.
      Hecha con Chromium headless (capturas de 1280×900): las seis sobrias en claro, las tres con modo
      oscuro en oscuro, y `matrix` con y sin movimiento reducido —con movimiento reducido pedido **en
      caliente** el canvas deja de cambiar, o sea que la lluvia se para sin recargar—. Destapó y cerró el
      defecto de legibilidad descrito en «Desviaciones del plan (fase 3)».
- [ ] 5.4 **PENDIENTE DE OK EXPLÍCITO DEL USUARIO**: commit, PR, etiqueta `v0.8.0`, imagen, crate y
      Release. Sin ese OK no se ejecuta nada de esta fase.
      → **Se queda sin marcar, y a propósito**: al archivar, el commit y los PR están hechos, pero la
      etiqueta `v0.8.0` —la que publica la imagen, el crate y el Release— se crea **después** del merge
      a `main`, así que marcarla ahora sería marcarla **por adelantado**. Una casilla marcada sobre algo
      que aún no ha pasado es peor que una sin marcar, y hay precedente: `release-hardening` se archivó
      con sus casillas de release sin marcar.
- [x] 5.5 `openspec archive themes-as-pages` **después** del OK y con la fase 3 en verde; el archivado
      fusiona las deltas en `openspec/specs/error-page/spec.md` y `openspec/specs/http-server/spec.md`.
      → Lo corre el director **justo después** de este commit: el archivo es el commit siguiente.
