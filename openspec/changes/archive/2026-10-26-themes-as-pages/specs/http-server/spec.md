# http-server Specification (delta)

## MODIFIED Requirements

### Requirement: La configuración se lee solo del entorno

La aplicación SHALL leer su configuración exclusivamente de variables de entorno: `PORT` (puerto de escucha, por defecto `8080`), `LOG_LEVEL` (filtro de registro, por defecto `info`), `LOG_FORMAT` (formato del registro, `text` o `json`, por defecto `text`), `CONTENT_DIR` (directorio de las páginas y los assets del operador, por defecto `content`; es un *override* de las páginas embebidas en el binario, no la única fuente de páginas), `DEFAULT_ERROR_PAGE` (código por defecto, por defecto `404`; NO SHALL nombrar ningún fichero), `RETRY_AFTER` (segundos de la cabecera `Retry-After` de los códigos que la llevan, por defecto `120`) y `THEME` (tema de las páginas HTML, por defecto `default`, uno de los **siete** temas incorporados al binario). NO SHALL leer ningún fichero de configuración como `config.yml`.

Una variable presente pero vacía NO SHALL confundirse con una ausente. Una variable numérica —`PORT`, `DEFAULT_ERROR_PAGE`, `RETRY_AFTER`— presente pero no interpretable SHALL abortar el arranque con un error que nombre la variable. Un `THEME` que no sea uno de los siete nombres —`default`, `minimal`, `terminal`, `paper`, `amber`, `compact` o `matrix`—, o la cadena vacía, SHALL abortar el arranque con un error que nombre `THEME` y enumere los siete, en vez de degradar en caliente al tema por defecto.

Que el directorio de `CONTENT_DIR` no exista NO SHALL impedir el arranque, ni cambiar el `200` de un asset, ni degradar ninguna página de error a un `500`: las páginas embebidas cubren ese caso.

#### Scenario: Sin PORT escucha en 8080
- **GIVEN** un entorno sin `PORT`
- **WHEN** la aplicación arranca
- **THEN** SHALL escuchar en `0.0.0.0:8080`

#### Scenario: PORT cambia el puerto
- **GIVEN** `PORT=9999`
- **WHEN** la aplicación arranca
- **THEN** SHALL escuchar en `0.0.0.0:9999`

#### Scenario: RETRY_AFTER cambia el valor de la cabecera
- **GIVEN** `RETRY_AFTER=30`
- **WHEN** la aplicación arranca y sirve una página `503`
- **THEN** la respuesta SHALL incluir `Retry-After: 30`

#### Scenario: un RETRY_AFTER no numérico no arranca
- **GIVEN** `RETRY_AFTER=medio minuto`
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar con un error que nombre `RETRY_AFTER`

#### Scenario: un RETRY_AFTER vacío no arranca
- **GIVEN** `RETRY_AFTER` presente y vacía
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar, en vez de usar `120`

#### Scenario: sin CONTENT_DIR el defecto sigue siendo content
- **GIVEN** un entorno sin `CONTENT_DIR`
- **WHEN** la aplicación arranca
- **THEN** el directorio de contenido SHALL ser `content`
- **AND** el arranque NO SHALL fallar aunque ese directorio no exista

#### Scenario: sin THEME el tema por defecto es default
- **GIVEN** un entorno sin `THEME`
- **WHEN** la aplicación arranca
- **THEN** el tema SHALL ser `default`

#### Scenario: un THEME desconocido no arranca
- **GIVEN** `THEME=neon`
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar con un error que nombre `THEME` y enumere los siete temas válidos

### Requirement: La imagen se construye desde el árbol y trae las páginas dentro

La imagen SHALL construirse con `docker build` desde el árbol del repositorio, sin depender de nada que no esté versionado. El stage de **compilación** SHALL disponer de `content/` —las **siete** páginas de tema, `default.html`, `minimal.html`, `terminal.html`, `paper.html`, `amber.html`, `compact.html` y `matrix.html`—, porque el binario las embebe al compilar y `cargo` NO SHALL poder compilar el crate sin ellas. Que falten NO SHALL descubrirse por primera vez al publicar una etiqueta: el CI SHALL construir la imagen en cada propuesta de cambio.

El stage de **runtime** SHALL copiar `content/` solo si la imagen tiene que servir esas páginas y no las del binario. Es una decisión abierta y declarada (`design.md`, preguntas abiertas): `CONTENT_DIR` es la única ruta de estáticos y, como la capa del disco gana a la embebida, esa segunda copia decide **qué bytes se sirven en producción**. Quitarla haría que la imagen sirviese las páginas embebidas.

#### Scenario: la imagen se construye sin caché previa
- **GIVEN** el árbol del repositorio, sin ningún `target/` ni caché de construcción previa
- **WHEN** se construye la imagen con `docker build`
- **THEN** la construcción SHALL terminar sin error

#### Scenario: el stage de compilación dispone de las páginas
- **GIVEN** el stage `builder` de la imagen
- **WHEN** compila el crate
- **THEN** `content/` SHALL estar disponible en el contexto de compilación, con las siete páginas de tema
- **AND** el binario SHALL quedar con las siete páginas embebidas

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

## REMOVED Requirements

### Requirement: Los assets estáticos se sirven desde CONTENT_DIR

Retirado: sus escenarios de asset **embebido** (`la hoja embebida se sirve sin directorio de contenido`, `el fichero del directorio gana al asset embebido`, `un asset embebido no convierte una página de error en un 200`) describen la tabla de assets embebidos —la hoja `/css/error.css`—, que este change retira: las páginas de tema son autocontenidas y no hay ninguna hoja que servir como asset. Lo sustituye «Los ficheros de CONTENT_DIR se sirven como assets», sin tabla embebida y con las páginas de tema como ficheros crudos del directorio.

## ADDED Requirements

### Requirement: Los ficheros de CONTENT_DIR se sirven como assets

Las peticiones que no son páginas de error ni endpoints de servicio SHALL servirse como ficheros estáticos: el fichero de `CONTENT_DIR` cuando exista y se pueda leer, con su `Content-Type` correspondiente. NO SHALL existir una tabla de **assets embebidos**: la única fuente de assets es el disco de `CONTENT_DIR`, porque las páginas del proyecto van embebidas por la vía de la página de error y no por la de los assets.

Las **páginas de tema** SHALL servirse como assets **crudos** si se piden por su path (`GET /minimal.html`, `/terminal.html`, `/paper.html`, `/amber.html`, `/compact.html`, `/matrix.html`): responden `200` con el fichero tal cual y los marcadores a la vista. Es coherente con cómo se servían antes las hojas de `content/themes/`, y no se intercepta porque hacerlo rompería «un asset existente gana» y añadiría casos especiales al middleware. La excepción SHALL ser `/default.html` y `/default.htm`: su path se clasifica como el código por defecto y NO SHALL servirse como asset.

Una página HTML de error NO SHALL servirse por esta vía: una petición cuyo path normalizado resuelva un código de error SHALL pasar por la página de error, con el código resuelto y nunca con un `200`.

#### Scenario: un CSS se sirve desde el directorio de contenido
- **GIVEN** el fichero `content/css/404.css`
- **WHEN** se pide `GET /css/404.css`
- **THEN** SHALL responder `200`
- **AND** el `Content-Type` SHALL ser `text/css`

#### Scenario: una página de tema se sirve cruda si se pide por su path
- **GIVEN** un `CONTENT_DIR` con las siete páginas de tema
- **WHEN** se pide `GET /matrix.html`
- **THEN** SHALL responder `200`
- **AND** el cuerpo SHALL ser el fichero tal cual, con los marcadores a la vista

#### Scenario: una página de error no se sirve como asset con un 200
- **GIVEN** un servicio arrancado con las siete páginas de tema
- **WHEN** se pide `GET /404.html`
- **THEN** el estado SHALL ser `404`
- **AND** NO SHALL ser `200`
