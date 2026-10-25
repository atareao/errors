# http-server Specification (delta)

## ADDED Requirements

### Requirement: La imagen se construye desde el árbol y trae las páginas dentro

La imagen SHALL construirse con `docker build` desde el árbol del repositorio, sin depender de nada
que no esté versionado. El stage de **compilación** SHALL disponer de `content/` —las once páginas y
`css/error.css`—, porque el binario las embebe al compilar y `cargo` NO SHALL poder compilar el crate
sin ellas. Que falten NO SHALL descubrirse por primera vez al publicar una etiqueta: el CI SHALL
construir la imagen en cada propuesta de cambio.

#### Scenario: la imagen se construye sin caché previa
- **GIVEN** el árbol del repositorio, sin ningún `target/` ni caché de construcción previa
- **WHEN** se construye la imagen con `docker build`
- **THEN** la construcción SHALL terminar sin error

#### Scenario: el stage de compilación dispone de las páginas
- **GIVEN** el stage `builder` de la imagen
- **WHEN** compila el crate
- **THEN** `content/` SHALL estar disponible en el contexto de compilación, con las once páginas y la hoja `css/error.css`
- **AND** el binario SHALL quedar con las páginas embebidas

#### Scenario: la imagen sirve las páginas sin ficheros externos
- **GIVEN** la imagen arrancada sin montar ningún volumen
- **WHEN** se pide `GET /404` con `Accept: text/html`
- **THEN** el estado SHALL ser `404`
- **AND** el cuerpo SHALL ser la página del `404`
- **AND** NO SHALL ser el respaldo de texto

#### Scenario: la imagen no corre como root
- **GIVEN** la imagen construida
- **WHEN** se inspecciona el usuario con el que se ejecuta
- **THEN** el usuario SHALL ser `10001:10001`
