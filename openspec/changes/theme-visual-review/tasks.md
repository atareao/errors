# Tasks: theme-visual-review

> Repaso estético de las **seis** hojas —paleta, contraste, escala tipográfica, jerarquía y espaciado— sin
> tocar el conjunto de temas, un movimiento **muy sobrio** en `default` con su neutralización, dos elementos
> de marcado (`<main>` y `<footer>`) y, sobre todo, el **endurecimiento de la guarda de movimiento**: de
> «nombra `prefers-reduced-motion`» a «neutraliza cada animación declarada dentro del bloque reduce».

## 0. Prerrequisito

- [ ] 0.1 Dejar constancia del agujero **medido**: con el `incumplimientos()` de hoy, una hoja que anima y
      trae `@media (prefers-reduced-motion: reduce) { }` —vacío— o con un comentario dentro **no** incumple;
      y un `@keyframes` sin ninguna declaración que lo use **sí** pide la media query (tabla de `design.md`).
- [ ] 0.2 Dejar constancia del suelo: 189 pruebas unitarias + 4 de contenido + 5 de Traefik, todas en verde.
- [ ] 0.3 Dejar constancia de que `THEMES` sigue declarando **seis** nombres y de que el conjunto no se
      toca: ni `THEME_NAMES` ni la lista que enumera el error de arranque de `THEME`.
- [ ] 0.4 Dejar constancia de la asimetría de las cabeceras: las **cinco** hojas de `content/themes/*.css`
      mencionan `prefers-reduced-motion` en su línea 8, y `content/css/error.css` **no**.

## 1. RED

- [ ] 1.1 Casos de fixture que **fallan por aserción** contra el `incumplimientos()` de hoy. Ninguno necesita
      la función nueva, así que la fase RED no falla por compilación:
      - un bloque `reduce` **vacío** tiene que incumplir, y hoy pasa;
      - un bloque `reduce` con **solo un comentario** dentro tiene que incumplir, y hoy pasa;
      - un bloque de **otra** preferencia (`prefers-reduced-motion: no-preference`) que neutraliza tiene que
        incumplir, y hoy pasa;
      - una hoja que declara `transition` y cuyo bloque solo trae `animation: none` tiene que incumplir, y
        hoy pasa;
      - una hoja que **anima dentro** del propio bloque `reduce` tiene que incumplir, y hoy pasa.
- [ ] 1.2 El caso que hoy incumple y tiene que **dejar** de incumplir: una hoja cuyo único contenido son
      `@keyframes`, sin ninguna declaración de `animation` ni de `transition`.
- [ ] 1.3 Casos que **protegen la implementación** de la fase GREEN y por eso ya están en verde —se declaran
      como tales, no se cuentan como RED—: el bloque cuya neutralización vive en una **regla anidada**, y el
      bloque cuyo cierre es la llave **equilibrada** y no la primera que aparece.
- [ ] 1.4 Confirmar que las 198 pruebas previas siguen en verde, que las que fallan son **solo** las nuevas
      que se espera que fallen, y que fallan **por aserción**.

## 2. GREEN

- [ ] 2.1 `incumplimientos()`: la detección de lo que anima pasa a ser una **declaración con valor distinto
      de `none`** —`animation`, `animation-name`, `transition`, `transition-property`— y `@keyframes` deja de
      ser un disparador por sí solo.
- [ ] 2.2 Localizar el bloque `@media (prefers-reduced-motion: reduce)` por su llave de apertura y **su**
      cierre **equilibrado**, a mano sobre los bytes ya limpios de comentarios. Sin `regex`, sin parser de
      CSS y sin dependencias nuevas: el árbol compila sin red.
- [ ] 2.3 Exigir que el bloque **exista**, **no esté vacío** y neutralice **cada** animación declarada, con
      la lista **cerrada**: `animation: none` o `animation-name: none` para una animación, y
      `transition: none` o `transition-property: none` para una transición.
- [ ] 2.4 Repasar las **seis** hojas —paleta, contraste, escala, jerarquía y espaciado— conservando la
      identidad que declara su cabecera, y **sin tocar el conjunto de nombres**.
- [ ] 2.5 El movimiento de `default`: `@keyframes` propio y la `animation` sobre el bloque de contenido, del
      orden de 150–200 ms, con el bloque `@media (prefers-reduced-motion: reduce)` que la neutraliza. Los
      otros cinco temas **no** se mueven.
- [ ] 2.6 El marcado de `content/404.html`: `<main>` alrededor del contenido y `<footer>` con el nombre del
      servicio, **sin `href`**, conservando `h1`, `h2`, `hr`, `p`, los tres marcadores de plantilla, el
      marcador de la hoja y la clase `c{{code}}` del `body`.
- [ ] 2.7 Las **seis** hojas estilan el `<footer>`, y la guarda `cada_hoja_cubre_el_marcado` gana `footer`
      como elemento exigido. `main` **no** se le añade: es estructura, y no todos los temas lo necesitan.
- [ ] 2.8 Poner la frase del contrato de movimiento —en los términos nuevos: **neutralizar**, no
      «declarar»— en la cabecera de `content/css/error.css`, que es la única de las seis que no la tiene.
- [ ] 2.9 Todas las pruebas en verde, y `content/` sigue entregando **un solo** `*.html`: la página conserva
      su nombre y no se añade ningún fichero de página.

## 3. Verificación

- [ ] 3.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --offline` y
      `cargo deny check -D warnings`.
- [ ] 3.2 La paridad byte a byte de **cada** hoja con su fichero de `content/` y la de la **página** con
      `content/404.html` siguen en verde sin haberlas tocado, y `cargo package --list --offline --allow-dirty`
      sigue listando la página y las seis hojas.
- [ ] 3.3 Medir el contraste de cada par texto/fondo de las seis hojas y **dejar los números escritos**
      (objetivo AA). Límite declarado en `design.md` D10: **ninguna guarda** mide el aspecto.
- [ ] 3.4 Servir las seis hojas y mirar las seis páginas —claro y oscuro, y `default` con
      `prefers-reduced-motion: reduce`—, porque un número no distingue una jerarquía legible de una que no lo es.

## 4. Documentación

- [ ] 4.1 `README.md`: la fila de `default` en la tabla de temas —que hoy dice «con sus reglas intactas»— y
      el contrato de movimiento, en los términos nuevos.
- [ ] 4.2 `AGENTS.md`: el contrato de movimiento endurecido —qué exige, y qué **no** garantiza una guarda
      textual—, el marcado nuevo de la página y el movimiento de `default`.
- [ ] 4.3 Sin prometer que una guarda mide el aspecto: el contraste se mide y se deja escrito.

## 5. Cierre

- [ ] 5.1 `openspec validate theme-visual-review --strict` limpio.
- [ ] 5.2 `openspec archive theme-visual-review`, para fusionar la delta de `error-page`.
- [ ] 5.3 Borrar `PLAN.md` si el flujo lo dejó.
      > Sin commit ni PR hasta que el usuario lo decida.
