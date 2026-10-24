# error-page Specification (delta)

## ADDED Requirements

### Requirement: Los códigos que un proxy emite tienen su propia página

`CONTENT_DIR` SHALL contener un fichero `<código>.html` para los códigos que un proxy emite con normalidad: `400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`, `502`, `503` y `504`. La página HTML de un código SHALL identificar ese código en su título o en su cuerpo, de modo que el cuerpo nunca contradiga al estado.

Fuera de esa lista el respaldo sigue existiendo —el fichero del código por defecto—, pero un código de la lista NO SHALL depender de él.

#### Scenario: el 504 tiene página propia y la nombra
- **GIVEN** que `content/504.html` existe
- **WHEN** se pide `GET /504`
- **THEN** el estado SHALL ser `504`
- **AND** el cuerpo SHALL contener `504`

#### Scenario: ningún código de la lista cae al respaldo
- **GIVEN** la lista de códigos cubiertos
- **WHEN** se pide cada uno de ellos con `Accept: text/html`
- **THEN** el cuerpo SHALL contener su propio código

### Requirement: Las páginas no referencian ficheros que no existen

Toda página HTML de `CONTENT_DIR` SHALL referenciar únicamente ficheros presentes en `CONTENT_DIR`.

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

### Requirement: Cada página se identifica con la clase de su código

El elemento `body` de la página de un código cubierto SHALL llevar la clase `c<código>` —`c404`, `c504`—, de modo que la apariencia pueda diferenciarse por código con una sola hoja compartida y sin duplicarla.

#### Scenario: la página del 504 se identifica como tal
- **GIVEN** `content/504.html`
- **WHEN** se inspecciona su cuerpo
- **THEN** el elemento `body` SHALL llevar la clase `c504`
