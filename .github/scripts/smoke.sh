#!/usr/bin/env bash
#
# Smoke test de la imagen arrancada — el **único** sitio donde viven las aserciones
# sobre el servicio.
#
# POR QUÉ EXISTE
#   La `v0.8.0` se publicó al **segundo** intento. El primer fallo llegó con la
#   etiqueta ya empujada: la aserción `GET /css/error.css -> 200` vivía en un solo
#   `run:` de `release.yml`, y ese mismo release retiraba el asset. Ninguna de las
#   puertas del PR la vio. La lección no es «añadir el CI»: es que **dos copias de la
#   misma lista divergen**. Por eso las aserciones viven aquí, en un artefacto
#   versionado, y los dos workflows que necesitan probar la imagen lo invocan por su
#   ruta literal: el CI en cada propuesta de cambio, el release antes de publicar
#   nada.
#
#   Y hay una comprobación que **solo** cabe aquí: la imagen corre como uid/gid
#   10001 y lee el `CONTENT_DIR` que el **operador monta**, así que el barrido de ese
#   directorio al arrancar —el que convierte un `*.html` de más en un tema— es una
#   propiedad de la **imagen**, no del código: `cargo test` lee el directorio con el
#   uid de quien compila y no puede ver que un montaje en `0700` deje el fichero
#   ilegible para el contenedor. Por eso el script arranca un **segundo** contenedor,
#   con su propio `CONTENT_DIR` montado y un tema de más.
#
# QUÉ NO HACE
#   No construye la imagen ni publica nada: recibe una imagen ya construida
#   (`IMAGE`). No comprueba el contrato con Traefik —el leak del respaldo y la
#   exclusión de `/metrics` necesitan dos contenedores y un proxy delante—: eso vive
#   en `examples/traefik/check.sh`, que se corre a mano y no corre en el CI. Y el
#   `HEALTHCHECK` solo se puede exigir cuando la imagen lo declara: `podman build`
#   **sin** `--format docker` construye en formato OCI, que lo descarta —esta casa
#   sí pasa ese flag (`.justfile`), así que aquí sí se declara y sí se comprueba—,
#   y eso es un hecho sobre la construcción de quien lo corre, no sobre el servicio.
#
# CONTRATO
#   ENGINE  motor de contenedores (por defecto `docker`; esta máquina usa `podman`).
#   IMAGE   referencia de la imagen ya construida (OBLIGATORIA).
#   PORT    puerto de loopback (por defecto 8080).
#   PORT_VOLUMEN  puerto de loopback del contenedor con `CONTENT_DIR` montado
#                 (por defecto `PORT` + 1).

set -euo pipefail

: "${ENGINE:=docker}"
: "${IMAGE:?falta la variable IMAGE: hay que pasar la referencia de la imagen ya construida (p. ej. IMAGE=errserve:ci)}"
: "${PORT:=8080}"
: "${PORT_VOLUMEN:=$((PORT + 1))}"

readonly NOMBRE="errserve-smoke"
readonly NOMBRE_VOLUMEN="errserve-smoke-volumen"
readonly BASE="http://127.0.0.1:${PORT}"
readonly BASE_VOLUMEN="http://127.0.0.1:${PORT_VOLUMEN}"

# El contenedor de prueba se retira SIEMPRE, también cuando una aserción falla (el
# requisito lo exige): un contenedor vivo dejaría el runner sucio y, con `--rm`,
# podría esconder el estado que `fallo` acaba de volcar. Son **dos** contenedores —el
# de arriba y el del `CONTENT_DIR` montado—, y el directorio temporal de ese montaje
# (vacío mientras no exista: `limpiar` corre antes de que se cree).
limpiar() {
  "$ENGINE" rm --force "$NOMBRE" >/dev/null 2>&1 || true
  "$ENGINE" rm --force "$NOMBRE_VOLUMEN" >/dev/null 2>&1 || true
  if [[ -n "${CONTENIDO:-}" ]]; then rm -rf "$CONTENIDO" || true; fi
}
trap limpiar EXIT

# Al fallar: anotación de Actions, el registro y el estado de los contenedores que
# existan, y salida con error. Un `test` a secas dejaría "exit code 1" y ninguna
# pista de qué respondió el servicio; y volcar solo el primero daría un volcado del
# contenedor equivocado cuando el segundo es el que no arrancó —peor que no dar
# ninguno—.
fallo() {
  echo "::error::$1"
  for nombre in "$NOMBRE" "$NOMBRE_VOLUMEN"; do
    # Un contenedor que todavía **no existe** no se vuelca: si el fallo es del
    # primero —el arranque con `THEME=neon` aborta, o falla la aserción 1—, el
    # segundo aún no se ha creado y el motor imprimiría "no such container",
    # ensuciando el diagnóstico. Se comprueba la existencia y se salta en silencio.
    if ! "$ENGINE" inspect "$nombre" >/dev/null 2>&1; then continue; fi
    echo "--- $ENGINE logs $nombre (últimas 40 líneas) ---"
    "$ENGINE" logs "$nombre" 2>&1 | tail -n 40 || true
    echo "--- estado del contenedor $nombre ---"
    "$ENGINE" inspect --format '{{json .State}}' "$nombre" 2>&1 || true
  done
  exit 1
}

# Un resto anterior del mismo nombre rompería el arranque: se retira antes.
limpiar

# Se publica **solo** en loopback: el puerto no tiene por qué quedar expuesto en el
# runner, y lo único que hace falta es hablar con él desde el propio job.
"$ENGINE" run --detach --rm --name "$NOMBRE" \
  --publish "127.0.0.1:${PORT}:8080" \
  "$IMAGE" >/dev/null

# 1) Que arranque. `/healthz` responde siempre 200 y no pasa por la resolución de
#    páginas de error, así que sirve de puerta sin depender de nada más.
listo=0
for _ in $(seq 1 20); do
  if curl -fsS -o /dev/null "$BASE/healthz"; then listo=1; break; fi
  sleep 1
done
[[ "$listo" == "1" ]] || fallo "el contenedor no respondió en /healthz tras 20 intentos"

# 2) El estado de cada código. La garantía del proyecto: una página de error devuelve
#    **su** código, nunca 200.
comprobar() {
  local real
  real=$(curl -s -o /dev/null -w '%{http_code}' "$BASE$1")
  [[ "$real" == "$2" ]] || fallo "GET $1 devolvió $real; se esperaba $2"
  echo "ok   GET $1 -> $real"
}

comprobar /404 404                                   # página de error
comprobar /503 503                                   # 5xx, con Retry-After
# El `502` es el código con el que un proxy responde a un upstream caído: el
# escenario que justifica el servicio. NO lleva `Retry-After` —solo `429` y `503` la
# llevan—, y su ausencia se comprueba abajo con `sin_cabecera`.
comprobar /502 502
comprobar /404.json 404                              # el formato no cambia el código
comprobar /healthz 200                               # endpoint de servicio
comprobar /metrics 200                               # contador Prometheus
# La garantía 4, con una página de tema que no es el alias: un asset existente gana a
# la página de error, y ese asset es una página de tema que NO es
# `/default.html`. El `200` es CRUDO —el fichero tal cual, con sus marcadores
# `{{code}}`/`{{reason}}`/`{{description}}` a la vista—, que es justo lo que la
# garantía promete; el bloqueo de ese leak va en el proxy, no en el servicio.
comprobar /minimal.html 200
# La excepción de la garantía 4: `/default.html` (y `/default.htm`) es el **alias del
# código por defecto**, se intercepta y responde el código resuelto
# (`DEFAULT_ERROR_PAGE`, `404` de fábrica) — **nunca `200`**.
comprobar /default.html 404
comprobar /default.htm 404

# El cuerpo HTML servido, que es el corazón de la entrega: la página del tema lleva su
# `<style>` **incrustado** y **no** enlaza ninguna hoja. Ojo al detalle que este
# repositorio documenta: un `curl` sin `Accept` cae en **texto plano** por defecto, así
# que una comprobación de cuerpo TIENE que mandar `-H 'Accept: text/html'` o está
# midiendo el formato equivocado.
cuerpo() {
  curl -s -H 'Accept: text/html' "$BASE$1"
}

cuerpo /404 | grep -q '<style' \
  || fallo "el cuerpo de /404 en HTML no lleva su <style> incrustado"
echo "ok   GET /404 (Accept: text/html) -> lleva <style> incrustado"

if cuerpo /404 | grep -q '<link'; then
  fallo "el cuerpo de /404 en HTML lleva un <link>: no es autocontenido"
fi
echo "ok   GET /404 (Accept: text/html) -> sin <link>"

# 3) Cabeceras que la documentación promete.
cabecera() {
  curl -s -D - -o /dev/null "$BASE$1" | grep -qi "^$2:" \
    || fallo "GET $1 no trae la cabecera $2"
  echo "ok   GET $1 -> $2"
}

# La ausencia de una cabecera es tan contrato como su presencia: `Retry-After` solo la
# llevan `429` y `503`, así que el `502` —su vecino— la demuestra por contraste.
sin_cabecera() {
  if curl -s -D - -o /dev/null "$BASE$1" | grep -qi "^$2:"; then
    fallo "GET $1 trae la cabecera $2, que no debería llevar"
  fi
  echo "ok   GET $1 -> sin $2"
}

cabecera /404 x-robots-tag
cabecera /502 x-robots-tag
sin_cabecera /502 retry-after
cabecera /503 retry-after
curl -s -D - -o /dev/null "$BASE/503" | grep -qi '^retry-after: 120' \
  || fallo "el 503 no trae Retry-After: 120"
echo "ok   GET /503 -> Retry-After: 120"

# 4) El HEALTHCHECK del Dockerfile (wget de busybox contra /healthz), ejercitado en
#    docker de verdad. Su `interval` es 30s, así que el primer veredicto tarda ~30s:
#    de ahí los 90s de margen. Si la imagen NO declara healthcheck —`podman build`
#    **sin** `--format docker` construye en formato OCI, que lo descarta; esta casa
#    sí pasa ese flag (`.justfile`) y el healthcheck sí se comprueba— se dice en voz
#    alta y NO se cuenta como fallo: es un hecho sobre la construcción de quien la
#    corre, no sobre el servicio.
declarado=$("$ENGINE" inspect --format '{{if .Config.Healthcheck}}si{{else}}no{{end}}' "$NOMBRE")
if [[ "$declarado" == "si" ]]; then
  estado=""
  for _ in $(seq 1 45); do
    estado=$("$ENGINE" inspect --format '{{.State.Health.Status}}' "$NOMBRE" 2>/dev/null \
      || echo sin-healthcheck)
    if [[ "$estado" == "healthy" ]]; then break; fi
    sleep 2
  done
  [[ "$estado" == "healthy" ]] \
    || fallo "el healthcheck de la imagen quedó en '$estado' (se esperaba 'healthy')"
  echo "ok   healthcheck -> healthy"
else
  echo "aviso la imagen no declara HEALTHCHECK (podman build sin --format docker construye en OCI y lo descarta): no se comprueba"
fi

# 5) La imagen no corre como root (uid/gid 10001 fijos, lo promete el README).
usuario=$("$ENGINE" inspect --format '{{.Config.User}}' "$NOMBRE")
[[ "$usuario" == "10001:10001" ]] \
  || fallo "la imagen declara USER '$usuario'; se esperaba 10001:10001"
echo "ok   USER -> $usuario"

# 6) El directorio del operador **añade** temas (change `themes-from-content-dir`).
#
# El registro de temas deja de ser una tabla cerrada en el binario: se construye **al
# arrancar** con los **temas embebidos** más los `*.html` de la **raíz** de
# `CONTENT_DIR` —un fichero de más **es** un tema, y su tallo es su nombre—. En la
# imagen `CONTENT_DIR` es la ruta que el **operador monta** y el proceso corre como
# uid/gid **10001**, así que el barrido depende de que el montaje sea legible por ese
# uid: con el directorio en `0700` el contenedor no puede ni atravesarlo y `neon.html`
# responde `404`; con `0755` responde `200`. Es una propiedad de la **imagen** que
# `cargo test` **no puede ver** —ahí el directorio se lee con el uid de quien compila—,
# y el modo de fallo es silencioso si el operador se apoya solo en `X-Theme` —el tema
# configurado se sirve igual, con los temas embebidos detrás—. Por eso la primera
# puerta es el bucle de `/healthz`: con `THEME=neon` y el barrido sin ver el fichero,
# el arranque **aborta** enumerando los temas embebidos.
#
# Va en un contenedor **aparte** y **no** en el de arriba: montar un `CONTENT_DIR`
# propio deja **fuera** el `content/` que la imagen trae, y con él la aserción del leak
# (`/minimal.html -> 200`, la garantía 4), que es una propiedad de lo que la imagen
# **entrega**, no del montaje.
CONTENIDO="$(mktemp -d)"
cat > "$CONTENIDO/neon.html" <<'PAGINA'
<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8">
    <title>{{code}} {{reason}}</title>
    <style>
      body { background: #101014; color: #e6e6e6; font-family: sans-serif; }
      h1 { font-size: 4rem; margin: 0; }
    </style>
  </head>
  <body class="c{{code}}">
    <main>
      <h1>{{code}}</h1>
      <p>{{description}}</p>
      <p>NEON-SMOKE</p>
    </main>
    <footer>errserve</footer>
  </body>
</html>
PAGINA
# `mktemp -d` crea el directorio en **0700**, y el contenedor corre como uid 10001:
# sin esto no puede ni atravesarlo (medido: `404` con `0700`, `200` con `0755`).
chmod 755 "$CONTENIDO"
chmod 644 "$CONTENIDO/neon.html"

# Sin `--rm` **a propósito**: el fallo probable de este arranque es que `THEME=neon`
# aborte por no ver el fichero, y con `--rm` el contenedor se retiraría al salir y
# `fallo` no podría volcar el registro que lo dice —que es justo el volcado que hace
# falta—. Lo retira `limpiar`, que corre siempre.
"$ENGINE" run --detach --name "$NOMBRE_VOLUMEN" \
  --publish "127.0.0.1:${PORT_VOLUMEN}:8080" \
  --volume "$CONTENIDO:/contenido:ro" \
  --env CONTENT_DIR=/contenido \
  --env THEME=neon \
  "$IMAGE" >/dev/null

listo=0
for _ in $(seq 1 20); do
  if curl -fsS -o /dev/null "$BASE_VOLUMEN/healthz"; then listo=1; break; fi
  sleep 1
done
[[ "$listo" == "1" ]] || fallo "el contenedor con CONTENT_DIR montado no respondió en /healthz tras 20 intentos: el barrido no vio 'neon.html' —el arranque aborta enumerando los temas embebidos— o el montaje no es legible por el uid 10001"

# 6.a) El tema descubierto arrancó y sirve **su** fichero, que es su **única** copia
#      —no tiene página embebida a la que caer—: si el barrido no lo hubiera visto, el
#      arranque habría abortado y aquí no habría ni servicio (de ahí el bucle de
#      arriba como primera puerta).
neon=$(curl -s -H 'Accept: text/html' "$BASE_VOLUMEN/404")
grep -q 'NEON-SMOKE' <<<"$neon" \
  || fallo "GET /404 con THEME=neon no trae 'NEON-SMOKE': el tema descubierto no sirvió el fichero montado"
echo "ok   GET /404 (THEME=neon, Accept: text/html) -> sirve la página del fichero montado"

# 6.b) `X-Theme` elige **por petición** contra el registro: un tema **conocido** gana
#      al configurado, y `minimal` es uno de los temas embebidos, así que esto no
#      depende del montaje.
minimal=$(curl -s -H 'Accept: text/html' -H 'X-Theme: minimal' "$BASE_VOLUMEN/404")
if grep -q 'NEON-SMOKE' <<<"$minimal"; then
  fallo "GET /404 con X-Theme: minimal trae 'NEON-SMOKE': un tema conocido tiene que ganar a THEME=neon"
fi
grep -q '<style' <<<"$minimal" \
  || fallo "GET /404 con X-Theme: minimal no lleva <style>: no es la página de un tema"
[[ "$minimal" != "$neon" ]] \
  || fallo "el cuerpo de X-Theme: minimal es idéntico al de THEME=neon: la cabecera no eligió otro tema"
echo "ok   GET /404 (X-Theme: minimal) -> otro tema: sin 'NEON-SMOKE' y con <style>"

# 6.c) Un `X-Theme` **desconocido** cae al configurado, **sin error**: esa cabecera la
#      elige quien llama, así que un valor que no esté en el registro no puede romper
#      el servicio ni cambiar la página a otra cosa. El estado se comprueba aparte
#      porque el cuerpo del configurado saldría igual si la petición fallara.
codigo=$(curl -s -o /dev/null -w '%{http_code}' -H 'X-Theme: noexiste' "$BASE_VOLUMEN/404")
[[ "$codigo" == "404" ]] \
  || fallo "GET /404 con X-Theme: noexiste devolvió $codigo; un X-Theme desconocido no puede romper el servicio"
desconocido=$(curl -s -H 'Accept: text/html' -H 'X-Theme: noexiste' "$BASE_VOLUMEN/404")
grep -q 'NEON-SMOKE' <<<"$desconocido" \
  || fallo "GET /404 con X-Theme: noexiste no trae 'NEON-SMOKE': tiene que caer al tema configurado (neon)"
echo "ok   GET /404 (X-Theme: noexiste) -> $codigo y el tema configurado: cae sin error"

echo "--- $ENGINE logs $NOMBRE ---"
"$ENGINE" logs "$NOMBRE"

echo "smoke test: todas las aserciones pasaron contra $IMAGE"
