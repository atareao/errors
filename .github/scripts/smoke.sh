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
# QUÉ NO HACE
#   No construye la imagen ni publica nada: recibe una imagen ya construida
#   (`IMAGE`). No comprueba el contrato con Traefik —el leak del respaldo y la
#   exclusión de `/metrics` necesitan dos contenedores y un proxy delante—: eso vive
#   en `examples/traefik/check.sh`, que se corre a mano y no corre en el CI. Y el
#   `HEALTHCHECK` solo se puede exigir cuando la imagen lo declara: `podman`
#   construye en formato OCI, que lo descarta, y eso es un hecho sobre la
#   construcción de quien lo corre, no sobre el servicio.
#
# CONTRATO
#   ENGINE  motor de contenedores (por defecto `docker`; esta máquina usa `podman`).
#   IMAGE   referencia de la imagen ya construida (OBLIGATORIA).
#   PORT    puerto de loopback (por defecto 8080).

set -euo pipefail

: "${ENGINE:=docker}"
: "${IMAGE:?falta la variable IMAGE: hay que pasar la referencia de la imagen ya construida (p. ej. IMAGE=errserve:ci)}"
: "${PORT:=8080}"

readonly NOMBRE="errserve-smoke"
readonly BASE="http://127.0.0.1:${PORT}"

# El contenedor de prueba se retira SIEMPRE, también cuando una aserción falla (el
# requisito lo exige): un contenedor vivo dejaría el runner sucio y, con `--rm`,
# podría esconder el estado que `fallo` acaba de volcar.
limpiar() {
  "$ENGINE" rm --force "$NOMBRE" >/dev/null 2>&1 || true
}
trap limpiar EXIT

# Al fallar: anotación de Actions, el registro del contenedor y su estado, y salida
# con error. Un `test` a secas dejaría "exit code 1" y ninguna pista de qué respondió
# el servicio.
fallo() {
  echo "::error::$1"
  echo "--- $ENGINE logs $NOMBRE (últimas 40 líneas) ---"
  "$ENGINE" logs "$NOMBRE" 2>&1 | tail -n 40 || true
  echo "--- estado del contenedor ---"
  "$ENGINE" inspect --format '{{json .State}}' "$NOMBRE" 2>&1 || true
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
# la página de error, y ese asset es una de las seis páginas de tema que NO son
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
#    de ahí los 90s de margen. Si la imagen NO declara healthcheck —`podman` construye
#    en formato OCI, que lo descarta— se dice en voz alta y NO se cuenta como fallo:
#    es un hecho sobre la construcción de quien la corre, no sobre el servicio.
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
  echo "aviso la imagen no declara HEALTHCHECK (podman/OCI lo descarta): no se comprueba"
fi

# 5) La imagen no corre como root (uid/gid 10001 fijos, lo promete el README).
usuario=$("$ENGINE" inspect --format '{{.Config.User}}' "$NOMBRE")
[[ "$usuario" == "10001:10001" ]] \
  || fallo "la imagen declara USER '$usuario'; se esperaba 10001:10001"
echo "ok   USER -> $usuario"

echo "--- $ENGINE logs $NOMBRE ---"
"$ENGINE" logs "$NOMBRE"

echo "smoke test: todas las aserciones pasaron contra $IMAGE"
