## ADDED Requirements

### Requirement: Las páginas del proyecto se descubren al compilar

El conjunto de páginas del proyecto SHALL descubrirse al compilar: `build.rs` lee el directorio `content/`, filtra los ficheros `*.html` cuyo tallo sea UTF-8 no vacío y los ordena por nombre para que el build sea determinista. El binario SHALL embeber lo que haya, sin lista ni recuento fijo en el código. Si `content/` no existe o no contiene ningún `.html`, la compilación SHALL abortar con un mensaje claro: el crate siempre lleva sus temas.

El conjunto de páginas embebidas NO SHALL fijarse en el código: su número y sus nombres SHALL derivarse de los ficheros presentes en `content/` al compilar. Añadir un tema SHALL ser dejar un fichero `*.html` en `content/` y reconstruir. NO SHALL verificarse el contenido de ninguna página al compilar ni al servir: las páginas son opacas y el servicio no las juzga. El motivo de esta decisión SHALL quedar escrito en `design.md`.

#### Scenario: añadir un tema es dejar el fichero y reconstruir
- **GIVEN** el directorio `content/` con sus páginas
- **WHEN** se añade un `nuevo.html` y se reconstruye el binario
- **THEN** el tema `nuevo` SHALL estar disponible sin editar ninguna lista
- **AND** su página SHALL ser la de `content/nuevo.html`

#### Scenario: sin content/ la compilación aborta
- **GIVEN** un árbol sin el directorio `content/`
- **WHEN** se compila el crate
- **THEN** la compilación SHALL abortar con un mensaje que nombre el directorio
- **AND** el binario NO SHALL generarse

#### Scenario: content/ sin html aborta
- **GIVEN** un `content/` sin ningún `.html`
- **WHEN** se compila el crate
- **THEN** la compilación SHALL abortar con un mensaje que explique que el crate siempre lleva sus temas

### Requirement: El tema del HTML elige una página del registro

Un **tema** SHALL ser **una página**: su **nombre** SHALL ser el de su fichero y la ruta SHALL derivarse de él —`CONTENT_DIR/<nombre>.html`, en la **raíz** de `CONTENT_DIR`—, de modo que no exista una tabla de rutas que pueda divergir del nombre. El conjunto de páginas SHALL descubrirse al compilar desde `content/*.html` (requisito «Las páginas del proyecto se descubren al compilar»): un servicio arrancado sin `content/` sirve los diseños embebidos. El motivo SHALL quedar escrito: el usuario rechazó el modelo «tema = solo CSS», porque un tema que quiere entregar una experiencia —su marcado, su `<script>`, su `<canvas>`— no cabe en una hoja, y una página por tema es mantener los diseños, no uno por código.

La página de un tema SHALL resolverse **como un asset**: un fichero **legible** en `CONTENT_DIR/<nombre>.html` SHALL ganar a la copia embebida; un fichero **ausente** SHALL caer a la copia embebida **sin** dejar constancia en el registro; un fichero **presente pero ilegible** SHALL dejar un `warn!` que nombre el fichero y su causa y SHALL responder el respaldo de texto con el estado intacto (requisito «Un fallo al leer el HTML no cambia el estado»). La página se lee **al servir**, así que un operador que edite la suya la ve cambiar sin reiniciar.

El **punto de personalización** del operador SHALL ser la página del tema: `content/default.html` para el tema por defecto. El operador NO SHALL conservar la capa por código —la pierde con este modelo— y NO SHALL tener una hoja suelta que incrustar.

El **nombre** de un tema descubierto SHALL ser el **tallo** de su fichero, y la comparación de nombres SHALL seguir siendo **exacta**. Las **reglas** de cada página PUEDEN cambiar de una versión a otra: son del proyecto y su aspecto no es un contrato; lo que sí lo es, y SHALL comprobarse, es el **nombre** del tema y la **ruta** de su página.

Un tema **descubierto** NO SHALL tener copia embebida: su fichero es su **única** copia. Si en el momento de servir ese fichero ha **desaparecido**, la respuesta SHALL ser el **respaldo de texto** con el estado intacto y SHALL dejar constancia nombrando el fichero —al revés que un tema embebido ausente, que cae a su copia embebida **sin** dejar constancia—. El motivo SHALL quedar escrito: sin copia a la que caer, el silencio escondería un fichero que se ha esfumado.

El nombre que llega en **`X-Theme` NO SHALL usarse nunca para componer una ruta**. La ruta SHALL salir del **registro**, que se construye del barrido del directorio. El motivo SHALL quedar escrito: si se implementara como `content_dir.join(format!("{}.html", x_theme))`, un `X-Theme: ../../etc/passwd` sería una lectura arbitraria de ficheros.

La variable de entorno `THEME` SHALL elegir el tema del servicio, con `default` por defecto. Un valor que no sea uno de los nombres del **registro**, **o la cadena vacía**, SHALL abortar el arranque con un error que **nombre `THEME`** y que **enumere los nombres del registro** —que con un `CONTENT_DIR` vacío son los embebidos—, como `LOG_FORMAT`: es una variable que escribe el operador, y quien la escribe cree que está configurando algo.

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
- **AND** el mensaje SHALL enumerar los nombres del registro

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

#### Scenario: el conjunto de temas se descubre al compilar
- **GIVEN** un árbol con `content/` y sus ficheros `*.html`
- **WHEN** se compila el binario
- **THEN** el conjunto de temas SHALL derivarse de los ficheros presentes en `content/`
- **AND** NO SHALL existir una lista ni un recuento fijo en el código
- **AND** añadir un `nuevo.html` a `content/` y reconstruir SHALL añadir el tema `nuevo` al conjunto

#### Scenario: cada tema declara su página y el tallo del fichero es su nombre
- **GIVEN** el registro de temas embebidos al compilar
- **WHEN** se comprueba la ruta de página de cada tema
- **THEN** la ruta SHALL ser `<nombre>.html`
- **AND** cada tema SHALL tener un fichero en `content/` con ese nombre

#### Scenario: un fichero de más en CONTENT_DIR es un tema
- **GIVEN** un `CONTENT_DIR` con `neon.html` y `THEME=neon`
- **WHEN** se pide `GET /404` con `Accept: text/html`
- **THEN** la página SHALL ser la de `CONTENT_DIR/neon.html`
- **AND** NO SHALL ser la del tema por defecto

#### Scenario: un tema descubierto se elige por X-Theme
- **GIVEN** un servicio con el tema `default` y un `CONTENT_DIR` con `neon.html`
- **WHEN** se pide `GET /404` con `X-Theme: neon`
- **THEN** la página SHALL ser la de `neon`

#### Scenario: el fichero de un tema descubierto desaparecido responde el respaldo
- **GIVEN** un tema descubierto cuyo fichero se borra entre el arranque y la petición
- **WHEN** se pide una página HTML de ese tema
- **THEN** el estado SHALL ser el código resuelto
- **AND** el cuerpo SHALL ser el respaldo de texto
- **AND** NO SHALL ser `500`

#### Scenario: el nombre de X-Theme no compone ninguna ruta
- **GIVEN** una petición con `X-Theme: ../../etc/passwd`
- **WHEN** se responde
- **THEN** la página SHALL ser la del tema configurado
- **AND** NO SHALL leerse ningún fichero fuera de `CONTENT_DIR`

### Requirement: Cada página de tema se identifica con la clase de su código

El elemento `body` de cada página de tema PUEDE llevar la clase `c<código>` del **código resuelto** —`c404`, `c451`—, de modo que la apariencia pueda diferenciarse por código con el CSS de la propia página y sin duplicarla. La clase SHALL quedar garantizada por la **plantilla**, que la rellena al servir con el código resuelto. NO SHALL verificarse el contenido de las páginas: las páginas PUEDEN llevar los marcadores o no. El motivo SHALL quedar escrito en `design.md`: las páginas son opacas y el servicio no las juzga.

#### Scenario: la página del 504 se identifica como tal
- **GIVEN** un servicio con la página del proyecto y sin fichero propio para `504`
- **WHEN** se sirve su página HTML
- **THEN** el elemento `body` SHALL llevar la clase `c504`

#### Scenario: la clase del cuerpo es la del código pedido aunque no tenga fichero propio
- **GIVEN** un servicio sin fichero para `451`
- **WHEN** se sirve su página HTML
- **THEN** el `body` SHALL llevar la clase `c451`

#### Scenario: cada página se identifica con su clase
- **GIVEN** las páginas de `content/`
- **WHEN** se sirve cada una con un código resuelto
- **THEN** el `body` de cada una SHALL llevar la clase `c<código>` del código resuelto
- **AND** NO SHALL haber una página sin la clase

### Requirement: El directorio es el registro

Al arrancar, el servicio SHALL construir el **registro efectivo** de temas: los **embebidos** al compilar desde `content/*.html` **más** los **descubiertos** en la **raíz** de `CONTENT_DIR`. Los embebidos NO SHALL retirarse: un `cargo install` sigue sirviendo los diseños sin ficheros de datos, y el directorio **añade**, no sustituye.

SHALL descubrirse un fichero **regular** —o un enlace simbólico que apunte a uno legible— cuya extensión sea **exactamente** `html` (ni `HTML`, ni `htm`, ni `css`). El nombre del tema SHALL ser su **tallo** (`neon.html` → `neon`), y **NO SHALL** descubrirse nada en **subdirectorios**: el nombre de un tema no es una ruta. El motivo de mirar solo la raíz SHALL quedar escrito: un tema es **una página** con un nombre, y admitir rutas convertiría el nombre en una ruta que el registro tendría que validar.

Un fichero cuyo tallo sea el de un **embebido** NO SHALL añadir un tema: SHALL ser la página de ese tema en el disco, que es la regla de siempre —un fichero legible gana a la copia embebida—. Así `content/default.html` **no** crea un `default` de más, sino que es la página del `default` embebido.

Un fichero cuyo tallo **reclame la resolución de códigos** NO SHALL ser un tema: hoy eso es **cualquier tallo que se pueda leer como un `u16`** —`404` y `451`, pero también `999`, `200` o `0`—, porque el clasificador decide con `code_from_path`, que **parsea el número y NO filtra el rango**: el `400..=599` se aplica al **resolver** el código, no al **clasificar** el path. Su path, por tanto, NO SHALL servirse como asset: `/404.html` responde `404`, y `/999.html` y `/200.html` responden el **código por defecto**, que es una página de error **distinta** de la suya. El servicio SHALL dejarlo fuera del registro y SHALL dejar constancia nombrando el fichero, para que el operador sepa por qué su `404.html` no es un tema.

Un `CONTENT_DIR` que **no exista** SHALL dar un registro con los embebidos y **sin** error: el binario sin ficheros de datos sirve los diseños, y el arranque NO SHALL fallar. Un `CONTENT_DIR` que exista y **no se pueda listar** SHALL dejar constancia nombrando el directorio y su causa, y el registro SHALL quedarse con los embebidos: NO SHALL impedir el arranque. El reparto es el mismo que el de las páginas: lo que falta no tumba el servicio, lo que existe y no se puede usar se delata.

El barrido SHALL ocurrir **una vez, al arrancar**. Un fichero añadido con el servicio en marcha NO SHALL ser un tema hasta el siguiente arranque, y SHALL seguir sirviéndose como **asset crudo** con `200` —la regla de los ficheros de `CONTENT_DIR`—. Los **bytes** de la página SHALL seguir leyéndose **al servir**, así que **editar** el contenido de un tema SHALL verse sin reiniciar. El motivo SHALL quedar escrito: validar `THEME` exige una lista **en el arranque**, y un barrido por petición pondría un `read_dir` en el camino de cada página de error, que es justo cuando más se piden.

El servicio NO SHALL inspeccionar el contenido de ninguna página: las páginas son opacas, tanto las embebidas como las descubiertas. NO SHALL haber guardas de red, movimiento ni cobertura de marcado. El motivo SHALL quedar escrito en `design.md`.

#### Scenario: un fichero de más es un tema
- **GIVEN** un `CONTENT_DIR` con `neon.html` y un registro con los embebidos
- **WHEN** se construye el registro al arrancar
- **THEN** `neon` SHALL estar entre los nombres del registro

#### Scenario: un fichero con el nombre de un embebido no añade un tema
- **GIVEN** un `CONTENT_DIR` con `default.html`
- **WHEN** se construye el registro
- **THEN** SHALL haber **un solo** `default`
- **AND** su página SHALL ser la del disco, no la copia embebida

#### Scenario: un fichero con nombre de código no es un tema
- **GIVEN** un `CONTENT_DIR` con `404.html`, `999.html` y `200.html`
- **WHEN** se construye el registro
- **THEN** `404`, `999` y `200` NO SHALL estar entre los nombres del registro
- **AND** SHALL dejar constancia nombrando los ficheros

#### Scenario: solo se mira la raíz de CONTENT_DIR
- **GIVEN** un `CONTENT_DIR` con `temas/neon.html`
- **WHEN** se construye el registro
- **THEN** `neon` NO SHALL ser un tema
- **AND** el fichero SHALL seguir sirviéndose como asset

#### Scenario: la extensión tiene que ser html
- **GIVEN** un `CONTENT_DIR` con `neon.htm`, `neon.HTML` y `neon.css`
- **WHEN** se construye el registro
- **THEN** ninguno de los tres SHALL ser un tema

#### Scenario: sin directorio de contenido el registro son los embebidos
- **GIVEN** un `CONTENT_DIR` que no existe
- **WHEN** se construye el registro
- **THEN** los nombres SHALL ser los embebidos
- **AND** el arranque NO SHALL fallar

#### Scenario: un directorio ilegible no impide el arranque
- **GIVEN** un `CONTENT_DIR` que existe y no se puede listar
- **WHEN** se construye el registro
- **THEN** el registro SHALL quedarse con los embebidos
- **AND** SHALL dejar constancia nombrando el directorio y su causa
- **AND** el arranque NO SHALL fallar

#### Scenario: añadir un tema con el servicio en marcha no lo convierte en tema
- **GIVEN** un servicio arrancado sin `neon.html` en `CONTENT_DIR`
- **WHEN** se crea el fichero con el servicio en marcha
- **THEN** `THEME=neon` NO SHALL ser válido hasta reiniciar
- **AND** `GET /neon.html` SHALL seguir siendo un asset crudo con `200`

## REMOVED Requirements

### Requirement: El tema del HTML elige una página

**Reason**: The number and names of the themes are no longer fixed in the code. The set of themes is now discovered at compile time from `content/*.html`, and `CONTENT_DIR` adds themes to the registry. The fixed "seven" model contradicts the new discovery mechanism. The replacement requirement is "El tema del HTML elige una página del registro" in the ADDED section.

### Requirement: Cada página se identifica con la clase de su código

**Reason**: The set of pages is no longer a fixed count of seven. The set is discovered at compile time from `content/*.html`, so a requirement tied to "seven" pages is obsolete. The replacement requirement is "Cada página de tema se identifica con la clase de su código" in the ADDED section, which covers the same mechanism without asserting a fixed count.

### Requirement: Las siete páginas del proyecto

**Reason**: The number and names of the project's pages are now determined at compile time by the files present in `content/`. Fixing "seven" in the code contradicts the new discovery model.

### Requirement: El directorio añade temas al registro

**Reason**: The theme registry is no longer built from a fixed list of themes. The embedded themes are discovered at compile time from `content/`, and `CONTENT_DIR` adds its own themes to the registry. The old fixed-list model contradicts the new discovery mechanism.

### Requirement: Ninguna página pide nada por la red

**Reason**: The pages are opaque. The service does not judge their content. This contract is removed without replacement.

### Requirement: Una página que anima puede apagarse

**Reason**: The pages are opaque. The service does not judge their content. This contract is removed without replacement.
