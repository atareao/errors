# Design: themes-from-content-dir

## El hecho que abre el change: la tabla cerrada y su precio

Hoy la lista de temas es una **tabla cerrada en el binario**. `THEMES` es `static [Theme; 7]` en
`src/theme.rs`, `THEME_NAMES` es una constante de texto, las siete páginas van embebidas con
`include_str!`, `Config.theme` es `&'static Theme` (`src/config.rs:172`) y `resolve()` devuelve
`&'static Theme` (`src/theme.rs:204`). Esa tabla protege dos cosas que el proyecto quiere conservar:
un `cargo install` —un binario y ningún fichero de datos— sirve los siete diseños, y `THEME` se puede
validar **al arrancar** enumerando los nombres válidos.

El precio está **medido**: un `content/neon.html` de más **no es un tema**. Se sirve **crudo** como
asset, `200 text/html`, con `class="c{{code}}"` y `<h1>{{code}}</h1>` a la vista —el middleware no lo
intercepta, porque `is_error_page_path` solo dice que sí para un código resoluble o para los alias
`/default.html`/`/default.htm`—; `THEME=neon` **aborta** el arranque enumerando los siete; y **dos**
guardas de conteo fallan con mensajes distintos: `tests/content_pages.rs` («content/ tiene que
entregar exactamente 7 páginas HTML (una por tema), no 8») y `src/embedded.rs` (`left` con 8 ficheros
frente a `right` con 7).

El usuario pide lo contrario: **añadir tantos temas como quiera con solo añadir el fichero HTML**. Y
ha decidido cuatro cosas que gobiernan el diseño: los siete incorporados **se quedan** y el directorio
**añade**; el barrido es **al arrancar**; `THEME` con un nombre inexistente **sigue abortando**; y la
página del operador **no** se inspecciona.

## D1 — El registro es incorporados **más** descubiertos

El **registro efectivo** de temas SHALL ser los siete incorporados **más** los descubiertos en la raíz
de `CONTENT_DIR`. La tabla estática `THEMES` deja de ser la única fuente de temas y pasa a ser el
registro **incorporado**; una función nueva construye el registro efectivo a partir del directorio.

**Por qué se rechaza «el directorio es la única fuente».** Es la alternativa que deja el modelo más
limpio —una sola procedencia de temas—, pero rompe la primera garantía del proyecto: quien instale el
crate con `cargo install` tiene un **binario y ningún fichero de datos**, con `CONTENT_DIR` apuntando a
un `content` que no existe, y con el directorio como única fuente se quedaría **sin ningún tema**,
sirviendo el respaldo de texto en vez de los siete diseños. El precio de conservar los incorporados,
escrito: el registro tiene **dos** procedencias —una tabla compilada y un barrido—, y las dos tienen
que concordar en el nombre del tema y en la forma de su página. El precio de la alternativa rechazada
—perder el binario sin ficheros de datos— es peor.

## D2 — El barrido es **al arrancar**, no por petición

El servicio SHALL barrer `CONTENT_DIR` **una vez, al arrancar**, y construir el registro con eso. Los
**bytes** de la página de un tema se siguen leyendo **al servir** (la regla de asset de
[`Theme::page`]), así que **editar** un tema se ve sin reiniciar; lo que exige reiniciar es **añadir**
un tema nuevo.

**Por qué no en caliente.** Dos motivos, y los dos son de coste y de corrección. Primero, un barrido
por petición pondría un `read_dir` en el camino de **cada** página de error, que es justo cuando el
servicio más se pide: durante una incidencia. Segundo, validar `THEME` **exige** una lista conocida
**en el arranque** —la variable la escribe el operador y un valor que no esté en el registro aborta—,
así que con un barrido perezoso la validación de `THEME` no podría ser estricta en el arranque sin
leer el directorio antes, que es exactamente el barrido que se quiere evitar. El precio, escrito:
añadir un tema exige **reiniciar**; un fichero añadido con el servicio en marcha **no** es un tema
hasta el siguiente arranque y se sigue sirviendo como **asset crudo** con `200`.

## D3 — El tipo deja de ser `&'static`: el registro es de la `Config`

`Config.theme` y `resolve()` dejan de ser `&'static Theme`. Un tema **descubierto** vive en el
directorio del operador y su nombre no se conoce al compilar, así que no puede ser una referencia
`'static` sin filtrar memoria. Se **rechaza** `Box::leak`: filtraría el registro entero en cada
arranque —una fuga de memoria deliberada y permanente— para comprar una firma que ya no hace falta.

La forma: el **registro** —incorporados más descubiertos— pasa a ser propiedad de la `Config`, que ya
vive en el **estado del router** y ya viaja por referencia a cada petición. `Config.theme` guarda el
nombre o un índice del tema elegido, y la resolución de `X-Theme` busca en el registro del estado.
**El precio, escrito**: `Config` deja de ser `Copy` en lo que al tema respecta —ya era `Clone`— y las
pruebas que asumían `&'static` se retocan; a cambio, el registro no se filtra y un tema descubierto
cabe en el modelo sin trucos.

## D4 — El nombre de `X-Theme` **nunca** compone una ruta

Invariante, escrita en el requisito y fijada por una prueba: **el valor de `X-Theme` NO SHALL usarse
nunca para componer una ruta**. La ruta de la página de un tema sale **del registro**, que se construyó
del barrido del directorio: se resuelve el nombre a una entrada del registro y se sirve **su** ruta.

**El motivo, sin rodeos**: si se implementara como `content_dir.join(format!("{}.html", x_theme))`, un
`X-Theme: ../../etc/passwd` sería una **lectura arbitraria de ficheros** —una cabecera de petición, que
la elige quien llama, controlando qué se lee del disco—. Es la misma lección que la normalización del
path: la entrada del usuario nunca se usa como ruta. La prueba fija que un `X-Theme` con `..` no lee
nada fuera de `CONTENT_DIR` y cae al tema configurado.

## D5 — Un tema descubierto no tiene copia embebida

Un tema **descubierto** no tiene `include_str!` —no existía al compilar—, así que su fichero es su
**única** copia. Si en el momento de servir ese fichero ha **desaparecido**, la respuesta SHALL ser el
**respaldo de texto** con el estado intacto y SHALL dejar constancia nombrando el fichero.

**El contraste con un incorporado, escrito**: un incorporado **ausente** cae a su copia embebida
**sin** dejar constancia —es lo normal en un binario sin ficheros de datos—; un descubierto
**ausente** no tiene a qué caer, así que el silencio escondería un fichero que se ha esfumado. Deja
constancia y responde el respaldo. Es la misma doctrina de la Opción A: la página es **contenido** y
un fallo de E/S no se tapa en silencio; el `warn!` lo delata y el estado no cambia. **El precio**:
quien borre el fichero de un tema descubierto con el servicio en marcha verá texto plano hasta el
siguiente arranque, en vez de su diseño.

## D6 — Las reglas del barrido

El barrido mira la **raíz** de `CONTENT_DIR` y **no** los subdirectorios: un tema es **una página** con
un nombre, y admitir rutas convertiría el nombre en una ruta que el registro tendría que validar. Se
descubre un fichero **regular** —o un enlace simbólico que apunte a uno legible— cuya extensión sea
**exactamente** `html` (ni `HTML`, ni `htm`, ni `css`), y el **nombre** del tema es su **tallo**
(`neon.html` → `neon`).

Tres casos con su regla, escritos uno a uno:

- **El tallo de un incorporado no añade un tema.** `content/default.html` **no** crea un `default` de
  más: es la página de ese tema en el disco, que es la regla de siempre —un fichero legible gana a la
  copia embebida—.
- **El nombre de un código no es un tema.** Un fichero cuyo tallo **reclame la resolución de códigos**
  se queda **fuera** del registro: su path no se sirve como asset **ni** como página de tema. El
  criterio **no** es el rango utilizable, y esto está **medido**: `code_from_path` **parsea** el número
  y `is_error_page_path` **no** filtra el rango —el `400..=599` se aplica al **resolver** el código,
  no al **clasificar** el path—, así que **cualquier** tallo que se lea como un `u16` reclama el path.
  Con un `CONTENT_DIR` de prueba, medido contra el binario:

  | path | respuesta |
  |---|---|
  | `/404.html` | `404`, con el cuerpo del `404` |
  | `/451.html` | `451`, con el cuerpo del `451` |
  | `/999.html` | **`404`** —el código por defecto: el `999` no es utilizable— |
  | `/200.html` | **`404`** —el código por defecto: el `200` está fuera del rango— |
  | `/neon.html` | `200` crudo, con los marcadores a la vista |

  De ahí la regla: **cualquier** tallo numérico queda fuera, y el servicio deja constancia nombrando el
  fichero. El motivo de dejarlo fuera y no aceptarlo: quien escriba un `404.html` espera que su fichero
  se sirva, y **no se sirve** —responde **otra** página de error—, así que aceptarlo como tema sería
  aceptar un nombre cuyo path está reclamado por otra cosa.
- **Los enlaces simbólicos valen** si apuntan a algo legible: es una forma legítima de tener el
  contenido en otro sitio. La lectura se hace **al servir**, así que un enlace roto cae en la regla de
  D5 (el respaldo con constancia).

## D7 — La trampa del montaje: `THEME` aborta y el volumen puede no estar montado

Mantener el aborto de `THEME` tiene una contrapartida que va escrita para que no sorprenda: el error
enumera los nombres que el registro **tenga en ese arranque**, así que un `THEME` **correcto** cuyo
fichero **todavía** no esté en `CONTENT_DIR` —un volumen que aún no se ha montado cuando el contenedor
arranca— **aborta** el arranque. Es el precio de la validación estricta y el usuario lo ha aceptado:
la variable la escribe el operador, y un arranque con un tema que no existe es peor que un arranque que
falla diciendo por qué. La alternativa —degradar en caliente al tema por defecto— convertiría un error
de configuración en un servicio que sirve el tema equivocado en silencio, que es justo lo que el
aborto evita. La mitigación es del operador: declarar la dependencia del volumen, o arrancar sin
`THEME` y con el defecto.

## D8 — Las dos guardas de conteo: el `content/` del **repositorio** sigue siendo siete

El `content/` del **repositorio** sigue entregando exactamente **siete** páginas —una por tema
incorporado—, y las dos guardas de conteo lo siguen exigiendo: `src/embedded.rs`
(`el_directorio_entrega_exactamente_las_siete_paginas_de_tema`) y `tests/content_pages.rs`
(`cada_tema_tiene_su_pagina_y_el_tallo_coincide`). **No** se relajan.

**Por qué.** El mecanismo de descubrimiento es para el `CONTENT_DIR` **del operador**, no para el
`content/` del repositorio: un octavo fichero ahí sería un octavo tema **incorporado** —una página del
proyecto, con sus tres contratos y su copia embebida— y exigiría cambiar el requisito «Las siete
páginas del proyecto». Que un operador pueda tener **veinte** temas en su directorio no cambia cuántas
páginas **entrega** el proyecto: las guardas miden lo que el proyecto entrega, y eso sigue siendo siete.
El precio, escrito: las guardas **no** vigilan el directorio del operador —ni pueden—; su
descubrimiento lo fijan las pruebas de router y de `theme.rs` con un directorio de fixtures propio.

## D9 — Lo que no se hace: la página del operador no se inspecciona, y el leak no se reabre

**Los tres contratos no cubren un tema descubierto.** El contrato de red (no pedir nada por la red), el
de movimiento (animar solo si puede apagarse) y el de cobertura de marcado siguen midiendo **solo** las
páginas **incorporadas** al binario. Un tema descubierto es la página **del operador**, y el servicio no
la inspecciona. Es una **decisión explícita del usuario**, y su motivo: el escáner vive hoy en
`#[cfg(test)]` y cubre lo que el **proyecto entrega**; llevarlo al binario para validar el directorio
del operador haría que el arranque dependiera del contenido de un directorio que el operador edita —y
un HTML del operador podría tumbar el arranque—. El precio, escrito: un tema descubierto puede pedir
por la red, animar sin poder apagarse o no cubrir el marcado, y **ninguna guarda lo verá**.

**El leak no se reabre, porque el respaldo de Traefik bloquea por forma.** `GET /neon.html` sigue
siendo un `200` **crudo** con los marcadores a la vista —la garantía «un asset existente gana a la
página de error» no se toca—, pero el respaldo que el repositorio entrega lo cierra **gratis**:
`!PathRegexp(\.html?((%2[fF])|/|$))` bloquea por **forma**, no por nombre, así que un `content/neon.html`
nuevo queda protegido sin añadirlo a ninguna lista. Eso es un argumento **a favor** del diseño: la
mitigación del leak no mantiene una lista de temas que el descubrimiento haría crecer.

## Preguntas abiertas

1. **El bloque del smoke test.** Si el contenedor puede montar un `CONTENT_DIR` propio con un tema de
   más en las **dos** puertas que lo corren —`ci.yml` y `release.yml`—, y cómo montarlo sin duplicar
   aserciones fuera de `.github/scripts/smoke.sh`. Se decide y se **reporta** en la fase 3
   (`tasks.md` 3.3), no aquí: depende de que el motor de contenedores del runner admita un montaje.

   **Resuelta en la fase 3: sí se añade, en un contenedor aparte.** El bloque vive en un **segundo**
   contenedor, distinto del que prueba la imagen tal cual, y no duplica ninguna aserción fuera del
   script. El motivo es justo el que hace que esta comprobación **solo** quepa aquí: la imagen corre
   como uid **10001** y lee un `CONTENT_DIR` **montado**, así que el barrido de ese directorio depende
   de que el montaje sea legible por ese uid —medido: un directorio en **`0700`** responde `404` y con
   **`chmod 755`** responde `200`, algo que `cargo test` **no puede ver** porque lee el directorio con el
   uid de quien compila—. Y **no** se monta en el primer contenedor porque dejaría **fuera** el
   `content/` de la imagen y con él la aserción del leak (`/minimal.html -> 200`, la garantía 4), que es
   una propiedad de lo que la imagen **entrega**. Medido después: `just engine=podman smoke` → **21
   `ok`**, exit 0.
2. **El coste en el arranque del barrido.** Un `read_dir` y un `metadata` por entrada. En un directorio
   de operador con decenas de ficheros es despreciable; no se ha medido con miles ni con un directorio
   en red. **Sigue abierta**: la fase 3 no la cerró ni la midió.
3. **La firma exacta del registro.** Si `Config` guarda el **nombre** del tema elegido y resuelve
   contra el registro en cada petición, o un **índice** al tema. Las dos caben; se decide en la fase RED
   con las pruebas delante.

   **Resuelta en la fase RED: `Config` posee el registro y el tema por valor.** La implementación:
   `Config` **posee** el registro (`themes: Themes`, con `Vec<Theme>`) **y** el tema elegido **por
   valor** (`theme: Theme`, ya no `&'static`); la resolución de `X-Theme` busca en el registro del
   estado con `Themes::by_name`. `Theme` es `Clone` y su `name` es `Cow<'static, str>` (incorporado →
   `Borrowed`, descubierto → `Owned`), y se rechazó `Box::leak` porque filtraría el registro entero en
   cada arranque.

## Lo que no está medido

- **El coste del barrido en un directorio grande.** No se ha medido con miles de entradas ni el efecto
  de un `read_dir` sobre un directorio con muchos ficheros no-`html`.
- **El comportamiento con un `CONTENT_DIR` en una red lenta.** Un directorio montado por NFS o similar
  podría hacer que el barrido al arranque tardase; no se ha medido. Es el precio del barrido al
  arranque (D2), declarado.
- **El coste por petición** de resolver `X-Theme` contra un registro en vez de contra una tabla
  `static`. Se espera despreciable —una búsqueda lineal por nombre en un vector corto—, pero no se ha
  medido.
- **El `clone()` de la `Config` por petición clona ahora el registro entero.** En `src/main.rs`, el
  `spawn_blocking` de la respuesta de error mueve `config.clone()` —lo exige el `'static` del cierre—.
  Antes de este change la `Config` guardaba un `theme: &'static Theme`; ahora **posee** `themes:
  Themes`, así que ese `clone` copia el `Vec<Theme>` completo —una asignación por cada tema
  **descubierto** (`Cow::Owned`) y nada por los incorporados (`Cow::Borrowed`)— en el camino que más se
  recorre durante una incidencia. Y lo que lo hace **evitable**: `render_page` solo consume
  `config.content_dir` y `config.retry_after` (el tema viaja aparte, y ese `clone` sí es necesario por
  el `'static` del cierre). **No se arregla en este change** a propósito: el arreglo cambia la firma de
  `render.rs`, que es otro asunto, y este change decidió no tocarlo. Queda declarado, con el número de
  temas descubiertos como la variable que lo escala.
- **Las cláusulas de «dejar constancia» (los `warn`) no las fija ninguna prueba.** Dos escenarios del
  delta exigen además un `warn` que **nombre** el fichero (el tallo que reclama la resolución de
  códigos) o el **directorio y su causa** (el directorio que existe y no se puede listar); el código
  **sí** los emite, pero el crate **no tiene arnés de captura de `tracing`**, así que las pruebas solo
  fijan el **registro** resultante, no el aviso. Añadir ese arnés es un asunto propio —y una
  dependencia que este change no introduce—: queda declarado como límite de las pruebas.
- **Que la página del operador cumpla los tres contratos.** Ninguna guarda lo mide ni lo medirá: el
  servicio no inspecciona el directorio del operador (D9).
