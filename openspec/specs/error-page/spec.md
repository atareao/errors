# error-page Specification

## Purpose
El contrato que hace útil a este servicio detrás de Traefik: cómo se resuelve el código de error —el path, la cabecera `X-Code` o el defecto configurado— y cómo se negocia el formato de la respuesta —HTML, JSON, XML o texto plano—, con la garantía que justifica el proyecto: la respuesta lleva el **código de error real y nunca un 200**. Fija además el rango de códigos aceptados (`400..=599`), la normalización del path que impide que una página de error se sirva como asset, las cabeceras que acompañan a toda página y el respaldo cuando falta el HTML.

## Requirements

### Requirement: El código se resuelve en cascada

El código de error SHALL resolverse aplicando el primer origen que haga match, en este orden: (1) el path —`/404`, `/404.html`, `/404.json`, `/503.xml`, `/404.txt`—, del que SHALL extraerse el entero del primer segmento; (2) la cabecera `X-Code`; (3) el código por defecto configurado en `DEFAULT_ERROR_PAGE`. Un path cuyo primer segmento no sea un entero válido NO SHALL resolver código por esa vía.

El clasificador SHALL tratar además `/default.html` y `/default.htm` como una petición del **código por defecto** configurado (`DEFAULT_ERROR_PAGE`, `404` de fábrica), igual que `/404.html` se trata como el código 404. NO SHALL servirse como asset. El alias SHALL aplicarse **después** de la normalización del path, de modo que `/./default.html` y `/default%2Ehtml` se resuelvan también como el código por defecto. El alias es del **código**, no de la página: `/default.html` SHALL renderizarse con la página del tema elegido. El motivo SHALL quedar escrito: el path es el origen más explícito y quien escribe `default.html` está pidiendo el código por defecto, no un asset; y sin el alias ese fichero se serviría crudo, con los marcadores a la vista. El origen del código para métricas SHALL ser `Path` —el path lo pide explícitamente—, no `Default`.

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

#### Scenario: el path default.html se resuelve como el código por defecto
- **GIVEN** un servicio con el defecto `404`
- **WHEN** se pide `GET /default.html`
- **THEN** el código resuelto SHALL ser `404`
- **AND** NO SHALL servirse como asset
- **AND** el origen del código para métricas SHALL ser `Path`

#### Scenario: default.htm también es el código por defecto
- **GIVEN** un servicio con `DEFAULT_ERROR_PAGE=503`
- **WHEN** se pide `GET /default.htm`
- **THEN** el código resuelto SHALL ser `503`

#### Scenario: default.html gana a X-Code
- **GIVEN** una petición a `GET /default.html` con cabecera `X-Code: 503`
- **WHEN** se resuelve el código, con defecto `404`
- **THEN** el código resuelto SHALL ser `404`
- **AND** NO SHALL ser `503`

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

El motivo SHALL quedar escrito: el servidor de ficheros decodifica y normaliza el path, así que clasificar sobre el path crudo deja pasar la página de error como si fuera un asset, servida con `200`. Es exactamente el fallo que este proyecto corrige. El alias `/default.html`/`/default.htm` SHALL aplicarse **después** de esta normalización, de modo que sus formas equivalentes resuelvan también el código por defecto y no se sirvan crudas.

#### Scenario: un componente `.` no esquiva la página de error
- **GIVEN** que el tema por defecto tiene su página (`content/default.html` o la copia embebida)
- **WHEN** se pide `GET /./404.html`
- **THEN** el estado SHALL ser `404`
- **AND** NO SHALL ser `200`

#### Scenario: un path percent-codificado no esquiva la página de error
- **GIVEN** que el tema por defecto tiene su página
- **WHEN** se pide `GET /404%2Ehtml`
- **THEN** el estado SHALL ser `404`

#### Scenario: no se puede salir del directorio de contenido
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /css/../../Cargo.toml`
- **THEN** el estado SHALL estar en `400..=599`
- **AND** el cuerpo NO SHALL contener el contenido de `Cargo.toml`

#### Scenario: el alias se resuelve después de normalizar
- **GIVEN** un servicio con el defecto `404`
- **WHEN** se pide `GET /./default.html`
- **THEN** el código resuelto SHALL ser `404`
- **AND** NO SHALL servirse como asset

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

La excepción SHALL ser `/default.html` y `/default.htm`: su path se clasifica como el código por defecto —requisito «El código se resuelve en cascada»— y NO SHALL servirse como asset. Las **otras seis páginas de tema** —`minimal.html`, `terminal.html`, `paper.html`, `amber.html`, `compact.html` y `matrix.html`— NO SHALL interceptarse: son assets y se sirven como tales si se piden por su path. El motivo SHALL quedar escrito: interceptar seis paths rompería la regla «un asset existente gana» y añadiría seis casos especiales al middleware; `default.html` es la única excepción porque su path es el alias del código por defecto.

El leak de las seis páginas es cosmético —no hay secretos, solo una plantilla sin rellenar— pero con las páginas en la raíz de `CONTENT_DIR` es **inevitable**: viven en el mismo directorio que se sirve como assets. La mitigación SHALL ser **externa**, en el proxy (bloquear `*.html` salvo `/default.html`); el servicio NO SHALL prometer que las oculta.

#### Scenario: un asset con X-Code sigue siendo un asset
- **GIVEN** que `content/css/404.css` existe
- **WHEN** se pide `GET /css/404.css` con `X-Code: 503`
- **THEN** SHALL responder `200`
- **AND** el `Content-Type` SHALL ser `text/css`

#### Scenario: default.html no es un asset: es el código por defecto
- **GIVEN** un `CONTENT_DIR` con `content/default.html`
- **WHEN** se pide `GET /default.html` con `Accept: text/html` y `THEME=default`
- **THEN** el estado SHALL ser el código por defecto (`404` por defecto)
- **AND** NO SHALL responder `200` como asset
- **AND** el cuerpo SHALL ser la página del tema `default`, con el código por defecto rellenado

#### Scenario: las seis páginas de tema se sirven crudas por su path
- **GIVEN** un `CONTENT_DIR` con `matrix.html`
- **WHEN** se pide `GET /matrix.html`
- **THEN** el estado SHALL ser `200`
- **AND** el cuerpo SHALL ser el fichero tal cual, con los marcadores a la vista

### Requirement: Un fallo al leer el HTML no cambia el estado

La cascada del cuerpo HTML SHALL ser **un solo paso**: la página del tema. Un fichero **legible** en `CONTENT_DIR/<nombre>.html` SHALL ganar a la copia embebida; un fichero **ausente** SHALL caer a la copia embebida **sin** dejar constancia en el registro; y un fichero **presente pero ilegible** SHALL ser un fallo de E/S (`RenderError::Io`). NO SHALL existir capa por código: `CONTENT_DIR/<código>.html` y `CONTENT_DIR/<código por defecto>.html` NO SHALL consultarse.

Cuando la página no se pueda leer, la aplicación SHALL responder el **código resuelto** con el cuerpo del formato resuelto —el respaldo de texto— y NO SHALL responder `500`. El respaldo NO SHALL retirarse.

Un fichero **ausente** NO SHALL llegar al respaldo: la copia embebida cubre cualquier código de `400..=599`, así que ningún `DEFAULT_ERROR_PAGE` legal se queda sin página. El respaldo queda reservado al fallo de **E/S** —una página del tema que existe y no se puede leer—, y el motivo SHALL quedar escrito: la página es **contenido**, y decidir que un directorio del operador roto se tapa en silencio con la copia embebida ocultaría el fallo; el `warn!` lo delata y el estado no cambia.

#### Scenario: sin fichero HTML se mantiene el código
- **GIVEN** un `CONTENT_DIR` sin ningún fichero HTML y con el defecto `418`
- **WHEN** se pide `GET /418.html`
- **THEN** el estado SHALL ser `418`
- **AND** NO SHALL ser `500`
- **AND** el cuerpo SHALL ser la página embebida del tema rellenada con `418`

#### Scenario: el respaldo cubre también un código distinto del defecto
- **GIVEN** un `CONTENT_DIR` sin ningún fichero HTML y con el defecto `418`
- **WHEN** se pide `GET /599.html`
- **THEN** el estado SHALL ser `599`
- **AND** NO SHALL ser `500`

#### Scenario: una página del operador ilegible no tumba la respuesta
- **GIVEN** un `CONTENT_DIR` cuyo `default.html` existe y no se puede leer, y el defecto `404`
- **WHEN** se pide `GET /404.html`
- **THEN** el estado SHALL ser `404`
- **AND** el cuerpo SHALL ser el respaldo de texto del código resuelto
- **AND** NO SHALL ser `500`

#### Scenario: la página del tema ausente cae a la copia embebida sin dejar constancia
- **GIVEN** un `CONTENT_DIR` sin `default.html` y con el tema por defecto
- **WHEN** se pide una página HTML
- **THEN** el cuerpo SHALL ser la página embebida del tema
- **AND** NO SHALL quedar constancia en el registro de que el fichero faltaba

### Requirement: Los códigos que un proxy emite tienen su propia página

Los códigos que un proxy emite con normalidad —`400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`, `502`, `503` y `504`— SHALL tener una página HTML, y SHALL tenerla **cualquier** otro código del rango `400..=599`: la **página embebida del tema**, rellenada con el código resuelto, de modo que un servicio instalado sin ficheros de datos sirva su propia página para todos ellos.

Cada tema SHALL entregar **una sola** página —la suya—, no una por código, y esa página SHALL cubrir cualquier código: el código, la razón y la descripción SHALL ser marcadores de plantilla, no valores escritos a mano. El motivo SHALL quedar escrito: un fichero por código repetiría lo que el servidor ya sabe y un código sin fichero propio recibiría el cuerpo de **otro**; y **siete** páginas —una por tema— es la misma corrección aplicada al eje del tema.

`CONTENT_DIR` PUEDE contener la página de un tema (`CONTENT_DIR/<nombre>.html`), que es el *override* **opcional** del operador y gana a la copia embebida: el proyecto NO SHALL exigirla, y su ausencia NO SHALL degradar ninguna página. NO SHALL existir el *override* por código.

Las **siete** páginas embebidas SHALL ser idénticas, byte a byte, a los ficheros del repositorio de los que se embeben: las dos copias NO SHALL poder divergir en silencio.

La página HTML de un código SHALL identificar ese código en su título o en su cuerpo, de modo que el cuerpo nunca contradiga al estado, y esto SHALL cumplirse tanto en la copia embebida como en el *override* del operador.

Ningún código del rango SHALL depender del respaldo de texto: SHALL bastar la página embebida del tema. El respaldo NO SHALL retirarse, pero queda reservado al caso de un fichero de `CONTENT_DIR` que **exista y no se pueda leer**.

#### Scenario: el 504 tiene página propia y la nombra
- **GIVEN** un servicio con la página del proyecto y sin fichero propio para `504`
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
- **GIVEN** los siete ficheros de página del árbol del repositorio y sus copias embebidas en el binario
- **WHEN** se comparan sus bytes
- **THEN** SHALL ser idénticos

#### Scenario: un código que no está tabulado también tiene página
- **GIVEN** un servicio sin ficheros de datos
- **WHEN** se pide `GET /451` con `Accept: text/html`
- **THEN** el estado SHALL ser `451` y el cuerpo SHALL ser la página del tema rellenada con `451`
- **AND** NO SHALL ser el respaldo de texto

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

### Requirement: El cuerpo HTML es la página del tema

El cuerpo HTML SHALL ser la **página del tema** elegido, en **un solo paso**: la página **legible** de `CONTENT_DIR/<nombre>.html` si la hay y, si no, su copia **embebida** en el binario. NO SHALL existir capa por código: `CONTENT_DIR/<código>.html` y `CONTENT_DIR/<código por defecto>.html` NO SHALL consultarse. `DEFAULT_ERROR_PAGE` SHALL conservar su significado —el código que se sirve cuando ninguna fuente aporta uno utilizable— pero ya NO SHALL nombrar ningún fichero. La diferencia de aspecto por código SHALL expresarse con la clase `c<código>` del `body` y con el CSS de la propia página.

El motivo SHALL quedar escrito: el repositorio **entrega** las páginas de los siete temas, así que una capa por código del operador quedaría **siempre sombreada** —y una capa que nunca se lee es una mentira del contrato—; el diseño por código tiene un sitio mejor, la clase del `body`, que además no puede discrepar del estado. El precio SHALL quedar escrito: un operador con su `content/404.html` que ponga `THEME=matrix` verá la página del tema, no la suya; su punto de personalización es la página del tema.

La página SHALL ser **una plantilla**: el código, la razón canónica y la descripción SHALL ser marcadores (`{{code}}`, `{{reason}}`, `{{description}}`) que el servidor SHALL sustituir **al servir** por el código resuelto y por lo que devuelven `canonical_reason` y `description`. Una página **sin** marcadores SHALL servirse **byte a byte**, sin tocar el disco; SHALL sustituirse **cada** aparición, no solo la primera; y el escaneo SHALL ser **por bytes**, porque una página del operador puede no estar en UTF-8. Lo insertado SHALL ir **escapado** para HTML (`&`, `<`, `>`): lo escribe el servidor, no un humano dentro del fichero.

El **código resuelto** NO SHALL depender del formato ni del origen del cuerpo. Los cuerpos JSON, XML y texto SHALL generarse a partir del código, su razón canónica y su descripción, y SHALL **ignorar** el tema. El respaldo de texto de una página ilegible está fijado en «Un fallo al leer el HTML no cambia el estado».

#### Scenario: la página embebida se sirve sin directorio de contenido
- **GIVEN** un servicio cuyo `CONTENT_DIR` no contiene ningún fichero
- **WHEN** se pide `GET /503.html`
- **THEN** el estado SHALL ser `503`
- **AND** el cuerpo SHALL ser la página embebida del tema rellenada con `503`
- **AND** el `Content-Type` SHALL ser HTML

#### Scenario: el fichero del directorio gana a la copia embebida
- **GIVEN** un `CONTENT_DIR` con su propio `default.html`, distinto del embebido
- **WHEN** se pide `GET /404.html` con el tema por defecto
- **THEN** el cuerpo SHALL ser el de `CONTENT_DIR/default.html`
- **AND** NO SHALL ser la página embebida rellenada con `404`

#### Scenario: una página sin marcadores se sirve byte a byte
- **GIVEN** un `CONTENT_DIR` cuya página `default.html` no lleva ningún marcador de plantilla
- **WHEN** se pide `GET /404` en HTML
- **THEN** el cuerpo SHALL ser esa página **byte a byte**, sin añadidos ni sustituciones

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

#### Scenario: un código sin página propia recibe su propio cuerpo
- **GIVEN** un servicio con la página del proyecto y sin fichero para `451`
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

#### Scenario: el código por defecto ya no nombra ningún fichero
- **GIVEN** un `CONTENT_DIR` con un `404.html` —nombre de código— y sin `default.html`
- **WHEN** se pide `GET /404` con el tema por defecto
- **THEN** el cuerpo SHALL ser la copia embebida del tema, no `404.html`
- **AND** el `404.html` del operador NO SHALL leerse

### Requirement: Las páginas del proyecto se descubren al compilar

El conjunto de páginas del proyecto SHALL descubrirse al compilar: `build.rs` lee el directorio `content/`, filtra los ficheros `*.html` cuyo tallo sea UTF-8 no vacío y los ordena por nombre para que el build sea determinista. El binario SHALL embeber lo que haya, sin lista ni recuento fijo en el código. Si `content/` no existe o no contiene ningún `.html`, la compilación SHALL abortar con un mensaje claro: el crate siempre lleva sus temas.

El conjunto de páginas embebidas NO SHALL fijarse en el código: su número y sus nombres SHALL derivarse de los ficheros presentes en `content/` al compilar. Añadir un tema SHALL ser dejar un fichero `*.html` en `content/` y reconstruir. NO SHALL verificarse el contenido de ninguna página al compilar ni al servir: las páginas son opacas y el servicio no las juzga. El motivo de esta decisión SHALL quedar escrito en `design.md`.

#### Scenario: añadir un tema es dejar el fichero y reconstruir
- **GIVEN** el directorio `content/` con sus páginas
- **WHEN** se añade un `nuevo.html` y se reconstruye el binario
- **THEN** el tema `nuevo` SHALL estar disponible sin editar ninguna lista
- **AND** su página SHALL ser la de `content/nuevo.html`

#### Scenario: sin content/ la compilación aborta
- **GIVEN** un árbol sin el directorio `content/`
- **WHEN** se compila el crate
- **THEN** la compilación SHALL abortar con un mensaje que nombre el directorio
- **AND** el binario NO SHALL generarse

#### Scenario: content/ sin html aborta
- **GIVEN** un `content/` sin ningún `.html`
- **WHEN** se compila el crate
- **THEN** la compilación SHALL abortar con un mensaje que explique que el crate siempre lleva sus temas

### Requirement: El tema del HTML elige una página del registro

Un **tema** SHALL ser **una página**: su **nombre** SHALL ser el de su fichero y la ruta SHALL derivarse de él —`CONTENT_DIR/<nombre>.html`, en la **raíz** de `CONTENT_DIR`—, de modo que no exista una tabla de rutas que pueda divergir del nombre. El conjunto de páginas SHALL descubrirse al compilar desde `content/*.html` (requisito «Las páginas del proyecto se descubren al compilar»): un servicio arrancado sin `content/` sirve los diseños embebidos. El motivo SHALL quedar escrito: el usuario rechazó el modelo «tema = solo CSS», porque un tema que quiere entregar una experiencia —su marcado, su `<script>`, su `<canvas>`— no cabe en una hoja, y una página por tema es mantener los diseños, no uno por código.

La página de un tema SHALL resolverse **como un asset**: un fichero **legible** en `CONTENT_DIR/<nombre>.html` SHALL ganar a la copia embebida; un fichero **ausente** SHALL caer a la copia embebida **sin** dejar constancia en el registro; un fichero **presente pero ilegible** SHALL dejar un `warn!` que nombre el fichero y su causa y SHALL responder el respaldo de texto con el estado intacto (requisito «Un fallo al leer el HTML no cambia el estado»). La página se lee **al servir**, así que un operador que edite la suya la ve cambiar sin reiniciar.

El **punto de personalización** del operador SHALL ser la página del tema: `content/default.html` para el tema por defecto. El operador NO SHALL conservar la capa por código —la pierde con este modelo— y NO SHALL tener una hoja suelta que incrustar.

El **nombre** de un tema descubierto SHALL ser el **tallo** de su fichero, y la comparación de nombres SHALL seguir siendo **exacta**. Las **reglas** de cada página PUEDEN cambiar de una versión a otra: son del proyecto y su aspecto no es un contrato; lo que sí lo es, y SHALL comprobarse, es el **nombre** del tema y la **ruta** de su página.

Un tema **descubierto** NO SHALL tener copia embebida: su fichero es su **única** copia. Si en el momento de servir ese fichero ha **desaparecido**, la respuesta SHALL ser el **respaldo de texto** con el estado intacto y SHALL dejar constancia nombrando el fichero —al revés que un tema embebido ausente, que cae a su copia embebida **sin** dejar constancia—. El motivo SHALL quedar escrito: sin copia a la que caer, el silencio escondería un fichero que se ha esfumado.

El nombre que llega en **`X-Theme` NO SHALL usarse nunca para componer una ruta**. La ruta SHALL salir del **registro**, que se construye del barrido del directorio. El motivo SHALL quedar escrito: si se implementara como `content_dir.join(format!("{}.html", x_theme))`, un `X-Theme: ../../etc/passwd` sería una lectura arbitraria de ficheros.

La variable de entorno `THEME` SHALL elegir el tema del servicio, con `default` por defecto. Un valor que no sea uno de los nombres del **registro**, **o la cadena vacía**, SHALL abortar el arranque con un error que **nombre `THEME`** y que **enumere los nombres del registro** —que con un `CONTENT_DIR` vacío son los embebidos—, como `LOG_FORMAT`: es una variable que escribe el operador, y quien la escribe cree que está configurando algo.

La cabecera de petición `X-Theme` SHALL poder elegir el tema **por petición**, con la precedencia de siempre —el **primer** origen que haga match, así que un `X-Theme` conocido gana a `THEME`—. Un `X-Theme` desconocido, ausente o vacío NO SHALL ser un error: cae al tema configurado, porque esa cabecera la elige quien llama y un cliente no puede romper el servicio. La comparación de nombres SHALL ser **exacta**. `X-Theme` NO SHALL afectar a los formatos que no son HTML: sin página que incrustar, el tema se ignora.

El cuerpo de la respuesta depende de `X-Theme`, así que esa cabecera SHALL declararse en `Vary` (spec `response-extras`, «Las cabeceras de correlación se declaran en Vary»).

#### Scenario: sin THEME el tema es el de siempre
- **GIVEN** un entorno sin `THEME`
- **WHEN** se sirve una página HTML
- **THEN** la página SHALL ser `content/default.html` o, si no la hay, su copia embebida

#### Scenario: THEME cambia el tema de todo el servicio
- **GIVEN** `THEME=terminal`
- **WHEN** se pide `GET /404` con `Accept: text/html`
- **THEN** la página SHALL ser la del tema `terminal`
- **AND** NO SHALL ser la del tema `default`

#### Scenario: un THEME desconocido no arranca
- **GIVEN** `THEME=neon`
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar con un error que nombre `THEME`
- **AND** el mensaje SHALL enumerar los nombres del registro

#### Scenario: un THEME vacío no arranca
- **GIVEN** `THEME` presente y vacía
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar, en vez de usar `default`

#### Scenario: X-Theme elige el tema de esa petición
- **GIVEN** un servicio con el tema `default`
- **WHEN** se pide `GET /404` con `X-Theme: paper`
- **THEN** la página servida SHALL ser la del tema `paper`

#### Scenario: un X-Theme desconocido cae al tema configurado
- **GIVEN** `THEME=minimal` y una petición con `X-Theme: neon`
- **WHEN** se responde
- **THEN** la página servida SHALL ser la de `minimal`
- **AND** la respuesta NO SHALL ser un error

#### Scenario: un X-Theme que no puede leerse no impide servir la página
- **GIVEN** un tema cuya página en `CONTENT_DIR` existe y no se puede leer
- **WHEN** se pide una página HTML de ese código
- **THEN** el estado SHALL ser el código resuelto
- **AND** el cuerpo SHALL ser el respaldo de texto del código resuelto
- **AND** NO SHALL ser `500`

#### Scenario: X-Theme no afecta al JSON
- **GIVEN** una petición `GET /404` con `Accept: application/json` y `X-Theme: terminal`
- **WHEN** se responde
- **THEN** el cuerpo SHALL ser el JSON de siempre

#### Scenario: el conjunto de temas se descubre al compilar
- **GIVEN** un árbol con `content/` y sus ficheros `*.html`
- **WHEN** se compila el binario
- **THEN** el conjunto de temas SHALL derivarse de los ficheros presentes en `content/`
- **AND** NO SHALL existir una lista ni un recuento fijo en el código
- **AND** añadir un `nuevo.html` a `content/` y reconstruir SHALL añadir el tema `nuevo` al conjunto

#### Scenario: cada tema declara su página y el tallo del fichero es su nombre
- **GIVEN** el registro de temas embebidos al compilar
- **WHEN** se comprueba la ruta de página de cada tema
- **THEN** la ruta SHALL ser `<nombre>.html`
- **AND** cada tema SHALL tener un fichero en `content/` con ese nombre

#### Scenario: un fichero de más en CONTENT_DIR es un tema
- **GIVEN** un `CONTENT_DIR` con `neon.html` y `THEME=neon`
- **WHEN** se pide `GET /404` con `Accept: text/html`
- **THEN** la página SHALL ser la de `CONTENT_DIR/neon.html`
- **AND** NO SHALL ser la del tema por defecto

#### Scenario: un tema descubierto se elige por X-Theme
- **GIVEN** un servicio con el tema `default` y un `CONTENT_DIR` con `neon.html`
- **WHEN** se pide `GET /404` con `X-Theme: neon`
- **THEN** la página SHALL ser la de `neon`

#### Scenario: el fichero de un tema descubierto desaparecido responde el respaldo
- **GIVEN** un tema descubierto cuyo fichero se borra entre el arranque y la petición
- **WHEN** se pide una página HTML de ese tema
- **THEN** el estado SHALL ser el código resuelto
- **AND** el cuerpo SHALL ser el respaldo de texto
- **AND** NO SHALL ser `500`

#### Scenario: el nombre de X-Theme no compone ninguna ruta
- **GIVEN** una petición con `X-Theme: ../../etc/passwd`
- **WHEN** se responde
- **THEN** la página SHALL ser la del tema configurado
- **AND** NO SHALL leerse ningún fichero fuera de `CONTENT_DIR`

### Requirement: Cada página de tema se identifica con la clase de su código

El elemento `body` de cada página de tema PUEDE llevar la clase `c<código>` del **código resuelto** —`c404`, `c451`—, de modo que la apariencia pueda diferenciarse por código con el CSS de la propia página y sin duplicarla. La clase SHALL quedar garantizada por la **plantilla**, que la rellena al servir con el código resuelto. NO SHALL verificarse el contenido de las páginas: las páginas PUEDEN llevar los marcadores o no. El motivo SHALL quedar escrito en `design.md`: las páginas son opacas y el servicio no las juzga.

#### Scenario: la página del 504 se identifica como tal
- **GIVEN** un servicio con la página del proyecto y sin fichero propio para `504`
- **WHEN** se sirve su página HTML
- **THEN** el elemento `body` SHALL llevar la clase `c504`

#### Scenario: la clase del cuerpo es la del código pedido aunque no tenga fichero propio
- **GIVEN** un servicio sin fichero para `451`
- **WHEN** se sirve su página HTML
- **THEN** el `body` SHALL llevar la clase `c451`

#### Scenario: cada página se identifica con su clase
- **GIVEN** las páginas de `content/`
- **WHEN** se sirve cada una con un código resuelto
- **THEN** el `body` de cada una SHALL llevar la clase `c<código>` del código resuelto
- **AND** NO SHALL haber una página sin la clase

### Requirement: El directorio es el registro

Al arrancar, el servicio SHALL construir el **registro efectivo** de temas: los **embebidos** al compilar desde `content/*.html` **más** los **descubiertos** en la **raíz** de `CONTENT_DIR`. Los embebidos NO SHALL retirarse: un `cargo install` sigue sirviendo los diseños sin ficheros de datos, y el directorio **añade**, no sustituye.

SHALL descubrirse un fichero **regular** —o un enlace simbólico que apunte a uno legible— cuya extensión sea **exactamente** `html` (ni `HTML`, ni `htm`, ni `css`). El nombre del tema SHALL ser su **tallo** (`neon.html` → `neon`), y **NO SHALL** descubrirse nada en **subdirectorios**: el nombre de un tema no es una ruta. El motivo de mirar solo la raíz SHALL quedar escrito: un tema es **una página** con un nombre, y admitir rutas convertiría el nombre en una ruta que el registro tendría que validar.

Un fichero cuyo tallo sea el de un **embebido** NO SHALL añadir un tema: SHALL ser la página de ese tema en el disco, que es la regla de siempre —un fichero legible gana a la copia embebida—. Así `content/default.html` **no** crea un `default` de más, sino que es la página del `default` embebido.

Un fichero cuyo tallo **reclame la resolución de códigos** NO SHALL ser un tema: hoy eso es **cualquier tallo que se pueda leer como un `u16`** —`404` y `451`, pero también `999`, `200` o `0`—, porque el clasificador decide con `code_from_path`, que **parsea el número y NO filtra el rango**: el `400..=599` se aplica al **resolver** el código, no al **clasificar** el path. Su path, por tanto, NO SHALL servirse como asset: `/404.html` responde `404`, y `/999.html` y `/200.html` responden el **código por defecto**, que es una página de error **distinta** de la suya. El servicio SHALL dejarlo fuera del registro y SHALL dejar constancia nombrando el fichero, para que el operador sepa por qué su `404.html` no es un tema.

Un `CONTENT_DIR` que **no exista** SHALL dar un registro con los embebidos y **sin** error: el binario sin ficheros de datos sirve los diseños, y el arranque NO SHALL fallar. Un `CONTENT_DIR` que exista y **no se pueda listar** SHALL dejar constancia nombrando el directorio y su causa, y el registro SHALL quedarse con los embebidos: NO SHALL impedir el arranque. El reparto es el mismo que el de las páginas: lo que falta no tumba el servicio, lo que existe y no se puede usar se delata.

El barrido SHALL ocurrir **una vez, al arrancar**. Un fichero añadido con el servicio en marcha NO SHALL ser un tema hasta el siguiente arranque, y SHALL seguir sirviéndose como **asset crudo** con `200` —la regla de los ficheros de `CONTENT_DIR`—. Los **bytes** de la página SHALL seguir leyéndose **al servir**, así que **editar** el contenido de un tema SHALL verse sin reiniciar. El motivo SHALL quedar escrito: validar `THEME` exige una lista **en el arranque**, y un barrido por petición pondría un `read_dir` en el camino de cada página de error, que es justo cuando más se piden.

El servicio NO SHALL inspeccionar el contenido de ninguna página: las páginas son opacas, tanto las embebidas como las descubiertas. NO SHALL haber guardas de red, movimiento ni cobertura de marcado. El motivo SHALL quedar escrito en `design.md`.

#### Scenario: un fichero de más es un tema
- **GIVEN** un `CONTENT_DIR` con `neon.html` y un registro con los embebidos
- **WHEN** se construye el registro al arrancar
- **THEN** `neon` SHALL estar entre los nombres del registro

#### Scenario: un fichero con el nombre de un embebido no añade un tema
- **GIVEN** un `CONTENT_DIR` con `default.html`
- **WHEN** se construye el registro
- **THEN** SHALL haber **un solo** `default`
- **AND** su página SHALL ser la del disco, no la copia embebida

#### Scenario: un fichero con nombre de código no es un tema
- **GIVEN** un `CONTENT_DIR` con `404.html`, `999.html` y `200.html`
- **WHEN** se construye el registro
- **THEN** `404`, `999` y `200` NO SHALL estar entre los nombres del registro
- **AND** SHALL dejar constancia nombrando los ficheros

#### Scenario: solo se mira la raíz de CONTENT_DIR
- **GIVEN** un `CONTENT_DIR` con `temas/neon.html`
- **WHEN** se construye el registro
- **THEN** `neon` NO SHALL ser un tema
- **AND** el fichero SHALL seguir sirviéndose como asset

#### Scenario: la extensión tiene que ser html
- **GIVEN** un `CONTENT_DIR` con `neon.htm`, `neon.HTML` y `neon.css`
- **WHEN** se construye el registro
- **THEN** ninguno de los tres SHALL ser un tema

#### Scenario: sin directorio de contenido el registro son los embebidos
- **GIVEN** un `CONTENT_DIR` que no existe
- **WHEN** se construye el registro
- **THEN** los nombres SHALL ser los embebidos
- **AND** el arranque NO SHALL fallar

#### Scenario: un directorio ilegible no impide el arranque
- **GIVEN** un `CONTENT_DIR` que existe y no se puede listar
- **WHEN** se construye el registro
- **THEN** el registro SHALL quedarse con los embebidos
- **AND** SHALL dejar constancia nombrando el directorio y su causa
- **AND** el arranque NO SHALL fallar

#### Scenario: añadir un tema con el servicio en marcha no lo convierte en tema
- **GIVEN** un servicio arrancado sin `neon.html` en `CONTENT_DIR`
- **WHEN** se crea el fichero con el servicio en marcha
- **THEN** `THEME=neon` NO SHALL ser válido hasta reiniciar
- **AND** `GET /neon.html` SHALL seguir siendo un asset crudo con `200`
