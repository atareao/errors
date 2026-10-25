#!/usr/bin/env sh
# Comprobación de la integración, caso por caso. Uso:
#
#   ./check.sh              # puerto 8474, el que publica compose.yml
#   ./check.sh 9090         # otro puerto
#
# Requiere que el ejemplo esté levantado (compose.yml) y curl en el host. Sale
# con estado 1 si algún caso no cuadra, para que valga también en CI.
#
# Comprueba seis bloques: los cuatro estados de la tabla del ejemplo, las
# cabeceras de la página servida, un bloque nuevo sobre la página del 503 y, como
# sexto, que el respaldo NO publique el endpoint de servicio `/metrics`.
#
# El bloque nuevo va entre las cabeceras y `/metrics`, sobre `app.localtest.me/boom`
# —el 503 del backend, que es la reproducción local exacta del bug medido en
# producción— y manda `Accept: text/html`: sin esa pista la cascada de formato no
# elige HTML (curl manda `*/*` y recibiría texto plano) y el bloque no probaría
# nada. Comprueba que el cuerpo lleva la hoja INCRUSTADA (`<style`), que NO enlaza
# `/css/error.css` —la ruta absoluta que en el host de la aplicación no sirve
# errserve, el `404` con `Content-Type: text/plain` que hacía que el navegador
# rechazara la hoja— y que el cuerpo cambia al pedir con `X-Theme: terminal`. Con
# el árbol de hoy los tres casos salen en ROJO: la página enlaza la hoja en vez de
# incrustarla y no hay ningún tema que elegir.
#
# Y en el sexto bloque, el de `/metrics`, quien DISCRIMINA entre la fuga y el
# arreglo es el estado, el `Content-Type` y el cuerpo: en la fuga la respuesta es
# `200` con `Content-Type: text/plain; version=0.0.4` y lleva el contador, y con
# el arreglo es `404` de Traefik con `Content-Type: text/plain; charset=utf-8` y
# sin contador. La ausencia de `X-Robots-Tag` es solo una GUARDA, no un
# discriminante: `/metrics` es un endpoint de servicio del binario, nunca una
# página de error, así que no lleva esa cabecera ni en la fuga. Se comprueba para
# que nadie «arregle» la fuga convirtiendo `/metrics` en una página de error.
#
# Los nombres de host se mandan en la cabecera `Host`, así que no hace falta DNS
# (localtest.me y sus subdominios apuntan a 127.0.0.1, pero esto funciona igual
# sin red).
set -eu

port="${1:-8474}"
base="http://127.0.0.1:${port}"
failed=0

# code <host> <path> <estado esperado> <qué demuestra>
code() {
    got=$(curl -sS -m 10 -o /dev/null -w '%{http_code}' -H "Host: $1" "${base}$2")
    if [ "$got" = "$3" ]; then
        printf 'ok     %-22s %-6s %s  %s\n' "$1" "$2" "$got" "$4"
    else
        printf 'FALLO  %-22s %-6s %s (esperaba %s)  %s\n' "$1" "$2" "$got" "$3" "$4"
        failed=1
    fi
}

# header <host> <path> <cabecera>
header() {
    if ! raw=$(curl -sS -m 10 -D - -o /dev/null -H "Host: $1" "${base}$2"); then
        printf 'FALLO  %s%s no respondio el servicio: no hay cabeceras donde buscar %s\n' \
            "$1" "$2" "$3"
        failed=1
        return
    fi
    line=$(printf '%s\n' "$raw" | tr -d '\r' | grep -i "^$3:" || true)
    if [ -n "$line" ]; then
        printf 'ok     %s\n' "$line"
    else
        printf 'FALLO  falta la cabecera %s en %s%s\n' "$3" "$1" "$2"
        failed=1
    fi
}

# Los cuatro ayudantes negativos: afirman lo que NO debe pasar. El fichero sólo
# sabía afirmar, y la exclusión de `/metrics` se demuestra negando —no es un
# estado, es la ausencia de una firma y de un contador—. Los cuatro tratan un
# curl que no responde como FALLO explícito: un contenedor caído no puede colarse
# como «la firma no está» (sería un falso verde en una comprobación de
# seguridad).

# not_code <host> <path> <estado prohibido> <qué demuestra>
not_code() {
    if ! got=$(curl -sS -m 10 -o /dev/null -w '%{http_code}' -H "Host: $1" "${base}$2"); then
        printf 'FALLO  %-22s %-6s no respondio el servicio  %s\n' "$1" "$2" "$4"
        failed=1
        return
    fi
    if [ "$got" != "$3" ]; then
        printf 'ok     %-22s %-6s %s (no %s)  %s\n' "$1" "$2" "$got" "$3" "$4"
    else
        printf 'FALLO  %-22s %-6s %s (prohibido)  %s\n' "$1" "$2" "$got" "$4"
        failed=1
    fi
}

# not_header <host> <path> <cabecera> <qué demuestra>
not_header() {
    if ! raw=$(curl -sS -m 10 -D - -o /dev/null -H "Host: $1" "${base}$2"); then
        printf 'FALLO  %-22s %-6s no respondio el servicio  %s\n' "$1" "$2" "$4"
        failed=1
        return
    fi
    line=$(printf '%s\n' "$raw" | tr -d '\r' | grep -i "^$3:" || true)
    if [ -z "$line" ]; then
        printf 'ok     %-22s %-6s sin %s  %s\n' "$1" "$2" "$3" "$4"
    else
        printf 'FALLO  %-22s %-6s %s presente  %s\n' "$1" "$2" "$3" "$4"
        failed=1
    fi
}

# not_type <host> <path> <tipo prohibido> <qué demuestra> — falla si el
# `Content-Type` CONTIENE ese valor.
not_type() {
    if ! raw=$(curl -sS -m 10 -D - -o /dev/null -H "Host: $1" "${base}$2"); then
        printf 'FALLO  %-22s %-6s no respondio el servicio  %s\n' "$1" "$2" "$4"
        failed=1
        return
    fi
    line=$(printf '%s\n' "$raw" | tr -d '\r' | grep -i '^content-type:' || true)
    case "$line" in
        *"$3"*)
            printf 'FALLO  %-22s %-6s el Content-Type lleva %s  %s\n' "$1" "$2" "$3" "$4"
            failed=1
            ;;
        *)
            printf 'ok     %-22s %-6s el Content-Type no lleva %s  %s\n' "$1" "$2" "$3" "$4"
            ;;
    esac
}

# not_body <host> <path> <texto> <qué demuestra>
not_body() {
    if ! body=$(curl -sS -m 10 -H "Host: $1" "${base}$2"); then
        printf 'FALLO  %-22s %-6s no respondio el servicio  %s\n' "$1" "$2" "$4"
        failed=1
        return
    fi
    if printf '%s' "$body" | grep -q -- "$3"; then
        printf 'FALLO  %-22s %-6s el cuerpo lleva %s  %s\n' "$1" "$2" "$3" "$4"
        failed=1
    else
        printf 'ok     %-22s %-6s el cuerpo no lleva %s  %s\n' "$1" "$2" "$3" "$4"
    fi
}

# Los tres ayudantes del bloque autocontenido. Los dos primeros son la pareja
# positiva/negativa que afirma y niega sobre el CUERPO, así que heredan el trato
# del curl mudo: no responder es FALLO explícito, no un falso «el texto no está».
# Los tres mandan `Accept: text/html` a propósito —sin esa pista la cascada de
# formato no elige HTML y lo que se miraría sería el texto plano—, y ninguno de
# los cuatro ayudantes de arriba la manda.

# html_body <host> <path> <texto> <qué demuestra> — positiva: falla si el cuerpo
# NO contiene ese texto.
html_body() {
    if ! body=$(curl -sS -m 10 -H "Host: $1" -H 'Accept: text/html' "${base}$2"); then
        printf 'FALLO  %-22s %-6s no respondio el servicio  %s\n' "$1" "$2" "$4"
        failed=1
        return
    fi
    if printf '%s' "$body" | grep -q -- "$3"; then
        printf 'ok     %-22s %-6s el cuerpo lleva %s  %s\n' "$1" "$2" "$3" "$4"
    else
        printf 'FALLO  %-22s %-6s el cuerpo no lleva %s  %s\n' "$1" "$2" "$3" "$4"
        failed=1
    fi
}

# not_html_body <host> <path> <texto> <qué demuestra> — negativa: falla si el
# cuerpo SÍ lo contiene.
not_html_body() {
    if ! body=$(curl -sS -m 10 -H "Host: $1" -H 'Accept: text/html' "${base}$2"); then
        printf 'FALLO  %-22s %-6s no respondio el servicio  %s\n' "$1" "$2" "$4"
        failed=1
        return
    fi
    if printf '%s' "$body" | grep -q -- "$3"; then
        printf 'FALLO  %-22s %-6s el cuerpo lleva %s  %s\n' "$1" "$2" "$3" "$4"
        failed=1
    else
        printf 'ok     %-22s %-6s el cuerpo no lleva %s  %s\n' "$1" "$2" "$3" "$4"
    fi
}

# html_differs <host> <path> <cabecera> <qué demuestra> — pide el mismo path DOS
# veces, con y sin esa cabecera (siempre con `Accept: text/html`), y falla si
# algún cuerpo viene vacío o si los dos son IGUALES. Es lo que demuestra que el
# tema se elige por petición de extremo a extremo, a través de Traefik.
#
# Se comparan CUERPOS y no se busca un color en el CSS a propósito: un color es un
# detalle de la hoja, se puede retocar sin avisar y ataría la comprobación a cómo
# esté escrita hoy; «el cuerpo cambia con la cabecera» es el contrato, y es lo que
# se mide.
html_differs() {
    if ! plain=$(curl -sS -m 10 -H "Host: $1" -H 'Accept: text/html' "${base}$2"); then
        printf 'FALLO  %-22s %-6s no respondio el servicio  %s\n' "$1" "$2" "$4"
        failed=1
        return
    fi
    if ! themed=$(curl -sS -m 10 -H "Host: $1" -H 'Accept: text/html' -H "$3" "${base}$2"); then
        printf 'FALLO  %-22s %-6s no respondio el servicio  %s\n' "$1" "$2" "$4"
        failed=1
        return
    fi
    if [ -z "$plain" ] || [ -z "$themed" ]; then
        printf 'FALLO  %-22s %-6s algun cuerpo vino vacio  %s\n' "$1" "$2" "$4"
        failed=1
        return
    fi
    if [ "$plain" = "$themed" ]; then
        printf 'FALLO  %-22s %-6s el cuerpo no cambia con %s  %s\n' "$1" "$2" "$3" "$4"
        failed=1
    else
        printf 'ok     %-22s %-6s el cuerpo cambia con %s  %s\n' "$1" "$2" "$3" "$4"
    fi
}

echo "== estados (Traefik + errserve en ${base}) =="
code app.localtest.me / 404 \
    "el 404 del backend: estado de Traefik, cuerpo y cabeceras nuestros"
code app.localtest.me /boom 503 \
    "el 503 del backend: idem, con Retry-After"
code nada.localtest.me /x 404 \
    "catch-all SIN upstream: aqui el codigo lo decidimos nosotros"
code diverge.localtest.me /boom 503 \
    "backend 503 con middleware pidiendo /404: manda Traefik"

echo
echo "== cabeceras de la página servida (app.localtest.me/boom) =="
header app.localtest.me /boom Retry-After
header app.localtest.me /boom X-Robots-Tag
header app.localtest.me /boom Content-Type

echo
echo "== la página servida es autocontenida y el tema se elige por petición =="
html_body app.localtest.me /boom '<style' \
    "el 503 del backend lleva la hoja INCRUSTADA: la pagina no pide nada para verse"
not_html_body app.localtest.me /boom 'rel="stylesheet"' \
    "y NO enlaza /css/error.css en el host de la aplicacion, donde no lo sirve errserve: es el bug medido, el 404 con Content-Type text/plain que hacia que el navegador rechazara la hoja"
html_differs app.localtest.me /boom 'X-Theme: terminal' \
    "el tema se elige por peticion, extremo a extremo a traves de Traefik"

echo
echo "== el respaldo NO publica el endpoint de servicio =="
not_code nada.localtest.me /metrics 200 \
    "un Host sin router NO recibe el contador: es el perfil de fallos de la infraestructura"
not_type nada.localtest.me /metrics 'text/plain; version=0.0.4' \
    "ni la sirve el endpoint de metricas: la firma de su Content-Type (version=0.0.4) no esta en la respuesta"
not_header nada.localtest.me /metrics X-Robots-Tag \
    "guarda, y se cumple en los dos estados: /metrics es un endpoint de servicio, no una pagina, asi que no lleva X-Robots-Tag ni en la fuga; impide 'arreglarla' convirtiendolo en pagina"
not_body nada.localtest.me /metrics errors_pages_total \
    "ni el contador viaja en el cuerpo"

echo
[ "$failed" -eq 0 ] && echo "todo correcto" || echo "hay fallos"
exit "$failed"
