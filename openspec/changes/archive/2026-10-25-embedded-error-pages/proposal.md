# Proposal: embedded-error-pages

## Why

`errserve` 0.3.0 está publicado en crates.io, así que además de la imagen de Docker hay una
segunda vía de instalación: `cargo install errserve`. Medida en un directorio vacío y con el
binario instalado desde el registro, esa vía funciona en todo lo que el código **genera**:
`/healthz` responde `200`, `/version` responde `{"version":"0.3.0"}`, `GET /404` responde `404` con
`content-type: text/plain` y 72 bytes, `GET /503.xml` responde `503` con su XML y su
`retry-after: 120`, y `GET /429` responde `429` con su `Retry-After: 120`. Ninguno de esos cuerpos
necesita un fichero de datos.

El HTML sí. `GET /404.html` y `GET /404` con `Accept: text/html` responden
`content-type: text/plain` con el cuerpo pelado del respaldo: sin estilo y sin página. Con
`CONTENT_DIR` apuntando al `content/` del repositorio, esas dos peticiones devuelven el HTML de
verdad —`content-type: text/html`, 460 bytes, con su `<link rel="stylesheet" href="/css/error.css" />`—,
y sin él la hoja compartida tampoco existe, porque se sirve como asset estático de ese mismo
directorio.

El motivo está en el código y no es un defecto: `render_body` con `Format::Html` lee del disco y,
si no encuentra ni el fichero del código pedido ni el del código por defecto, devuelve
`MissingHtml` y el router responde `render_text_fallback`, conservando el estado resuelto y las
cabeceras (requisito vivo «Un fallo al leer el HTML no cambia el estado»). Esa red es correcta y se
queda. Lo que no es aceptable es que hoy sea también el camino **normal** de quien instaló el
crate: un servidor que funciona, pero que presenta en texto plano y sin estilo lo único que este
servicio existe para hacer.

## What Changes

1. **Las once páginas HTML y la hoja `content/css/error.css` se embeben en el binario** con
   `include_str!`/`include_bytes!`, en tiempo de compilación y sin dependencias nuevas. El `.crate`
   que publica crates.io ya lleva `content/` dentro (verificado con `cargo package --list --offline`),
   así que las macros encuentran los ficheros al construir desde el registro.
2. **La copia embebida es la base, y el fichero del operador que se pueda leer gana.** La cascada de
   hoy —fichero del código pedido y, si no existe, fichero del código por defecto— se intenta primero,
   **entera**, sobre `CONTENT_DIR`; solo si no da nada se repite sobre las copias embebidas.
   `CONTENT_DIR` no cambia de nombre ni de defecto: pasa a ser el *override* de una base que ya no falta.
3. **El asset `css/error.css` sigue la misma regla**: el fichero de `CONTENT_DIR` cuando exista y se
   pueda leer, la copia embebida si no. Sin esto, el HTML embebido enlazaría a una hoja que responde
   página de error, es decir un `404` dentro del `404`, y la página seguiría saliendo sin estilo.
4. **`MissingHtml` y `render_text_fallback` se conservan tal cual, y el requisito que los fija no se
   revoca.** Con las páginas embebidas ese camino deja de ser alcanzable para los once códigos de la
   lista, pero sigue teniendo trabajo: con `DEFAULT_ERROR_PAGE=418` —valor legal, fuera de la
   lista— ninguna página cubriría un código sin página propia, y sin el respaldo eso sería un `500`.
5. **Una prueba fija que la copia embebida y la del repositorio son idénticas**, byte a byte, para
   las once páginas y para la hoja, y en los dos sentidos: lo que está embebido existe en `content/`
   y todo `content/*.html` está embebido. Sin ella las dos copias divergen en silencio: alguien
   retoca `content/503.html` y quien instaló el crate sigue viendo la vieja, sin que nadie se entere.
6. **Ningún `200` nuevo.** Las páginas embebidas se sirven por la misma vía que las del disco —la
   página de error, con el código resuelto, nunca como asset— y el rango de códigos sigue siendo
   `400..=599`.

## Capabilities

### New Capabilities

(ninguna: el cambio es **de dónde salen unos bytes que ya existen**, no una capacidad nueva del
servicio)

### Modified Capabilities

- `error-page`: cuatro requisitos. «El cuerpo depende del formato y el código es ortogonal» pasa a
  describir la copia embebida y la precedencia del fichero; «Un fallo al leer el HTML no cambia el
  estado» se conserva, con su redacción ajustada a que el respaldo deja de ser el camino normal;
  «Los códigos que un proxy emite tienen su propia página» pasa a exigir página **embebida** para
  los once códigos, idéntica a la del directorio; y «Las páginas no referencian ficheros que no
  existen» cubre también las referencias de la copia embebida —la hoja, sin `CONTENT_DIR`—.
- `http-server`: dos requisitos. «Los assets estáticos se sirven desde CONTENT_DIR» pasa a servir el
  asset embebido cuando el fichero no existe; «La configuración se lee solo del entorno» describe
  `CONTENT_DIR` como *override* de los recursos embebidos —misma variable, mismo defecto—.

## Non-goals

- **No se toca la resolución del código** —la cascada path → `X-Code` → `DEFAULT_ERROR_PAGE`—, ni la
  del formato, ni la normalización del path, ni las cabeceras de acompañamiento, ni el contador de
  métricas. Ninguna de esas piezas cambia de comportamiento con este change.
- **`CONTENT_DIR` no se elimina**, ni cambia de nombre, ni de defecto, ni deja de ganar —un fichero
  que se pueda leer gana—. Sigue siendo la única ruta de estáticos.
- **No se embebe nada que no esté ya en `content/`**: ni plantillas, ni temas, ni un motor. Los bytes
  embebidos son esos mismos ficheros y su paridad es un requisito, no una promesa.
- **No se añaden dependencias.** El embedding lo hace el compilador; `rust-embed`, `include_dir` o un
  tar propio serían una dependencia —o un formato— nuevos para hacer lo que `include_str!` ya hace, y
  la puerta de dependencias (`cargo deny check -D warnings`) se paga por cada una.
- **No se cubren más códigos.** La lista sigue siendo los once; fuera de ella, la caída al código por
  defecto funciona igual que hoy, y ahora suele acabar en la página embebida del código por defecto.
- **No se decide aquí qué hacer con `content/` en la imagen de Docker.** Va como *Open Question* en
  `design.md`, con la recomendación de conservarlo tal cual.

## Impact

- `src/embedded.rs` (**nuevo**) — la tabla de páginas y de assets embebidos, sus
  `include_str!`/`include_bytes!` y su consulta por código y por path.
- `src/render.rs` — `html_body` consulta el fichero y, si no, la copia embebida, con la precedencia de
  D1; `MissingHtml` y `render_text_fallback` no cambian de forma ni de contrato.
- `src/main.rs` — el *fallback* de `ServeDir` consulta la tabla de assets cuando no encuentra el
  fichero. El resto del router —endpoints de servicio, middleware, cabeceras— no se toca.
- `tests/fixtures/content/css/error.css` (**nuevo**) — la hoja propia del directorio de fixtures, para
  fijar que un fichero del directorio que se pueda leer gana también en el asset.
- `content/` — **sin cambios**: los bytes embebidos son estos ficheros.
- `README.md` y `AGENTS.md` — `CONTENT_DIR` como *override* y qué sirve hoy un `cargo install`.
- `openspec/specs/` — `error-page` y `http-server`, vía las deltas de este change.
- `Dockerfile`, `.dockerignore` y `release.yml` — **sin cambios en este change** (ver *Open Question*
  en `design.md`).
- tests — ninguna aserción existente cambia de valor esperado, salvo la del respaldo de texto, que
  pasa a usar un `DEFAULT_ERROR_PAGE` fuera de la lista embebida para seguir siendo alcanzable; se
  añaden la página embebida sin ficheros de datos, la precedencia disco-sobre-embebido, la hoja
  embebida y la paridad de las dos copias.
