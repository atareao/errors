#!/usr/bin/env sh
# Comprobación de la integración, caso por caso. Uso:
#
#   ./check.sh              # puerto 8474, el que publica compose.yml
#   ./check.sh 9090         # otro puerto
#
# Requiere que el ejemplo esté levantado (compose.yml) y curl en el host. Sale
# con estado 1 si algún caso no cuadra, para que valga también en CI.
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
