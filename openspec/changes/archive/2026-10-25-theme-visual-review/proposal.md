# Proposal: theme-visual-review

## Why

Las **seis hojas** de tema nunca se han repasado como lo que son: la cara del servicio. Ninguna ha cambiado
**una sola regla** desde que se entregó —los changes posteriores solo han corregido su prosa—, así que
paleta, contraste, escala tipográfica, jerarquía y espaciado siguen sin revisarse, y `default` —el tema que
ve todo el mundo, y la hoja del asset público `/css/error.css`— no tiene ninguna firma propia.

Y la guarda de movimiento mide menos de lo que anuncia: se conforma con que la hoja **nombre**
`prefers-reduced-motion` en cualquier parte, así que un bloque `@media (prefers-reduced-motion: reduce)
{ }` —vacío— la pasa y el movimiento sigue sin poder apagarse. **Medido** con el mismo `incumplimientos()`
que corre en `cargo test`: una hoja que anima con ese bloque vacío —o con un comentario dentro— sale **sin
incumplimiento**. Es el defecto que `theme-sheet-contracts` ya corrigió una vez en el contrato de red.

## What Changes

1. **Repaso estético de las seis hojas, sin tocar la lista de temas.** Se revisan
   `content/css/error.css` (tema `default`) y `content/themes/{minimal,terminal,paper,amber,compact}.css`
   en **paleta, contraste, escala tipográfica, jerarquía y espaciado**. **Siguen siendo seis nombres**
   —no se añade ni se quita ninguno, y ni la lista que enumera el error de arranque de `THEME` ni
   `THEME_NAMES` cambian—. Cada hoja conserva la identidad que ya declara su cabecera, y ninguna se
   convierte en otra:
   - **`default`** — el sobrio de la casa, con un acento por código. Se conservan la paleta con su paso
     claro/oscuro y el acento por código —que es lo que distingue un `404` de un `503` sobre una hoja
     única—, y se ajusta lo que nunca se revisó: la escala del título, la del subtítulo, el ritmo del
     espaciado y el contraste del texto atenuado.
   - **`minimal`** — lo más desnudo posible: solo tipografía, sin color de acento, claro u oscuro según
     decida el sistema. Se afinan la escala y el aire; **no** se le añade nada, ni acento ni adorno.
   - **`terminal`** — fósforo verde sobre negro, monoespaciado. Se conserva el tono y **no** se le añade
     parpadeo, cursor ni brillo: un terminal apagado es su identidad. Se ajustan el contraste del verde
     y la escala.
   - **`paper`** — papel cálido, serif y tinta. Se conservan los márgenes generosos y la regla discreta;
     se ajustan el contraste de la tinta sobre el crema y el ritmo vertical.
   - **`amber`** — CRT ámbar. Monocromo en tonos de ámbar, sin brillos ni rastro: se conserva y se
     ajustan contraste y escala.
   - **`compact`** — denso, para pantallas pequeñas. Título y subtítulo **en línea**, para leer «503
     Service Unavailable» de un golpe: se conserva y se ajusta el espaciado para que siga cabiendo.
2. **El tema `default` es el que anima, y muy poco.** Un movimiento de entrada de **150–200 ms** sobre
   el bloque de contenido, con su bloque `@media (prefers-reduced-motion: reduce)` que lo **neutraliza**.
   Es el tema por defecto —el que ve todo el mundo— y su hoja es además el asset público
   `/css/error.css`. Los otros **cinco no se mueven**: el movimiento es la firma de la casa, no un
   adorno que se reparta.
3. **El marcado crece: `<main>` y `<footer>`.** El contenido se envuelve en `<main>` y la página gana
   un `<footer>` con el nombre del servicio —una firma, no un enlace: sin `href`, no aparece en la lista
   de referencias locales que el directorio tiene que resolver—. El `h1` con el código, el `h2` con la
   razón, el `hr` y el `p` con la descripción se conservan tal cual, y con ellos los tres marcadores de
   la plantilla, el marcador de la hoja y la clase `c<código>` del `body`. Las **seis hojas** estilan el
   `<footer>`: el marcado crece, y el contrato de que cada hoja cubre el marcado crece con él.
4. **La guarda de movimiento se endurece, y es la delta normativa de este change.** Pasa de conformarse
   con que la hoja **nombre** `prefers-reduced-motion` en cualquier parte a exigir que el bloque
   `@media (prefers-reduced-motion: reduce)` **exista**, **no esté vacío** y **neutralice cada animación
   que la hoja declare**. Y se precisa **qué anima**: una **declaración con valor distinto de `none`**;
   `@keyframes` por sí solo **no** obliga a nada, porque definir fotogramas no anima mientras ninguna
   declaración los use. El límite se declara en lugar de esconderse: una guarda textual garantiza que el
   **patrón** está, no que el CSS apague el movimiento en todos los navegadores.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `error-page`: **modificado** «El tema del HTML es configurable» —la cláusula del conjunto de temas, que
  hoy afirma que la hoja de `default` va «con sus reglas intactas», y el contrato de movimiento, que pasa
  de «declara `prefers-reduced-motion`» a «neutraliza lo que anima dentro del bloque de movimiento
  reducido», con los escenarios que lo fijan—.

## Non-goals

- **No se añade ni se retira ningún tema.** Siguen siendo **seis** nombres: `default`, `minimal`,
  `terminal`, `paper`, `amber` y `compact`.
- **No se toca** el registro `THEMES` en lo que no sean los bytes de las hojas, ni `THEME`, ni `X-Theme`,
  ni la declaración en `Vary`, ni `THEME_NAMES`, ni la lista de nombres del error de arranque.
- **No se toca la cascada del código**, ni el escáner del marcador de la hoja, ni el respaldo de texto,
  ni la resolución de la hoja del operador.
- **No se añade ningún marcador de plantilla.** Siguen siendo `{{code}}`, `{{reason}}` y
  `{{description}}`; el nombre del servicio en el `<footer>` es texto del fichero, no un valor del
  servidor.
- **No se valida la hoja del operador.** La guarda sigue cubriendo las hojas **incorporadas** al binario.
- **No se promete medir el aspecto.** Ninguna guarda comprueba contraste, escala ni espaciado: el repaso
  estético se revisa a ojo y con una medición de contraste escrita, y eso es un límite declarado, no una
  puerta del CI.
- **No se toca el nombre del fichero de la plantilla** (`404.html`), ni la paridad byte a byte entre la
  copia embebida y la de `content/`.
- **No se mueven los otros cinco temas.** El movimiento es de `default` y de nadie más.

## Impact

- `content/404.html` — el marcado nuevo: `<main>` alrededor del contenido y el `<footer>`.
- `content/css/error.css` — las reglas del repaso, el movimiento y la neutralización; y su **cabecera**,
  que es la única de las seis que **no** menciona el contrato de movimiento.
- `content/themes/{minimal,terminal,paper,amber,compact}.css` — las reglas del repaso y el `<footer>`.
  Cada una conserva su identidad y su cabecera.
- `src/theme.rs` — la guarda de movimiento endurecida (`Incumplimiento`, `incumplimientos`) con sus casos
  de fixture, y la guarda `cada_hoja_cubre_el_marcado`, que gana el `footer`. Sin dependencias nuevas.
- `tests/content_pages.rs` — lo que el marcado nuevo exija fijar en las pruebas de contenido.
- `openspec/specs/error-page/spec.md` — vía la delta de este change.
- `README.md`, `AGENTS.md` — la tabla de temas (la fila de `default` dice hoy «con sus reglas intactas») y
  el contrato de movimiento.
- `src/render.rs`, `src/embedded.rs`, `src/resolve.rs`, `src/config.rs`, `src/main.rs`, `Cargo.toml`,
  `Dockerfile` — **sin cambios**.
