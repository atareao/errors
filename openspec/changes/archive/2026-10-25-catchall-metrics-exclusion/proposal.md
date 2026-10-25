# Proposal: catchall-metrics-exclusion

## Why

Medido en un host de producción real. Ese host llevaba el **servicio de errores de este proyecto**
delante de su Traefik, con el router de respaldo que este repositorio documenta:

    traefik.http.routers.errserve-catchall.rule: HostRegexp(`.+`)

En cuanto la sustitución quedó hecha —el respaldo pasó de `error-pages` a este servicio—, `/metrics`
quedó **alcanzable desde cualquier Host sin router propio**:

    GET https://x-desconocido.example/metrics  ->  200  text/plain; version=0.0.4  246 B
    # HELP errors_pages_total Páginas de error servidas, por código.
    errors_pages_total{code="404",source="path"} 47
    errors_pages_total{code="404",source="default"} 4

No fue un descuido de esa instalación: el host había desplegado **material de este repositorio**, y
el repositorio define el respaldo sin excluir `/metrics` en **tres** sitios desplegables —los labels
del ejemplo (`examples/traefik/compose.yml`), los de la plantilla de producción (`compose.yml`) y el
fragmento de *provider de fichero* del README del ejemplo—. Y el host que se filtró **no** tenía el
ejemplo: tenía **la plantilla de producción** copiada a mano.

Antes no se veía porque el respaldo lo servía `error-pages`, que responde su propio `404` en
`/metrics`: el `404` no era una defensa, era la coincidencia de que el respaldo **no** era este
servicio. Y el `README.md` raíz **ya avisa** —«No publiques `/metrics` en el router de respaldo»—
tres párrafos antes de una plantilla que lo publica: quien copia los labels que el repositorio
entrega se lleva la fuga, y la documentación le está diciendo que no lo haga.

## What Changes

1. **La regla del respaldo excluye `/metrics`**, en los tres sitios donde el repositorio define un
   router de respaldo: `HostRegexp(`.+`) && !Path(`/metrics`)`.
2. **La exclusión se comprueba de extremo a extremo.** `check.sh` gana el caso que hoy no tiene:
   `GET /metrics` en un Host sin router propio NO SHALL servir el contador, y la respuesta NO SHALL
   llevar la firma de una página del servicio (`X-Robots-Tag`). Necesita un motor de contenedores, así
   que **no** corre en el CI.
3. **Una prueba de Rust guarda la propiedad**, porque el CI no puede levantar `docker compose`: lee
   los ficheros de despliegue versionados, localiza cada router de respaldo **por su nombre**
   (`catchall`, en su forma de label o de *provider de fichero*) y falla si alguno vuelve a definirse
   sin la exclusión. Sin dependencias nuevas, en el estilo de `tests/content_pages.rs`.
4. **La prosa que faltaba se escribe.** El párrafo de `/metrics` del `README.md` raíz y el quirk de
   `AGENTS.md` dan por hecho que el operador ya sabe *cómo* se deja fuera del respaldo; se les añade
   la regla concreta que se midió funcionando.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `http-server`: **un requisito nuevo** —el respaldo no publica el endpoint de servicio `/metrics`—
  con sus escenarios. Es el envés de un requisito que ya existe («Los endpoints de servicio siempre
  responden 200»): si `observability` exige que `/metrics` sea un endpoint de servicio y no una
  página de error, la plantilla que entrega el respaldo no puede ofrecérselo al primero que pregunte.

## Non-goals

- **No se toca `src/**` ni el comportamiento del binario.** El servicio hace lo correcto: publica sus
  endpoints de servicio porque es lo que se le pide. Quien no debe publicarlos es la **regla** del
  respaldo. Ni una línea de Rust cambia.
- **No se añade autenticación.** El sitio para eso sigue siendo el proxy (`basicAuth`, `IPAllowList`),
  como ya dice el README: aquí solo se cierra la vía por la que el endpoint llegaba a internet.
- **No se rediseña el respaldo.** No se cambia su prioridad ni su `HostRegexp`: la regla es la misma
  con una conjunción más.
- **No se decide sobre los demás endpoints de servicio.** `/version` se queda a la vista a propósito
  (paridad con `error-pages`) y `/healthz` no lo toca este change: los dos quedan como pregunta
  abierta en el `design.md`, con su medición delante.
- **No se corrige el resto de la documentación del ejemplo** —el recuento de casos del README del
  ejemplo y que `check.sh` solo compare estados y nunca el cuerpo—: es deuda declarada y ajena a esta
  fuga.

## Impact

- `examples/traefik/compose.yml` — la regla del respaldo y su comentario.
- `compose.yml` — la misma regla en los labels comentados de la plantilla de producción.
- `examples/traefik/README.md` — el fragmento de *provider de fichero* y una nota.
- `examples/traefik/check.sh` — un caso y una aserción negativa.
- `tests/traefik_catchall.rs` — **nuevo**; lee los ficheros de despliegue, sin dependencias nuevas.
- `README.md` — el párrafo de `/metrics`, con la regla concreta.
- `AGENTS.md` — el quirk de `/metrics`, con la regla concreta.
- `openspec/specs/http-server/spec.md` — vía la delta de este change.
- `src/**`, `content/**`, `Dockerfile`, `Cargo.toml`, `Cargo.lock` — **sin cambios**.
