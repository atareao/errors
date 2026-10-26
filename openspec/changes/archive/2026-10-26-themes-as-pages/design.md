# Design: themes-as-pages

## El hecho que abre el change: un tema que no puede ser una hoja

Hoy un tema es **una sola hoja de estilos**, y el contrato está escrito en el spec vivo de `error-page`
(«El tema del HTML es configurable»). El eje del tema solo toca el **aspecto**: el marcado lo pone el
operador —o la plantilla embebida— y el tema rellena el `<style>` que sustituye al marcador
`<link rel="stylesheet" href="/css/error.css" />`. Para un tema como `matrix` —lluvia digital en
`<canvas>`, terminal, pastillas y un minijuego— ese eje no llega: su marcado, su script y su `<canvas>`
no caben en una hoja. El usuario rechazó «tema = solo CSS» y aprobó que **todos los temas sean
páginas**.

El segundo hecho es de **inventario**: el conjunto es **seis** nombres y está escrito en prosa y en
constantes por medio repositorio (`compose.yml`, `README.md`, `AGENTS.md`, `openspec/specs/error-page/spec.md`,
`src/theme.rs`, `src/config.rs`). El conjunto pasa a **siete**.

El tercero es de **precedencia**: el repositorio **entrega** la página del tema por defecto, así que la
capa por código del operador quedaría siempre sombreada. La decisión de qué gana decide si los temas
existen.

## D1 — El nombre del tema es la ruta del fichero

`Theme` pasa a:

```rust
pub struct Theme {
    pub name: &'static str,
    pub embedded: &'static [u8],   // la página del tema, embebida con include_str!
}

impl Theme {
    /// "<nombre>.html", relativo a CONTENT_DIR. Derivada, no una tabla aparte.
    pub fn page_path(&self) -> String { format!("{}.html", self.name) }

    /// La página del tema: disco legible > embebida; ausente > embebida sin warn;
    /// presente pero ilegible > std::io::Error (el render lo convierte en RenderError::Io).
    pub fn page(&self, content_dir: &Path) -> Result<Vec<u8>, std::io::Error>;
}
```

**El motivo de derivar la ruta y no guardarla en la tabla**: una tabla `name -> ruta` puede divergir del
nombre —un tema `matrix` con su página en `themes/matrix.html`— y eso es un fallo que el compilador no
ve. Con la ruta derivada, la divergencia es **inexpresable**. El precio, escrito: el fichero tiene que
llamarse exactamente como el tema, y así lo exige la prueba «cada nombre tiene su fichero y el tallo
coincide con el nombre». Las siete páginas van **embebidas** con `include_str!`: borrar una es un
**error de compilación**, no un respaldo silencioso.

La resolución aplica a la página la regla de un asset: un fichero **legible** en
`CONTENT_DIR/<nombre>.html` gana a la copia embebida, y como se lee **al servir**, un operador que edite
su página la ve cambiar sin reiniciar. Un fichero **ausente** cae a la embebida **sin** `warn` —es lo
normal en un binario instalado sin ficheros de datos—.

## D2 — La cascada pierde la capa por código, y el diseño por código se muda a la clase del `body`

La cascada del cuerpo HTML pasa de tres capas —`CONTENT_DIR/<código>.html` →
`CONTENT_DIR/<código por defecto>.html` → la plantilla embebida— a **una**: la página del tema. Se
retiran `raw_html`/`read_html` y sus pruebas.

**Por qué se retira la capa por código.** Con una página por tema que el proyecto **entrega siempre**, la
capa por código quedaría **sombreada** en todos los temas: una capa que nunca se lee es una mentira del
contrato y un *override* que no *override*a nada. Mantenerla habría exigido una regla del tipo «si el
fichero por código existe, gana a la página del tema», y esa regla decide, en la práctica, que las
páginas de los temas distintos del por defecto no existan: el repositorio entrega `default.html`, así
que la capa del operador siempre tiene página. La precedencia forzada es el precio de que los temas
existan.

**Qué la sustituye.** El diseño por código no desaparece: se muda a la clase `c{{code}}` del `body`, que
la plantilla ya rellena con el código resuelto, y al CSS de la propia página. Es un sitio **mejor**:
la clase y el estado no pueden discrepar, porque las dos salen del mismo código resuelto. `DEFAULT_ERROR_PAGE`
conserva su significado —el código que se sirve cuando ninguna fuente aporta uno utilizable— y ya no
nombra ningún fichero.

**Lo que no cambia**: los tres marcadores `{{code}}`, `{{reason}}` y `{{description}}`; el escape de
`&`, `<` y `>`; el escaneo por bytes; la sustitución de **cada** aparición; y que una página **sin**
marcadores se sirve byte a byte sin tocar el disco.

## D3 — `default` también es una página, y el operador pierde su capa por código

`content/404.html` pasa a `content/default.html`: es la página del tema `default`, **no** una capa
global que sustituya a las de los demás temas. Para los siete temas, cada uno usa su página; con
`THEME=matrix`, `matrix.html`.

**Qué pierde el operador, escrito sin maquillarla**: un operador con su `content/404.html` que ponga
`THEME=matrix` verá la página del tema, no la suya. Su **punto de personalización** es la página del
tema: `content/default.html` para el tema por defecto, `content/matrix.html` para `matrix`. **Qué
conserva**: su página sigue siendo suya y sigue ganando a la copia embebida, y sigue sirviéndose tal
cual —sin marcadores, byte a byte—. La capa por código no se puede conservar: conservarla era no tener
temas.

**Por qué `default` no se libra de ser página.** Se consideró que `default` fuese la capa compartida y
los demás temas páginas encima. Se rechaza porque el tema por defecto tiene que poder tener también su
documento propio —su `<style>`, su estructura— sin depender de la plantilla embebida, y porque una
asimetría «seis son páginas y uno es la capa» vuelve a introducir dos mecanismos donde el modelo quiere
uno. El precio es la pérdida de la capa por código para todos, incluido el tema por defecto; la salida
—sobrescribir la página del tema— es la misma para los siete.

## D4 — Una página ilegible va al respaldo de texto, no a la copia embebida

A diferencia de la hoja —adorno, que caía a la copia embebida con un `warn`—, la **página** es
**contenido**: una página del operador que existe y no se puede leer responde el **respaldo de texto**
con el código resuelto, sin cambiar el estado, y deja un `warn!` que nombra el fichero y su causa. Es la
Opción A de `embedded-error-pages`, sin cambios: si la copia embebida tapara en silencio un directorio
del operador roto, el operador creería que su página se sirve cuando no es así.

Un fichero **ausente** no llega a ese camino: la copia embebida cubre cualquier código de `400..=599`,
así que el respaldo queda reservado al fallo de **E/S**. El precio, escrito: un operador con un
`default.html` ilegible ve texto plano en vez de su diseño —y el `warn!` se lo dice—.

## D5 — El alias de `/default.html` es del **código**, no de la página

`/default.html` y `/default.htm` se clasifican como el **código por defecto** (`DEFAULT_ERROR_PAGE`),
después de la normalización del path y antes de `ServeDir`, y se tratan como el código: la respuesta
lleva el estado del código por defecto y la página del tema elegido; el origen para las métricas es
`Path`.

**Por qué del código y no de la página.** Es lo que evita que el fichero se sirva crudo, con los
marcadores a la vista: el alias entra en la misma cascada que `/404.html` y se renderiza. Es **del
código** porque el path lo elige quien llama y lo que pide es una página de error, no un asset;
asociarlo a la página del tema `default` daría `200` con los marcadores, justo el defecto que el
proyecto corrige. **Por qué solo esos dos paths**: son el nombre del fichero del tema por defecto, el
único que el proyecto **entrega** con nombre propio; interceptar `/minimal.html` o `/matrix.html`
rompería «un asset existente gana» y añadiría seis casos especiales. Se documenta el leak y se bloquea
`*.html` salvo `/default.html` en el proxy.

## D6 — Dónde vive cada guarda, y qué no puede demostrar

Las guardas se *retargetizan* de las hojas a las páginas, y viven en el `#[cfg(test)]` de `src/theme.rs`
—es lo que el proyecto entrega—. Son tres:

1. **Red** (`ninguna_pagina_puede_hacer_una_peticion`): lista **cerrada** de tokens —`@import` en
   cualquiera de sus formas; `url(...)` cuyo destino no sea `url(#…)` o `url(data:…)`; `<script src=…>`
   en cualquier forma; `<img>`, `<source>`, `<video>`, `<audio>`, `<iframe>`, `<embed>`, `<object>`,
   `<form action=…>` y `<link href=…>` con destino que no sea `data:` o `#…`; `srcset`; y en JavaScript
   `fetch(`, `XMLHttpRequest`, `WebSocket`, `EventSource`, `sendBeacon` y `new Image`—. Limpia los
   comentarios antes de escanear (HTML `<!-- -->`, y en los scripts `//` y `/* */`, respetando las
   cadenas); la detección HTML es insensible a mayúsculas y la de JavaScript sensible. **No puede
   demostrar** que un script no construya una URL en tiempo de ejecución, ni que una librería no pida
   por su cuenta, ni que un `<img>`/`<link>` se cree por JavaScript después de cargar: solo ve los tokens
   literales que están en los bytes.
2. **Movimiento** (`una_pagina_que_anima_puede_apagarse`): en el CSS, la declaración de `animation`,
   `animation-name`, `transition` o `transition-property` con valor distinto de `none` tiene que quedar
   neutralizada —cada familia por su forma canónica— dentro de un bloque
   `@media (prefers-reduced-motion: reduce)` **existente y no vacío**; en el script, la página que anime
   con `requestAnimationFrame` o `setInterval` tiene que **mencionar** `prefers-reduced-motion`. **No
   puede demostrar** que la rama del `if` sea correcta, ni que la animación no arranque o se detenga: eso
   exigiría ejecutar el script en un navegador. Un script que menciona la preferencia y luego anima igual
   **pasa**; la revisión humana es la que comprueba la rama. Y en el CSS, la neutralización se mide **por
   familia, no por declaración**: la guarda comprueba que el bloque reduce trae la forma canónica de cada
   familia declarada, pero **no liga** cada declaración con *su* neutralización —no pregunta si la
   `.caret` que anima tiene su propio `animation: none`—, de modo que dos animaciones declaradas y **una
   sola** neutralizada **pasan**. El motivo, escrito: ligarlas exigiría un parser de selectores —el árbol
   compila sin red y sin `regex`— y **rechazaría neutralizaciones legítimas** hechas con otro selector (un
   comodín `* { animation: none }`, una lista de selectores), que es peor que el hueco. La revisión humana
   es la que comprueba que cada animación quede cubierta.
3. **Cobertura de marcado** (`cada_pagina_sobria_cubre_el_marcado`): cada una de las **seis páginas
   sobrias** estila `body`, `h1`, `h2`, `hr`, `p` y `footer`. **`matrix` queda fuera**: es un documento
   propio y no tiene por qué llevar `h2` ni `hr`. **No puede demostrar** que las seis sobrias no
   diverjan; eso lo vigila la prueba de paridad byte a byte de `tests/content_pages.rs`.

Las guardas cubren las páginas **incorporadas** al binario: la página del operador en `CONTENT_DIR` es
suya y el servicio no la parsea al servir. Ninguna guarda mide el **aspecto** —paleta, escala,
jerarquía, espaciado—: el contraste se mide a mano y se deja escrito; el resto se revisa a ojo.

## D7 — El relic de nombres de escenario: no se necesitó ninguno

Una versión anterior de este change conservaba el escenario `el conjunto de temas sigue siendo de seis
nombres` con el cuerpo reescrito a siete, porque `openspec` 1.13.2 no permite a un `MODIFIED` omitir un
escenario del spec vivo y renombrarlo equivale a quitarlo. Este diseño **evita el relic**: los
requisitos cuyos escenarios pertenecen a un artefacto retirado —la cascada por código, la hoja, el
marcador— se **retiran enteros** (`## REMOVED Requirements`) y su sustituto se **añade** con un nombre
que dice la verdad (`## ADDED Requirements`). Así ningún escenario conserva un nombre que mienta:

- «El cuerpo depende del formato y el código es ortogonal» → «El cuerpo HTML es la página del tema».
- «El tema del HTML es configurable» → «El tema del HTML elige una página»; su escenario
  «el conjunto de temas sigue siendo de seis nombres» pasa a «el conjunto de temas son siete nombres», y
  sus diez escenarios de hoja pasan a los de página de «Una página que anima puede apagarse».
- «El HTML servido es autocontenido» y «Las páginas no referencian ficheros que no existen» → «Ninguna
  página pide nada por la red» y «Las siete páginas del proyecto».
- «Los assets estáticos se sirven desde CONTENT_DIR» (de `http-server`) → «Los ficheros de CONTENT_DIR
  se sirven como assets».

Los `MODIFIED` que quedan conservan **todos** sus escenarios vivos, porque ninguno describe un artefacto
retirado: la cascada del código, la normalización del path, la precedencia del asset, el fallo de
lectura, la página de cada código y la clase del `body` siguen siendo ciertos. El precio de la
concesión, escrito: retirar un requisito central y añadir su sustituto rompe la continuidad del
histórico de ese requisito en `openspec/specs/`; a cambio, el spec resultante no arrastra nombres falsos.
Si una versión futura de `openspec` permitiera renombrar escenarios dentro de un `MODIFIED`, se podría
volver a la forma «modificar y conservar»; hoy no.

## D8 — La asimetría del `Dockerfile`

El `Dockerfile` copia `content/` **dos veces**, y la **primera no es un «por si acaso»**: el stage
`builder` necesita las siete páginas **para compilar** —`include_str!` resuelve la ruta en tiempo de
compilación—, así que su `COPY content ./content` es obligatorio: sin él, el `cargo build` del crate
muere y no hay imagen. Un `Dockerfile` que solo copiase `content/` al stage de runtime ya rompió una
imagen una vez (medido: `exit code 101`), y el job `imagen` del CI lo caza en cada propuesta.

La **segunda** copia —la del stage `runtime`— es la decisión que sigue **abierta**: `CONTENT_DIR` es la
única ruta de estáticos y, como la capa del disco gana a la embebida (D1), la imagen sirve esos ficheros
y no los del binario, de modo que **quitarla cambiaría qué bytes se sirven en producción**. Las dos
opciones son defendibles —servir las páginas de la imagen (más fácil de inspeccionar y de parchear) o
servir las del binario (una sola fuente de verdad, la que se publica)—, y el change no la resuelve: la
deja en Open Questions, ahora con las siete páginas en juego.

## D9 — Lo que se retira, y por qué no queda andamiaje

Con la hoja fuera, se retiran de una pieza: el marcador
`<link rel="stylesheet" href="/css/error.css" />`; `render.rs::autocontenido()`, `marcador()`,
`atributos_de_etiqueta()`, `es_el_marcador()`, `MARCADOR_ETIQUETA` y `MARCADOR_HREF`; `Theme::sheet` y
`sheet_path`; `embedded::PAGE` (sustituido por la página de cada tema), `DEFAULT_SHEET_BYTES`,
`DEFAULT_SHEET`, `ASSETS` y `asset()`; el asset público `/css/error.css` y su requisito en el spec de
`http-server`; y `main.rs::embedded_assets()` con su `fallback` de `ServeDir`, al quedarse **sin tabla**.
El motivo de retirarlo todo junto es el que ya enseñó `self-contained-themes`: dejar la mitad de un
mecanismo invita a que alguien lo «reactive» sin leer por qué se fue.

El **respaldo de assets embebidos** es el caso que hay que **confirmar en RED antes de retirarlo**: un
`CONTENT_DIR` sin ficheros y la tabla vacía tienen que dar el mismo `404` que da hoy un path fuera de la
tabla, y el middleware de páginas de error lo sustituye por la página del tema. Si al vaciar `ASSETS`
algún path dejase de responder `404`, la fase RED lo pone de manifiesto antes de tocar `main.rs`.

## D10 — El tema `matrix`: documento propio, y cobertura solo de las sobrias

`matrix.html` es un documento propio —canvas de lluvia, glitch, scanlines, pastillas roja y azul,
terminal, minijuego y toast—; las **seis sobrias** comparten el esqueleto `main > h1 / h2 / hr / p` más
`footer`, con el `footer` sin `href` y el `body class="c{{code}}"`. Dos consecuencias, escritas:

- La **cobertura de marcado** solo se exige a las seis sobrias: `matrix` no tiene por qué llevar `h2` ni
  `hr`, y forzárselos sería medir el aspecto, no el contrato.
- La **paridad byte a byte** de las seis sobrias —quitado el bloque `<style>`— se vigila con una prueba
  de `tests/content_pages.rs`, no con una guarda: una guarda textual no puede demostrar que seis
  ficheros no diverjan.

Sus animaciones CSS neutralizan con las formas canónicas dentro de `@media (prefers-reduced-motion: reduce)`,
y su script **menciona** `prefers-reduced-motion` para no arrancar la lluvia cuando el usuario pide
movimiento reducido; el límite de esa guarda —solo ve la mención— está en D6. El contraste de su paleta
se mide a mano y se deja en su cabecera, como en las seis sobrias: **ninguna guarda mide el aspecto**.

## Preguntas abiertas

1. **La segunda copia de `content/` al stage de runtime del `Dockerfile`.** Decide qué páginas sirve la
   imagen publicada (las del disco o las del binario). Las dos opciones son defendibles; no se resuelve
   aquí (D8).
2. **El leak de las seis páginas planas.** `GET /minimal.html`, `/terminal.html`, `/paper.html`,
   `/amber.html`, `/compact.html` y `/matrix.html` responden `200` con los marcadores a la vista. Con las
   páginas en la raíz de `CONTENT_DIR` es inevitable; la mitigación es externa (bloquear `*.html` salvo
   `/default.html` en el proxy). Declarado, no resuelto.
3. **El punto de personalización del operador.** Es la página del tema; la capa por código se pierde. Si
   más adelante se quisiera recuperar granularidad por código, el sitio natural es un *override* del
   `<style>` de la página, no una capa de ficheros por código.
4. **El tamaño del binario y de la imagen.** Siete páginas con su `<style>` y su `<script>` dentro pesan
   más que una página y seis hojas; no se ha medido el delta. `matrix.html` es la que puede pesar más, y
   tampoco se ha medido su tamaño.
5. **El coste por petición.** La página del tema se resuelve **al servir** (lectura de disco o bytes
   embebidos). No se ha medido el coste real por petición, ni comparado con el modelo anterior —que leía
   el HTML del código y la hoja—; el respaldo de assets embebidos corría en el hilo *async* y el render
   en `spawn_blocking`, y esa diferencia sigue igual.

## Lo que no está medido

- **El delta de tamaño** del binario y de la imagen al pasar de una página más seis hojas a siete
  páginas con su CSS dentro, y el tamaño de `matrix.html` en particular.
- **El coste real por petición** de resolver la página del tema al servir, frente al modelo anterior.
- **La paridad visual** de las seis sobrias: son idénticas **quitado el `<style>`**, que es lo que mide
  la prueba; su CSS es distinto y su aspecto se revisa **a ojo**. Ninguna guarda mide paleta, escala,
  jerarquía ni espaciado.
- **El contraste** de cada paleta: se mide **a mano** y se deja en la cabecera de cada página, como en
  las hojas actuales. **Ninguna guarda lo mide.**
- **Que el script de `matrix` apague la animación de verdad**: la guarda solo comprueba que menciona
  `prefers-reduced-motion`. La revisión humana es la que comprueba la rama.
- **Que el proxy bloquee `*.html`**: la mitigación del leak es externa y este repositorio no entrega un
  respaldo que la implemente.
