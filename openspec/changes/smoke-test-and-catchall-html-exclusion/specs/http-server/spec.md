# http-server Specification (delta)

## MODIFIED Requirements

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

## ADDED Requirements

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
