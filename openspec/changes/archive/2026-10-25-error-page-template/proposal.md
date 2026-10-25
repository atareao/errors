# Proposal: error-page-template

## Why

La página HTML del proyecto son **once ficheros**, uno por código, con **el mismo esqueleto** y cinco
valores escritos a mano en cada uno: el código, la clase, el `<title>`, la razón y la descripción.
Difieren en **cinco líneas** de quince. Y esos cinco valores ya los sabe el servidor: `resolve.rs` es
la fuente única de la razón (`canonical_reason`, del crate `http`) y de la descripción para **JSON,
XML y texto**.

De ahí salen dos defectos, los dos **medidos** contra el binario:

**1. El mismo error dice dos cosas distintas según el `Accept`.**

```
GET /400 (HTML) → <p>The server cannot process the request because the client sent something malformed.</p>
GET /400 (JSON) → "description": "The server could not understand the request because of invalid syntax."
```

Difieren **5 de los 11** códigos —`400`, `401`, `405`, `408` y `504`—, y nada vigila que coincidan: la
única prueba sobre el HTML comprueba que la página **contiene** el número del código, nada más.

**2. El respaldo miente sobre el código, y le pasa a 189 códigos.**

```
GET /418 → estado 418,  <body class="c404">  <h1>404</h1>  <h2>Not Found</h2>
GET /451 → estado 451,  <body class="c404">  <h1>404</h1>  <h2>Not Found</h2>
GET /451 (JSON) → {"code":451,"message":"Unavailable For Legal Reasons", …}
```

No son once: son **189** los códigos de `400..=599` que sirven un cuerpo que dice `404`. La causa es
el paso «y si no, la página del código **por defecto**» de la cascada: al otro código le llega un
fichero que **dice qué código es**. Es el estado y el cuerpo en desacuerdo — el fallo que este
proyecto existe para no cometer.

No es un incumplimiento del spec: el spec **declara** esa caída. Es un hueco del spec, que declara
*qué fichero* servir sin darse cuenta de que el fichero declara *qué código* es.

Y el andamiaje está sostenido por nada: `embedded::PAGE_CODES` —«estos once códigos tienen página»—
**no lo usa nadie** fuera de `src/embedded.rs`.

## What Changes

1. **La página del proyecto es UNA plantilla, rellenada al servir.** Lleva el código, la razón y la
   descripción como **marcadores** (`{{code}}`, `{{reason}}`, `{{description}}`) que el servidor
   sustituye al servir por el código resuelto y por lo que devuelven `canonical_reason` y
   `description`. Las reglas son **las mismas** que las del marcador de la hoja: sin marcadores se
   sirve **byte a byte**, se sustituye **cada** aparición y el escaneo es **por bytes**.
2. **Una página, no once.** `content/` pasa de once ficheros a uno, y `embedded::PAGES` (once
   entradas) con `PAGE_CODES` se retiran. El fichero conserva el nombre del **código por defecto**:
   está medido que un fichero con nombre de código **nunca** se sirve crudo —la cascada del código lo
   intercepta—, mientras que un nombre neutro se serviría como asset con los marcadores a la vista.
3. **`resolve.rs` es la fuente única del texto de los cuatro formatos.** Gana la redacción del HTML en
   los cinco códigos que hoy divergen, así que la razón y la descripción se escriben **una vez**.
4. **Cualquier código de `400..=599` se sirve con su propio cuerpo** —su código, su razón canónica, su
   clase—, no con el cuerpo de otro.
5. **El escape es parte del contrato:** la razón y la descripción se escapan (`&`, `<`, `>`) al
   insertarse en HTML, porque las escribe el servidor y no un humano dentro del fichero.
6. **El operador conserva su granularidad y gana una herramienta:** `CONTENT_DIR/<código>.html` sigue
   ganando y se sigue sirviendo byte a byte, y si quiere que su respaldo se adapte al código, **ya
   puede** usar los marcadores en su propia página.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `error-page`: **modificados** «El cuerpo depende del formato y el código es ortogonal» —la plantilla,
  las reglas de sustitución y el escape—, «Los códigos que un proxy emite tienen su propia página»
  —cualquier código del rango, no once—, «Cada página se identifica con la clase de su código» —la del
  **código resuelto**, garantizada por la plantilla— y «Las páginas no referencian ficheros que no
  existen» —una página, no once—.
- `http-server`: **modificado** el requisito del contexto de compilación, que dice «las once páginas».

## Non-goals

- **No se toca el reparto `content/css/` y `content/themes/`.** La hoja `default` es **además** el
  asset público `/css/error.css`, así que el reparto va cargado: merece su propio change, con la
  guarda del asset medida.
- **No se toca el tema animado** ni la fase estética de los temas.
- **No se toca la cobertura de la tabla de descripciones:** sigue cubriendo 15 códigos, y un código no
  tabulado recibe la genérica —«An unexpected error occurred while processing the request.»—, que es
  honesta. El hueco queda **declarado**, no escondido.
- **No se retira el paso «página del operador del código por defecto».** Si el operador deja un fichero
  con texto fijo y a otro código le llega ese texto, es su decisión y su fichero; y si quiere el
  respaldo correcto, ya puede usar los marcadores.
- **No se persigue la paridad de *forma de ficheros* con `error-pages`.** La paridad declarada del
  proyecto es el **código real** y la **negociación de formato**, no cuántos ficheros se entregan.

## Impact

- `src/render.rs` — la sustitución de los marcadores, el escape para HTML y el respaldo de un código
  sin fichero propio.
- `src/embedded.rs` — de once páginas a una; `PAGES` y `PAGE_CODES` se retiran.
- `src/resolve.rs` — la redacción de la descripción en los cinco códigos que divergen.
- `content/` — **diez ficheros borrados** y la página única con marcadores. Las **seis hojas** cambian
  **solo su prosa**: no cambia ni una regla.
- `tests/content_pages.rs` y los fixtures, que hoy iteran once ficheros.
- `AGENTS.md`, `README.md` — la página única, los marcadores y qué gana el operador.
- `openspec/specs/error-page/spec.md` y `openspec/specs/http-server/spec.md` — vía la delta de este
  change.
