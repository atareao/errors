# Design: theme-sheet-contracts

## El hecho que abre el change: una prueba que dice más de lo que puede justificar

La guarda de `self-contained-themes` prohíbe seis tokens y los justifica con un solo motivo —que la
página no necesite la red—. Tres de ellos no tocan la red jamás. No es un matiz: es una prueba cuyo
nombre y cuyo comentario afirman algo falso sobre tres de sus seis reglas, y el efecto no es cosmético,
porque quien la lea mañana creerá que animar rompe la autocontención.

El `SHALL` del spec, en cambio, es más estrecho y más correcto: habla de `url(...)` **remotos** y de
fuentes descargadas. Es la prueba la que se pasó de frenada, y el origen está a la vista en `design.md`
D8: la autocontención y la sobriedad se enumeraron juntas, como si fueran lo mismo.

## D1 — Dos contratos, porque son dos cosas

| Contrato | Qué mide | Por qué |
|---|---|---|
| `ninguna_hoja_puede_hacer_una_peticion` | `@import`, y `url(...)` que no sea del propio documento | Es el defecto que `self-contained-themes` corrigió: una página que pide algo para verse se rompe en el host de la aplicación |
| `una_hoja_que_anima_respeta_reduced_motion` | si hay `animation`/`transition`/`@keyframes`, tiene que haber `prefers-reduced-motion` | Accesibilidad y legibilidad: una página de error que se mueve mientras intentas leer qué ha pasado |

El segundo **no prohíbe el adorno**: prohíbe el adorno que no se puede apagar. Es una regla mejor que la
que había, y es la que permite que exista un tema con movimiento.

## D2 — `url(#…)` es una referencia, no una petición; y `@font-face` no puede pedir nada por su cuenta

- `url(#gradiente)` apunta a un elemento **del propio documento** (un filtro o degradado SVG). No hay
  petición, así que prohibirlo era prohibir algo que no rompe nada.
- `@font-face` solo pide algo por la red a través de `url(...)` o `@import`, que ya están prohibidos, y
  `src: local("Arial")` usa una fuente **ya instalada**. Con esos dos fuera, `@font-face` no puede hacer
  ninguna petición: se cae de la lista, y el motivo se escribe para que nadie lo reponga «por si acaso».

La comprobación de `url(` mira el carácter que sigue al paréntesis —un token y su variante—, no un
contador de apariciones.

## D3 — La lógica sale de dentro de la prueba, y eso es lo que la hace probable

Sale de **dentro de la función de prueba**, no de la compilación de test: la función sigue bajo
`#[cfg(test)]`, a nivel de módulo, porque el crate es solo binario y el servicio no valida CSS.

Hoy la comprobación vive **dentro** de la prueba, así que solo se puede verificar *su conjunto* sobre las
seis hojas del binario: no hay forma de preguntar «¿y una hoja con `url(#f)`?». Sale a una función pura
sobre bytes que devuelve los incumplimientos, **cada uno con el token que los rompe**, y con eso cada
cláusula se prueba con un caso en línea.

## D4 — La fase RED necesita una costura, y se declara

Las pruebas de fixture no compilan si la función no existe, y la regla del repositorio es que la fase RED
falle **por aserción, no por compilación**. Así que la fase RED empieza **extrayendo la lógica actual
verbatim**, sin cambiar comportamiento, y solo después se añaden las pruebas que fallan: las que exigen
`url(#…)`, `@font-face` con `local(...)` y `animation` junto a `prefers-reduced-motion`.

La prueba de que la extracción no cambia nada no es una promesa: la guarda sobre `THEMES` sigue en verde
con las seis hojas, byte a byte las mismas **que antes de la extracción**.

## D5 — La guarda cubre lo que el proyecto entrega, y eso se declara

La guarda mira `theme.embedded`, los bytes del binario, y **no** mira
`CONTENT_DIR/themes/<nombre>.css`: la hoja del operador es del operador, y el servicio no debe parsear CSS
al servir. Validarla en el arranque haría que el arranque dependiera del sistema de ficheros, que es justo
lo que el proyecto evita.

No es un hueco: es el alcance correcto, y es lo que permite que un operador tenga hoy mismo una hoja con
movimiento —y que el proyecto pueda entregar la suya en la fase siguiente—.

## D6 — Los comentarios no son el contrato, así que el escaneo los ignora

Hoy la guarda mira los **bytes crudos** de la hoja, y ahí están también las cabeceras `/* … */`. Un
comentario puede decir «esta hoja no usa `animation`» —y esa frase es precisamente la documentación que
el repositorio quiere—, pero el escáner la leería como una regla y la guarda diría, con el tema en la
mano, que la hoja anima cuando no anima. Es el mismo defecto que este change corrige, en pequeño: una
guarda que mide algo distinto de lo que anuncia.

Se limpian los comentarios antes de buscar los tokens, con un limpiador **textual sobre bytes**
—`hoja_sin_comentarios`, el mismo criterio que el `sin_comentarios` que ya usan las pruebas del
módulo—, sin `regex` y sin parser.
El limpiador **sí conoce las cadenas**: copia tal cual los literales `"…"` y `'…'` —saltando sus
escapes—, así que un `/*` dentro de una cadena **no abre un comentario**. Es CSS válido
(`content: "/*";`, o la URL `url("a/*b.png")`) y no puede tapar una petición que venga detrás, que es
justo lo que el escaneo tiene que ver. El único resto que se lleva por delante es **un `/*` que no
cierra**, del que nadie puede recuperar lo que hay dentro.

El mismo criterio vale para los dos contratos: un `url(` en un comentario no es una petición y un
`animation` en un comentario no anima.

## D7 — La cabecera de las cinco hojas de tema dice la verdad

Las cabeceras de los cinco `content/themes/*.css` afirmaban por escrito que la hoja «no puede
referenciar recursos externos ni animarse». La primera mitad ya era cierta; la segunda deja de serlo
con esta delta, porque una hoja **sí** puede animar si declara `prefers-reduced-motion`. La de
`content/css/error.css` queda **intacta** porque ya decía la verdad —habla de recursos externos y no
prohíbe el movimiento—, no porque se decidiera dejarla.

No se toca ni una regla de esas hojas —eso es la fase estética—, pero sí esa frase: dejar en el
entregable un comentario que contradice el spec es la clase de mentira que este change existe para
quitar. La prueba de paridad byte a byte entre la hoja embebida y la de `content/` sigue en pie:
`include_bytes!` resuelve al compilar, así que las dos copias siguen siendo el mismo fichero.

## Open Questions

1. **El tema que anime** (fase siguiente): cuál, si uno nuevo o un repaso de los seis, y si el movimiento
   es decorativo o informativo. No entra aquí.
2. **`prefers-reduced-motion` en las hojas actuales**: ninguna anima, así que cumplen por vacío. Si algún
   día una incorpora una transición de `:hover`, tendrá que traer la media query.
3. **Avisar del incumplimiento en la hoja del operador** (sin bloquear): se descarta. Implica parsear CSS
   en el servicio y un aviso por hoja y petición, para una decisión que es del operador.
