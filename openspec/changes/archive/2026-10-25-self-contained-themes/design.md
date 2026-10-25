# Design: self-contained-themes

## El hecho que abre el change

Una página de error servida por el middleware `errors` de Traefik llega a un navegador **sin estilos**. El
cuerpo y el código son correctos —`503 text/html`—; lo que falla es la **hoja**: la página la enlaza con
una ruta absoluta (`/css/error.css`) y ese path, en el host de la aplicación, no lo sirve este servicio
sino el upstream; su `404` lo intercepta el middleware y lo vuelve a pedir como página de error con un
`Accept` que no negocia HTML, así que la cascada de formato cae al texto plano y el navegador rechaza la
hoja por su `Content-Type`. En el host del respaldo, donde este servicio **sí** sirve el asset, la página
se ve bien. Es un hueco de paridad con `error-pages`, que incrusta los estilos en el propio HTML.

## D1 — La página se arregla incrustando la hoja, no enrutando el CSS

Tres formas, evaluadas:

1. **Incrustar la hoja en el HTML, al servir — elegida.** Es lo que hace `error-pages` y es lo único que
   arregla el caso del middleware sin tocar el proxy ni el DNS de nadie: la página deja de pedir nada para
   verse.
2. **Documentar que hay que enrutar `/css/error.css` a este servicio en cada vhost.** No arregla nada por
   defecto y multiplica por dos las reglas de Traefik de cada aplicación. Descartada.
3. **Servir el CSS con el formato correcto cuando la petición lo pida.** No es alcanzable: quien contesta
   a `/css/error.css` en el host de la aplicación es el upstream, no este servicio. Descartada.

Se elige la 1, y con ella **la prestación de temas sale casi gratis**: si el servidor tiene que saber qué
CSS incrustar, ya tiene que saber **cuál** — y eso es exactamente un tema. Por eso este change hace las dos
cosas en uno: el arreglo aterriza primero y los temas aprovechan la misma máquina.

## D2 — El marcador vive en el fichero; la hoja se sustituye al servir

Las páginas de `content/` conservan `<link rel="stylesheet" href="/css/error.css" />`, y el servidor lo
sustituye por un bloque `<style>` al servir. Tres razones:

- Las páginas siguen siendo **legibles y previsualizables** como ficheros, y el operador que las copie
  sigue viendo de dónde venía el aspecto.
- **No se duplican las once páginas por tema.** La alternativa —un juego de once páginas autocontenidas
  por tema— multiplica por seis el marcado y obliga a propagar a mano cualquier cambio de estructura.
- El marcador es un **dato del operador**: quien escriba su propia página sin él obtiene exactamente su
  página. Y por eso mismo, **sin marcador no se lee ninguna hoja**: la sustitución es la única razón para
  leerla, y no leerla evita que un CSS roto pueda afectar a una página que no lo necesita.

## D3 — Un tema es una hoja, y la hoja del `default` es la de siempre

El registro es una tabla en el binario con `name`, `sheet_path` y los bytes embebidos. La ruta de cada tema
dentro de `CONTENT_DIR` es estable y documentada: `css/error.css` para `default` y `themes/<nombre>.css`
para los demás. Que `default` no viva en `themes/` es deliberado y no es una asimetría gratuita: su ruta es
el **path público** del asset que el proyecto ya sirve, y moverlo rompería el contrato del asset y las
pruebas que lo fijan. Los cinco temas nuevos no tienen path público que preservar.

La hoja de `default` es **la misma constante** que alimenta el asset `/css/error.css`: un solo
`include_bytes!`, cero copias de los mismos bytes en el binario.

## D4 — La hoja se resuelve como un asset, y no puede divergir de él

La hoja de un tema se resuelve con la misma regla que cualquier asset: un fichero legible en
`CONTENT_DIR/<sheet_path>` gana a la copia embebida. La alternativa —incrustar siempre los bytes del
binario e ignorar el disco— es más simple pero tiene un defecto que el proyecto ya rechazó una vez:
**divergencia silenciosa**. Con un `content/css/error.css` propio del operador, `/css/error.css` serviría
una hoja y la página incrustaría otra, y el aspecto de la página no tendría nada que ver con el asset que
el mismo servicio publica. Resolviendo las dos por la misma regla, discrepar es imposible.

Y por eso la hoja se lee **al servir** (el render ya corre en `spawn_blocking`, así que la E/S no bloquea el
hilo async): un operador que edite su hoja la ve cambiar sin reiniciar, como le pasa hoy con sus páginas.

## D5 — Un fallo al leer la hoja no tumba la página (divergencia deliberada de la Opción A)

Si el fichero de la hoja existe y **no se puede leer**, se registra un `warn!` nombrando el fichero y se
usa la copia embebida. **No** se degrada la página al respaldo de texto.

Es una divergencia **consciente** de la Opción A de `embedded-error-pages` —que para las **páginas** manda
el respaldo de texto y el `warn`, para no tapar en silencio un directorio del operador roto—. El motivo de
la diferencia: la página es **contenido** y su ausencia es un hecho sobre el error que se está contando; la
hoja es **adorno**. Degradar un `503` a texto plano porque un CSS no se pudo leer empeora el síntoma que el
servicio existe para aliviar, y el `warn` ya impide que el fallo pase inadvertido. Ni la garantía 1 (el
estado es siempre el código resuelto) ni la 4 (el asset ganó) se ven afectadas.

## D6 — `THEME` falla al arrancar; `X-Theme` cae al tema configurado

La asimetría es deliberada y es la misma que ya rige en el proyecto entre configuración y petición:

- **`THEME`** la escribe el **operador**, al desplegar, y cree que está configurando algo: un valor que no
  sea uno de los seis nombres —o la cadena vacía— aborta el arranque con un error que **nombra la
  variable** y **lista los nombres válidos**. Exactamente el patrón de `LOG_FORMAT` y `DEFAULT_ERROR_PAGE`.
- **`X-Theme`** la elige **quien llama**, y un cliente no puede romper el servicio: un valor desconocido cae
  al tema configurado, sin error. Exactamente el patrón de `X-Code` y `X-Format`.

Precedencia: `X-Theme` conocido gana a `THEME`, con el principio de siempre —el **primer** origen que haga
match—. Comparación **exacta**, sensible a mayúsculas: `X-Theme: Terminal` no es el tema `terminal`, y eso
se documenta en lugar de adivinar la intención de quien lo escribe.

`X-Theme` solo afecta al formato HTML: un JSON no tiene hoja y el tema se ignora.

## D7 — `Vary` declara `X-Theme`, y el resto de la cabecera no se toca

El cuerpo de la respuesta HTML **depende** de `X-Theme`. Sin declararlo, una caché intermedia podría
servirle a un cliente el tema que pidió otro —y, con un CDN por medio, el cuerpo entero de otro—. Se añade
a la lista fija de `Vary`, con la misma regla que ya vale para las cabeceras de correlación: la lista
**no** cambia según la petición, porque una lista variable no le sirve a una caché.

El resto de las cabeceras no se toca: `X-Theme` no se copia a la respuesta (no es una cabecera de
correlación) y no entra en el registro ni en el contador.

## D8 — Los seis temas, y por qué sobrios

`default` es la hoja que el proyecto ya trae, con sus reglas intactas: quien no configure `THEME` no ve
ningún cambio de aspecto. Los otros cinco son deliberadamente **discretos** —sin animaciones, sin
imágenes, sin recursos externos, sin fuentes descargadas—: son páginas de **error**, y su trabajo es decir
qué ha pasado sin llamar más la atención de la que ya reclama un fallo. Ninguno de los cinco usa la clase
`c<código>` del `body`; la clase sigue en el marcado y cada tema puede usarla, pero ninguno la necesita
para ser útil.

Ninguna hoja puede referenciar un recurso externo: una página que necesita la red para verse es
exactamente el defecto que este change corrige.

## Open Questions

1. **Temas personalizados.** Hoy el registro es incorporado: `THEME` no admite un tema que no esté en el
   binario. Un `CONTENT_DIR/themes/<nombre>.css` **sí** puede reemplazar la hoja de un tema incorporado,
   pero no **añadir** un nombre nuevo, porque eso haría que `THEME` dependiera del sistema de ficheros en
   el arranque. Si alguien lo pide, el sitio natural es un descubrimiento del directorio al arrancar.
2. **Rotación.** `error-pages` tiene `ROTATION_MODE` (aleatorio al arrancar, horario, diario). Es paridad,
   pero añade estado y pruebas de aleatoriedad, y no está pedido.
3. **Acento por código en los temas nuevos.** Ninguno de los cinco usa `body.c<código>`. Si se quiere que
   un tema diferencie 4xx de 5xx, el marcado ya lo permite y es un cambio de una hoja.
4. **La hoja `default` sigue siendo un asset público.** `/css/error.css` se sigue sirviendo con `200` a
   propósito: no es una página de error y hay instalaciones que lo enlazan. Incrustar la hoja en el HTML no
   lo invalida.
