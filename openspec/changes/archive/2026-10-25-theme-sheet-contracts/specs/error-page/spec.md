# error-page Specification (delta)

## MODIFIED Requirements

### Requirement: El HTML servido es autocontenido

La respuesta en formato HTML de toda página de error SHALL ser autocontenida: el marcador
`<link rel="stylesheet" href="/css/error.css" />` SHALL sustituirse, al servir, por un bloque `<style>` con
la hoja del tema elegido, y el cuerpo servido NO SHALL contener ningún `<link rel="stylesheet">` que apunte
a `css/error.css`.

El motivo SHALL quedar escrito: en el patrón del middleware `errors` de Traefik —el que usa cualquier
aplicación con un dominio propio— la página se sirve **en el host de la aplicación**, donde este servicio
no sirve assets. El navegador pide `/css/error.css`, recibe el `404` del upstream, el middleware lo
intercepta y lo vuelve a pedir como página de error con un `Accept` que no negocia HTML, así que la cascada
de formato cae al texto plano y el navegador rechaza la hoja —«MIME type ('text/plain') is not a supported
stylesheet MIME type»—. Medido. En el host del respaldo el mismo enlace funciona, y esa asimetría es la que
escondía el fallo. `error-pages` no lo sufre porque sus páginas son autocontenidas: incrusta estilos e
imágenes en el propio HTML, sin cargar ningún recurso externo. Es paridad, no un adorno.

Una página **sin** el marcador —una página propia del operador, ya autocontenida— SHALL servirse **tal
cual**, sin añadir ni sustituir nada, y sin que el servicio lea hoja alguna para servirla. Un `<link>` que
**no** sea el marcador NO SHALL tocarse: es del operador.

Ninguna hoja de tema SHALL poder hacer que el navegador **pida** algo por la red. En concreto: NO SHALL
contener `@import` en ninguna de sus formas —incluida `@import "otra.css";`, que no lleva `url(`—, ni
`url(...)` cuyo destino no sea **el propio documento**: una referencia interna como `url(#gradiente)`
—un filtro o un degradado SVG— NO SHALL contar como recurso externo, porque no hay ninguna petición que
hacer. El motivo SHALL quedar escrito: una página que necesita la red para verse es el mismo defecto que
este requisito corrige, y se manifiesta sobre todo en el host de la aplicación, donde este servicio no
sirve assets.

`@font-face` NO SHALL estar prohibido, y el motivo SHALL quedar escrito: solo puede pedir algo por la red
a través de `url(...)` o `@import`, que ya están prohibidos, y un `src: local(...)` usa una fuente que el
usuario **ya tiene instalada**. Prohibirlo era prohibir un token sospechoso en vez de un contrato.

La guarda automática SHALL medir **este contrato y nada más**, y SHALL cubrir las hojas **incorporadas al
binario**: la hoja del operador en `CONTENT_DIR` es suya, y el servicio NO SHALL parsear CSS al servir
—la hoja es adorno y un CSS roto no puede tumbar una página—.

La guarda SHALL leer las **reglas** de la hoja, no su prosa: un comentario `/* … */` NO SHALL contar
como una petición, porque un comentario no puede hacer que el navegador pida nada, y el escaneo
SHALL ignorarlo. El motivo SHALL quedar escrito: las cabeceras de las hojas documentan por escrito
qué no pueden hacer, así que leerlas como si fueran reglas convertiría la propia documentación de la
hoja en un incumplimiento.

#### Scenario: el 503 de una aplicación llega con los estilos dentro
- **GIVEN** el ejemplo levantado, con el middleware `errors` delante de un backend que responde `503` en `/boom`
- **WHEN** se pide `GET /boom` con `Accept: text/html`
- **THEN** el estado SHALL ser `503` y el `Content-Type` `text/html`
- **AND** el cuerpo SHALL llevar un bloque `<style>` con la hoja del tema
- **AND** el cuerpo NO SHALL llevar `<link rel="stylesheet" href="/css/error.css">`

#### Scenario: la página servida no pide ningún fichero para verse
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /404` con `Accept: text/html`
- **THEN** el cuerpo NO SHALL contener `rel="stylesheet"`
- **AND** el cuerpo SHALL contener `<style`

#### Scenario: una página propia y ya autocontenida se sirve sin tocar
- **GIVEN** un `CONTENT_DIR` cuya página `404.html` no lleva el marcador
- **WHEN** se pide `GET /404` en HTML
- **THEN** el cuerpo SHALL ser esa página **byte a byte**, sin añadidos ni sustituciones

#### Scenario: un enlace que no es el marcador se deja en paz
- **GIVEN** una página con el marcador **y** otro `<link rel="stylesheet" href="/css/otra.css">`
- **WHEN** se sirve en HTML
- **THEN** el marcador SHALL quedar sustituido por el bloque `<style>`
- **AND** el otro `<link>` SHALL seguir en el cuerpo

#### Scenario: una referencia al propio documento no es una petición
- **GIVEN** una hoja de tema con `url(#gradiente)` y con un `@font-face` cuyo `src` es `local("Arial")`
- **WHEN** se comprueba contra el contrato de autocontención
- **THEN** SHALL cumplirlo, porque no hay ninguna petición que el navegador tenga que hacer

#### Scenario: una hoja que pide un fichero incumple
- **GIVEN** una hoja de tema con `url(foto.png)`, otra con `url(https://ejemplo/x.png)` y otra con `@import "otra.css"`
- **WHEN** se comprueban contra el contrato
- **THEN** cada una SHALL incumplirlo, y el incumplimiento SHALL nombrar la hoja y el motivo

#### Scenario: un comentario no es una petición
- **GIVEN** una hoja cuyo único `url(` y cuyo único `@import` están dentro de un comentario `/* … */`
- **WHEN** se comprueba contra el contrato de autocontención
- **THEN** SHALL cumplirlo, porque un comentario no puede pedir nada
- **AND** una hoja que mencione `url(` en un comentario **y** traiga un `url(foto.png)` de verdad SHALL incumplirlo

### Requirement: El tema del HTML es configurable

La hoja que se incrusta en el HTML SHALL elegirse entre los temas **incorporados al binario**. Un tema SHALL
ser **una sola hoja de estilos**: añadir un tema es añadir un fichero, no duplicar las once páginas. El
conjunto de temas SHALL ser `default` —la hoja que el proyecto ya trae, con sus reglas intactas, de modo que
quien no configure nada no vea ningún cambio de aspecto—, `minimal`, `terminal`, `paper`, `amber` y
`compact`.

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
autocontención. Pero una hoja que traiga `animation`, `transition` o `@keyframes` SHALL respetar también
`prefers-reduced-motion`. El motivo SHALL quedar escrito: hay gente a la que el movimiento le produce
mareo, y una página de error que se mueve mientras el usuario intenta leer qué ha pasado es peor página
de error. La regla no prohíbe el adorno: prohíbe el adorno que no se puede apagar. El mismo principio
rige para el movimiento: una hoja que mencione `animation` **solo dentro de un
comentario** NO SHALL tener que declarar `prefers-reduced-motion`, porque un comentario no anima.
Una hoja que no anima NO SHALL tener que declarar nada.

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

#### Scenario: una hoja que anima declara prefers-reduced-motion
- **GIVEN** una hoja de tema con `animation`, `transition` o `@keyframes`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo si no declara `prefers-reduced-motion`
- **AND** SHALL cumplirlo si lo declara
- **AND** una hoja sin animación SHALL cumplirlo sin declarar nada

#### Scenario: un comentario no anima
- **GIVEN** una hoja que menciona `animation` únicamente dentro de un comentario `/* … */`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo sin declarar `prefers-reduced-motion`
