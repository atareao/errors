# Design: error-page-template

## El hecho que abre el change: once ficheros que repiten lo que el servidor ya sabe

Las once páginas son el mismo esqueleto y difieren en cinco valores escritos a mano. Esos valores ya
viven en `resolve.rs` —la razón, del crate `http`, y la descripción, de una tabla propia—, y son los
que sirven JSON, XML y texto. El fichero por código no añade información: **duplica**.

Y no duplica en silencio: medido, cinco de los once códigos dicen una cosa en HTML y otra en JSON, y
**189** códigos del rango sirven un cuerpo que nombra al `404`, porque la cascada les presta el
fichero del código por defecto —un fichero que declara qué código es—.

## D1 — La página es una plantilla, y se rellena **al servir**

El mecanismo no se inventa: es el que ya sustituye el `<link>` de la hoja en
`render.rs::autocontenido`, con las mismas tres reglas probadas —sin marcador → **byte a byte** sin
tocar el disco; **cada** aparición, no solo la primera; escaneo **por bytes** y avance por el
documento de **entrada**, para no rescanear lo insertado—. Cambia el conjunto de agujeros: en vez de
uno, tres.

Rellenar al servir, y no al construir, es lo que mantiene las dos garantías que el operador ya tiene:
su página se sirve **tal cual** si no lleva marcadores, y se lee **al servir**, así que editarla no
exige reiniciar. Generar los once ficheros con un `build.rs` habría conservado la duplicación y
añadido un paso de construcción, sin arreglar el `451`.

## D2 — Los marcadores, y el escape

| Marcador | Se sustituye por |
|---|---|
| `{{code}}` | el **código resuelto**, el que decide el estado |
| `{{reason}}` | `canonical_reason(code)` |
| `{{description}}` | `description(code)` |

Se usan también en la clase (`class="c{{code}}"`), que es como un tema da acentos distintos por
código.

El **escape** no es un detalle de implementación: hasta hoy la razón y la descripción las escribía un
humano **dentro** del HTML, así que escaparlas era implícito; a partir de ahora las inserta el
servidor, y `&`, `<` y `>` tienen que salir escapados o la página se rompe. Es exactamente lo que ya
hace `escape_xml` para el XML, con las tres entidades del HTML.

## D3 — Una página, no once; y por qué conserva el nombre del código por defecto

Con los marcadores, las once páginas serían **byte a byte la misma**. Se retiran: `PAGES` (once
entradas) y `PAGE_CODES` desaparecen —`PAGE_CODES` no lo usaba nadie, y `render.rs` deja de necesitar
«la página de ese código»—.

El fichero conserva el nombre del **código por defecto** (`content/404.html` con el
`DEFAULT_ERROR_PAGE` de fábrica) en vez de adoptar un nombre neutro. El motivo está **medido**:
`GET /404.html` devuelve **404 con la página**, porque un fichero con nombre de código lo intercepta
la cascada del código **antes** de servirse, mientras que `GET /css/error.css` devuelve **200
`text/css`** —un asset existente gana, y eso sigue siendo cierto—. Un fichero de nombre neutro, en
cambio, **sí** se serviría como asset: `GET /error.html` con los marcadores a la vista.

El precio, que hay que escribir en la documentación: el fichero se llama `404.html` y sirve también el
`451`. La lectura honesta es la que el propio `resolve` ya hace: la página del código por defecto **es**
la página del proyecto.

## D4 — `resolve.rs` como fuente única, y qué redacción gana

Decidido: **gana la del HTML**. La del `<p>` es más específica en `405` («…the resource you **asked
for**»), `408` («…waiting for **the client to finish sending** the request») y `504` («An upstream
server did not send a response **in time**»), y equivalente en `400` y `401`. Así que la tabla de
`description` se actualiza en esos cinco códigos y las páginas dejan de tener prosa propia.

Efecto secundario que se acepta: la descripción **baja** al HTML, no solo sube al JSON — el HTML deja
de tener una voz propia y pasa a decir lo que dicen los cuatro formatos. Es el precio de que no puedan
divergir.

## D5 — El respaldo deja de mentir, y `MissingHtml` deja de poder pasar

Con la plantilla siempre disponible (viaja en el binario), la cascada **no puede** quedarse sin página:
`CONTENT_DIR/<código>.html` → `CONTENT_DIR/<código por defecto>.html` → la plantilla. Así que
`RenderError::MissingHtml` se vuelve **inalcanzable** y se retira, y el respaldo de texto queda para
lo que sí puede pasar: una página del operador **ilegible** (fallo de E/S), que ya tenía su escenario.

Nótese que la cascada conserva su **forma**: el paso «el fichero del código por defecto» sigue ahí,
pero ahora ese fichero **es** la plantilla, así que deja de prestar una identidad ajena.

## D6 — Lo que el operador conserva, y lo que gana

- Conserva: `CONTENT_DIR/<código>.html` gana a lo embebido, y su página sin marcadores se sirve **byte
  a byte** — su texto fijo sigue siendo suyo.
- Gana: si quiere un respaldo que se adapte al código, **ya puede** escribir los marcadores en su
  propia página. Antes esa posibilidad no existía: el respaldo siempre mentía.

## D7 — Un nombre de escenario no se puede retirar, y eso se declara

Hay un caso en el que el nombre del escenario se queda **aunque ya no diga la verdad**:
`sin ficheros, un código sin página propia cae a la embebida del defecto` — el cuerpo sigue siendo
cierto (el cuerpo **es** la plantilla rellenada con ese código), pero el nombre habla de «la embebida
del defecto» y con la plantilla lo que hay es la plantilla. **Está medido, no supuesto**:
`openspec validate --strict` responde que el bloque `MODIFIED` «omits scenario(s) the current spec
still has» y que «a MODIFIED requirement replaces the whole block, so archive refuses to drop them», y
`openspec archive` aborta con `current spec contains scenario(s) not present in the modified block`.
Renombrar equivale a quitar y añadir, así que **rompe el archivado**.

Se declara en lugar de esconderlo: manda el **cuerpo** del escenario, y el nombre se conserva porque el
`archive` no admite que se pierda. Es el precio de que un `MODIFIED` reemplace el requisito entero.

Y el límite tiene una segunda cara, en `http-server`, también medida: el delta conserva el nombre de
su requisito —`La imagen se construye desde el árbol y trae las páginas dentro`— y dos nombres de
escenario en plural (`el stage de compilación dispone de las páginas`, `la imagen sirve las páginas sin
ficheros externos`), porque **tampoco se pueden cambiar**: renombrar un **escenario** pone
`openspec validate --strict` en rojo con el mismo error de «omits scenario(s)», y renombrar el
**header del requisito** pasa el `validate` (solo avisa con un `INFO`: «Archive would refuse this
delta … not found») pero haría que **el `archive` rechazara el delta entero**. `openspec` 1.13.2 sí
tiene `## RENAMED Requirements` con `FROM:`/`TO:` para eso, y **no se usa aquí**: el plural no es
falso —la imagen sirve páginas, para cualquier código del rango— y usarlo dejaría el delta con un
header en singular y dos escenarios en plural, que es peor que dejarlo coherente.

## Open Questions

1. **La descripción de los códigos sin prosa propia** (`451` y otros 184): hoy reciben la genérica. Se
   puede crecer la tabla, o hacer que sin prosa propia el `<p>` caiga a la razón canónica. No entra
   aquí, y el hueco queda declarado.
2. **El reparto `content/css/` y `content/themes/`**: la hoja `default` es además el asset público
   `/css/error.css`, así que unificarlo pide conservar ese path. Change propio.
3. **La fase estética de los temas**, incluido el tema que anime con `prefers-reduced-motion`, que la
   delta de `theme-sheet-contracts` ya dejó permitido y medido.
