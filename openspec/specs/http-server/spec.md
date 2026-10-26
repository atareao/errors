# http-server Specification

## Purpose
El contrato de arranque y servicio: qué variables de entorno se leen y con qué valores por defecto, que el puerto por defecto es el `8080` que asume la guía de Traefik, que los endpoints de salud y de versión responden siempre sin pasar por la resolución de páginas de error, y que los ficheros de `CONTENT_DIR` se sirven como assets estáticos con su `Content-Type`.

## Requirements

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

### Requirement: La imagen se construye desde el árbol y trae las páginas dentro

La imagen SHALL construirse con `docker build` desde el árbol del repositorio, sin depender de nada que no esté versionado. El stage de **compilación** SHALL disponer de `content/` —las **siete** páginas de tema, `default.html`, `minimal.html`, `terminal.html`, `paper.html`, `amber.html`, `compact.html` y `matrix.html`—, porque el binario las embebe al compilar y `cargo` NO SHALL poder compilar el crate sin ellas. Que falten NO SHALL descubrirse por primera vez al publicar una etiqueta: el CI SHALL construir la imagen **y SHALL probarla arrancada** en cada propuesta de cambio. Construir NO SHALL contar como probar: una imagen que compila y no responde el contrato del servicio SHALL poner la puerta en rojo.

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

#### Scenario: la propuesta de cambio prueba la imagen, no solo la construye

- **GIVEN** una propuesta de cambio cuyo job `imagen` construye la imagen y no la arranca
- **WHEN** se evalúa la puerta del CI
- **THEN** la puerta SHALL considerarse incumplida
- **AND** el smoke test SHALL haberse corrido contra la imagen recién construida

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

### Requirement: El smoke test del servicio es uno solo

Las aserciones sobre la **imagen arrancada** —el estado de sus respuestas, sus cabeceras, el cuerpo del HTML, el usuario con el que corre y su healthcheck— SHALL vivir en **un solo artefacto versionado**, `.github/scripts/smoke.sh`, y los dos workflows que las necesitan SHALL **invocarlo**: el del CI en cada propuesta de cambio, después de construir la imagen, y el del release **antes** de publicar nada. Copiar una aserción en un `run:` de un workflow NO SHALL contar como cumplirlo: dos copias divergen, y la `v0.8.0` falló contra una aserción propia que afirmaba `GET /css/error.css -> 200`, el asset que ese mismo release retiraba.

El script SHALL recibir el motor de contenedores y la referencia de la imagen por el entorno —`docker` y el motor por defecto—, porque la máquina de desarrollo de este proyecto no tiene `docker` y sí `podman`, y el motor NO SHALL hardcodearse. SHALL arrancar el contenedor en loopback, esperar a que `/healthz` responda, comprobar el contrato y **retirar el contenedor** al terminar, también cuando una aserción falle. SHALL nombrar la aserción que falla y el valor que devolvió.

El script NO SHALL construir la imagen ni publicar nada: recibe una imagen ya construida, y publicar SHALL seguir siendo del workflow del release. Su lista de códigos SHALL incluir el `502` con la aserción negativa de `Retry-After` que pide el requisito «El código con el que un proxy responde a un upstream caído se ejercita».

Una prueba automática SHALL guardar la propiedad sobre los ficheros versionados, porque la divergencia que este requisito prohíbe es una propiedad de esos ficheros y no del servicio.

#### Scenario: el CI prueba la imagen en cada propuesta de cambio

- **GIVEN** una propuesta de cambio que construye la imagen
- **WHEN** corre la puerta `imagen` del CI
- **THEN** el smoke test SHALL correrse contra esa imagen
- **AND** un fallo del smoke test SHALL poner la puerta en rojo

#### Scenario: las aserciones no se copian en los workflows

- **GIVEN** un workflow que define o llama a los ayudantes de aserción del smoke test —`comprobar(`, `cabecera(`, `fallo `— fuera del script, o que lleva su propio bucle de arranque contra el contenedor
- **WHEN** se inspecciona el repositorio versionado
- **THEN** ese workflow SHALL considerarse incumplidor
- **AND** la prueba SHALL fallar nombrando el fichero

#### Scenario: los dos workflows invocan el mismo script

- **GIVEN** los dos workflows que necesitan probar la imagen
- **WHEN** se inspecciona lo que invocan
- **THEN** los dos SHALL invocar `.github/scripts/smoke.sh`
- **AND** un workflow que no lo invoque SHALL considerarse incumplidor, y la prueba SHALL nombrarlo

#### Scenario: el smoke test corre también sin docker

- **GIVEN** una máquina sin `docker` y con `podman`
- **WHEN** se corre el script con el motor y la imagen por el entorno
- **THEN** SHALL comprobar el mismo contrato
- **AND** el `HEALTHCHECK` SHALL comprobarse cuando la imagen lo declare
- **AND** cuando la imagen no lo declare —`podman` construye en formato OCI, que descarta el `HEALTHCHECK`— SHALL decirlo en voz alta y NO SHALL contarlo como fallo

#### Scenario: el contenedor de prueba no queda vivo

- **GIVEN** una aserción que falla y el contenedor ya arrancado
- **WHEN** el script termina
- **THEN** el contenedor de prueba NO SHALL quedar corriendo
- **AND** el script SHALL salir con un estado distinto de cero

### Requirement: El respaldo no publica las páginas de tema crudas

Toda definición de router de respaldo que este repositorio entregue —los labels del ejemplo, los labels de la plantilla de producción, el bloque de labels del `README.md` raíz y las **dos** definiciones que ilustra `examples/traefik/README.md`: la del bloque de prosa y la del fragmento de *provider de fichero*— SHALL impedir que el respaldo sirva los paths de las **páginas de tema** crudas: ``HostRegexp(`.+`) && !Path(`/metrics`) && (!PathRegexp(`\.html?((%2[fF])|/|$)`) || Path(`/default.html`) || Path(`/default.htm`))``. El motivo es que el nombre del tema **es** la ruta de su página (`content/<nombre>.html`): las seis que no son el alias se sirven como assets **crudos**, con los marcadores `{{code}}`/`{{reason}}`/`{{description}}` a la vista, y un Host sin router propio las publica a cualquiera.

La exclusión SHALL cubrir las **tres** formas con las que un cliente alcanza esa página cruda, porque las tres están **medidas** con `200` y con los marcadores a la vista contra Traefik v3.6.25: el path terminado en `.html`/`.htm`, el mismo con **barra final** o con un segmento detrás (`/minimal.html/`, `/matrix.html/`) y el mismo con la barra **percent-encoded**, en las dos cajas (`/minimal.html%2F`, `/matrix.html%2f`). Una exclusión anclada al fin de cadena —``!PathRegexp(`\.html?$`)``— NO SHALL contar como cumplirla: medida, dejaba **diecisiete** formas sirviendo el fichero crudo.

La rama `%2[fF]` NO SHALL retirarse por parecer redundante. Es un hecho **medido**: Traefik contrasta la regla contra el path de la petición y normaliza los escapes de los caracteres **no reservados** —`GET /minimal%2Ehtml` y `GET /minimal.htm%6C` quedan bloqueados porque los lee como `minimal.html`— pero NO SHALL normalizar el de la **barra**, que sobrevive como `%2F`; sin esa rama, la familia `%2F` sigue sirviendo el asset crudo. La exclusión NO SHALL depender de la configuración del *entrypoint*: medido, la tabla de resultados es idéntica con `sanitizePath` en su valor por defecto y con `sanitizePath=false`, mientras que la forma que solo hace opcional la barra final **sí** filtra con `sanitizePath=false`.

`/default.html` y `/default.htm` SHALL quedar **fuera** de esa exclusión, porque no son un leak: su path se clasifica como el código por defecto, el servicio lo intercepta y responde el código resuelto, **nunca `200`**. Excluirlos quitaría la única página de tema que no filtra.

La conjunción NO SHALL llevarse por delante el respaldo: en un Host desconocido, `GET /` y `GET /503` SHALL seguir recibiendo la página de su código con sus cabeceras. Y el precio SHALL quedar escrito: la conjunción manda a la `404` de Traefik todo path cuyo `.html`/`.htm` vaya seguido de fin de cadena, de `/` o de `%2F` —`GET /404.html`, `GET /503.html`, `GET /sub/x.html`, `GET /x.html/y`, `GET /default.html%2F`—, lo que NO SHALL romper el flujo real, porque el middleware `errors` pide `/404` **sin** extensión (`errors.query: /{status}`).

El matcher `PathRegexp` SHALL haberse medido contra la versión de Traefik que el ejemplo fija, porque una sintaxis que no case con nada deja el router `enabled` sin bloquear lo que debía bloquear, en silencio.

La regla SHALL escribirse de modo que YAML la lea como se pretende: en un valor **entrecomillado** la barra invertida va doblada, y escribirla simple ahí NO SHALL contar —el parser falla con `found unknown escape character` y el *provider* **entero** deja de cargar, con el contenedor arriba y sin ningún router, en silencio—. Los sitios que este repositorio entrega la escriben como escalar **plano**, que es donde la barra va simple y donde los dos `$` de la regla llegan a Traefik verbatim.

Una prueba automática SHALL guardar la propiedad sobre los ficheros versionados, con el mismo método que la exclusión de `/metrics`, porque la comprobación de extremo a extremo necesita un motor de contenedores y NO corre en el CI.

Límite declarado y **medido**: `PathRegexp` es sensible a mayúsculas, así que `GET /MINIMAL.HTML` no casa con la exclusión y llega al servicio. NO SHALL contarse como incumplimiento: el servicio responde **su página de error** —no el fichero crudo—, porque el sistema de ficheros distingue mayúsculas y `ServeDir` contesta `404`. Es una dependencia del sistema de ficheros, no una propiedad del servicio.

Segundo límite, declarado y **medido**: la regla se contrasta contra el path tal como Traefik lo presenta, así que las formas que Traefik **normaliza** las cierra la normalización y no la regla (`/minimal%2Ehtml`, `/minimal.htm%6C`), y las que sobreviven a ella sin llevar una barra (`/minimal.html%3F`, `/minimal.html%252E`, `/minimal%2Fhtml`) llegan al servicio y reciben **su** `404`, sin marcadores. Ninguna prueba textual SHALL intentar demostrar que la regla cierra **todas** las codificaciones: eso lo mide `examples/traefik/check.sh` con el ejemplo levantado, forma a forma.

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

### Requirement: El código con el que un proxy responde a un upstream caído se ejercita

El `502` es el código con el que un proxy responde cuando no puede hablar con la aplicación que tiene detrás, y es el escenario que justifica este servicio: el proxy no puede conservar el código del upstream porque no hay respuesta que conservar, así que el código lo decide el proxy y la página la sirve este servicio. La capacidad `error-page` ya exige que todo código del rango `400..=599` tenga página y nombra el `502` entre los que un proxy emite; lo que este requisito añade es que **algo lo ejercite**.

El smoke test SHALL incluir el `502` en su lista de códigos y SHALL comprobar que su respuesta NO lleva `Retry-After`, porque esa cabecera describe el estado que se sirve y solo `429` y `503` la llevan. El smoke test NO SHALL intentar producir el `502` con un proxy —no lo tiene delante—: comprueba el código que sirve el servicio.

El camino de extremo a extremo —un upstream que **rechaza** la conexión, el proxy que emite `502` y el middleware `errors` que lo traduce a la página del servicio— SHALL quedar ejercitado en `examples/traefik/check.sh`, que es donde se puede medir: el CI no levanta contenedores. El caso NO SHALL construirse parando el contenedor del backend, porque un servicio **sin servidores** disponibles produce `503` y no `502` (medido): SHALL apuntar a un servidor declarado que rechaza la conexión.

Una prueba automática SHALL guardar que el caso sigue escrito en `check.sh`, porque un caso que desaparece en silencio deja el escenario del proyecto sin ninguna puerta.

#### Scenario: el smoke test cubre el código de un upstream caído

- **GIVEN** la imagen arrancada
- **WHEN** se pide `GET /502`
- **THEN** el estado SHALL ser `502`
- **AND** SHALL llevar `X-Robots-Tag`
- **AND** NO SHALL llevar `Retry-After`

#### Scenario: el proxy caído acaba en la página del servicio

- **GIVEN** el ejemplo levantado, con un router cuyo upstream **rechaza** la conexión y el middleware `errors` delante
- **WHEN** se pide un path a ese Host
- **THEN** el estado SHALL ser `502`
- **AND** el cuerpo SHALL ser la página del servicio para el `502`, no el error del proxy

#### Scenario: el caso del upstream caído no se pierde

- **GIVEN** el repositorio versionado
- **WHEN** se inspecciona `examples/traefik/check.sh`
- **THEN** SHALL contener el caso del `502` por un upstream que rechaza la conexión
- **AND** un `check.sh` que lo haya perdido SHALL considerarse incumplidor, y la prueba SHALL nombrarlo
