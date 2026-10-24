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
    line=$(curl -sS -m 10 -D - -o /dev/null -H "Host: $1" "${base}$2" \
        | tr -d '\r' | grep -i "^$3:" || true)
    if [ -n "$line" ]; then
        printf 'ok     %s\n' "$line"
    else
        printf 'FALLO  falta la cabecera %s en %s%s\n' "$3" "$1" "$2"
        failed=1
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
[ "$failed" -eq 0 ] && echo "todo correcto" || echo "hay fallos"
exit "$failed"
