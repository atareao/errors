# Proposal: theme-sheet-contracts

## Why

La guarda que se publicó con `self-contained-themes` se llama `ninguna_hoja_referencia_recursos_externos`
y prohíbe seis tokens: `@import`, `url(`, `@font-face`, `animation`, `transition` y `keyframes`. Su
comentario los justifica **todos** con un solo motivo: «una página que necesita la red para verse es el
mismo defecto que este change corrige».

**Tres de esos seis no tocan la red jamás.** `animation`, `transition` y `@keyframes` son cómputo local:
el navegador los ejecuta sin pedirle nada a nadie. Y los otros tres también son más gruesos de lo que
dicen: `url(#filtro)` es una referencia **al propio documento** —un filtro o degradado SVG—, y un
`@font-face` con `src: local("Arial")` usa una fuente **ya instalada**.

El `SHALL` del spec es más estrecho que la prueba. Literal, en el requisito «El HTML servido es
autocontenido»:

> Ninguna hoja de tema SHALL referenciar recursos externos —ni `@import`, ni `url(...)` **remotos**, ni
> fuentes descargadas—: una página que necesita la red para verse es el mismo defecto que este requisito
> corrige.

El origen de la mezcla está a la vista en `design.md` D8 de `self-contained-themes`, donde la
autocontención y la sobriedad se enumeraron **juntas** («sin animaciones, sin imágenes, sin recursos
externos, sin fuentes descargadas»), como si fueran la misma clase de regla. Una es un **contrato** —no
puedes hacer que el navegador pida algo por la red— y la otra una **decisión de diseño** —las seis hojas
que el proyecto entrega son discretas a propósito—. La prueba se quedó con las dos y le puso el nombre de
una.

**Consecuencia:** quien lea esa prueba mañana creerá que una animación rompe la autocontención, y no es
verdad; y un tema que quiera animar respetando `prefers-reduced-motion` —que es lo correcto— no puede
entregarse con el proyecto.

Dato que acota el alcance: la guarda recorre `THEMES` y comprueba `theme.embedded`, los bytes compilados
en el binario. Una hoja propia del operador en `CONTENT_DIR/themes/<nombre>.css` **no la mira nadie**:
`Theme::sheet` lee los bytes y los incrusta sin inspeccionarlos. La guarda es, y debe seguir siendo,
sobre lo que el proyecto **entrega**.

## What Changes

1. **La guarda se parte en dos contratos con nombres que dicen la verdad:**
   - `ninguna_hoja_puede_hacer_una_peticion`: `@import` siempre, y `url(...)` salvo cuando apunta al
     propio documento (`url(#…)`).
   - `una_hoja_que_anima_respeta_reduced_motion`: si la hoja trae `animation`, `transition` o
     `@keyframes`, tiene que traer `prefers-reduced-motion`.
2. **`@font-face` deja de estar prohibido**, con el motivo escrito: solo puede pedir algo por la red a
   través de `url(...)` o `@import`, que ya están prohibidos, y `src: local(...)` usa una fuente que el
   usuario ya tiene instalada.
3. **El spec gana el contrato que faltaba:** una hoja PUEDE animar y, si anima, SHALL respetar
   `prefers-reduced-motion`. Y el contrato de red se precisa para que diga exactamente lo que se mide.
4. **La comprobación pasa a ser una función pura sobre bytes**, para que cada cláusula del contrato se
   pueda probar con un caso de fixture y no solo su conjunto.
5. **Ninguna hoja cambia de reglas.** Ni una regla de CSS: solo se corrige el comentario de cabecera de
   los **cinco** `content/themes/*.css`, que afirmaban por escrito que una hoja «no puede referenciar
   recursos externos **ni animarse**». `content/css/error.css` **no se toca**: su cabecera nunca llevó
   esa frase y ya decía la verdad (ver Non-goals).
6. **La guarda deja de leer los comentarios.** Un `/* … */` no puede pedir nada ni animar, así que
   el escaneo lo ignora. Si no, la propia documentación de una hoja —que dice qué no puede hacer—
   se convertiría en un incumplimiento y la guarda volvería a medir algo distinto de su contrato.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `error-page`: **modificado** «El HTML servido es autocontenido» —el contrato de red, preciso y con dos
  escenarios nuevos— y **modificado** «El tema del HTML es configurable» —el contrato de movimiento, con
  un escenario nuevo—.

## Non-goals

- **No se añade ningún tema.** El número sigue siendo seis, y ni el spec ni los escenarios de `THEME` que
  lo dicen se tocan. Un tema que anime es posible **a partir de ahora**, y es una fase declarada
  **posterior**.
- **Las seis hojas no cambian de reglas.** Ni una regla de `content/css/error.css` ni de
  `content/themes/*.css` se toca: la fase estética es otra y tiene que partir de un suelo firme. Sí se
  corrige el comentario de cabecera de los **cinco** `content/themes/*.css`, que afirmaban por escrito
  que una hoja «no puede referenciar recursos externos **ni animarse**» —falso desde esta delta, porque
  una hoja sí puede animar si declara `prefers-reduced-motion`—. `content/css/error.css` **no se toca**,
  porque **nunca llevó esa frase**: su cabecera ya decía la verdad —prohíbe los recursos externos y no se
  pronuncia sobre el movimiento—. Es prosa dentro del entregable, no CSS, y dejar esas cinco cabeceras
  sería dejar cinco documentos que contradicen el spec.
- **No se valida la hoja del operador.** El servicio no parsea CSS al servir: la hoja es adorno y ya está
  decidido que un CSS roto no puede tumbar una página. Leer `CONTENT_DIR` al arrancar para validar hojas
  haría que el arranque dependiera del sistema de ficheros.
- **No se toca** el registro de temas, `THEME`, `X-Theme`, `Vary`, el escáner del marcador ni el
  `Dockerfile`.

## Impact

- `src/theme.rs` — `Incumplimiento`, `incumplimientos(hoja: &[u8])`, las dos guardas y sus pruebas con
  fixtures; y el bloque de documentación del módulo, que hoy presenta la sobriedad y la autocontención
  como una sola regla.
- `openspec/specs/error-page/spec.md` — vía la delta de este change.
- `AGENTS.md`, `README.md` — la prosa de los dos contratos y el alcance de la guarda.
- `content/themes/*.css` — **solo su comentario de cabecera** (los **cinco** ficheros de
  `content/themes/`), para que su prosa diga lo que la delta dice: no puede pedir nada por la red, y si
  animase tendría que declarar `prefers-reduced-motion`. Ninguna regla cambia. `content/css/error.css`
  queda **sin cambios**: su cabecera ya decía la verdad y nunca llevó la frase falsa.
- `src/render.rs`, `src/main.rs`, `src/config.rs`, `Cargo.toml`, `Dockerfile` — **sin cambios**.
