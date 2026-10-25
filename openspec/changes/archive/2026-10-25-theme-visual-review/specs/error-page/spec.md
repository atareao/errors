# error-page Specification (delta)

## MODIFIED Requirements

### Requirement: El tema del HTML es configurable

La hoja que se incrusta en el HTML SHALL elegirse entre los temas **incorporados al binario**. Un tema SHALL
ser **una sola hoja de estilos**: añadir un tema es añadir un fichero, no duplicar la página. El
conjunto de temas SHALL ser, **sin cambios**, `default`, `minimal`, `terminal`, `paper`, `amber` y
`compact`: **seis** nombres, ni uno más ni uno menos. `default` SHALL seguir siendo el tema por defecto y
SHALL seguir teniendo su hoja en el path `css/error.css` —el mismo asset público `/css/error.css`—, de
modo que un servicio arrancado sin `THEME` siga sirviendo la misma hoja, del mismo sitio, que el asset
que él mismo publica. Las **reglas** de esa hoja PUEDEN cambiar de una versión a otra: son del proyecto y
su aspecto no es un contrato; lo que sí lo es, y SHALL comprobarse, es el **nombre** del tema y el
**path** de su hoja. El motivo SHALL quedar escrito: quien leyese la cláusula anterior —«con sus reglas
intactas, de modo que quien no configure nada no vea ningún cambio de aspecto»— creería que el repaso
estético de las hojas está prohibido, y no lo está: lo que está fijado es el conjunto de nombres y el
sitio de la hoja del defecto, no su contenido.

La variable de entorno `THEME` SHALL elegir el tema del servicio, con `default` por defecto. Un valor que no
sea uno de los seis, **o la cadena vacía**, SHALL abortar el arranque con un error que **nombre `THEME`** y
que **enumere los nombres válidos**, como `LOG_FORMAT`: es una variable que escribe el operador, y quien la
escribe cree que está configurando algo.

La cabecera de petición `X-Theme` SHALL poder elegir el tema **por petición**, con la precedencia de
siempre —el **primer** origen que haga match, así que `X-Theme` gana a `THEME`—. Un `X-Theme` desconocido o
vacío NO SHALL ser un error: cae al tema configurado, porque esa cabecera la elige quien llama y un cliente
no puede romper el servicio. La comparación de nombres SHALL ser **exacta**. `X-Theme` NO SHALL afectar a
los formatos que no son HTML: sin hoja que incrustar, el tema se ignora.

La hoja de un tema SHALL resolverse con la misma regla que un asset: un fichero **legible** en
`CONTENT_DIR` SHALL ganar a la copia embebida del mismo path, de modo que la hoja que el servicio incrusta y
la que sirve en `/css/error.css` no puedan discrepar. La ruta de cada tema dentro de `CONTENT_DIR` SHALL ser
estable y documentada: `css/error.css` para `default` y `themes/<nombre>.css` para los demás. Un fallo al
leer la hoja NO SHALL cambiar el estado de la respuesta ni impedir que la página se sirva: la hoja es adorno
y el servicio SHALL usar la copia embebida, dejando constancia en el registro.

El cuerpo de la respuesta depende de `X-Theme`, así que esa cabecera SHALL declararse en `Vary` (spec
`response-extras`, «Las cabeceras de correlación se declaran en Vary»).

Una hoja de tema PUEDE animar: el movimiento es una decisión de aspecto, no un defecto de
autocontención. Pero lo que anima SHALL poder apagarse: una hoja que declare una animación SHALL
**neutralizarla** dentro de un bloque `@media (prefers-reduced-motion: reduce)`, y ese bloque SHALL
**existir**, NO SHALL estar vacío y SHALL neutralizar **cada** animación que la hoja declare. El motivo
SHALL quedar escrito: hay gente a la que el movimiento le produce mareo, y una página de error que se
mueve mientras el usuario intenta leer qué ha pasado es peor página de error. La regla no prohíbe el
adorno: prohíbe el adorno que no se puede apagar.

Lo que anima SHALL ser una **declaración con un valor distinto de `none`**: `animation`, `animation-name`,
`transition` o `transition-property`. Una declaración con valor `none` NO SHALL contar como animación, y
`@keyframes` **por sí solo NO SHALL obligar a nada**: define fotogramas y no anima mientras ninguna
declaración los use, así que una hoja que solo traiga `@keyframes` SHALL cumplir sin necesitar ningún
bloque de movimiento reducido. El motivo SHALL quedar escrito: exigir la media query a una hoja que no
puede animar es medir algo distinto del contrato, que es justo el defecto que este requisito ya corrigió
una vez.

La neutralización SHALL aceptarse en estas formas, y **solo en estas**: `animation: none` o
`animation-name: none` para una animación declarada con `animation` o `animation-name`, y
`transition: none` o `transition-property: none` para una declaración con `transition` o
`transition-property`. Una declaración SHALL quedar neutralizada por la forma que le corresponde:
`animation: none` NO SHALL neutralizar una transición, ni `transition: none` una animación. El motivo de
que la lista sea **cerrada** SHALL quedar escrito: una guarda textual no puede demostrar que otras formas
de apagar una animación —`animation-duration: 0s`, `animation-play-state: paused`— son equivalentes en
todos los navegadores, así que exige la forma canónica y rechaza de más, que es la dirección segura.

Y el mismo principio rige para los **comentarios**: una hoja que mencione `animation` **solo dentro de un
comentario** NO SHALL tener que neutralizar nada, porque un comentario no anima; y un bloque de movimiento
reducido cuyo contenido sea **solo** un comentario NO SHALL contar como bloque que neutraliza, porque un
comentario tampoco apaga. Una hoja que no anima NO SHALL tener que declarar nada.

La guarda SHALL medir **el patrón canónico de neutralización y nada más**, y su límite SHALL quedar
escrito: es **textual** y sobre los **bytes** de la hoja —una hoja puede no estar en UTF-8—, así que lo
que garantiza es que la media query está, que su bloque no está vacío y que trae la neutralización que
corresponde a cada animación declarada, y NO SHALL garantizar que el CSS apague el movimiento en todos
los navegadores. Ese límite SHALL quedar escrito precisamente porque una guarda que promete lo que no
puede medir es el defecto que este requisito ya corrigió.

#### Scenario: sin THEME el tema es el de siempre
- **GIVEN** un entorno sin `THEME`
- **WHEN** se sirve una página HTML
- **THEN** la hoja incrustada SHALL ser la de `content/css/error.css`

#### Scenario: THEME cambia el tema de todo el servicio
- **GIVEN** `THEME=terminal`
- **WHEN** se pide `GET /404` con `Accept: text/html`
- **THEN** la hoja incrustada SHALL ser la del tema `terminal`
- **AND** NO SHALL ser la del tema `default`

#### Scenario: un THEME desconocido no arranca
- **GIVEN** `THEME=neon`
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar con un error que nombre `THEME`
- **AND** el mensaje SHALL enumerar los seis nombres válidos

#### Scenario: un THEME vacío no arranca
- **GIVEN** `THEME` presente y vacía
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar, en vez de usar `default`

#### Scenario: X-Theme elige el tema de esa petición
- **GIVEN** un servicio con el tema `default`
- **WHEN** se pide `GET /404` con `X-Theme: paper`
- **THEN** la hoja incrustada SHALL ser la del tema `paper`

#### Scenario: un X-Theme desconocido cae al tema configurado
- **GIVEN** `THEME=minimal` y una petición con `X-Theme: neon`
- **WHEN** se responde
- **THEN** la hoja incrustada SHALL ser la de `minimal`
- **AND** la respuesta NO SHALL ser un error

#### Scenario: un X-Theme que no puede leerse no impide servir la página
- **GIVEN** un tema cuyo fichero en `CONTENT_DIR` existe y no se puede leer
- **WHEN** se pide una página HTML de ese código
- **THEN** el estado SHALL ser el código resuelto
- **AND** el cuerpo SHALL llevar la hoja embebida de ese tema

#### Scenario: X-Theme no afecta al JSON
- **GIVEN** una petición `GET /404` con `Accept: application/json` y `X-Theme: terminal`
- **WHEN** se responde
- **THEN** el cuerpo SHALL ser el JSON de siempre

#### Scenario: el conjunto de temas sigue siendo de seis nombres
- **GIVEN** el registro de temas incorporados al binario
- **WHEN** se enumeran sus nombres y el path de la hoja del defecto
- **THEN** los nombres SHALL ser `default`, `minimal`, `terminal`, `paper`, `amber` y `compact`
- **AND** `default` SHALL seguir siendo el tema por defecto
- **AND** la hoja del tema `default` SHALL seguir siendo `css/error.css`
- **AND** NO SHALL haber un séptimo nombre

#### Scenario: una hoja que anima declara prefers-reduced-motion
- **GIVEN** una hoja de tema con una declaración de `animation`, `animation-name`, `transition` o
  `transition-property` con un valor distinto de `none`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo si no neutraliza esa declaración dentro de un bloque
  `@media (prefers-reduced-motion: reduce)`
- **AND** SHALL cumplirlo si la neutraliza
- **AND** una hoja sin animación SHALL cumplirlo sin declarar nada

#### Scenario: un comentario no anima
- **GIVEN** una hoja que menciona `animation` únicamente dentro de un comentario `/* … */`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo sin declarar `prefers-reduced-motion`

#### Scenario: una hoja que solo nombra prefers-reduced-motion incumple
- **GIVEN** una hoja con `main { animation: entrada 180ms ease-out; }` y un bloque
  `@media (prefers-reduced-motion: reduce) { /* esta animación se apaga sola */ }` cuyo contenido es
  únicamente un comentario
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo: nombrar la media query no neutraliza nada

#### Scenario: una media query que no es la de movimiento reducido no basta
- **GIVEN** una hoja con `main { animation: entrada 180ms ease-out; }` y un bloque
  `@media (prefers-reduced-motion: no-preference) { main { animation: none; } }`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo: neutralizar en la preferencia contraria no es la puerta que el usuario cierra

#### Scenario: un bloque reduce vacío no basta
- **GIVEN** una hoja con `main { animation: entrada 180ms ease-out; }` y un bloque
  `@media (prefers-reduced-motion: reduce) { }` sin nada dentro
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo: el bloque tiene que existir y **tener** la neutralización

#### Scenario: un bloque reduce que neutraliza la animación cumple
- **GIVEN** una hoja con `@keyframes entrada { … }`, `main { animation: entrada 180ms ease-out; }` y
  `@media (prefers-reduced-motion: reduce) { main { animation: none; } }`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo

#### Scenario: neutralizar otra animación distinta de la declarada no basta
- **GIVEN** una hoja que declara `transition: opacity 200ms;` y cuyo bloque
  `@media (prefers-reduced-motion: reduce)` solo trae `animation: none`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo: la transición declarada sigue sin neutralizarse

#### Scenario: una animación declarada dentro del bloque reduce no se neutraliza sola
- **GIVEN** una hoja cuyo bloque `@media (prefers-reduced-motion: reduce)` trae
  `main { animation: entrada 180ms ease-out; }`, que anima en vez de apagar
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo

#### Scenario: unos fotogramas sin declaración que los use no obligan
- **GIVEN** una hoja cuyo único contenido es `@keyframes entrada { from { opacity: 0 } to { opacity: 1 } }`,
  sin ninguna declaración de `animation`, `animation-name`, `transition` ni `transition-property`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo sin necesitar ningún bloque `@media (prefers-reduced-motion: reduce)`, porque
  definir fotogramas no anima nada

#### Scenario: una regla anidada dentro del bloque reduce cuenta
- **GIVEN** una hoja con `main { animation: entrada 180ms ease-out; }` y un bloque
  `@media (prefers-reduced-motion: reduce) { main { animation: none; } }`, con la neutralización dentro de
  una regla anidada
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo: el bloque se delimita por su llave de apertura y **su** cierre equilibrado, no
  por la primera llave que aparezca
