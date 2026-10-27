## MODIFIED Requirements

### Requirement: La configuración se lee solo del entorno

La aplicación SHALL leer su configuración exclusivamente de variables de entorno: `PORT` (puerto de escucha, por defecto `8080`), `LOG_LEVEL` (filtro de registro, por defecto `info`), `LOG_FORMAT` (formato del registro, `text` o `json`, por defecto `text`), `CONTENT_DIR` (directorio de las páginas y los assets del operador, por defecto `content`; es un *override* de las páginas embebidas en el binario, no la única fuente de páginas), `DEFAULT_ERROR_PAGE` (código por defecto, por defecto `404`; NO SHALL nombrar ningún fichero), `RETRY_AFTER` (segundos de la cabecera `Retry-After` de los códigos que la llevan, por defecto `120`) y `THEME` (tema de las páginas HTML, por defecto `default`: uno de los **embebidos** al compilar desde `content/` **o un tema descubierto en `CONTENT_DIR`**). NO SHALL leer ningún fichero de configuración como `config.yml`.

Una variable presente pero vacía NO SHALL confundirse con una ausente. Una variable numérica —`PORT`, `DEFAULT_ERROR_PAGE`, `RETRY_AFTER`— presente pero no interpretable SHALL abortar el arranque con un error que nombre la variable. Un `THEME` que no sea uno de los nombres del **registro** —los embebidos más los descubiertos en `CONTENT_DIR`—, o la cadena vacía, SHALL abortar el arranque con un error que nombre `THEME` y **enumere los nombres del registro**, en vez de degradar en caliente al tema por defecto. SHALL quedar escrito el **precio medido**: el error enumera los nombres que el registro **tenga en ese arranque**, así que un `THEME` correcto cuyo fichero **todavía** no esté en `CONTENT_DIR` —un volumen que aún no se ha montado— aborta el arranque; es la contrapartida de mantener el aborto, y va escrito para que no sorprenda.

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
- **THEN** el arranque SHALL fallar con un error que nombre `THEME` y enumere los nombres del registro

#### Scenario: un THEME descubierto arranca
- **GIVEN** un `CONTENT_DIR` con `neon.html` y `THEME=neon`
- **WHEN** la aplicación arranca
- **THEN** SHALL arrancar
- **AND** el tema SHALL ser `neon`

#### Scenario: un THEME correcto con el volumen sin montar no arranca
- **GIVEN** `THEME=neon` y un `CONTENT_DIR` sin `neon.html`
- **WHEN** la aplicación arranca
- **THEN** SHALL fallar nombrando `THEME` y enumerando los nombres del registro

### Requirement: La imagen se construye desde el árbol y trae las páginas dentro

La imagen SHALL construirse con `docker build` desde el árbol del repositorio, sin depender de nada que no esté versionado. El stage de **compilación** SHALL disponer de `content/` —las páginas de tema—, porque el binario las embebe al compilar mediante `build.rs`, que lee `content/`, y `cargo` NO SHALL poder compilar el crate sin ellas. Que falten NO SHALL descubrirse por primera vez al publicar una etiqueta: el CI SHALL construir la imagen **y SHALL probarla arrancada** en cada propuesta de cambio. Construir NO SHALL contar como probar: una imagen que compila y no responde el contrato del servicio SHALL poner la puerta en rojo.

El stage de **runtime** SHALL copiar `content/` solo si la imagen tiene que servir esas páginas y no las del binario. Es una decisión abierta y declarada (`design.md`, preguntas abiertas): `CONTENT_DIR` es la única ruta de estáticos y, como la capa del disco gana a la embebida, esa segunda copia decide **qué bytes se sirven en producción**. Quitarla haría que la imagen sirviese las páginas embebidas.

#### Scenario: la imagen se construye sin caché previa

- **GIVEN** el árbol del repositorio, sin ningún `target/` ni caché de construcción previa
- **WHEN** se construye la imagen con `docker build`
- **THEN** la construcción SHALL terminar sin error

#### Scenario: el stage de compilación dispone de las páginas

- **GIVEN** el stage `builder` de la imagen
- **WHEN** compila el crate
- **THEN** `content/` SHALL estar disponible en el contexto de compilación, con las páginas de tema
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

#### Scenario: la propuesta de cambio prueba la imagen, no solo la construye

- **GIVEN** una propuesta de cambio cuyo job `imagen` construye la imagen y no la arranca
- **WHEN** se evalúa la puerta del CI
- **THEN** la puerta SHALL considerarse incumplida
- **AND** el smoke test SHALL haberse corrido contra la imagen recién construida

### Requirement: Los ficheros de CONTENT_DIR se sirven como assets

Los ficheros de `CONTENT_DIR` SHALL servirse como assets estáticos con su `Content-Type`. Los ficheros de tema (`<nombre>.html`) PUEDEN servirse como assets si no se piden como página de error; el servicio no los juzga. NO SHALL contarse ni enumerarse el número de assets.

#### Scenario: un asset existente se sirve con su tipo
- **GIVEN** un `CONTENT_DIR` con `estilo.css`
- **WHEN** se pide `GET /estilo.css`
- **THEN** el estado SHALL ser `200`
- **AND** el `Content-Type` SHALL ser `text/css`

#### Scenario: una página de tema no se juzga como asset
- **GIVEN** un `CONTENT_DIR` con `matrix.html`
- **WHEN** se pide `GET /matrix.html`
- **THEN** el estado SHALL ser `200`
- **AND** el cuerpo SHALL ser el fichero crudo
- **AND** el servicio NO SHALL parsearlo

#### Scenario: un CSS se sirve desde el directorio de contenido
- **GIVEN** el fichero `content/css/404.css`
- **WHEN** se pide `GET /css/404.css`
- **THEN** SHALL responder `200`
- **AND** el `Content-Type` SHALL ser `text/css`

#### Scenario: una página de tema se sirve cruda si se pide por su path
- **GIVEN** un `CONTENT_DIR` con las páginas de tema
- **WHEN** se pide `GET /matrix.html`
- **THEN** SHALL responder `200`
- **AND** el cuerpo SHALL ser el fichero tal cual, con los marcadores a la vista

#### Scenario: una página de error no se sirve como asset con un 200
- **GIVEN** un servicio arrancado con las páginas de tema
- **WHEN** se pide `GET /404.html`
- **THEN** el estado SHALL ser `404`
- **AND** NO SHALL ser `200`

### Requirement: El respaldo no publica las páginas de tema crudas

El respaldo de Traefik SHALL excluir las páginas de tema crudas. La exclusión SHALL cubrir las **tres** formas con las que un cliente alcanza esa página cruda: el path terminado en `.html`/`.htm`, el mismo con **barra final** o con un segmento detrás y el mismo con la barra **percent-encoded**, en las dos cajas. Una exclusión anclada al fin de cadena NO SHALL contar como cumplirla. NO SHALL enumerarse una lista fija de nombres de página: la exclusión SHALL cubrir la familia completa de `.html`/`.htm`.

El matcher `PathRegexp` SHALL haberse medido contra la versión de Traefik que el ejemplo fija, porque una sintaxis que no case con nada deja el router `enabled` sin bloquear lo que debía bloquear, en silencio.

La regla SHALL escribirse de modo que YAML la lea como se pretende: en un valor **entrecomillado** la barra invertida va doblada, y escribirla simple ahí NO SHALL contar. Los sitios que este repositorio entrega la escriben como escalar **plano**, que es donde la barra va simple y donde los dos `$` de la regla llegan a Traefik verbatim.

Una prueba automática SHALL guardar la propiedad sobre los ficheros versionados, con el mismo método que la exclusión de `/metrics`, porque la comprobación de extremo a extremo necesita un motor de contenedores y NO corre en el CI.

Límite declarado y **medido**: `PathRegexp` es sensible a mayúsculas, así que `GET /MINIMAL.HTML` no casa con la exclusión y llega al servicio. NO SHALL contarse como incumplimiento: el servicio responde **su página de error** —no el fichero crudo—, porque el sistema de ficheros distingue mayúsculas y `ServeDir` contesta `404`. Es una dependencia del sistema de ficheros, no una propiedad del servicio.

#### Scenario: el respaldo no sirve ninguna página de tema cruda

- **GIVEN** el ejemplo levantado, con Traefik delante y un Host que no case con ningún otro router
- **WHEN** se pide `GET /minimal.html` y `GET /matrix.html` a ese Host
- **THEN** el estado NO SHALL ser `200`
- **AND** el cuerpo NO SHALL llevar los marcadores `{{code}}`, `{{reason}}` ni `{{description}}` a la vista

#### Scenario: la barra final no reabre el leak

- **GIVEN** el ejemplo levantado y un Host que no case con ningún otro router
- **WHEN** se pide `GET /minimal.html/`, `GET /minimal.html%2F` y `GET /matrix.html%2f`
- **THEN** el estado NO SHALL ser `200`
- **AND** el cuerpo NO SHALL llevar los marcadores a la vista
- **AND** el resultado NO SHALL depender del *entrypoint*: los mismos paths SHALL quedar bloqueados con `sanitizePath` en su defecto y con `sanitizePath=false`

#### Scenario: la página del código por defecto no se excluye

- **GIVEN** el ejemplo levantado y un Host que no case con ningún otro router
- **WHEN** se pide `GET /default.html` y `GET /default.htm`
- **THEN** el estado SHALL ser el del código por defecto
- **AND** el cuerpo SHALL ser la página de ese código, nunca el fichero crudo
- **AND** NO SHALL ser `200`

#### Scenario: el precio de la exclusión queda escrito

- **GIVEN** el ejemplo levantado y un Host que no case con ningún otro router
- **WHEN** se pide `GET /404.html`, `GET /503.html` y `GET /sub/x.html`
- **THEN** los SHALL contestar Traefik con su `404`, sin pasar por el servicio
- **AND** el flujo real NO SHALL romperse, porque el middleware `errors` pide `/{status}` **sin** extensión

#### Scenario: el leak no vuelve a colarse en una definición de respaldo

- **GIVEN** el repositorio versionado, con los ficheros que entregan un respaldo desplegable
- **WHEN** se inspecciona la regla de cada router de respaldo que el repositorio define
- **THEN** cada una SHALL llevar, además de la exclusión de `/metrics`, la exclusión de las páginas de tema crudas
- **AND** la prueba SHALL fallar nombrando el fichero que la haya perdido
- **AND** un respaldo que lleve la exclusión de `/metrics` y **no** la del leak SHALL incumplir

#### Scenario: la exclusión no puede quedar en un comentario de cola

- **GIVEN** un fichero de despliegue cuya regla de respaldo lleva la exclusión del leak sólo **después** de un `#` de cola, que en YAML abre un comentario
- **WHEN** se inspecciona el **valor** de la regla
- **THEN** esa definición SHALL considerarse incumplidora
- **AND** la prueba SHALL fallar nombrándola
