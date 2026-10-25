# Design: embedded-error-pages

## Context

Ver `proposal.md` — Why. Lo que condiciona el diseño:

- **Un binario instalado no lleva ficheros de datos.** `cargo install errserve` deja un ejecutable y
  nada más: el directorio de trabajo está vacío y `CONTENT_DIR` apunta por defecto a un `content`
  que no existe. El servicio arranca igual —`ServeDir` no falla por un directorio ausente, responde
  `404`—, pero todo lo que sale del disco falta.
- **El HTML del disco es el único cuerpo que no se genera.** `render_body(code, default_code, Format::Html,
  content_dir)` → `html_body` → `read_html`, que devuelve `Ok(None)` cuando el fichero no existe;
  agotadas las dos lecturas, `RenderError::MissingHtml`. El router responde entonces
  `render_text_fallback(code, retry_after)`: cuerpo de texto **con el estado resuelto y las
  cabeceras de la página**, nunca un `500`.
- **Las once páginas enlazan a una sola hoja, `css/error.css`**, que se sirve como asset estático de
  `CONTENT_DIR` a través del *fallback* de `ServeDir`. No es un fichero del render: es un asset, y
  sin directorio de contenido no existe.
- **Un asset existente gana a la página de error.** El `200` de `ServeDir` se sirve tal cual; el
  middleware solo sustituye estado de error del servidor de ficheros. Y `/404.html` **no** llega a
  `ServeDir`: `is_error_page_path` lo clasifica como página de error antes de ceder el paso.
- **El crate no tiene *lib target*.** `tests/` es un crate aparte que solo puede leer ficheros por
  ruta (`tests/content_pages.rs` recorre `content/` con `CARGO_MANIFEST_DIR`); no puede ver ninguna
  constante del binario. Eso decide dónde vive la prueba de paridad (D5).
- **El repositorio compila sin red** (`cargo --offline`) y su puerta de dependencias es
  `cargo deny check -D warnings`: cualquier dependencia nueva es un coste real, no una línea en un
  `Cargo.toml`.

## Goals / Non-Goals

**Goals**

- Que un binario instalado sin ficheros de datos sirva las páginas de verdad y su hoja, con el mismo
  estado, el mismo `Content-Type` y las mismas cabeceras que sirve hoy con `content/` delante.
- Que la lista de códigos con página quede vigilada en **tiempo de compilación**, no por una prueba
  que alguien puede olvidar correr.
- Que el *override* del operador siga funcionando sin tocar una sola variable de entorno.

**Non-Goals**

- No se rediseña el render: `RenderedPage` sigue siendo la respuesta completa y lista para servir.
  Este change solo cambia **de dónde salen los bytes** del cuerpo HTML y de un asset.
- No se embebe nada que no esté ya en `content/`: no hay plantillas, ni temas, ni motor, ni
  parametrización de las páginas.
- No se decide aquí qué hacer con `content/` en la imagen de Docker (Open Question, abajo).

## Decisions

**D1. La capa del operador se intenta entera y, solo si no da nada, entra la capa embebida.** El
orden exacto es: fichero del código pedido en `CONTENT_DIR` → fichero del código por defecto en
`CONTENT_DIR` → copia embebida del código pedido → copia embebida del código por defecto →
`MissingHtml`. Es la cascada de hoy —código → defecto— sobre el disco y, después, la misma cascada
sobre la tabla embebida. `CONTENT_DIR` no cambia de nombre ni de defecto (`content`).
*Alternativa descartada:* intercalar las dos capas **por código** —disco del código, embebido del
código, disco del defecto, embebido del defecto—; rompe la lectura por capas («lo que hay en
`CONTENT_DIR` es lo que se sirve, y lo que no está, se respalda») y cambia lo que ve hoy un operador
con un directorio parcial: con su `404.html` propio y el defecto `404`, un `503` pasaría de servir
**su** página a servir la del binario sin que él haya tocado nada. El coste de esta decisión es el
simétrico y conviene tenerlo dicho: ese mismo operador seguirá viendo su `404.html` en el cuerpo de
un `503`, que es exactamente lo que ve hoy y lo que el requisito del código por defecto manda.

Ese «gana» hay que leerlo como **«gana cuando se puede leer»**: un fichero que existe y no se puede
abrir no desplaza siempre a la copia embebida, y las dos vías del change no degradan igual ante esa
causa. No se desarrolla aquí —va a la pregunta del final del documento, ya resuelta con la Opción A—;
lo único que hace falta aquí es que ningún texto de este change prometa un «gana siempre» que no se
cumple, y por eso el requisito dice «un fichero que se pueda leer» en vez de «siempre».

**D2. Se embebe también la hoja `css/error.css`, y se sirve por la misma vía que un asset de disco:
el fichero de `CONTENT_DIR` cuando exista y se pueda leer, la copia embebida si no.** Si solo se
embebieran las once páginas, el HTML embebido enlazaría a `/css/error.css` y esa petición —que no es
una página de error y no resuelve a ningún fichero— acabaría sustituida por la página de error del
código por defecto: un
`404` dentro del `404`, con la página sin estilo. El mecanismo es **`ServeDir::fallback`** —*tower-http*
**0.6.11**, la versión que fija `Cargo.lock`; `src/services/fs/serve_dir/mod.rs`, líneas 194-221—:
`ServeDir` sigue siendo el primero en intentarlo —así el fichero del operador gana cuando se pueda
leer, que es D1— y solo cuando no encuentra el fichero entra la tabla embebida, que responde el asset
—`200` con su `Content-Type`— si su path está en ella y `404` si no; ese `404`, como el de cualquier
asset inexistente, lo sustituye el middleware por la página de error. Que el respaldo pueda responder `200`
es exactamente lo que aporta `fallback`: no altera el estado que devuelve el servicio de respaldo
(doc del propio método, líneas 198-199: «The status code returned by the fallback **will not be
altered**»). La consulta se hace sobre el path **ya normalizado**, porque el middleware reescribió la
URI antes de ceder el paso, así que no hay una segunda forma de llegar al asset.

El respaldo se consulta «el fichero no está» y nada más, y eso también está medido en la misma
versión: entra con `FileNotFound`/`InvalidFilename` (`serve_dir/future.rs`, líneas 108-114) y con
`NotFound`, `PermissionDenied` y `ENOTDIR` (líneas 132-153); **cualquier otro error de E/S** sale como
`Err` y `ServeDir` lo convierte en su `500` de siempre (`mod.rs`, líneas 410-424), que el middleware
sigue sustituyendo por la página, así que la garantía «un segmento de path imposible da la página, no
un `500` vacío» no se toca. Y un método que no sea `GET` ni `HEAD` **no** llega al respaldo:
`call_fallback_on_method_not_allowed` vale `false` por defecto (`mod.rs`, líneas 81 y 244-249) y
`ServeDir` devuelve su `405` de hoy (líneas 317-327).

*Alternativa descartada:* `ServeDir::not_found_service`, que es la API que **el nombre sugiere** para
este caso y la que uno escribiría sin mirar la fuente. Fuerza el estado de la respuesta del respaldo a
`404`, y no por casualidad: en la misma versión, `not_found_service` es literalmente
`self.fallback(SetStatus::new(new_fallback, StatusCode::NOT_FOUND))` (`mod.rs`, líneas 240-242) y
`SetStatus` escribe el estado sobre la respuesta que recibe (`src/set_status.rs`, línea 134:
`*response.status_mut() = this.status...`), con su propia doc diciendo «The response status code will
be `status` **regardless of what the inner service returns**» (líneas 55 y 81). Con ella, el
`200 text/css` de la hoja embebida saldría como un `404` **con el cuerpo del CSS**: el middleware lo
tomaría por un estado de error del servidor de ficheros y lo sustituiría por la página de error, así
que el CSS embebido no se serviría nunca y el HTML embebido seguiría enlazando a una hoja que responde
la página de error —el «`404` dentro del `404`» que esta decisión existe para evitar, y la prueba 1.9
en rojo en GREEN—. *Alternativa descartada:* una ruta explícita en el router para `/css/error.css`
—ganaría **siempre** sobre `ServeDir`, de modo que la hoja del operador dejaría de servirse en
silencio, que es justo lo que D1 prohíbe—. *Alternativa descartada:* resolver el asset embebido dentro
del middleware de páginas de error —mezcla la tabla de assets con la decisión de qué es una página de
error, y repite en un `if` lo que `fallback` ya expresa—. *Alternativa descartada:* incrustar la hoja en
cada página (`<style>`) en vez de embeberla como asset —las páginas dejarían de compartir la hoja
única que el requisito exige, el HTML embebido divergiría del del repositorio y con él la paridad de
D5, y `/css/error.css` seguiría respondiendo la página de error para quien la pida directamente—.

**D3. `include_str!`/`include_bytes!` en tiempo de compilación, sin dependencia nueva ni formato
propio.** Tres consecuencias, todas comprobadas: (a) **cero dependencias** —el repositorio sigue
compilando con `--offline` y `cargo deny check -D warnings` no ve nada nuevo—; (b) **el fallo es de
compilación**, no de ejecución (D6); y (c) **funciona al construir desde el registro**: el `.crate`
publicado ya lleva `content/` dentro —verificado con `cargo package --list --offline`, que lista las
once páginas y `content/css/error.css`—, porque el `exclude` del manifiesto solo retira `openspec`,
`.github`, `AGENTS.md`, `cliff.toml`, `deny.toml` y `.justfile`. Sin ese hecho, `include_str!`
compilaría aquí y fallaría en la máquina de quien instala. *Alternativa descartada:* `rust-embed` o
`include_dir` —una dependencia nueva para hacer lo que la macro del compilador ya hace, y con
`-D warnings` cada dependencia se paga—. *Alternativa descartada:* un archivo empaquetado
(`.tar`) embebido y decodificado en ejecución —un formato propio, un fallo en ejecución en lugar de
en compilación, y el contenido habría que empaquetarlo igual—. *Alternativa descartada:* un *build
script* que genere un `.rs` con las tablas —añade una fase de compilación, un fichero generado que
nadie revisa, y los ficheros de origen tendrían que seguir en el paquete; se justifica cuando hay
cientos de ficheros o cuando hay que comprimirlos, y aquí hay doce—.

**D4. Se conservan `MissingHtml` y `render_text_fallback`, y el requisito «Un fallo al leer el HTML
no cambia el estado» no se revoca.** Con las páginas embebidas, el camino deja de ser alcanzable
para los once códigos de la lista —siempre hay una copia—, pero **no** deja de tener trabajo: con
`DEFAULT_ERROR_PAGE=418`, valor legal y fuera de la lista, y sin ficheros en `CONTENT_DIR`, ningún
código tiene página; sin el respaldo eso sería un `500` (o un `404` mentiroso), y con él el estado
sigue siendo el código resuelto. Es la red que garantiza que un `CONTENT_DIR` roto o un defecto
exótico nunca produzcan un `500`. *Alternativa descartada:* eliminar el respaldo, ahora que «siempre
hay página» —daría por imposible un `DEFAULT_ERROR_PAGE` legítimo que sí lo alcanza, y borraría un
requisito vivo por comodidad—.

**D5. Una prueba de paridad byte a byte entre las dos copias, en los dos sentidos, dentro del
crate.** Compara los bytes embebidos con la lectura de `content/<code>.html` y de
`content/css/error.css`, y además exige que **todo** `*.html` de `content/` tenga entrada en la
tabla: el primero de los sentidos detecta una copia embebida obsoleta o distinta —alguien retoca
`content/503.html` y quien instaló el crate sigue viendo la vieja—, y el segundo detecta una página
nueva del directorio que nadie embebió. La comparación por bytes y no por una propiedad blanda: «el
cuerpo contiene su código» pasa aunque las dos copias difieran en estilo, en texto o en el `href` de
la hoja. La prueba vive en los `#[cfg(test)]` del módulo que guarda la tabla, no en `tests/`: el
crate no tiene *lib target*, así que una prueba de integración no puede ver las constantes del
binario —para verlas habría que añadir un `lib` que cambiaría la forma del crate para todos—, y leer
`content/` por `CARGO_MANIFEST_DIR` es lo que ya hace `tests/content_pages.rs`. *Alternativa
descartada:* una prueba que solo compruebe que cada página embebida lleva su código —no fija la
paridad, que es lo que este requisito protege—. *Alternativa descartada:* generar la tabla desde un
*build script* y confiar en que no hay copia que comparar —elimina el problema por construcción, y es
más maquinaria de la que este change necesita (D3)—.

**D6. `include_str!` exige que el fichero exista al compilar, y eso es la mitad del atractivo de
D3.** La lista de códigos con página pasa a estar vigilada por el compilador: borrar o renombrar
`content/503.html` deja de degradarse en silencio al respaldo de texto en ejecución —nadie se
enteraba— y pasa a ser un error de compilación. El reverso también hay que decirlo: **añadir** una
página a `content/` no la embebe nadie hasta que la tabla la nombre, y eso no lo ve el compilador;
por eso la prueba de paridad recorre el directorio en los dos sentidos (D5).

## Risks / Trade-offs

- [Dos copias del mismo contenido: los ficheros del repositorio y los bytes del binario] → la prueba
  de paridad de D5, en los dos sentidos. Sin ella la divergencia es silenciosa por definición: la
  copia que ve quien instaló el crate no la mira nadie.
- [El respaldo de texto deja de ser alcanzable por la vía normal, y la prueba que lo ejercitaba usa
  un `CONTENT_DIR` sin HTML con el defecto `404`] → esa prueba pasa a fijar un defecto **fuera** de
  la tabla embebida (`DEFAULT_ERROR_PAGE=418`), que es el único camino que queda a `MissingHtml`; el
  requisito no se revoca y el camino sigue cubierto por dos escenarios, uno con el código del defecto
  y otro con un código distinto.
- [Una página embebida servida como asset con `200`] → imposible por construcción: `/404.html` se
  clasifica como página de error **antes** de ceder el paso a `ServeDir`, y la tabla embebida solo se
  consulta desde el `fallback` de `ServeDir`, al que una página de error nunca llega. Un
  escenario y una prueba lo fijan, porque es la garantía que justifica el proyecto.
- [El asset embebido no tiene fecha] → se sirve `200`, nunca `304`. No es una regresión: hoy esa
  petición no era un asset sino una página de error; y el `304` de una petición condicional sobre un
  asset **del disco** no pasa por aquí ni cambia.
- [`HEAD` llega al respaldo, y eso obliga a que el respaldo sea un `Service` de verdad] → `ServeDir`
  copia método, URI y cabeceras en la petición del respaldo y `HEAD` está entre los métodos admitidos,
  así que `HEAD /css/error.css` pasa por la tabla y tiene que responder como el `GET` —estado y
  `Content-Type`— pero **sin cuerpo**; hoy ninguna prueba lo cubre (todas montan `GET` con
  `Body::empty()`). Además `ServeDir::poll_ready` **delega en el `poll_ready` del respaldo**
  (`mod.rs`, líneas 401-407), así que no vale un cierre suelto: tiene que ser `Clone + Send` y estar
  listo sin bloquear, y **no puede hacer E/S de disco** —este camino corre en el hilo *async*, a
  diferencia del render, que va en `spawn_blocking` por la auditoría I-1; los bytes embebidos son una
  tabla en memoria, así que no hace falta—.
- [`InvalidFilename` también entra al respaldo] → es lo que `ServeDir` produce para un directorio con
  `append_index_html_on_directories(false)` y para cualquier path sin nombre de fichero utilizable
  (p. ej. terminado en `/`), y nada de eso puede casar con la tabla: la consulta del asset embebido
  tiene que comparar el path **de forma exacta** —sin prefijos, sin normalizar barras finales—, de modo
  que `/css/error.css/` dé la página de error y no un `200 text/css`.
- [Un fichero del directorio que existe y no se puede leer] → las dos vías degradan distinto ante la
  misma causa, y así se ha decidido: la Opción A, al final de este documento, con su análisis y su
  coste. Lo que sí se hace aquí es no prometer de más: los requisitos dicen «un fichero que se pueda
  leer».
- [`fallback` es un camino de `ServeDir` que hasta ahora no se usaba] → solo entra cuando el fichero
  no está (`FileNotFound`/`InvalidFilename`) o cuando abrirlo da `NotFound`, `PermissionDenied` o
  `ENOTDIR`: el `405` de un método no admitido **no** llega al respaldo
  (`call_fallback_on_method_not_allowed` es `false` por defecto), el error de un path imposible sigue
  saliendo como el `500` de `ServeDir` y el `304` de una petición condicional lo resuelve `ServeDir`
  antes de consultar nada. Las pruebas actuales de assets (`200` del CSS del disco, `304` condicional)
  siguen verdes.
- [`fallback` no fuerza el estado, y con `not_found_service` sí lo forzaba] → el respaldo se consulta
  también con un directorio (`InvalidFilename`) y con cualquier path inexistente, así que el servicio
  embebido **tiene que responder `404` a todo lo que no sea un asset de su tabla**. Con
  `not_found_service` ese `404` estaba garantizado por la API, hiciera lo que hiciera el respaldo; con
  `fallback` pasa a ser responsabilidad nuestra y un `200` de más saldría tal cual, sin convertirse en
  la página de error. Lo fijan la prueba 1.9 (el asset embebido responde `200` con su `Content-Type`)
  y las que ya existen de un asset inexistente (`404` → página de error).
- [La tabla embebida es otra lista de códigos escrita en el código] → no es una lista nueva: **es**
  la lista de códigos con página embebida, y su contenido tiene que coincidir con `content/`. El
  compilador vigila que sus entradas existan (D6) y la paridad vigila que no falte ninguna de las que
  hay en el directorio (D5). La lista de la spec y la de `tests/content_pages.rs` no se tocan.

## Open Questions

Una pregunta **abierta** y una **resuelta**: la de la imagen de Docker sigue esperando decisión, y la
del fichero que existe y no se puede leer se cerró con la **Opción A** —se deja escrita con todo su
análisis, porque es el registro de por qué no se eligió la B—.

**Abierta — ¿Se retira `content/` de la imagen de Docker?** El `Dockerfile` copia hoy `content/`
dentro de la imagen y el `.dockerignore` lo deja pasar. Con las páginas embebidas esa copia pasa a ser
redundante, pero **no** inocua: como la capa del disco gana (D1), la imagen seguiría sirviendo los ficheros, no
las copias del binario, así que borrarla cambia qué bytes se sirven en producción.

- **Recomendación: conservarla tal cual en este change.** El comportamiento de la imagen queda
  idéntico, sin superficie nueva, y la paridad (D5) garantiza que las dos copias dicen lo mismo, de
  modo que la redundancia no puede convertirse en divergencia. Coste: la imagen sigue llevando doce
  ficheros que ya van dentro del binario.
- **Alternativa: quitarla del `Dockerfile` y del `.dockerignore`.** Una sola fuente de verdad —lo que
  sirve la imagen es lo que compila el binario— y unos kilovatios menos de imagen. Coste: se toca el
  empaquetado y hay que volver a verificar la imagen entera (incluido el `HEALTHCHECK` y los chequeos
  con Docker real del workflow de release) en un change que no necesita nada de eso.
- Resolverla **no** cambia ni los specs de este change ni su tabla de tareas: es un `COPY` menos (o
  ninguno). Por eso se deja abierta aquí y no bloquea el trabajo.

**Resuelta por la Opción A — ¿Qué pasa cuando un fichero del directorio existe y no se puede
leer?** Un fichero del directorio gana «cuando se puede leer». En concreto: una **hoja** que existe y
no se puede leer pasa a servirse desde la copia embebida, con `200` —hoy responde la página de
error—, y una **página** que existe y no se puede leer sigue respondiendo el respaldo de texto con el
código resuelto, como hasta hoy. Las dos vías siguen degradando distinto ante la misma causa, y eso es
la decisión, no un descuido: se elige la alternativa que no mueve el comportamiento actual de una
página ilegible.

Por qué la A y no la B: es la de menor superficie de cambio —la B tocaría lo que hoy hace una página
ilegible (`Io` → página embebida en vez de respaldo de texto), que este change no necesita para su
motivo, y convertiría en silencio un directorio del operador roto en una respuesta normal, apagando el
`warn` que hoy lo delata—. El agujero que le queda a la A —un `CONTENT_DIR` que no sea un directorio
legible deja las páginas en el respaldo de texto— es una **configuración equivocada**, y el `warn` del
registro ya la delata: se acepta a cambio de no mover nada más.

Lo que la A obliga, y ya no es opcional: ningún texto de este change puede prometer un «gana
siempre». Los requisitos dicen que gana «un fichero que **se pueda leer**» —está en las dos deltas—, y
D1 lo dice con esas palabras y remite aquí. La elección no añade ni cambia ningún escenario ni ninguna
tarea: los deltas ya lo decían así y con la A siguen diciendo lo mismo.

Lo que sigue es el análisis, tal como se escribió cuando la pregunta todavía estaba abierta. No es
hipotético: si `content/503.html` o `content/css/error.css` existen pero el proceso no puede abrirlos
—modo `000`, un componente del directorio sin `+x`—, el disco tiene algo que decir y no puede decirlo.
Las dos vías de este change no reaccionan igual, y la causa de la asimetría está medida: el respaldo
del asset **solo recibe la petición**, así que no puede distinguir «el fichero no está» de «el fichero
no se puede leer».

- La **página** no llega al respaldo por esta causa. `read_html` (`src/render.rs`, líneas 247-253) solo
  convierte `ErrorKind::NotFound` en `Ok(None)`; un `PermissionDenied` sale como `Err(RenderError::Io)`
  y lo recoge `error_response` (`src/main.rs`, líneas 536-548), que responde `render_text_fallback`
  tanto para `MissingHtml` como para `Io` —su comentario ya dice «Falta el HTML del código y el del
  código por defecto (o no se pudo leer)»—. Es decir: hoy, y con cualquiera de las dos opciones, una
  página que existe y no se puede leer responde el respaldo de texto con el código resuelto.
- El **asset** sí llega al respaldo: `serve_dir/future.rs` (líneas 132-153) mete `NotFound`,
  `PermissionDenied` y `ENOTDIR` en la misma rama y llama al respaldo. Hoy, sin respaldo, esa rama
  termina en el `404` de `ServeDir` —**no** en su `500`: el `500` es para los demás errores de E/S— y
  el middleware lo sustituye por la página de error.

Dos causas iguales, dos degradaciones distintas: la página al respaldo de texto, el asset a la página
de error. Las opciones, con su coste:

- **Opción A — la elegida: calificar la garantía, un fichero gana *cuando se puede leer*.** El asset
  ilegible pasa a responder la copia embebida (`200`), donde hoy responde la página de error; la
  página ilegible sigue respondiendo el respaldo de texto, como hoy. Coste: las dos vías siguen degradando distinto
  ante la misma causa, y la garantía hay que escribirla como «gana un fichero que se pueda leer» en vez
  del «gana siempre» que prometía el texto original; y hay un caso en el que ese coste se nota de
  verdad, porque no es un fichero suelto sino el directorio entero: un `CONTENT_DIR` que sea un fichero
  (`ENOTDIR`) o un directorio sin permiso de recorrido (`EACCES`) hacen fallar **todas** las lecturas
  con `Io`, así que las páginas se quedan en el respaldo de texto —el motivo del change, que un
  `cargo install` sirva las páginas de verdad, no se cumple en esa configuración— mientras la hoja
  embebida sí se sirve. A favor: para los assets de la tabla —que hoy es solo la hoja—, un fichero que
  existe pasa a servirse como asset, que es lo que el requisito «Un asset existente gana a la página de
  error» pide y que ese caso incumple hoy; para un asset que no esté en la tabla, la cosa no cambia
  (sigue dando la página de error). La B no tiene ese agujero.
- **Opción B — descartada: tratar el error de lectura como ausencia en las dos vías**, de modo que
  un fichero ilegible caiga a la copia embebida tanto en la página como en el asset. Coste: cambia el
  comportamiento actual de las páginas —`Io` pasaría de responder el respaldo de texto a responder la
  página embebida, un cambio que este change no necesita para su motivo— y el `warn` de
  `error_response` deja de ser la señal de que el directorio del operador está roto: un directorio
  ilegible se convertiría en una respuesta normal y silenciosa.

Ya está decidida: la A, y con sus dos consecuencias escritas arriba —lo que obliga y lo que cuesta—.
La B queda en la lista como el registro de por qué no se eligió, y la pregunta sigue aquí en vez de
borrarse por eso mismo.
