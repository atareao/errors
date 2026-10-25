# error-page Specification

## Purpose
El contrato que hace útil a este servicio detrás de Traefik: cómo se resuelve el código de error —el path, la cabecera `X-Code` o el defecto configurado— y cómo se negocia el formato de la respuesta —HTML, JSON, XML o texto plano—, con la garantía que justifica el proyecto: la respuesta lleva el **código de error real y nunca un 200**. Fija además el rango de códigos aceptados (`400..=599`), la normalización del path que impide que una página de error se sirva como asset, las cabeceras que acompañan a toda página y el respaldo cuando falta el HTML.

## Requirements

### Requirement: El código se resuelve en cascada

El código de error SHALL resolverse aplicando el primer origen que haga match, en este orden: (1) el path —`/404`, `/404.html`, `/404.json`, `/503.xml`, `/404.txt`—, del que SHALL extraerse el entero del primer segmento; (2) la cabecera `X-Code`; (3) el código por defecto configurado en `DEFAULT_ERROR_PAGE`. Un path cuyo primer segmento no sea un entero válido NO SHALL resolver código por esa vía.

#### Scenario: el código del path se resuelve
- **GIVEN** un servicio con el defecto `404`
- **WHEN** se pide `GET /404`
- **THEN** el código resuelto SHALL ser `404`

#### Scenario: el path gana a la cabecera
- **GIVEN** una petición a `GET /404` con cabecera `X-Code: 500`
- **WHEN** se resuelve el código
- **THEN** el código resuelto SHALL ser `404`

#### Scenario: sin código en el path gana la cabecera
- **GIVEN** una petición a `GET /` con cabecera `X-Code: 503`
- **WHEN** se resuelve el código
- **THEN** el código resuelto SHALL ser `503`

#### Scenario: sin pistas gana el defecto
- **GIVEN** una petición a `GET /` sin cabecera `X-Code`
- **WHEN** se resuelve el código, con defecto `404`
- **THEN** el código resuelto SHALL ser `404`

### Requirement: La respuesta devuelve el código resuelto

Toda página de error SHALL responder con un `StatusCode` igual al código resuelto. NO SHALL responder `200`.

#### Scenario: un 404 no se sirve como 200
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /404`
- **THEN** la respuesta SHALL tener el estado `404`
- **AND** NO SHALL tener el estado `200`

#### Scenario: un 503 llega como 503 por cabecera
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /` con `X-Code: 503`
- **THEN** la respuesta SHALL tener el estado `503`

### Requirement: El formato se resuelve en cascada

El formato SHALL resolverse con el primer origen que haga match, en este orden: (1) la extensión del path (`.html`/`.htm` → HTML, `.json` → JSON, `.xml` → XML, `.txt` → texto); (2) el `Content-Type` de la petición; (3) la cabecera `X-Format`; (4) la cabecera `Accept`; (5) por defecto **texto plano**.

#### Scenario: la extensión manda
- **GIVEN** una petición a `GET /404.json`
- **WHEN** se resuelve el formato
- **THEN** el formato SHALL ser JSON

#### Scenario: X-Format decide sin extensión
- **GIVEN** una petición a `GET /404` con `X-Format: application/xml`
- **WHEN** se resuelve el formato
- **THEN** el formato SHALL ser XML

#### Scenario: Accept decide faltando lo demás
- **GIVEN** una petición a `GET /404` con `Accept: text/html`
- **WHEN** se resuelve el formato
- **THEN** el formato SHALL ser HTML

#### Scenario: sin pistas el defecto es texto plano
- **GIVEN** una petición a `GET /404` sin extensión, sin `Content-Type`, sin `X-Format` y sin `Accept`
- **WHEN** se resuelve el formato
- **THEN** el formato SHALL ser texto plano

### Requirement: El cuerpo depende del formato y el código es ortogonal

El cuerpo HTML SHALL servirse del fichero `CONTENT_DIR/<code>.html` y, si el del código pedido no existe, del fichero del código por defecto. Si no existe **ninguno** de los dos ficheros, SHALL servirse la plantilla **embebida** en el binario, que cubre cualquier código. La cascada del directorio SHALL intentarse **entera** antes que la embebida, y un fichero de `CONTENT_DIR` **que se pueda leer** NO SHALL ser desplazado por la copia embebida. Los cuerpos JSON, XML y texto SHALL generarse a partir del código, su razón canónica y su descripción. El código resuelto NO SHALL depender del formato ni del origen del cuerpo.

La página HTML del proyecto SHALL ser **una plantilla**, no once ficheros: la misma página SHALL
servir para cualquier código, con el código, la razón canónica y la descripción como **marcadores**
(`{{code}}`, `{{reason}}`, `{{description}}`) que el servidor SHALL sustituir **al servir** por el
código resuelto y por lo que devuelven `canonical_reason` y `description`. El motivo SHALL quedar
escrito: el fichero por código repetía cinco valores escritos a mano en cada uno de los once, y un
código sin fichero propio recibía el cuerpo de **otro** código —un `451` con el cuerpo de un `404`—,
con el estado y el cuerpo en desacuerdo.

Las reglas de la sustitución SHALL ser **las mismas** que las del marcador de la hoja (`El HTML servido
es autocontenido`): una página **sin** marcadores SHALL servirse **byte a byte**, y SHALL sustituirse
**cada** aparición, no solo la primera. La razón y la descripción SHALL insertarse **escapadas** para
HTML (`&`, `<`, `>`): las escribe el servidor, no un humano dentro del fichero.

La cascada del cuerpo HTML SHALL ser: `CONTENT_DIR/<código>.html` —el *override* del operador—; si no,
`CONTENT_DIR/<código por defecto>.html`; y si no, la plantilla embebida. Un fichero **ilegible** SHALL
seguir siendo un fallo de E/S, con el respaldo de texto y sin cambiar el estado.

#### Scenario: HTML desde el fichero del código
- **GIVEN** que `content/404.html` existe y no lleva marcadores de plantilla
- **WHEN** se pide `GET /404.html`
- **THEN** el cuerpo SHALL ser el contenido de `content/404.html`

#### Scenario: HTML cae al código por defecto
- **GIVEN** que `content/418.html` no existe, que el defecto es `404` y que `content/404.html` existe y no lleva marcadores de plantilla
- **WHEN** se pide `GET /418.html`
- **THEN** el cuerpo SHALL ser el contenido de `content/404.html`
- **AND** el estado SHALL ser `418`

#### Scenario: la página embebida se sirve sin directorio de contenido
- **GIVEN** un servicio cuyo `CONTENT_DIR` no contiene ningún fichero
- **WHEN** se pide `GET /503.html`
- **THEN** el estado SHALL ser `503`
- **AND** el cuerpo SHALL ser la plantilla embebida rellenada con `503`
- **AND** el `Content-Type` SHALL ser HTML

#### Scenario: el fichero del directorio gana a la copia embebida
- **GIVEN** un `CONTENT_DIR` con un `404.html` propio, distinto del embebido
- **WHEN** se pide `GET /404.html`
- **THEN** el cuerpo SHALL ser el de `CONTENT_DIR/404.html`
- **AND** NO SHALL ser la plantilla embebida rellenada con `404`

#### Scenario: el directorio se agota antes de entrar en la copia embebida
- **GIVEN** un `CONTENT_DIR` con su propio `404.html` y sin `503.html`, y el defecto `404`
- **WHEN** se pide `GET /503.html`
- **THEN** el cuerpo SHALL ser el de `CONTENT_DIR/404.html`
- **AND** NO SHALL ser la plantilla embebida rellenada con `503`

#### Scenario: sin ficheros, un código sin página propia cae a la embebida del defecto
- **GIVEN** un servicio sin ningún fichero en `CONTENT_DIR` y con el defecto `404`
- **WHEN** se pide `GET /418.html`
- **THEN** el estado SHALL ser `418`
- **AND** el cuerpo SHALL ser la plantilla embebida rellenada con `418`
- **AND** NO SHALL nombrar `404`

#### Scenario: una página embebida no se sirve como asset
- **GIVEN** un servicio sin ningún fichero en `CONTENT_DIR`
- **WHEN** se pide `GET /404.html`
- **THEN** el estado SHALL ser `404`
- **AND** NO SHALL ser `200`
- **AND** el cuerpo SHALL ser la plantilla embebida rellenada con `404`

#### Scenario: JSON se genera
- **GIVEN** una petición a `GET /404.json`
- **WHEN** se responde
- **THEN** el cuerpo SHALL ser JSON con las claves `error`, `code`, `message` y `description`
- **AND** `code` SHALL ser `404`

#### Scenario: XML en el mismo código
- **GIVEN** una petición a `GET /503.xml`
- **WHEN** se responde
- **THEN** el estado SHALL ser `503`
- **AND** el cuerpo SHALL ser XML con el elemento `code` igual a `503`

#### Scenario: un código sin fichero propio recibe su propio cuerpo
- **GIVEN** un servicio con la plantilla del proyecto y sin fichero para `451`
- **WHEN** se pide `GET /451` con `Accept: text/html`
- **THEN** el estado SHALL ser `451`
- **AND** el cuerpo SHALL nombrar `451` y su razón canónica
- **AND** el cuerpo NO SHALL nombrar `404`

#### Scenario: la razón y la descripción del HTML son las de los demás formatos
- **GIVEN** un código con prosa propia en la tabla de descripciones
- **WHEN** se pide ese código en HTML y en JSON
- **THEN** la razón y la descripción SHALL ser las mismas en los dos

#### Scenario: la plantilla escapa lo que inserta
- **GIVEN** una razón canónica o una descripción que contenga `&`, `<` o `>`
- **WHEN** se rellena la plantilla con ese valor
- **THEN** el valor SHALL insertarse escapado, y el cuerpo servido SHALL seguir siendo HTML válido

### Requirement: Las cabeceras de acompañamiento

Toda página de error SHALL incluir `Content-Type` según el formato y `Content-Length` del cuerpo, y SHALL incluir `X-Robots-Tag: noindex, nofollow, nosnippet, noarchive`. Los códigos `429` y `503` SHALL incluir además `Retry-After` con el valor configurado en `RETRY_AFTER` —`120` por defecto, el mismo que `error-pages`—; el resto NO SHALL incluirlo.

Qué códigos llevan la cabecera NO SHALL ser configurable: lo decide el código, no el entorno.

El valor SHALL ser un número entero de segundos (`delay-seconds` de RFC 9110), nunca una fecha HTTP.

#### Scenario: toda página lleva X-Robots-Tag
- **GIVEN** una petición a `GET /404`
- **WHEN** se responde
- **THEN** la respuesta SHALL incluir `X-Robots-Tag: noindex, nofollow, nosnippet, noarchive`

#### Scenario: el 503 lleva Retry-After
- **GIVEN** un servicio arrancado sin `RETRY_AFTER`
- **WHEN** se responde una página `503`
- **THEN** la respuesta SHALL incluir `Retry-After: 120`

#### Scenario: el 503 lleva el valor configurado
- **GIVEN** un servicio arrancado con `RETRY_AFTER=30`
- **WHEN** se responde una página `503`
- **THEN** la respuesta SHALL incluir `Retry-After: 30`

#### Scenario: el 429 también lo lleva
- **GIVEN** un servicio arrancado con `RETRY_AFTER=45`
- **WHEN** se responde una página `429`
- **THEN** la respuesta SHALL incluir `Retry-After: 45`

#### Scenario: el 404 no lleva Retry-After
- **GIVEN** un servicio arrancado con `RETRY_AFTER=30`
- **WHEN** se responde una página `404`
- **THEN** la respuesta NO SHALL incluir `Retry-After`

#### Scenario: el valor no depende del formato ni del origen del código
- **GIVEN** un servicio arrancado con `RETRY_AFTER=30`
- **WHEN** se pide un `503` en JSON y un asset inexistente con `X-Code: 503`
- **THEN** las dos respuestas SHALL incluir `Retry-After: 30`

### Requirement: El path se normaliza antes de clasificarse

El path SHALL normalizarse —decodificando los percent-encodings y plegando los componentes `.`— antes de decidir si la petición es una página de error, de modo que `/./404.html` y `/404%2Ehtml` se clasifiquen como `/404.html`. Un componente `..` NO SHALL permitir salir de `CONTENT_DIR`.

El motivo SHALL quedar escrito: el servidor de ficheros decodifica y normaliza el path, así que clasificar sobre el path crudo deja pasar la página de error como si fuera un asset, servida con `200`. Es exactamente el fallo que este proyecto corrige.

#### Scenario: un componente `.` no esquiva la página de error
- **GIVEN** que `content/404.html` existe
- **WHEN** se pide `GET /./404.html`
- **THEN** el estado SHALL ser `404`
- **AND** NO SHALL ser `200`

#### Scenario: un path percent-codificado no esquiva la página de error
- **GIVEN** que `content/404.html` existe
- **WHEN** se pide `GET /404%2Ehtml`
- **THEN** el estado SHALL ser `404`

#### Scenario: no se puede salir del directorio de contenido
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /css/../../Cargo.toml`
- **THEN** el estado SHALL estar en `400..=599`
- **AND** el cuerpo NO SHALL contener el contenido de `Cargo.toml`

### Requirement: Solo se resuelven códigos de error utilizables

El código resuelto SHALL estar en el rango `400..=599`. Una fuente que aporte un código fuera de ese rango NO SHALL usarse: esa fuente SHALL considerarse vacía y SHALL continuarse con la siguiente de la cascada; si ninguna aporta un código utilizable, SHALL usarse el código por defecto. `DEFAULT_ERROR_PAGE` fuera de `400..=599` SHALL ser un error de arranque que nombre la variable. La respuesta de una página de error NO SHALL tener nunca un estado `2xx`, ni `1xx`, ni `3xx`.

El motivo SHALL quedar escrito: el rango es el que Traefik intercepta (`errors.status: 400-599`), y códigos fuera de él no son representables de forma fiable como respuesta final —`1xx` no lo es en absoluto, `204` y `304` no admiten cuerpo, y un `2xx` contradice la razón de existir del proyecto—.

#### Scenario: un 200 pedido no responde 200
- **GIVEN** un servicio con defecto `404`
- **WHEN** se pide `GET /200`
- **THEN** el estado SHALL ser `404`
- **AND** NO SHALL ser `200`

#### Scenario: un código de información cae al defecto
- **GIVEN** un servicio con defecto `404`
- **WHEN** se pide `GET /100`
- **THEN** el estado SHALL ser `404`

#### Scenario: un código fuera de rango deja paso a la cabecera
- **GIVEN** una petición a `GET /200` con `X-Code: 503` y defecto `404`
- **WHEN** se resuelve el código
- **THEN** el código resuelto SHALL ser `503`

#### Scenario: un X-Code fuera de rango cae al defecto
- **GIVEN** una petición a `GET /` con `X-Code: 200` y defecto `404`
- **WHEN** se resuelve el código
- **THEN** el código resuelto SHALL ser `404`

#### Scenario: un DEFAULT_ERROR_PAGE fuera de rango no arranca
- **GIVEN** `DEFAULT_ERROR_PAGE=200`
- **WHEN** se carga la configuración
- **THEN** SHALL fallar con un error que nombre `DEFAULT_ERROR_PAGE`

### Requirement: Un asset existente gana a la página de error

Una petición cuyo path normalizado corresponda a un fichero existente de `CONTENT_DIR` y que no sea una página de error SHALL servirse como asset, con su `Content-Type`. La cabecera `X-Code` NO SHALL convertir un asset existente en una página de error: la garantía del código de error aplica a las **páginas de error**, no a los assets.

#### Scenario: un asset con X-Code sigue siendo un asset
- **GIVEN** que `content/css/404.css` existe
- **WHEN** se pide `GET /css/404.css` con `X-Code: 503`
- **THEN** SHALL responder `200`
- **AND** el `Content-Type` SHALL ser `text/css`

### Requirement: Un fallo al leer el HTML no cambia el estado

Si el fichero HTML del código y el del código por defecto faltan o no se pueden leer —ni en `CONTENT_DIR` ni entre lo embebido—, la aplicación SHALL responder el **código resuelto** con el cuerpo del formato resuelto, y NO SHALL responder `500`.

Con la plantilla embebida esa situación NO SHALL ser alcanzable por **ausencia** de fichero: la plantilla cubre cualquier código de `400..=599`, así que ya no hay ningún `DEFAULT_ERROR_PAGE` legal que se quede sin página. SHALL quedar al alcance de un fichero de `CONTENT_DIR` que **exista y no se pueda leer**, que es un fallo de E/S: la aplicación SHALL responder el respaldo de texto con el código resuelto, dejando constancia en el registro, en vez de tapar el fichero roto del operador con la copia embebida. El respaldo NO SHALL retirarse.

#### Scenario: sin fichero HTML se mantiene el código
- **GIVEN** un `CONTENT_DIR` sin ningún fichero HTML y con el defecto `418`
- **WHEN** se pide `GET /418.html`
- **THEN** el estado SHALL ser `418`
- **AND** NO SHALL ser `500`
- **AND** el cuerpo SHALL ser la plantilla embebida rellenada con `418`

#### Scenario: el respaldo cubre también un código distinto del defecto
- **GIVEN** un `CONTENT_DIR` sin ningún fichero HTML y con el defecto `418`
- **WHEN** se pide `GET /599.html`
- **THEN** el estado SHALL ser `599`
- **AND** NO SHALL ser `500`

#### Scenario: una página del operador ilegible no tumba la respuesta
- **GIVEN** un `CONTENT_DIR` cuyo `404.html` existe y no se puede leer, y el defecto `404`
- **WHEN** se pide `GET /404.html`
- **THEN** el estado SHALL ser `404`
- **AND** el cuerpo SHALL ser el respaldo de texto del código resuelto
- **AND** NO SHALL ser `500`

### Requirement: Los códigos que un proxy emite tienen su propia página

Los códigos que un proxy emite con normalidad —`400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`, `502`, `503` y `504`— SHALL tener una página HTML, y SHALL tenerla **cualquier** otro código del rango `400..=599`: la **plantilla embebida en el binario**, rellenada con el código resuelto, de modo que un servicio instalado sin ficheros de datos sirva su propia página para todos ellos. La plantilla SHALL entregarse **una sola vez** —una página, no once ficheros por código—, porque el fichero por código repetía cinco valores escritos a mano en cada uno de los once y un código sin fichero propio recibía el cuerpo de **otro**.

`CONTENT_DIR` PUEDE contener el fichero `<código>.html`, que es el *override* **opcional** del operador y gana a la plantilla: el proyecto NO SHALL exigirlo, y su ausencia NO SHALL degradar ninguna página.

La plantilla embebida SHALL ser idéntica, byte a byte, al fichero del repositorio del que se embebe: las dos copias NO SHALL poder divergir en silencio.

La página HTML de un código SHALL identificar ese código en su título o en su cuerpo, de modo que el cuerpo nunca contradiga al estado, y esto SHALL cumplirse tanto en la plantilla embebida como en el *override* del operador.

Ningún código del rango SHALL depender del respaldo de texto: SHALL bastar la plantilla embebida. El respaldo NO SHALL retirarse, pero queda reservado al caso de un fichero de `CONTENT_DIR` que **exista y no se pueda leer**.

#### Scenario: el 504 tiene página propia y la nombra
- **GIVEN** un servicio con la plantilla del proyecto y sin fichero propio para `504`
- **WHEN** se pide `GET /504`
- **THEN** el estado SHALL ser `504`
- **AND** el cuerpo SHALL contener `504`

#### Scenario: ningún código de la lista cae al respaldo
- **GIVEN** la lista de códigos cubiertos
- **WHEN** se pide cada uno de ellos con `Accept: text/html`
- **THEN** el cuerpo SHALL contener su propio código

#### Scenario: sin directorio de contenido los once códigos sirven su página
- **GIVEN** un servicio arrancado sin ningún fichero en `CONTENT_DIR`
- **WHEN** se pide cada uno de los once códigos con `Accept: text/html`
- **THEN** el estado SHALL ser ese código
- **AND** el cuerpo SHALL llevar su propio código
- **AND** NO SHALL ser el respaldo de texto

#### Scenario: las dos copias de una página no divergen
- **GIVEN** el fichero de la plantilla en el árbol del repositorio y su copia embebida en el binario
- **WHEN** se comparan sus bytes
- **THEN** SHALL ser idénticos

#### Scenario: un código que no está tabulado también tiene página
- **GIVEN** un servicio sin ficheros de datos
- **WHEN** se pide `GET /451` con `Accept: text/html`
- **THEN** el estado SHALL ser `451` y el cuerpo SHALL ser la plantilla rellenada con `451`
- **AND** NO SHALL ser el respaldo de texto

### Requirement: Las páginas no referencian ficheros que no existen

La página HTML del proyecto —la plantilla—, y su copia embebida, SHALL referenciar únicamente ficheros servibles:
presentes en `CONTENT_DIR` o embebidos en el binario. Esa referencia SHALL ser **una sola**: el marcador
`<link rel="stylesheet" href="/css/error.css" />`, que apunta a la hoja del tema `default` —que es también
el asset `/css/error.css`—.

La página SHALL usar **una sola** hoja de estilos, `css/error.css`, con un nombre neutro: hay **una**
página, no once, así que no hay once copias del mismo contenido que repartir. Ninguna página SHALL
referenciar una hoja cuyo nombre sea el de **otro** código: una página que carga `404.css` hace
que el error se presente con el nombre —y la apariencia— de otro, que es el defecto que este requisito
corrige. La diferencia de aspecto por código, si se quiere, se expresa con una clase en el elemento
`body`, no duplicando el fichero.

El **fichero** conserva el marcador; la página **servida** no: el marcador se sustituye al servir por la
hoja del tema elegido en un bloque `<style>` (requisito «El HTML servido es autocontenido»). El marcador es,
además, el único dato que el render necesita para saber **dónde** incrustar la hoja.

#### Scenario: ninguna página carga la hoja de otro código
- **GIVEN** la página de `content/`
- **WHEN** se revisan sus `href` de hojas de estilos
- **THEN** SHALL apuntar a `css/error.css`
- **AND** NO SHALL apuntar a `css/<código>.css`

#### Scenario: la hoja compartida existe y resuelve
- **GIVEN** que la página referencia `css/error.css`
- **WHEN** se comprueba el directorio de contenido
- **THEN** `content/css/error.css` SHALL existir

#### Scenario: cada enlace local resuelve
- **GIVEN** la página de `content/`
- **WHEN** se extraen sus `href` que no salen a otro host
- **THEN** cada ruta SHALL existir como fichero dentro de `CONTENT_DIR`

#### Scenario: la página embebida y su hoja se sirven juntas
- **GIVEN** un servicio arrancado sin ningún fichero en `CONTENT_DIR`
- **WHEN** se pide la página del `404` en HTML y, después, la hoja que el tema usa
- **THEN** el cuerpo SHALL llevar esa hoja incrustada en un bloque `<style>`, de modo que la página se sirva completa y no pida ningún fichero para verse
- **AND** el cuerpo NO SHALL llevar ningún `<link rel="stylesheet">`
- **AND** la petición de `GET /css/error.css` SHALL seguir respondiendo `200` con `Content-Type` `text/css`
- **AND** esa respuesta NO SHALL ser una página de error

### Requirement: Cada página se identifica con la clase de su código

El elemento `body` de la página SHALL llevar la clase `c<código>` del **código resuelto** —`c404`, `c451`—, de modo que la apariencia pueda diferenciarse por código con una sola hoja compartida y sin duplicarla. La clase SHALL quedar garantizada por la **plantilla**, que la rellena al servir con el código resuelto, y no por once ficheros que la escriben a mano: así la clase del cuerpo y el estado NO SHALL poder discrepar.

#### Scenario: la página del 504 se identifica como tal
- **GIVEN** un servicio con la plantilla del proyecto y sin fichero propio para `504`
- **WHEN** se sirve su página HTML
- **THEN** el elemento `body` SHALL llevar la clase `c504`

#### Scenario: la clase del cuerpo es la del código pedido aunque no tenga fichero propio
- **GIVEN** un servicio sin fichero para `451`
- **WHEN** se sirve su página HTML
- **THEN** el `body` SHALL llevar la clase `c451`

### Requirement: Ningún estado de error del servidor de ficheros se escapa

Cuando el servidor de ficheros responda con un estado de error —cualquiera de `4xx` o `5xx`, no solo el `404`—, la aplicación SHALL sustituir esa respuesta por la página de error, con cuerpo propio y con sus cabeceras.

El estado de la página SHALL ser **siempre** el código resuelto por la cascada —path, cabecera `X-Code` o defecto— y NO SHALL ser el estado que devolvió el servidor de ficheros. El estado del servidor de ficheros no es una de las fuentes de esa cascada, y conservarlo rompía tres cosas a la vez, todas medidas con el binario delante: obligaba a propagar cabeceras que describen *su* respuesta —`Allow` en el `405`, `Content-Range` en el `416`— so pena de incumplir un `MUST` de RFC 9110; dejaba el estado y el cuerpo en desacuerdo, sirviendo un `404` con `Retry-After: 120` y el cuerpo del `503`; y, con `DEFAULT_ERROR_PAGE=503`, convertía cualquier asset inexistente en un `404`, dejando además sin efecto a `X-Code`, que existe justo para los despliegues que no reescriben el path.

Por eso el estado SHALL derivarse del código resuelto, y el `Retry-After` —que describe el estado que se sirve, no el código de un cuerpo ajeno— SHALL corresponder al estado de la respuesta: `429` y `503` SHALL llevarlo, el resto NO SHALL incluirlo.

El motivo del cambio SHALL quedar escrito: el servidor de ficheros responde `500` —vacío y sin cabeceras— cuando un segmento del path excede el límite de nombre del sistema de ficheros, y permitir fabricar `5xx` con una URL larga es ruido de alertas a coste cero.

#### Scenario: un segmento de path imposible da la página, no un 500 vacío
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /` con un segmento de 300 caracteres
- **THEN** el estado SHALL ser el código resuelto —el defecto, `404`—
- **AND** el cuerpo NO SHALL estar vacío
- **AND** la respuesta SHALL llevar `X-Robots-Tag`

#### Scenario: un método no admitido sobre un asset da la página del código resuelto
- **GIVEN** un asset existente en el directorio de contenido
- **WHEN** se pide ese asset con un método que el servidor de ficheros no admite
- **THEN** el estado SHALL ser el código resuelto y NO SHALL ser `405`
- **AND** el cuerpo SHALL ser la página de error
- **AND** la respuesta SHALL llevar `X-Robots-Tag`

#### Scenario: un range no satisfacible da la página del código resuelto
- **GIVEN** un asset existente en el directorio de contenido
- **WHEN** se pide con varios rangos a la vez
- **THEN** el estado SHALL ser el código resuelto y NO SHALL ser `416`
- **AND** la respuesta SHALL llevar `X-Robots-Tag`

#### Scenario: un asset inexistente sigue la cascada y no el estado del servidor de ficheros
- **GIVEN** un servicio en marcha
- **WHEN** se pide un asset que no existe indicando el código con `X-Code: 503`
- **THEN** el estado SHALL ser `503`
- **AND** la respuesta SHALL llevar `Retry-After: 120`

#### Scenario: el defecto configurado manda sobre el not found del servidor de ficheros
- **GIVEN** un servicio arrancado con `DEFAULT_ERROR_PAGE=503`
- **WHEN** se pide un asset que no existe
- **THEN** el estado SHALL ser `503` y NO SHALL ser `404`

#### Scenario: el Retry-After corresponde al estado servido
- **GIVEN** un servicio en marcha
- **WHEN** se sirve una página de error con un código distinto de `429` y `503`
- **THEN** la respuesta NO SHALL incluir `Retry-After`

#### Scenario: el cuerpo y su formato siguen la cascada en una respuesta interceptada
- **GIVEN** un asset existente en el directorio de contenido
- **WHEN** se pide con un método que el servidor de ficheros no admite y con `Accept: text/html`
- **THEN** el `Content-Type` SHALL ser HTML
- **AND** el cuerpo SHALL ser la página HTML del código resuelto

#### Scenario: un asset sigue sirviéndose con 200
- **GIVEN** un asset existente en el directorio de contenido
- **WHEN** se pide con `GET`
- **THEN** el estado SHALL ser `200`
- **AND** NO SHALL ser una página de error

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
cual** en lo que a la hoja respecta: sin añadir ni sustituir el marcador, y sin que el servicio lea hoja
alguna para servirla. Un `<link>` que **no** sea el marcador NO SHALL tocarse: es del operador. La
sustitución de los **marcadores de plantilla** (`{{code}}`, `{{reason}}`, `{{description}}`) es un paso
**aparte**, con su propia regla —sin marcadores, **byte a byte**—, y vive en el requisito «El cuerpo
depende del formato y el código es ortogonal»: una página sin el marcador de la hoja pero con un marcador
de plantilla SHALL tener ese marcador rellenado.

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
- **GIVEN** un `CONTENT_DIR` cuya página `404.html` no lleva el marcador ni ningún marcador de plantilla
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
ser **una sola hoja de estilos**: añadir un tema es añadir un fichero, no duplicar la página. El
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
de error. La regla no prohíbe el adorno: prohíbe el adorno que no se puede apagar. Y el mismo
principio rige para los **comentarios**: una hoja que mencione `animation` **solo dentro de un
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
