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

El cuerpo HTML SHALL servirse del fichero `CONTENT_DIR/<code>.html`, con caída al fichero del código por defecto si el del código pedido no existe. Los cuerpos JSON, XML y texto SHALL generarse a partir del código, su razón canónica y su descripción. El código resuelto NO SHALL depender del formato.

#### Scenario: HTML desde el fichero del código
- **GIVEN** que `content/404.html` existe
- **WHEN** se pide `GET /404.html`
- **THEN** el cuerpo SHALL ser el contenido de `content/404.html`

#### Scenario: HTML cae al código por defecto
- **GIVEN** que `content/418.html` no existe, que el defecto es `404` y que `content/404.html` existe
- **WHEN** se pide `GET /418.html`
- **THEN** el cuerpo SHALL ser el contenido de `content/404.html`
- **AND** el estado SHALL ser `418`

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

### Requirement: Las cabeceras de acompañamiento

Toda página de error SHALL incluir `Content-Type` según el formato y `Content-Length` del cuerpo, y SHALL incluir `X-Robots-Tag: noindex, nofollow, nosnippet, noarchive`. Los códigos `429` y `503` SHALL incluir además `Retry-After: 120`; el resto NO SHALL incluirlo.

#### Scenario: toda página lleva X-Robots-Tag
- **GIVEN** una petición a `GET /404`
- **WHEN** se responde
- **THEN** la respuesta SHALL incluir `X-Robots-Tag: noindex, nofollow, nosnippet, noarchive`

#### Scenario: el 503 lleva Retry-After
- **GIVEN** una petición a `GET /503`
- **WHEN** se responde
- **THEN** la respuesta SHALL incluir `Retry-After: 120`

#### Scenario: el 404 no lleva Retry-After
- **GIVEN** una petición a `GET /404`
- **WHEN** se responde
- **THEN** la respuesta NO SHALL incluir `Retry-After`

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

Si no existe el fichero HTML del código ni el del código por defecto, la aplicación SHALL responder el **código resuelto** con el cuerpo del formato resuelto, y NO SHALL responder `500`.

#### Scenario: sin fichero HTML se mantiene el código
- **GIVEN** un `CONTENT_DIR` sin `418.html` ni `404.html`
- **WHEN** se pide `GET /418.html`
- **THEN** el estado SHALL ser `418`
- **AND** NO SHALL ser `500`
