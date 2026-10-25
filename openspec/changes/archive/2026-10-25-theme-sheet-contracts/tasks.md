# Tasks: theme-sheet-contracts

> La guarda publicada con `self-contained-themes` mezcló dos cosas —el contrato de autocontención y una
> decisión de diseño sobre la sobriedad— y le puso el nombre de una. Este change las separa, precisa el
> `SHALL`, añade el contrato que faltaba (`prefers-reduced-motion`) y **no toca ni una regla de las seis
> hojas** (solo corrige su comentario de cabecera).

## 0. Prerrequisito

- [x] 0.1 Dejar constancia de la medición: el `SHALL` del spec dice «`url(...)` **remotos**» y la prueba
      prohíbe **todo** `url(`, además de `animation`, `transition` y `keyframes`, que no piden nada a la
      red. La prueba es más estricta que el spec, y su comentario justifica los seis tokens con un motivo
      que solo vale para tres.
- [x] 0.2 Dejar constancia del origen: `design.md` D8 de `self-contained-themes` enumera autocontención y
      sobriedad **juntas**. Una es contrato; la otra, diseño.
- [x] 0.3 Dejar constancia de que la guarda mira `theme.embedded` —lo que el binario entrega— y **no** la
      hoja del operador en `CONTENT_DIR`.
- [x] 0.4 Dejar constancia de la ampliación decidida con el usuario: la guarda limpia los comentarios
      antes de escanear —un comentario no puede pedir nada ni animar—, y la cabecera de las seis hojas
      se corrige porque afirmaba lo contrario.

## 1. RED

- [x] 1.1 **Extraer, sin cambiar comportamiento**, la lógica de la prueba a
      `incumplimientos(hoja: &[u8]) -> Vec<Incumplimiento>`, con los seis tokens de hoy y su mismo
      sentido. Es una costura para poder probar, y se declara como tal: la prueba de que no cambia nada es
      que la guarda sobre `THEMES` sigue en verde con las seis hojas todavía sin tocar en ese paso.
- [x] 1.2 Pruebas de fixture que **fallan por aserción** contra esa lógica extraída (la de hoy):
      `url(#gradiente)` cumple, `@font-face` con `src: local("Arial")` cumple, y `animation` **con**
      `prefers-reduced-motion` cumple.
- [x] 1.3 Guardas de no-regresión, que hoy **ya** incumplen y tienen que seguir incumpliendo:
      `url(foto.png)`, `url(https://ejemplo/x.png)` y `@import "otra.css"`.
- [x] 1.4 Una hoja quieta y sin `url(` cumple los dos contratos.
- [x] 1.5 Confirmar que las 176 pruebas previas siguen en verde y que las que fallan son **solo** las
      nuevas que se espera que fallen.
- [x] 1.6 Pruebas de fixture que **fallan por aserción** contra la lógica extraída: una hoja cuyo
      único `url(`/`@import` está en un comentario cumple el contrato de red, y una hoja que menciona
      `animation` solo en un comentario cumple el de movimiento. Y las guardas de no-regresión: una
      hoja que menciona `url(` en un comentario **y** trae un `url(foto.png)` de verdad incumple.

## 2. GREEN

- [x] 2.1 `Incumplimiento` e `incumplimientos(hoja: &[u8])`: `@import` siempre; `url(...)` salvo
      `url(#…)`; y, si hay `animation`/`transition`/`@keyframes`, exigir `prefers-reduced-motion`. Escaneo
      de bytes, sin dependencias nuevas y sin parser de CSS.
- [x] 2.2 Las dos guardas sobre `THEMES`, con los nombres honestos
      (`ninguna_hoja_puede_hacer_una_peticion`, `una_hoja_que_anima_respeta_reduced_motion`), cada una con
      **su** motivo escrito, acumulando los fallos nombrando el tema.
- [x] 2.3 El bloque de documentación de `src/theme.rs`, que hoy dice «deliberadamente sobrios —sin
      animaciones, sin imágenes, sin recursos externos—» como si fuera una regla: distinguir **decisión de
      diseño** (las seis hojas son discretas a propósito) de **contrato** (los dos que miden las guardas).
- [x] 2.4 Todas las pruebas en verde, y `git diff content/` con **solo comentarios** de cabecera: ni
      una regla de CSS.
- [x] 2.5 Limpiar los comentarios `/* … */` antes de escanear, con un limpiador textual sobre bytes
      (mismo criterio que el `sin_comentarios` de las pruebas; sin `regex`, sin parser). El limpiador
      respeta las **cadenas** —un `/*` dentro de un `"…"` o un `'…'` es un literal, no un comentario,
      así que no abre nada—, y lo único que se lleva por delante es un `/*` que **no** cierra, que se
      queda con el resto de la hoja: eso SHALL quedar escrito en la documentación.
- [x] 2.6 Corregir el comentario de cabecera de las seis hojas para que no afirme que una hoja no
      puede animarse: no puede pedir nada por la red, y si animase tendría que declarar
      `prefers-reduced-motion`. Sin tocar ni una regla.

## 3. Verificación

- [x] 3.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --offline` y
      `cargo deny check -D warnings`.
- [x] 3.2 La prueba de paridad byte a byte de **cada** hoja con su fichero de `content/` sigue en verde
      sin haberla tocado, y `cargo package --list --offline --allow-dirty` sigue listando las seis.
- [x] 3.3 Un incumplimiento **se nombra**: el mensaje dice qué contrato se rompió y en qué hoja.

## 4. Documentación

- [x] 4.1 `AGENTS.md`: los dos contratos —qué mide cada guarda y por qué—, por qué `@font-face` y
      `url(#…)` **no** están prohibidos, que un tema **puede** animar si respeta `prefers-reduced-motion`,
      y que la guarda cubre las hojas **incorporadas**, no la del operador.
- [x] 4.2 `README.md`: en la sección de temas, que una hoja propia puede animar y cómo, y que la guarda
      del proyecto no mira la hoja del operador.
- [x] 4.3 Sin tocar las **reglas** de `content/themes/*.css` ni de `content/css/error.css`: solo su
      comentario de cabecera.

## 5. Cierre

- [x] 5.1 `openspec validate theme-sheet-contracts --strict` limpio.
- [x] 5.2 `openspec archive theme-sheet-contracts`, para fusionar la delta de `error-page`.
- [x] 5.3 Borrar `PLAN.md`.
      > Sin commit ni PR: el trabajo se queda en el árbol hasta que el usuario lo decida.
