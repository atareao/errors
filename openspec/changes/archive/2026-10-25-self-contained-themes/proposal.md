# Proposal: self-contained-themes

## Why

Dos hechos, y el primero es un bug en producción.

**1. El bug, medido.** En un host real, `GET https://app.territoriolinux.es/boom` responde `503` con la
página de este servicio y **sin estilos**:

    GET /boom                     -> 503  text/html   (el middleware `errors` pidió /503 y se negoció HTML)
    GET /css/error.css            -> 404  text/plain; charset=utf-8
       cuerpo: "404 Not Found\nThe requested resource could not be found on this server."
    GET /css/error.css en el host del respaldo -> 200 text/css
    Chrome: Refused to apply style from 'https://app.territoriolinux.es/css/error.css' because its
            MIME type ('text/plain') is not a supported stylesheet MIME type, and strict MIME
            checking is enabled.

La causa está en la línea 8 de cada una de las once páginas —`<link rel="stylesheet" href="/css/error.css" />`,
una ruta **absoluta**— y en cómo funciona el patrón del middleware: la página se sirve **en el host de la
aplicación**, donde este servicio no sirve assets. El `404` del CSS lo vuelve a interceptar el middleware
y lo pide como página de error con `Accept: text/css,*/*;q=0.1`, que **no negocia HTML**, así que la
cascada de formato cae al **texto plano** por defecto y el navegador rechaza la hoja. En el host del
**respaldo** la misma página se ve bien —un asset existente gana a la página de error—, y esa asimetría es
la que ha escondido el fallo: afecta a **todos** los dominios detrás del middleware, no solo al que lo
destapó.

`error-pages` es inmune por diseño y lo declara en su README: sus páginas son «fully self-contained — all
styles and images are embedded directly in the HTML, without loading any external resources». Es un
**hueco de paridad**, no un descuido del despliegue. Nota para no confundir el arreglo: el `503` **no** es
el bug —el backend del ejemplo lo devuelve a propósito y la página se sirve bien—; el bug es que sale sin
estilos.

**2. Los temas.** El mismo README de `error-pages` ofrece temas (`--template-name` / `$TEMPLATE_NAME`, con
once incorporados, y `--rotation-mode`). Este proyecto no tiene ninguno.

## What Changes

1. **El HTML servido es autocontenido.** Al servir, el servidor sustituye el `<link>` marcador por un
   bloque `<style>` con la hoja del tema elegido. Los ficheros de `content/` conservan el marcador: es lo
   que el servidor localiza. Una página **sin** marcador se sirve byte a byte, sin leer ninguna hoja.
2. **El tema es configurable:** `THEME` (variable de entorno, al arrancar, como `LOG_FORMAT`) y `X-Theme`
   (cabecera, por petición, como `X-Code` y `X-Format`).
3. **Seis temas incorporados, un tema = una hoja de estilos:** `default` (la hoja que ya existe, reglas sin
   cambios), `minimal`, `terminal`, `paper`, `amber` y `compact`. Sobrios: sin animaciones, sin recursos
   externos, sin fuentes descargadas.
4. **`Vary` declara `X-Theme`**, porque el cuerpo de la respuesta depende de esa cabecera.
5. **El bug se reproduce en el ejemplo y se guarda:** un bloque nuevo de `check.sh` sobre
   `app.localtest.me/boom` —el `503` del backend del ejemplo, que es la reproducción local exacta del
   fallo—, que hoy sale en rojo.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `error-page`: un requisito **modificado** —«Las páginas no referencian ficheros que no existen» pasa a
  hablar del marcador y de la página servida— y **dos nuevos**: «El HTML servido es autocontenido» y «El
  tema del HTML es configurable».
- `http-server`: **modificado** «La configuración se lee solo del entorno», que gana `THEME`.
- `response-extras`: **modificado** «Las cabeceras de correlación se declaran en Vary», que gana `X-Theme`.

## Non-goals

- **No se duplican las páginas por tema.** Un tema es **una hoja**, no once ficheros: once copias del
  mismo marcado no son once páginas, son una mal repartida. Un tema que quiera cambiar la estructura no
  es este mecanismo.
- **No hay rotación de temas** (`ROTATION_MODE` de `error-pages`) ni **temas personalizados**: el registro
  es incorporado al binario y un nombre que no esté en él no arranca.
- **No se toca la garantía del código.** El estado sigue siendo el código resuelto y el asset
  `/css/error.css` se sigue sirviendo con su `200 text/css`.
- **No cambia el aspecto de quien no configure `THEME`:** las reglas de la hoja `default` quedan intactas.
- **No se toca el resto de la documentación ajena** a esta prestación.

## Impact

- `src/theme.rs` — **nuevo**: el registro de temas.
- `src/config.rs` — `THEME` y su error de arranque.
- `src/render.rs` — la sustitución del marcador y el hilo del tema.
- `src/embedded.rs` — la constante única de la hoja de `default`, compartida con el asset.
- `src/main.rs` — `X-Theme` en las pistas de la petición, la resolución del tema y `Vary`.
- `content/themes/{minimal,terminal,paper,amber,compact}.css` — **nuevos**.
- `content/css/error.css` — solo el comentario de cabecera (ya no es «la única hoja del proyecto»).
- `examples/traefik/check.sh` — un bloque y un ayudante positivos.
- `README.md`, `AGENTS.md`, `examples/traefik/README.md`, `compose.yml`.
- `openspec/specs/error-page/`, `openspec/specs/http-server/` y `openspec/specs/response-extras/` — vía las
  deltas de este change.
- `Dockerfile`, `Cargo.toml`, `Cargo.lock`, `tests/content_pages.rs` — **sin cambios**.
