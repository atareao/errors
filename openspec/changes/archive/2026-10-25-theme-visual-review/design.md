# Design: theme-visual-review

## Los dos hechos que abren el change

**1. Ninguna de las seis hojas ha cambiado una sola regla desde que se entregó.** `self-contained-themes`
—el change que trajo los cinco temas— **creó** las cinco hojas y dejó las reglas de `default` intactas
—tocó de ella solo el comentario de cabecera—; `theme-sheet-contracts` corrigió la **prosa** de las cinco y
no tocó `content/css/error.css`; y `error-page-template` cambió **solo prosa** en las seis. Así que paleta,
contraste, escala, jerarquía y espaciado no se han revisado nunca, y `default` —el tema que ve todo el
mundo, y la hoja del asset público `/css/error.css`— no tiene ninguna firma propia.

**2. La guarda de movimiento mide menos de lo que anuncia.** Hoy
`una_hoja_que_anima_respeta_reduced_motion` se conforma con que la hoja **nombre**
`prefers-reduced-motion` en cualquier parte: `puede_apagarse` es, literalmente,
`contiene(hoja, "prefers-reduced-motion")` (`src/theme.rs`). **Medido** contra ese código, con el mismo
`incumplimientos()` que corre en `cargo test`, en una copia del árbol fuera del repositorio:

| Hoja | `incumplimientos()` de hoy |
|---|---|
| `body { animation: entrada 180ms ease-out; }` + `@media (prefers-reduced-motion: reduce) { }` | **vacío** — pasa |
| `body { animation: entrada 180ms ease-out; }` + `@media (prefers-reduced-motion: reduce) { /* apagado */ }` | **vacío** — pasa |
| `@keyframes entrada { … }` solo, sin ninguna declaración | `Movimiento("keyframes")` — incumple |

Las dos primeras filas son el agujero: un bloque **vacío**, o con un comentario dentro, satisface la guarda
y el movimiento no se puede apagar. La tercera es la otra cara: la guarda exige la media query a una hoja
que **no puede animar**, porque definir fotogramas no anima a nadie. Las dos son el mismo defecto que
`theme-sheet-contracts` corrigió en el contrato de red —una prueba que mide algo distinto de su contrato—
y las dos se arreglan aquí.

## D1 — Anima `default`, y solo `default`

Es el tema por defecto: el que ve quien no configura nada, y cuya hoja es **además** el asset público
`/css/error.css`. Un movimiento de entrada **muy sobrio** es la firma de la casa, y el sitio donde una
firma se ve. Los otros cinco **no** se mueven: cada tema tiene su identidad —fósforo, papel, ámbar,
desnudez, densidad— y el movimiento no es parte de ninguna de ellas. Extenderlo a los cinco convertiría
una decisión de aspecto en ruido, y multiplicaría por seis las hojas que tienen que neutralizarlo bien.

Medido, **ninguna de las seis anima hoy** —`grep -cE 'animation|transition'` sobre las seis hojas y sobre
`content/css/error.css` da `0` en todas—, así que el movimiento es una adición de `default` y de nadie más,
y la guarda endurecida entra en un árbol donde todavía no tiene nada que exigir.

## D2 — El movimiento: 180 ms sobre el bloque de contenido, y neutralizable

- **Sobre `<main>`**, no sobre `body`: el bloque de contenido es lo que entra; el fondo no se anima. En
  marcos donde el navegador pinta la página a plazos, animar `body` arrastra el lienzo entero.
- **Del orden de 150–200 ms** —se elige **180 ms**—: por debajo se percibe como un parpadeo de carga, y por
  encima una página de **error** empieza a hacerse esperar, que es justo lo que no debe.
- **Propiedad `animation`, con fotogramas propios** (`@keyframes` en la misma hoja) y no una `transition`:
  una transición necesita un cambio de estado que dispare, y aquí no hay interacción que lo produzca.
- **Neutralización**: `@media (prefers-reduced-motion: reduce) { main { animation: none; } }`. Es la forma
  canónica de la lista cerrada de D8, y deja el bloque **no vacío** y con la neutralización que le
  corresponde a la animación declarada —exactamente lo que la guarda endurecida exige—.

## D3 — El marcado: `<main>` alrededor del contenido y un `<footer>` con el nombre del servicio

Se añaden **dos** cosas, y se decide así:

- **`<main>`**: envuelve `h1`, `h2`, `hr`, `p`. Es el *landmark* del contenido —el elemento que un lector
  de pantalla anuncia como «contenido principal»—, y es el bloque que D2 anima. Con el `body` en columna
  flex, sigue siendo un solo hijo y el centrado no cambia.
- **`<footer>`**: con el nombre del servicio, **texto y ningún `href`**. Una firma que dice quién sirve la
  página —que es honesto: la página se sirve en el host de la aplicación—, y al no llevar `href` no entra
  en la lista de referencias locales que el directorio tiene que resolver.
- **No se añade un marcador de plantilla** para el nombre del servicio: los marcadores siguen siendo tres
  (`{{code}}`, `{{reason}}`, `{{description}}`), y el nombre es texto del fichero. Añadir un cuarto
  marcador cambiaría el contrato de sustitución, que vive en **otro** requisito y no se toca aquí.

### Lo que el marcado nuevo **no** rompe —y por eso no hay más delta

| Lo que hay hoy | Por qué sigue en pie |
|---|---|
| Los tres marcadores `{{code}}` / `{{reason}}` / `{{description}}` | Se conservan tal cual; el `<main>` y el `<footer>` se añaden **alrededor**, no en su lugar |
| El marcador `<link rel="stylesheet" href="/css/error.css" />` | Sigue en el `<head>`, intacto: es el único dato que el render necesita para saber **dónde** incrustar la hoja |
| La clase `c<código>` en el `body` | El `body` no se toca: la clase y el estado siguen sin poder discrepar |
| El nombre del fichero, `404.html` | No se renombra: un fichero con nombre de código lo intercepta la cascada del código antes de servirse, y uno neutro se serviría como asset con los marcadores a la vista |
| La paridad byte a byte de `src/embedded.rs` | `include_str!` resuelve al compilar: el fichero y la copia embebida siguen siendo el mismo contenido |
| `content/` entrega **un solo** `*.html` | Se sigue entregando **un** fichero, el mismo |

Contrastado con los tres requisitos vivos que el marcado podría haber tocado:

- «**Las páginas no referencian ficheros que no existen**»: la única referencia sigue siendo
  `css/error.css`, y el `<footer>` no aporta ningún `href`. Sin cambio.
- «**Cada página se identifica con la clase de su código**»: el `body` y su clase no se tocan. Sin cambio.
- «**El HTML servido es autocontenido**»: el `<main>` y el `<footer>` no son marcadores ni referencias.
  Sin cambio.

Ninguno de los tres cambia de comportamiento, así que **no llevan delta**: un cambio puramente estético y
estructural que no altera ningún contrato no se escribe en un spec. Lo único que crece con el marcado es la
guarda `cada_hoja_cubre_el_marcado` de `src/theme.rs` —una prueba, no un requisito—, que gana `footer` como
elemento que **cada** hoja tiene que estilar; `main` **no** se le añade a propósito, porque es estructura
—el bloque que `default` anima— y no todos los temas necesitan una regla para él.

## D4 — Los seis nombres no cambian, y la cláusula de las «reglas intactas» deja de ser cierta

La cláusula viva dice que `default` es «la hoja que el proyecto ya trae, **con sus reglas intactas**, de
modo que **quien no configure nada no vea ningún cambio de aspecto**». Este change hace que esa frase sea
**falsa dos veces**: le cambiamos el aspecto a `default` y encima le añadimos movimiento. Dejarla escrita
sería dejar un spec que prohíbe lo que el repositorio acaba de hacer.

La delta la sustituye por lo que **sí** es cierto y **comprobable**:

- `default` sigue siendo el tema por defecto;
- su hoja sigue en el path `css/error.css` —y por tanto sigue siendo el asset público `/css/error.css`, la
  misma hoja que el servicio publica y la que incrusta la página, sin posibilidad de divergir—;
- sus **reglas pueden cambiar** de una versión a otra, porque son del proyecto.

Y el **conjunto de nombres** sigue fijado en **seis**: no se añade ni se retira ninguno, y ni el registro
`THEMES`, ni `THEME_NAMES`, ni la lista que enumera el error de arranque de `THEME` cambian. **Por qué**: el
conjunto de nombres es un **contrato con el operador** —está escrito en su `THEME`, y su valor equivocado
aborta el arranque—, y el aspecto de una hoja no lo es. Un séptimo tema haría que un `THEME` válido hoy
dejase de serlo, o al revés; y quitar uno rompería a quien lo tenga configurado.

## D5 — La guarda se endurece: de «nombrarlo» a «neutralizarlo»

El contrato pasa de *declarar* `prefers-reduced-motion` a *neutralizar lo que anima dentro del bloque de
movimiento reducido*. Las tres cosas que la guarda exige ahora, y su porqué:

| Exige | Por qué |
|---|---|
| El bloque `@media (prefers-reduced-motion: reduce)` **existe** | Sin media query no hay nada que respete la preferencia: es la única puerta que el usuario puede cerrar |
| El bloque **no está vacío** | Es el agujero medido: un bloque vacío —o con un comentario dentro— satisface «nombrarlo» y no apaga nada |
| **Cada** animación declarada queda neutralizada | Un bloque que apaga la animación `A` y deja viva la transición `B` no cumple el contrato, aunque lo *parezca* por tener el bloque |

El bloque SHALL ser, además, el de **esa** preferencia: `@media (prefers-reduced-motion: reduce)`.
Neutralizar bajo `prefers-reduced-motion: no-preference` no cuenta, porque no es la puerta que el usuario
cierra: quien pide movimiento reducido no mira el bloque que solo existe cuando **no** lo pide.

## D6 — `@keyframes` por sí solo no obliga a nada

La precisión que entra en la delta: **lo que anima es una declaración con valor distinto de `none`**;
`@keyframes` **define fotogramas** y no anima mientras ninguna declaración los use. Hoy la guarda trata
`keyframes` como uno de los tres disparadores y pide la media query a una hoja que solo define fotogramas
—medido, la tercera fila de la tabla de arriba—: es el mismo defecto, en pequeño, que
`theme-sheet-contracts` corrigió cuando la guarda se llamaba `ninguna_hoja_referencia_recursos_externos` y
prohibía `animation` por no tocar la red.

**Qué se conserva del comportamiento actual**, y se dice para que nadie lo lea como una retirada:

- El **disparador** sigue siendo el mismo conjunto de conceptos —lo que anima—, con la propiedad nombrada:
  `animation`, `animation-name`, `transition`, `transition-property` con valor distinto de `none`.
- Un `@keyframes` **usado** por una declaración sigue obligando igual que hoy, porque lo que dispara es la
  declaración, no los fotogramas.
- La guarda sigue exigiéndose sobre las **hojas incorporadas** y sigue ignorando la **hoja del operador**
  (D10), y sigue limpiando los **comentarios** antes de escanear —un comentario no anima—.
- Lo único que **deja de incumplir** es el caso degenerado de hoy: una hoja que define fotogramas y **no
  los usa**. No puede animar nada, así que no hay nada que apagar; exigirle la media query era ruido, no
  contrato.

## D7 — Los límites de la guarda, declarados en lugar de escondidos

La guarda es **textual y sobre bytes**, y no puede ser otra cosa: el árbol **compila sin red** y no se
añaden dependencias, así que no entra ni `regex` ni un parser de CSS. Concretamente:

- **Escaneo por bytes**, nunca `str`: una hoja del proyecto está en ASCII, pero el limpiador de comentarios
  y el escaneo trabajan sobre `&[u8]` y no decodifican.
- **Limpieza de comentarios antes de escanear** (`hoja_sin_comentarios`, ya existe): un comentario no anima
  ni neutraliza. El limpiador **respeta las cadenas** —un `/*` dentro de un `"…"` o un `'…'` es un literal,
  no un comentario—, y un `/*` que **no** cierra se lleva el resto de la hoja. Consecuencia declarada: un
  bloque de movimiento reducido cuyo único contenido sea prosa se lee **vacío**, que es justo lo que la
  delta quiere.
- **Detección literal y sensible a mayúsculas**, como la de los otros dos contratos: las hojas son del
  proyecto y van en minúsculas. `ANIMATION: NONE` no cuenta.
- **Delimitación del bloque a mano**: se localiza la llave de apertura y se busca **su** cierre
  **equilibrado** contando llaves, sin parser. Es lo que hace que
  `@media (prefers-reduced-motion: reduce) { body { animation: none; } }` se lea entero y no se corte en
  la primera llave del `body`. Un bloque sin cerrar se lleva el resto de la hoja.
- **Qué es una declaración**: una aparición del nombre de propiedad con frontera de identificador por los
  dos lados, seguida de `:` y de un valor cuyo primer token se lee hasta el espacio, el `;` o el `}`. Es una
  aproximación textual: no entiende de `!important`, de herencia ni de `@supports`, y no distingue
  `animation` dentro de un `@keyframes` —donde no hay declaración de propiedad— de una declaración real.

Y la limitación que **no** se puede salvar con más código, y que la delta escribe en el propio spec: una
guarda textual **garantiza que el patrón está**, no que el CSS apague el movimiento en todos los
navegadores. Demostrar eso exigiría un motor de CSS y una matriz de navegadores, que no es lo que este
repositorio puede ni quiere tener. Lo que se puede —y se hace— es exigir la **forma canónica**, que es la
que todos los navegadores del parque entienden.

## D8 — La lista de neutralizaciones es cerrada, y esa estrechez es deliberada

Aceptadas: `animation: none` o `animation-name: none` para una animación; `transition: none` o
`transition-property: none` para una transición. Cada animación declarada tiene que quedar neutralizada por
la forma que le **corresponde**: `animation: none` no neutraliza una transición.

Fuera de la lista, **a propósito**: `animation-duration: 0s`, `animation-play-state: paused` y cualquier
otra forma que también apague en un navegador real. El motivo es el de D7: una guarda textual no puede
demostrar equivalencia entre dos formas de apagar una animación, así que aceptar una lista abierta sería
prometer una comprobación que no se hace. Exigir la canónica **rechaza de más**, y esa es la dirección
segura —la misma que el contrato de red ya eligió para un `url()` vacío o sin cerrar—.

## D9 — La cabecera de `content/css/error.css` dice lo que las otras cinco ya dicen

Medido —`grep -n 'prefers-reduced-motion' content/themes/*.css` → las **cinco** hojas de tema lo mencionan,
todas en su línea 8—:

```
content/themes/amber.css:8: * Si animase, tendría que declarar `prefers-reduced-motion`.
content/themes/compact.css:8: * Si animase, tendría que declarar `prefers-reduced-motion`.
content/themes/minimal.css:8: * Si animase, tendría que declarar `prefers-reduced-motion`.
content/themes/paper.css:8: * Si animase, tendría que declarar `prefers-reduced-motion`.
content/themes/terminal.css:8: * Si animase, tendría que declarar `prefers-reduced-motion`.
```

La hoja de `default` **no lo menciona**, y es la única de las seis que lo hace mal por omisión —y, tras D1,
la única que **anima**—. El repaso le pone la frase, y la pone **en los términos del contrato nuevo**: no
«declarar», sino **neutralizar**. No es cosmética de comentario: la cabecera de una hoja es su
documentación, y una hoja que anima sin documentar cómo se apaga deja al siguiente lector sin el dato. Es
exactamente el problema que `theme-sheet-contracts` arregló en las otras cinco.

## D10 — Lo que ninguna guarda mide: el aspecto

El repaso de paleta, contraste, escala, jerarquía y espaciado **no lo mide ninguna prueba**, y se dice en
lugar de disfrazarlo de puerta del CI. Lo que sí se hace, y queda en `tasks.md`:

- **Medir el contraste** de cada par texto/fondo de las seis hojas y dejar los números escritos; el objetivo
  de lectura es **AA** (4.5:1 para el texto normal, 3:1 para el grande), y las **cuatro** hojas que llevan
  acento —`default` con su acento por código, `terminal`, `paper` y `amber`; `minimal` y `compact` no tienen
  ninguno— miden además el acento sobre su fondo.
- **Revisar a ojo** la escala y el espaciado sobre las seis hojas servidas, porque un número no distingue
  una jerarquía legible de una que no lo es.

**La exención del `<hr>`, y por qué no se sube.** El separador de las seis hojas es **decorativo** —la
página se entiende igual sin él—, y su contraste sobre el fondo va de **1,31 a 1,55** según la hoja
—1,31 `terminal`, 1,39 `paper`, **1,43** claro y **1,55** oscuro en `default`/`minimal`/`compact`,
1,48 `amber`—. Queda **fuera** del 3:1 de WCAG 1.4.11 porque ese criterio cubre el **contenido no
textual necesario para entender el contenido** y los componentes de interfaz, **no** el adorno. Se
decide **no** subirlo: llevarlo a 3:1 obligaría a un trazo mucho más marcado y rompería el aire de las
seis hojas —el `<hr>` existe para separar, no para llamar la atención—. Y es una **decisión explícita
del usuario**, no el resultado de una guarda: **ninguna guarda mide el aspecto**. Lo que sí importa es
el contraste del **texto** —el que se lee—, que se mide **a mano** (**44 pares**, objetivo **AA**) y se
deja escrito; el `<hr>` queda con su ratio en la cabecera de cada hoja, para que quien vuelva a medirlo
no lo cuente como hallazgo.

Lo que **sí** es comprobable y entra en la guarda es lo de siempre: que ninguna de las seis hojas puede
hacer una petición y que la que anima puede apagarse. La hoja del operador sigue fuera del alcance —el
servicio no parsea CSS al servir, y validarla al arrancar haría que el arranque dependiera del sistema de
ficheros—.

## Open Questions

1. **El `<footer>` con el nombre del servicio**: se decide texto plano sin enlace. Si algún día el proyecto
   quiere enlazar al repositorio, sería un `href` **nuevo** y tendría que existir como fichero servible —y
   ahí sí cambiaría «Las páginas no referencian ficheros que no existen»—, que es un cambio de contrato y
   no un repaso estético.
2. **Un tema con más movimiento** —uno de los otros cinco animando, o un séptimo tema— es una fase
   posterior, y con la guarda endurecida ya tiene la puerta que necesita. No entra aquí.
3. **La medida de contraste como puerta**: automatizarla exigiría parsear CSS y calcular luminancias en una
   prueba. Se descarta: son seis hojas, se miden a mano y se dejan escritas.
