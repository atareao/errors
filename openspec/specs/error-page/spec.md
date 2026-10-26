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

### Requirement: Cada página se identifica con la clase de su código

El elemento `body` de cada una de las **siete páginas de tema** SHALL llevar la clase `c<código>` del **código resuelto** —`c404`, `c451`—, de modo que la apariencia pueda diferenciarse por código con el CSS de la propia página y sin duplicarla. La clase SHALL quedar garantizada por la **plantilla**, que la rellena al servir con el código resuelto, y no por siete ficheros que la escriben a mano: así la clase del cuerpo y el estado NO SHALL poder discrepar.

Las seis páginas sobrias y `matrix.html` SHALL compartir el contrato de la clase; `matrix.html` PUEDE añadir sus elementos propios sin romperlo.

#### Scenario: la página del 504 se identifica como tal
- **GIVEN** un servicio con la página del proyecto y sin fichero propio para `504`
- **WHEN** se sirve su página HTML
- **THEN** el elemento `body` SHALL llevar la clase `c504`

#### Scenario: la clase del cuerpo es la del código pedido aunque no tenga fichero propio
- **GIVEN** un servicio sin fichero para `451`
- **WHEN** se sirve su página HTML
- **THEN** el `body` SHALL llevar la clase `c451`

#### Scenario: cada una de las siete páginas se identifica con su clase
- **GIVEN** las siete páginas de `content/`
- **WHEN** se sirve cada una con un código resuelto
- **THEN** el `body` de cada una SHALL llevar la clase `c<código>` del código resuelto
- **AND** NO SHALL haber una página sin la clase

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

### Requirement: El tema del HTML elige una página

Un **tema** SHALL ser **una página**: su **nombre** SHALL ser el de su fichero y la ruta SHALL derivarse de él —`CONTENT_DIR/<nombre>.html`, en la **raíz** de `CONTENT_DIR`—, de modo que no exista una tabla de rutas que pueda divergir del nombre. Las siete páginas SHALL ir **embebidas** en el binario: un servicio arrancado sin `content/` sirve los siete diseños, y borrar una página SHALL ser un error de compilación, no un respaldo silencioso. El motivo SHALL quedar escrito: el usuario rechazó el modelo «tema = solo CSS», porque un tema que quiere entregar una experiencia —su marcado, su `<script>`, su `<canvas>`— no cabe en una hoja, y una página por tema es mantener siete diseños, no uno por código.

La página de un tema SHALL resolverse **como un asset**: un fichero **legible** en `CONTENT_DIR/<nombre>.html` SHALL ganar a la copia embebida; un fichero **ausente** SHALL caer a la copia embebida **sin** dejar constancia en el registro; un fichero **presente pero ilegible** SHALL dejar un `warn!` que nombre el fichero y su causa y SHALL responder el respaldo de texto con el estado intacto (requisito «Un fallo al leer el HTML no cambia el estado»). La página se lee **al servir**, así que un operador que edite la suya la ve cambiar sin reiniciar.

El **punto de personalización** del operador SHALL ser la página del tema: `content/default.html` para el tema por defecto. El operador NO SHALL conservar la capa por código —la pierde con este modelo— y NO SHALL tener una hoja suelta que incrustar.

El conjunto de temas SHALL ser `default`, `minimal`, `terminal`, `paper`, `amber`, `compact` y `matrix`: **siete** nombres, ni uno más ni uno menos. `default` SHALL seguir siendo el tema por defecto. Las **reglas** de cada página PUEDEN cambiar de una versión a otra: son del proyecto y su aspecto no es un contrato; lo que sí lo es, y SHALL comprobarse, es el **nombre** del tema y la **ruta** de su página.

La variable de entorno `THEME` SHALL elegir el tema del servicio, con `default` por defecto. Un valor que no sea uno de los siete, **o la cadena vacía**, SHALL abortar el arranque con un error que **nombre `THEME`** y que **enumere los siete nombres válidos**, como `LOG_FORMAT`: es una variable que escribe el operador, y quien la escribe cree que está configurando algo.

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
- **AND** el mensaje SHALL enumerar los siete nombres válidos

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

#### Scenario: el conjunto de temas son siete nombres
- **GIVEN** el registro de temas incorporados al binario
- **WHEN** se enumeran sus nombres
- **THEN** los nombres SHALL ser `default`, `minimal`, `terminal`, `paper`, `amber`, `compact` y `matrix`
- **AND** `default` SHALL seguir siendo el tema por defecto
- **AND** NO SHALL haber un octavo nombre

#### Scenario: cada tema declara su página y el tallo del fichero es su nombre
- **GIVEN** el registro de temas incorporados al binario
- **WHEN** se comprueba la ruta de página de cada tema
- **THEN** la ruta SHALL ser `<nombre>.html`
- **AND** cada tema SHALL tener un fichero en `content/` con ese nombre
- **AND** las siete páginas SHALL ir embebidas en el binario

### Requirement: Las siete páginas del proyecto

El repositorio SHALL entregar **siete** páginas HTML, una por tema, en la **raíz** de `content/`: `default.html`, `minimal.html`, `terminal.html`, `paper.html`, `amber.html`, `compact.html` y `matrix.html`. El **tallo** de cada fichero SHALL ser el nombre de su tema, y NO SHALL haber dos ficheros para el mismo nombre ni un nombre sin fichero.

Cada página SHALL ser **autocontenida**: su `<style>` dentro, y su `<script>` también dentro si lo lleva. Cada página SHALL llevar los **tres marcadores** (`{{code}}`, `{{reason}}`, `{{description}}`) y la clase `c{{code}}` del `body`. NO SHALL existir el marcador `<link rel="stylesheet" href="/css/error.css" />` ni ninguna hoja de estilos suelta: las reglas de cada tema viven en el `<style>` de su página.

Las **seis páginas sobrias** —`default`, `minimal`, `terminal`, `paper`, `amber` y `compact`— SHALL compartir su marcado **palabra por palabra**: quitado el bloque `<style>` de cada una, SHALL ser idénticas **byte a byte** entre sí. `matrix.html` es **libre**: es un documento propio —canvas de lluvia, glitch, scanlines, pastillas, terminal, minijuego y toast— y PUEDE tener un marcado distinto; NO SHALL exigírsele `h2` ni `hr`. El coste de mantener seis ficheros idénticos SHALL quedar escrito; la alternativa «página compartida» se rechazó. La paridad SHALL **vigilarse con una prueba**, no con una guarda, porque una guarda textual no puede demostrar que seis ficheros no diverjan.

#### Scenario: cada nombre tiene su fichero y el tallo coincide con el nombre
- **GIVEN** el directorio `content/` del repositorio
- **WHEN** se listan sus `*.html`
- **THEN** SHALL haber exactamente siete
- **AND** el tallo de cada uno SHALL ser uno de los siete nombres de tema
- **AND** cada nombre SHALL tener su fichero

#### Scenario: las seis páginas sobrias comparten su marcado palabra por palabra
- **GIVEN** las seis páginas sobrias de `content/`
- **WHEN** se quita de cada una su bloque `<style>` y se comparan los bytes
- **THEN** SHALL ser idénticas entre sí
- **AND** el marcado resultante SHALL llevar `<main>`, `<h1>`, `<h2>`, `<hr>`, `<p>` y `<footer>errserve</footer>`, con el `footer` sin `href`

#### Scenario: cada página lleva los tres marcadores
- **GIVEN** las siete páginas de `content/`
- **WHEN** se revisan una a una
- **THEN** cada una SHALL llevar `{{code}}`, `{{reason}}` y `{{description}}`

#### Scenario: cada página lleva su estilo dentro
- **GIVEN** las siete páginas de `content/`
- **WHEN** se revisan sus hojas de estilos
- **THEN** cada una SHALL llevar su CSS en un bloque `<style>`
- **AND** NO SHALL referenciar ninguna hoja externa

### Requirement: Ninguna página pide nada por la red

Ninguna de las **siete páginas** incorporadas al binario —su HTML, sus estilos incrustados y sus scripts— SHALL poder hacer que el navegador **pida** algo por la red. El cuerpo servido NO SHALL contener ningún `<link>`: la página no carga ninguna hoja, porque la lleva dentro. El motivo SHALL quedar escrito: en el patrón del middleware `errors` de Traefik la página se sirve **en el host de la aplicación**, donde este servicio no sirve assets; el navegador pediría un fichero que contesta el upstream —con un `Content-Type` que no es el suyo— y la página llegaría sin estilos. Una página que necesita la red para verse es el mismo defecto que la autocontención corrige, y `error-pages` no lo sufre porque sus páginas son autocontenidas.

El contrato SHALL ser una lista **cerrada** de tokens: NO SHALL haber `@import` en un `<style>` incrustado, ni `url(...)` cuyo destino no sea `url(#…)` —referencia al propio documento— o `url(data:…)` —dato incrustado—; NO SHALL haber `<script src=…>` en ninguna forma, ni `<img src=…>`, `<source src=…>`, `<video src=…>`, `<audio src=…>`, `<iframe src=…>`, `<embed src=…>`, `<object data=…>`, `<form action=…>` o `<link href=…>` con un destino que no sea `data:` o `#…`; NO SHALL haber `srcset`; y en JavaScript NO SHALL haber `fetch(`, `XMLHttpRequest`, `WebSocket`, `EventSource`, `sendBeacon` ni `new Image`. El motivo de la lista cerrada SHALL quedar escrito: una guarda textual no puede demostrar que una URL construida en tiempo de ejecución no pide nada, así que exige los tokens canónicos y rechaza de más, que es la dirección segura. `@font-face` NO SHALL estar prohibido: solo puede pedir algo por la red a través de `url(...)` o `@import`, ya prohibidos, y `src: local(...)` usa una fuente que el usuario **ya tiene instalada**.

La guarda SHALL limpiar los **comentarios** antes de escanear, y lo SHALL hacer **por regiones**: el comentario HTML `<!-- … -->` en todo el documento —se salta entero, sin mirar cadenas dentro—; el bloque `/* … */` solo dentro de `<style>` o `<script>` —es la sintaxis de comentario de CSS y de JavaScript, no de HTML—; y la línea `// …` solo dentro de `<script>` —fuera de un guion, `//` es **texto visible** (la página del tema `matrix` lo usa así, en su `<p class="boot">`), y tratarlo como comentario se comería el resto de la línea y taparía una petición que viniera detrás—. Dentro de una región, el limpiador SHALL respetar las **cadenas**. El motivo SHALL quedar escrito: un comentario no puede pedir nada y la propia cabecera de la página documenta qué no puede hacer. La detección de etiquetas y atributos HTML SHALL ser **insensible a mayúsculas** —HTML lo es— y la de tokens de JavaScript SHALL ser **sensible a mayúsculas** —son identificadores—. El escaneo SHALL ser textual y sobre **bytes**, sin parser y sin `regex`.

La guarda SHALL medir este contrato **y nada más**, y SHALL cubrir las páginas **incorporadas** al binario: la página del operador en `CONTENT_DIR` es suya y el servicio NO SHALL parsearla al servir —la página es contenido y un HTML del operador no puede tumbar el servicio—. El límite SHALL quedar escrito: la guarda NO SHALL garantizar que un script no construya una URL en tiempo de ejecución (`fetch(base + path)`), ni que una librería no pida por su cuenta, ni que un `<img>` o un `<link>` se cree por JavaScript después de cargar; solo ve los tokens literales que están en los bytes de la página.

#### Scenario: la página servida no lleva ningún `<link>`
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /404` con `Accept: text/html`
- **THEN** el cuerpo NO SHALL contener `<link`
- **AND** el cuerpo SHALL contener `<style`

#### Scenario: el 503 de una aplicación llega con los estilos dentro
- **GIVEN** el ejemplo levantado, con el middleware `errors` delante de un backend que responde `503` en `/boom`
- **WHEN** se pide `GET /boom` con `Accept: text/html`
- **THEN** el estado SHALL ser `503` y el `Content-Type` `text/html`
- **AND** el cuerpo SHALL llevar un bloque `<style>` con el CSS de la página
- **AND** el cuerpo NO SHALL llevar ningún `<link>`

#### Scenario: una página sin peticiones cumple
- **GIVEN** una página de tema con su `<style>` incrustado, su `<canvas>` y su script, sin ningún token de la lista cerrada
- **WHEN** se comprueba contra el contrato de red
- **THEN** SHALL cumplirlo

#### Scenario: una petición en el script incumple
- **GIVEN** una página de tema cuyo script contiene `fetch('/api')`, otra con `new XMLHttpRequest()` y otra con `new WebSocket('wss://ejemplo')`
- **WHEN** se comprueban contra el contrato de red
- **THEN** cada una SHALL incumplirlo, y el incumplimiento SHALL nombrar la página y el token

#### Scenario: un recurso externo en el marcado incumple
- **GIVEN** una página con `<script src="https://ejemplo/x.js">`, otra con `<img src="/foto.png">` y otra con `<link rel="stylesheet" href="https://ejemplo/x.css">`
- **WHEN** se comprueban contra el contrato de red
- **THEN** cada una SHALL incumplirlo

#### Scenario: un `url(...)` externo en un estilo incrustado incumple
- **GIVEN** una página con `<style>body { background: url(fondo.png); }</style>` y otra con `<style>@import "otra.css";</style>`
- **WHEN** se comprueban contra el contrato de red
- **THEN** cada una SHALL incumplirlo

#### Scenario: una referencia interna y un dato incrustado cumplen
- **GIVEN** una página con `url(#gradiente)` en un `<style>`, con `url(data:image/png;base64,…)` y con `<img src="data:image/png;base64,…">`
- **WHEN** se comprueba contra el contrato de red
- **THEN** SHALL cumplirlo, porque no hay ninguna petición que hacer

#### Scenario: un comentario no es una petición
- **GIVEN** una página cuyo único `fetch(` está dentro de un comentario `//` y cuyo único `<script src=` está dentro de un comentario `<!-- … -->`
- **WHEN** se comprueba contra el contrato de red
- **THEN** SHALL cumplirlo, porque un comentario no puede pedir nada

#### Scenario: una etiqueta en mayúsculas incumple
- **GIVEN** una página con `<SCRIPT SRC="https://ejemplo/x.js">`
- **WHEN** se comprueba contra el contrato de red
- **THEN** SHALL incumplirlo: HTML no distingue mayúsculas en el nombre de la etiqueta ni en el atributo

#### Scenario: un `//` en prosa HTML no tapa una petición
- **GIVEN** una página con `<p>// nota visible</p><img src="/foto.png" alt="" />`
- **WHEN** se comprueba contra el contrato de red
- **THEN** SHALL incumplirlo: el `//` solo es comentario **dentro de un `<script>`**, así que en prosa HTML no puede comerse el resto de la línea y tapar el `<img src=` que viene detrás

### Requirement: Una página que anima puede apagarse

Una página PUEDE animar: el movimiento es una decisión de aspecto, no un defecto de autocontención. Pero lo que anima SHALL poder apagarse. El motivo SHALL quedar escrito: hay gente a la que el movimiento le produce mareo, y una página de error que se mueve mientras el usuario intenta leer qué ha pasado es peor página de error. La regla no prohíbe el adorno: prohíbe el adorno que no se puede apagar.

En el **CSS** de la página, cada **familia** que anime —una declaración de `animation` o `animation-name`, o de `transition` o `transition-property`, con un valor **distinto de `none`**— SHALL quedar neutralizada dentro de un bloque `@media (prefers-reduced-motion: reduce)` que **exista**, NO SHALL estar vacío y SHALL traer la forma canónica de esa familia: `animation: none` o `animation-name: none` para una animación, `transition: none` o `transition-property: none` para una transición. La lista SHALL ser **cerrada** y **cada familia por su forma** —`animation: none` NO SHALL neutralizar una transición—, porque una guarda textual no puede demostrar que otras formas —`animation-duration: 0s`, `animation-play-state: paused`— apagan igual en todos los navegadores.

La guarda SHALL medir la neutralización **por familia, no por declaración**: SHALL comprobar que el bloque reduce trae la forma canónica de cada **familia** que la página declara, y NO SHALL ligar cada declaración concreta con *su* propia neutralización —no pregunta «esta `.caret` que anima, ¿tiene su propio `animation: none`?»—. El motivo SHALL quedar escrito: ligarlas exigiría resolver el selector de cada regla —un parser de CSS, que este árbol no tiene porque compila sin red y sin `regex`— y **rechazaría neutralizaciones legítimas** hechas con otro selector —un comodín `* { animation: none }`, o una lista de selectores—, que es peor que el hueco. `@keyframes` por sí solo NO SHALL obligar a nada: define fotogramas y no anima mientras ninguna declaración los use.

En el **script**, una página que anime —con `requestAnimationFrame` o `setInterval`— SHALL **consultar** `prefers-reduced-motion` —por ejemplo, `matchMedia('(prefers-reduced-motion: reduce)')`— y NO SHALL arrancar la animación cuando el usuario pida movimiento reducido. La página del tema `matrix` SHALL mencionarlo. La guarda SHALL medir **la mención y nada más**: un script que menciona la preferencia y luego anima igual **pasa**; comprobar que la rama es correcta exigiría ejecutar el script en un navegador. Ese límite SHALL quedar escrito, con el mismo criterio con el que se documentó el contraste bajo del `<hr>`: es una **decisión medida y declarada**, no un descuido.

Y el mismo principio rige para los **comentarios**: una página que mencione `animation` o `requestAnimationFrame` **solo dentro de un comentario** NO SHALL tener que neutralizar nada, porque un comentario no anima; y un bloque de movimiento reducido cuyo contenido sea **solo** un comentario NO SHALL contar como bloque que neutraliza, porque un comentario tampoco apaga. Una página que no anima NO SHALL tener que declarar nada.

La guarda SHALL ser **textual y sobre los bytes** de la página —una página puede no estar en UTF-8—: garantiza que el patrón está, y NO SHALL garantizar que todos los navegadores apaguen el movimiento. La guarda SHALL cubrir las páginas **incorporadas** al binario; la del operador es suya y el servicio no la parsea al servir.

#### Scenario: una página sin animación cumple sin declarar nada
- **GIVEN** una página cuyo único contenido es `body { color: #222; }`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo sin necesitar ningún bloque `@media (prefers-reduced-motion: reduce)`

#### Scenario: un bloque reduce que neutraliza la animación cumple
- **GIVEN** una página con `@keyframes entrada { … }`, `main { animation: entrada 180ms ease-out; }` y `@media (prefers-reduced-motion: reduce) { main { animation: none; } }`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo

#### Scenario: un bloque reduce vacío no basta
- **GIVEN** una página con `main { animation: entrada 180ms ease-out; }` y un bloque `@media (prefers-reduced-motion: reduce) { }` sin nada dentro
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo: el bloque tiene que existir y **tener** la neutralización

#### Scenario: una página que solo nombra prefers-reduced-motion incumple
- **GIVEN** una página con `main { animation: entrada 180ms ease-out; }` y un bloque `@media (prefers-reduced-motion: reduce) { /* esta animación se apaga sola */ }` cuyo contenido es únicamente un comentario
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo: nombrar la media query no neutraliza nada

#### Scenario: una media query que no es la de movimiento reducido no basta
- **GIVEN** una página con `main { animation: entrada 180ms ease-out; }` y un bloque `@media (prefers-reduced-motion: no-preference) { main { animation: none; } }`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo: neutralizar en la preferencia contraria no es la puerta que el usuario cierra

#### Scenario: neutralizar otra animación distinta de la declarada no basta
- **GIVEN** una página que declara `transition: opacity 200ms;` y cuyo bloque `@media (prefers-reduced-motion: reduce)` solo trae `animation: none`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo: la transición declarada sigue sin neutralizarse

#### Scenario: unos fotogramas sin declaración que los use no obligan
- **GIVEN** una página cuyo único contenido es `@keyframes entrada { from { opacity: 0 } to { opacity: 1 } }`, sin ninguna declaración de `animation`, `animation-name`, `transition` ni `transition-property`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo sin necesitar ningún bloque `@media (prefers-reduced-motion: reduce)`, porque definir fotogramas no anima nada

#### Scenario: una regla anidada dentro del bloque reduce cuenta
- **GIVEN** una página con `main { animation: entrada 180ms ease-out; }` y un bloque `@media (prefers-reduced-motion: reduce) { main { animation: none; } }`, con la neutralización dentro de una regla anidada
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo: el bloque se delimita por su llave de apertura y **su** cierre equilibrado, no por la primera llave que aparezca

#### Scenario: una animación declarada dentro del bloque reduce no se neutraliza sola
- **GIVEN** una página cuya **única** declaración que anima está **dentro** del bloque reduce: `@media (prefers-reduced-motion: reduce) { main { animation: entrada 180ms ease-out; } }`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo: el bloque reduce tiene que **apagar**, no animar, así que una declaración que anima dentro de él no neutraliza nada

#### Scenario: dos animaciones declaradas y una sola neutralizada pasan la guarda
- **GIVEN** una página con `.glitch { animation: glitch-x 1.6s infinite; }`, `.caret { animation: parpadeo 1s infinite; }` y un bloque `@media (prefers-reduced-motion: reduce) { .glitch { animation: none; } }`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo: la guarda mide la neutralización **por familia** —y `animation: none` está—, no que cada declaración quede ligada con *su* selector; ligarlas exigiría un parser de selectores y rechazaría neutralizaciones legítimas como `* { animation: none }`
- **AND** ese límite SHALL quedar escrito en la documentación del proyecto, con el mismo criterio que el del script que menciona la preferencia y anima igual

#### Scenario: un script que anima y menciona la preferencia cumple
- **GIVEN** una página cuyo script usa `requestAnimationFrame` y menciona `matchMedia('(prefers-reduced-motion: reduce)')`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo

#### Scenario: un script que anima sin mencionar la preferencia incumple
- **GIVEN** una página cuyo script usa `requestAnimationFrame` y no menciona `prefers-reduced-motion`
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL incumplirlo, y el incumplimiento SHALL nombrar la página y el token

#### Scenario: un script que menciona la preferencia y anima igual pasa la guarda
- **GIVEN** una página cuyo script menciona `prefers-reduced-motion` pero arranca la animación sin comprobar la preferencia
- **WHEN** se comprueba contra el contrato de movimiento
- **THEN** SHALL cumplirlo: la guarda mide la mención, no la eficacia de la rama
- **AND** ese límite SHALL quedar escrito en la cabecera de la página y en la documentación del proyecto
