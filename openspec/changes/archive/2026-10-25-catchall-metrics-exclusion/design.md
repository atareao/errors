# Design: catchall-metrics-exclusion

## El hecho que abre el change

En un host real, con el respaldo de Traefik sirviendo este servicio, `GET /metrics` en un Host sin
router propio responde `200` con el contador `errors_pages_total` — el perfil de fallos de la
infraestructura a la vista de cualquiera. La regla del respaldo que lo provocó es la que este
repositorio entrega, y está en tres sitios desplegables: los labels del ejemplo, los de la plantilla
de producción y el fragmento de *provider de fichero* del README del ejemplo. Con este mismo change
esa línea se escribe además en el bloque de labels del `README.md` raíz —copiada ya con la exclusión
puesta—, que no es un cuarto sitio desplegable sino la línea de la que se copia: cuatro definiciones
en el suelo de la guarda, tres desplegables y la del README.

## D1 — Dónde se cierra: en la regla, no en el servicio

Tres formas, evaluadas:

1. **`&& !Path(`/metrics`)` en la regla del respaldo — elegida.** Una conjunción, sin servicios
   nuevos, sin host nuevo, sin autenticación que mantener y sin DNS ni certificado que emitir. Está
   medida en producción: `/metrics` responde el `404` de Traefik (19 B, sin `X-Robots-Tag`) y el
   respaldo sigue sirviendo `404` y `503` reales en el mismo Host.
2. **Un router dedicado para `/metrics`**, con prioridad alta y `basicAuth` o `IPAllowList` delante.
   Mantiene el scrape, pero añade un host y un middleware que alguien tiene que mantener. **No
   compite** con la 1: son capas. Quien quiera seguir scrapeando añade su router por encima, y ese
   router se lleva la prioridad y el middleware de autenticación de su proxy.
3. **Que el binario no exponga `/metrics`.** Rompe el requisito de `observability` y el scrape
   legítimo. Descartada.

Se elige la 1 por ser la mínima que cierra la fuga, y se deja constancia de que la 2 sigue disponible
encima.

## D2 — La coincidencia es exacta, y eso es suficiente

`Path(`/metrics`)` es coincidencia **exacta**: `/metrics/` y `/METRICS` no son el endpoint —el router
del servicio tampoco los sirve— y caen en el respaldo, donde reciben la página del código resuelto,
**sin** datos del contador (medido: `404` de 72 B, sin `errors_pages_total`). No hay fuga por ahí.

`!PathPrefix(`/metrics`)` cerraría además esa rama, pero silenciaría un path que es corriente en una
aplicación del operador y no gana nada: lo que se filtraba era el contador, no la ruta. Se queda la
coincidencia exacta, que es exactamente la del endpoint.

## D3 — Cómo se guarda: prueba de Rust más `check.sh`

El CI **no** puede levantar `docker compose`, así que hay dos capas, y cada una cubre lo que la otra
no puede:

- **`check.sh`** es la comprobación de extremo a extremo —estado, `Content-Type` y cuerpo—, y necesita
  un motor de contenedores. Es la que un humano puede correr y la única que demuestra el hecho **sobre
  el servicio en marcha**: la guarda de Rust lee ficheros, no peticiones.
- **`tests/traefik_catchall.rs`** es la guarda que **sí** corre en cada propuesta de cambio: lee los
  ficheros versionados. Sin ella, la fuga solo se descubriría volviendo a desplegar.

De las cuatro aserciones del bloque de `/metrics`, la del `X-Robots-Tag` **no discrimina**: `/metrics`
es un endpoint de servicio y nunca lleva esa cabecera, así que se cumple en los dos estados —con la
fuga abierta y con el arreglo—. Se conserva a propósito, como **guarda** contra otro modo de
regresión —«arreglar» la fuga convirtiendo `/metrics` en una página de error—, y quien discrimina es
el estado, el `Content-Type` (la firma `text/plain; version=0.0.4` del endpoint) y el cuerpo.

La guarda identifica el respaldo **por el nombre del router** (`catchall`) y no por la forma de su
regla. Es lo que evita el falso positivo: el README del ejemplo ilustra la trampa v2/v3 con un router
llamado `errserve` —que **no** es un respaldo— y sus dos reglas, la mala y la buena, no llevan ni
deben llevar la exclusión.

Cuatro decisiones más, y las cuatro nacen de un modo de burlar la guarda:

- **Se descubre, no se lista.** La guarda recorre el árbol del repositorio y examina cada `*.yml`,
  `*.yaml` y `*.md` —saltando `.git/`, `target/`, `openspec/` y el fichero de trabajo `PLAN.md`—, en
  vez de leer una lista cerrada de tres ficheros. Medido: encuentra cuatro definiciones conformes
  —`compose.yml`, `examples/traefik/compose.yml`, `examples/traefik/README.md` y el bloque de labels
  del `README.md` raíz— y ninguna más. Con la lista cerrada, un cuarto fichero de despliegue con la
  regla **sin** la exclusión no se miraba: un `dynamic.yaml` nuevo con la fuga pasaba inadvertido y la
  prueba salía en verde con la fuga presente. Y el cuarto de hoy no es hipotético: el bloque del
  `README.md` raíz es la línea de labels que la gente copia, un vector de copia-pega real que una
  lista cerrada no ve.
- **La cobertura es un suelo, no un techo.** Los cuatro ficheros de arriba han de aportar **al menos**
  una definición cada uno, y un fichero descubierto que no esté en esa lista y sea conforme **no** es
  un fallo. Exigir «exactamente tres» castigaba una ampliación legítima —un respaldo correcto de más
  ponía la prueba en rojo—, mientras que un fichero con la regla y **sin** la exclusión falla aunque
  no esté en el suelo. El suelo conserva lo único que la guarda sí necesitaba de la lista cerrada: un
  renombrado del router no la deja huérfana y en verde.
- **Se exige sobre el *valor* de la regla, no sobre la línea.** Escribir la exclusión tras un ` #`
  —``… HostRegexp(`.+`) # && !Path(`/metrics`)``— deja la subcadena en el texto pero **no** en el
  valor: en YAML el ` #` abre un comentario, así que la regla que Traefik lee es ``HostRegexp(`.+`)``
  y `/metrics` queda publicado. Medido: `yaml.safe_load` de esa línea devuelve ``HostRegexp(`.+`)``,
  sin la exclusión, porque la búsqueda de subcadenas en la línea entera se la traga. Por eso la guarda
  corta el comentario de cola y quita las comillas que envuelvan el valor (el fragmento de *provider de
  fichero* del README va entrecomillado) antes de exigir. Es el hallazgo bloqueante de la auditoría.
- **Una regla con la sintaxis v2 no es un respaldo.** ``HostRegexp(`{host:.+}`)`` —placeholder
  `{nombre:regexp}` dentro del `HostRegexp`— **no casa con ninguna petición** en Traefik v3 (medido):
  no puede publicar `/metrics` ni nada, así que no es un vector de fuga y la guarda no la trata como
  respaldo. Es la excepción que evita exigirle la exclusión a la línea «MAL» del ejemplo didáctico si
  algún día se alinea al nombre del router del resto del repositorio.

## D4 — Alcance: los tres sitios, y por qué

Los tres definen un respaldo desplegable y los tres publicaban `/metrics`. El host que se filtró no
tenía el ejemplo: tenía la **plantilla de producción** copiada a mano. Un arreglo que se quedase en el
ejemplo habría dejado intacto precisamente el fichero que causó la incidencia.

Y una cuarta definición entra en el suelo de la guarda sin ser un cuarto sitio desplegable: el bloque
de labels del `README.md` raíz. No se despliega —es documentación—, sino que es **la misma línea de
labels escrita para que se copie ya con la exclusión puesta**; ahí el vector es el copia-pega, no el
despliegue, así que no se cuenta como sitio. El suelo de la guarda pasó de tres a cuatro por ese
bloque, y no por un respaldo nuevo.

## Open Questions

**Los demás endpoints de servicio.** Medidos también publicados por el mismo respaldo, en el mismo
Host desconocido: `GET /version` → `200 {"version":"0.4.0"}` y `GET /healthz` → `200`. Ninguno revela
el perfil de fallos —el contador sí lo revelaba— y `AGENTS.md` deja `/version` a la vista **a
propósito**, por paridad con `error-pages`. Excluirlos es una decisión de paridad, no de seguridad, y
este change no la toma: se decide aparte, con esta medición delante.
