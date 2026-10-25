# error-page Specification (delta)

## MODIFIED Requirements

### Requirement: El cuerpo depende del formato y el código es ortogonal

El cuerpo HTML SHALL servirse del fichero `CONTENT_DIR/<code>.html` y, si el del código pedido no existe, del fichero del código por defecto, como hasta ahora. Si no existe **ninguno** de los dos ficheros, SHALL servirse la copia **embebida** del código pedido y, si esa no existiera, la copia embebida del código por defecto. La cascada del directorio SHALL intentarse **entera** antes que la embebida, y un fichero de `CONTENT_DIR` **que se pueda leer** NO SHALL ser desplazado por la copia embebida de otro código. Los cuerpos JSON, XML y texto SHALL generarse a partir del código, su razón canónica y su descripción. El código resuelto NO SHALL depender del formato ni del origen del cuerpo.

#### Scenario: HTML desde el fichero del código
- **GIVEN** que `content/404.html` existe
- **WHEN** se pide `GET /404.html`
- **THEN** el cuerpo SHALL ser el contenido de `content/404.html`

#### Scenario: HTML cae al código por defecto
- **GIVEN** que `content/418.html` no existe, que el defecto es `404` y que `content/404.html` existe
- **WHEN** se pide `GET /418.html`
- **THEN** el cuerpo SHALL ser el contenido de `content/404.html`
- **AND** el estado SHALL ser `418`

#### Scenario: la página embebida se sirve sin directorio de contenido
- **GIVEN** un servicio cuyo `CONTENT_DIR` no contiene ningún fichero
- **WHEN** se pide `GET /503.html`
- **THEN** el estado SHALL ser `503`
- **AND** el cuerpo SHALL ser la página embebida del `503`
- **AND** el `Content-Type` SHALL ser HTML

#### Scenario: el fichero del directorio gana a la copia embebida
- **GIVEN** un `CONTENT_DIR` con un `404.html` propio, distinto del embebido
- **WHEN** se pide `GET /404.html`
- **THEN** el cuerpo SHALL ser el de `CONTENT_DIR/404.html`
- **AND** NO SHALL ser la copia embebida del `404`

#### Scenario: el directorio se agota antes de entrar en la copia embebida
- **GIVEN** un `CONTENT_DIR` con su propio `404.html` y sin `503.html`, y el defecto `404`
- **WHEN** se pide `GET /503.html`
- **THEN** el cuerpo SHALL ser el de `CONTENT_DIR/404.html`
- **AND** NO SHALL ser la página embebida del `503`

#### Scenario: sin ficheros, un código sin página propia cae a la embebida del defecto
- **GIVEN** un servicio sin ningún fichero en `CONTENT_DIR` y con el defecto `404`
- **WHEN** se pide `GET /418.html`
- **THEN** el estado SHALL ser `418`
- **AND** el cuerpo SHALL ser la página embebida del `404`

#### Scenario: una página embebida no se sirve como asset
- **GIVEN** un servicio sin ningún fichero en `CONTENT_DIR`
- **WHEN** se pide `GET /404.html`
- **THEN** el estado SHALL ser `404`
- **AND** NO SHALL ser `200`
- **AND** el cuerpo SHALL ser la página embebida del `404`

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

### Requirement: Un fallo al leer el HTML no cambia el estado

Si no existe el fichero HTML del código ni el del código por defecto —ni en `CONTENT_DIR` ni entre las copias embebidas—, la aplicación SHALL responder el **código resuelto** con el cuerpo del formato resuelto, y NO SHALL responder `500`.

Con las páginas embebidas esa situación NO SHALL ser alcanzable para los once códigos que un proxy emite, que siempre tienen copia; SHALL quedar al alcance de un `DEFAULT_ERROR_PAGE` fuera de esa lista, que es un valor legal y que ninguna copia cubre. El respaldo NO SHALL retirarse.

#### Scenario: sin fichero HTML se mantiene el código
- **GIVEN** un `CONTENT_DIR` sin ningún fichero HTML y con el defecto `418`, que no tiene página ni en el directorio ni embebida
- **WHEN** se pide `GET /418.html`
- **THEN** el estado SHALL ser `418`
- **AND** NO SHALL ser `500`

#### Scenario: el respaldo cubre también un código distinto del defecto
- **GIVEN** un `CONTENT_DIR` sin ningún fichero HTML y con el defecto `418`
- **WHEN** se pide `GET /599.html`
- **THEN** el estado SHALL ser `599`
- **AND** NO SHALL ser `500`

### Requirement: Los códigos que un proxy emite tienen su propia página

Los códigos que un proxy emite con normalidad —`400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`, `502`, `503` y `504`— SHALL tener una página HTML **embebida en el binario**, de modo que un servicio instalado sin ficheros de datos lo sirva con su propia página. `CONTENT_DIR` SHALL contener además el fichero `<código>.html` de esos mismos once códigos, como hasta ahora.

La copia embebida de una página SHALL ser idéntica, byte a byte, al fichero del repositorio, y toda página de `CONTENT_DIR` SHALL tener copia embebida: las dos listas NO SHALL poder divergir en silencio.

La página HTML de un código SHALL identificar ese código en su título o en su cuerpo, de modo que el cuerpo nunca contradiga al estado, y esto SHALL cumplirse igual en la copia embebida.

Fuera de esa lista el respaldo sigue existiendo —el fichero del código por defecto y, si no, su copia embebida—, pero un código de la lista NO SHALL depender de él.

#### Scenario: el 504 tiene página propia y la nombra
- **GIVEN** que `content/504.html` existe
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
- **GIVEN** las páginas del repositorio y las copias embebidas en el binario
- **WHEN** se comparan los bytes de cada una de las once
- **THEN** SHALL ser idénticos

### Requirement: Las páginas no referencian ficheros que no existen

Toda página HTML de `CONTENT_DIR`, y su copia embebida, SHALL referenciar únicamente ficheros servibles: presentes en `CONTENT_DIR` o embebidos en el binario.

Las páginas SHALL compartir **una sola** hoja de estilos, `css/error.css`, con un nombre neutro. Ninguna página SHALL referenciar una hoja cuyo nombre sea el de **otro** código: un `403` que carga `404.css` hace que el error se presente con el nombre —y la apariencia— de otro, que es el defecto que este change corrige. La hoja SHALL ser única porque once copias del mismo contenido no son once hojas, son una hoja mal repartida; la diferencia de aspecto por código, si se quiere, se expresa con una clase en el elemento `body`, no duplicando el fichero.

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
- **WHEN** se pide la página del `404` en HTML y, después, la hoja que esa página referencia
- **THEN** la página SHALL referenciar `css/error.css`
- **AND** esa petición SHALL responder `200` con `Content-Type` `text/css`
- **AND** NO SHALL ser una página de error
