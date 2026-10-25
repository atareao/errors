# error-page Specification (delta)

## MODIFIED Requirements

### Requirement: Las páginas no referencian ficheros que no existen

Toda página HTML de `CONTENT_DIR`, y su copia embebida, SHALL referenciar únicamente ficheros servibles:
presentes en `CONTENT_DIR` o embebidos en el binario. Esa referencia SHALL ser **una sola**: el marcador
`<link rel="stylesheet" href="/css/error.css" />`, que apunta a la hoja del tema `default` —que es también
el asset `/css/error.css`—.

Las páginas SHALL compartir **una sola** hoja de estilos, `css/error.css`, con un nombre neutro. Ninguna
página SHALL referenciar una hoja cuyo nombre sea el de **otro** código: un `403` que carga `404.css` hace
que el error se presente con el nombre —y la apariencia— de otro, que es el defecto que este requisito
corrige. La hoja SHALL ser única porque once copias del mismo contenido no son once hojas, son una hoja mal
repartida; la diferencia de aspecto por código, si se quiere, se expresa con una clase en el elemento
`body`, no duplicando el fichero.

El **fichero** conserva el marcador; la página **servida** no: el marcador se sustituye al servir por la
hoja del tema elegido en un bloque `<style>` (requisito «El HTML servido es autocontenido»). El marcador es,
además, el único dato que el render necesita para saber **dónde** incrustar la hoja.

#### Scenario: ninguna página carga la hoja de otro código
- **GIVEN** las páginas de `content/`
- **WHEN** se revisan sus `href` de hojas de estilos
- **THEN** todas SHALL apuntar a `css/error.css`
- **AND** ninguna SHALL apuntar a `css/<código>.css`

#### Scenario: la hoja compartida existe y resuelve
- **GIVEN** que las páginas referencian `css/error.css`
- **WHEN** se comprueba el directorio de contenido
- **THEN** `content/css/error.css` SHALL existir

#### Scenario: cada enlace local resuelve
- **GIVEN** las páginas de `content/`
- **WHEN** se extraen sus `href` que no salen a otro host
- **THEN** cada ruta SHALL existir como fichero dentro de `CONTENT_DIR`

#### Scenario: la página embebida y su hoja se sirven juntas
- **GIVEN** un servicio arrancado sin ningún fichero en `CONTENT_DIR`
- **WHEN** se pide la página del `404` en HTML y, después, la hoja que el tema usa
- **THEN** el cuerpo SHALL llevar esa hoja incrustada en un bloque `<style>`, de modo que la página se sirva completa y no pida ningún fichero para verse
- **AND** el cuerpo NO SHALL llevar ningún `<link rel="stylesheet">`
- **AND** la petición de `GET /css/error.css` SHALL seguir respondiendo `200` con `Content-Type` `text/css`
- **AND** esa respuesta NO SHALL ser una página de error

## ADDED Requirements

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

Ninguna hoja de tema SHALL referenciar recursos externos —ni `@import`, ni `url(...)` remotos, ni fuentes
descargadas—: una página que necesita la red para verse es el mismo defecto que este requisito corrige.

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
