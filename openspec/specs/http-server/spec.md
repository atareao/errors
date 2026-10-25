# http-server Specification

## Purpose
El contrato de arranque y servicio: qué variables de entorno se leen y con qué valores por defecto, que el puerto por defecto es el `8080` que asume la guía de Traefik, que los endpoints de salud y de versión responden siempre sin pasar por la resolución de páginas de error, y que los ficheros de `CONTENT_DIR` se sirven como assets estáticos con su `Content-Type`.

## Requirements

### Requirement: La configuración se lee solo del entorno

La aplicación SHALL leer su configuración exclusivamente de variables de entorno: `PORT` (puerto de
escucha, por defecto `8080`), `LOG_LEVEL` (filtro de registro, por defecto `info`), `LOG_FORMAT` (formato
del registro, `text` o `json`, por defecto `text`), `CONTENT_DIR` (directorio de páginas y assets del
operador, por defecto `content`; es un *override* de los recursos embebidos en el binario, no la única
fuente de páginas y hojas), `DEFAULT_ERROR_PAGE` (código por defecto, por defecto `404`), `RETRY_AFTER`
(segundos de la cabecera `Retry-After` de los códigos que la llevan, por defecto `120`) y `THEME` (tema de
las páginas HTML, por defecto `default`, uno de los temas incorporados al binario). NO SHALL leer ningún
fichero de configuración como `config.yml`.

Una variable presente pero vacía NO SHALL confundirse con una ausente. Una variable numérica —`PORT`,
`DEFAULT_ERROR_PAGE`, `RETRY_AFTER`— presente pero no interpretable SHALL abortar el arranque con un error
que nombre la variable. Un `THEME` que no sea uno de los temas incorporados —o la cadena vacía— SHALL
abortar el arranque con un error que nombre `THEME`, en vez de degradar en caliente al tema por defecto.

Que el directorio de `CONTENT_DIR` no exista NO SHALL impedir el arranque, ni cambiar el `200` de un asset
embebido, ni degradar ninguna página de error a un `500`: los recursos embebidos cubren ese caso.

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
- **THEN** el arranque SHALL fallar con un error que nombre `THEME` y enumere los temas válidos

### Requirement: Los endpoints de servicio siempre responden 200

Los endpoints `/healthz`, `/health`, `/health/live` y `/live` SHALL responder siempre `200`, sin pasar por la resolución de páginas de error. `GET /version` SHALL devolver `{"version":"..."}` en JSON.

#### Scenario: la salud no es una página de error
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /healthz`
- **THEN** SHALL responder `200`
- **AND** el cuerpo NO SHALL ser una página de error

#### Scenario: la versión es JSON
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /version`
- **THEN** SHALL responder `200` con `Content-Type` JSON
- **AND** el cuerpo SHALL incluir la clave `version`

### Requirement: Los assets estáticos se sirven desde CONTENT_DIR

Las peticiones que no son páginas de error ni endpoints de servicio SHALL servirse como ficheros estáticos: el fichero de `CONTENT_DIR` cuando exista y se pueda leer, con su `Content-Type` correspondiente, y, si no, el asset **embebido** equivalente si lo hubiera. Un fichero de `CONTENT_DIR` **que se pueda leer** SHALL ganar al asset embebido del mismo path.

Un asset embebido NO SHALL ser una página de error: una página HTML de error se sirve por la vía de la página de error, con el código resuelto, nunca como asset y nunca con un `200`. La hoja `css/error.css` SHALL estar embebida, de modo que el HTML embebido que la referencia se sirva completo.

#### Scenario: un CSS se sirve desde el directorio de contenido
- **GIVEN** el fichero `content/css/404.css`
- **WHEN** se pide `GET /css/404.css`
- **THEN** SHALL responder `200`
- **AND** el `Content-Type` SHALL ser `text/css`

#### Scenario: la hoja embebida se sirve sin directorio de contenido
- **GIVEN** un servicio arrancado sin ningún fichero en `CONTENT_DIR`
- **WHEN** se pide `GET /css/error.css`
- **THEN** SHALL responder `200`
- **AND** el `Content-Type` SHALL ser `text/css`
- **AND** el cuerpo NO SHALL estar vacío
- **AND** NO SHALL ser una página de error

#### Scenario: el fichero del directorio gana al asset embebido
- **GIVEN** un `CONTENT_DIR` con un `css/error.css` propio, distinto del embebido
- **WHEN** se pide `GET /css/error.css`
- **THEN** el cuerpo SHALL ser el de ese fichero
- **AND** NO SHALL ser el asset embebido

#### Scenario: un asset embebido no convierte una página de error en un 200
- **GIVEN** un servicio arrancado sin ningún fichero en `CONTENT_DIR`
- **WHEN** se pide `GET /404.html`
- **THEN** el estado SHALL ser `404`
- **AND** NO SHALL ser `200`

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

### Requirement: Un tag publica el crate además de la imagen

Empujar una etiqueta `v*` SHALL publicar la imagen en GHCR **y** el crate en crates.io. La versión de
`Cargo.toml` SHALL coincidir con la de la etiqueta: si no coinciden, la publicación SHALL fallar
nombrando el desajuste en vez de publicar una versión que no es la que se anuncia. Esa comprobación
SHALL ocurrir **antes** de publicar nada —ni imagen, ni Release, ni crate—, no al final del release.
Publicar en crates.io es **irreversible**, así que el crate NO SHALL publicarse antes que la imagen:
SHALL depender de que la imagen se haya construido, probado y publicado. Y el crate SHALL salir del
**árbol de la etiqueta**, no de la punta de la rama. Publicar NO SHALL romperse al re-ejecutarse
sobre una etiqueta cuya versión ya está en el registro: SHALL detectarlo y continuar. Y el paquete
SHALL verificarse como publicable en cada propuesta de cambio, no por primera vez al publicar.

#### Scenario: una etiqueta publica las dos cosas
- **GIVEN** una etiqueta `v*` cuya versión coincide con la de `Cargo.toml`
- **WHEN** se empuja la etiqueta
- **THEN** la imagen SHALL quedar publicada en GHCR
- **AND** el crate SHALL quedar publicado en crates.io

#### Scenario: el crate sale del árbol de la etiqueta
- **GIVEN** una etiqueta `vX.Y.Z` y una rama que ha avanzado desde entonces
- **WHEN** se publica el crate con el disparador manual
- **THEN** el crate SHALL empaquetarse desde el commit de la etiqueta
- **AND** NO SHALL publicarse el árbol de la rama

#### Scenario: la versión y la etiqueta no coinciden
- **GIVEN** `Cargo.toml` en `0.4.0` y una etiqueta `v0.4.1`
- **WHEN** se ejecuta la publicación
- **THEN** SHALL fallar nombrando el desajuste
- **AND** NO SHALL publicarse nada en absoluto: ni la imagen en GHCR, ni el Release, ni el crate

#### Scenario: la etiqueta no llega al shell como código
- **GIVEN** una etiqueta con caracteres de shell en el nombre
- **WHEN** corre el release
- **THEN** SHALL rechazarse por su forma, **sin que ningún fragmento de la etiqueta llegue a
  ejecutarse**
- **AND** NO SHALL quedar publicado nada

#### Scenario: el crate no se publica si la imagen falla
- **GIVEN** un fallo al construir o al probar la imagen
- **WHEN** el workflow termina
- **THEN** el crate NO SHALL quedar publicado

#### Scenario: republicar una etiqueta ya publicada no falla
- **GIVEN** una etiqueta cuya versión ya está en crates.io
- **WHEN** se republica con el disparador manual
- **THEN** el paso SHALL detectarlo y continuar sin error
- **AND** NO SHALL fallar **aunque lo publicado en el registro venga de otro commit**: SHALL avisar

#### Scenario: el paquete se verifica en cada propuesta de cambio
- **GIVEN** una propuesta de cambio
- **WHEN** corre el CI
- **THEN** el paquete SHALL verificarse como publicable **sin subirlo**

### Requirement: El respaldo no publica el endpoint de servicio `/metrics`

Toda definición de router de respaldo que este repositorio entregue —los labels del ejemplo, los
labels de la plantilla de producción, el fragmento de *provider de fichero* y el bloque de labels del
`README.md` raíz— SHALL excluir
`/metrics` de su regla: `HostRegexp(`.+`) && !Path(`/metrics`)`. Una exclusión que sólo aparezca tras
un `#` de cola NO SHALL contar: en YAML ese `#` abre un comentario, así que el **valor** de la regla
se queda sin ella y el respaldo vuelve a publicar `/metrics`. Un Host que no case con ningún otro
router NO SHALL recibir el contador, ni en el estado ni en el cuerpo, porque `/metrics` publica el
perfil de fallos de la infraestructura y la capacidad `observability` ya exige que sea un endpoint de
servicio y no una página de error.

La exclusión NO SHALL llevarse por delante el respaldo: en un Host desconocido, cualquier otro path
—`/` incluido— SHALL seguir recibiendo la página de error con el código real y sus cabeceras.

Una regla con la sintaxis de Traefik **v2** —un placeholder `{nombre:regexp}` dentro del `HostRegexp`,
como ``HostRegexp(`{host:.+}`)``— NO SHALL contar como definición de router de respaldo: no casa con
ninguna petición, así que no publica `/metrics` ni nada, y exigirle la exclusión sería exigírsela a un
router que no lo es.

Una prueba automática SHALL guardar la propiedad sobre los ficheros versionados, porque la
comprobación de extremo a extremo necesita un motor de contenedores y NO corre en el CI.

#### Scenario: el respaldo no sirve métricas
- **GIVEN** el ejemplo levantado, con Traefik delante y un Host que no case con ningún otro router
- **WHEN** se pide `GET /metrics` a ese Host
- **THEN** el estado SHALL NO ser `200`
- **AND** el cuerpo NO SHALL contener `errors_pages_total`
- **AND** la respuesta NO SHALL llevar `X-Robots-Tag`, que es la firma de una página servida por el
  servicio; se cumple también con la fuga abierta —`/metrics` es un endpoint de servicio y nunca lleva
  esa cabecera—, así que es una guarda contra otro modo de regresión, convertir `/metrics` en una
  página de error, y quien discrimina el arreglo es el estado, el `Content-Type` y el cuerpo
- **AND** la respuesta NO SHALL declarar el `Content-Type` del endpoint de métricas (`text/plain; version=0.0.4`)

#### Scenario: la exclusión no rompe el respaldo
- **GIVEN** el ejemplo levantado
- **WHEN** se pide `GET /` y `GET /503` a un Host que no case con ningún otro router
- **THEN** los estados SHALL ser `404` y `503`
- **AND** el cuerpo SHALL ser la página de ese código
- **AND** el `404` SHALL llevar `X-Robots-Tag` y el `503`, además, `Retry-After`

#### Scenario: ninguna definición de respaldo vuelve a publicar métricas
- **GIVEN** el repositorio versionado, con los tres ficheros que entregan un respaldo desplegable y el bloque de labels del `README.md` raíz
- **WHEN** se inspecciona la regla de cada router de respaldo que el repositorio define, en cualquier fichero versionado
- **THEN** cada una SHALL llevar la exclusión de `/metrics`
- **AND** los tres desplegables y el `README.md` raíz SHALL aportar al menos una definición cada uno, para que un renombrado del router no deje la prueba huérfana y en verde
- **AND** la prueba SHALL fallar nombrando el fichero que la haya perdido

#### Scenario: la exclusión ha de estar en la regla, no en un comentario
- **GIVEN** un fichero de despliegue cuya regla de respaldo lleva la exclusión sólo **después** de un `#` de cola, que en YAML abre un comentario
- **WHEN** se inspecciona el **valor** de la regla
- **THEN** esa definición SHALL considerarse incumplidora
- **AND** la prueba SHALL fallar nombrándola
