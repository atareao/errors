# Tasks: catchall-metrics-exclusion

> No hay lógica de negocio nueva, así que la fase RED no es una prueba de comportamiento del router:
> es **una prueba sobre los ficheros de despliegue** —que sí corre en el CI— y **un caso de
> `check.sh`**, que necesita un motor de contenedores. Los dos tienen que fallar contra el árbol de
> hoy. La GREEN es la conjunción que falta en la regla del respaldo.

## 0. Prerrequisito

- [x] 0.1 Dejar constancia de la medición que abre el change: con el respaldo sirviendo este
      servicio, `GET /metrics` en un Host sin router propio responde `200` con el contador.
- [x] 0.2 Confirmar el inventario de respaldos desplegables —`examples/traefik/compose.yml`,
      `compose.yml` y `examples/traefik/README.md`— y que no hay un cuarto.
      > **Desmentida por la realidad.** El inventario inicial era de tres ficheros y **no** había un
      > cuarto respaldo desplegable, pero la auditoría demostró que la guarda de lista cerrada no veía
      > un fichero nuevo con la fuga (un `dynamic.yaml` pasaba inadvertido), así que la guarda pasó de
      > lista cerrada a **descubrimiento en el árbol**. Ese descubrimiento encontró un cuarto fichero
      > que sí aporta una definición: el **bloque de labels del `README.md` raíz**, añadido por este
      > mismo change (tarea 4.1). Es un vector de copia-pega, no un sitio desplegable. Ver D4.

## 1. RED

- [x] 1.1 Escribir `tests/traefik_catchall.rs`: localiza cada router de respaldo **por su nombre**
      (`catchall`) en los tres ficheros y exige la exclusión de `/metrics` en su regla. Acumula los
      fallos nombrando fichero y línea. `cargo test traefik_catchall` **SHALL** fallar con los tres
      sin excluir.
      > **Ampliada por la auditoría:** la guarda terminó **descubriendo en el árbol** en vez de leer
      > los tres nombres, y el RED falló nombrando `compose.yml:144`, `examples/traefik/compose.yml:124`
      > y `examples/traefik/README.md:105`. Ver D3 y la nota de cierre.
- [x] 1.2 Añadir a `check.sh` el caso de `/metrics` en un Host sin router propio: el estado NO SHALL
      ser `200`, el cuerpo NO SHALL llevar `errors_pages_total` y la respuesta NO SHALL llevar
      `X-Robots-Tag`. Con el ejemplo de hoy **SHALL** salir en rojo.

## 2. GREEN

- [x] 2.1 `examples/traefik/compose.yml`: la regla del respaldo SHALL ser
      `HostRegexp(`.+`) && !Path(`/metrics`)`, con el comentario que diga por qué.
- [x] 2.2 `compose.yml`: la misma regla en los labels comentados de la plantilla de producción, y su
      comentario.
- [x] 2.3 `examples/traefik/README.md`: la misma regla en el fragmento de *provider de fichero*, y
      una nota en su sección del respaldo.
- [x] 2.4 `cargo test traefik_catchall` **SHALL** pasar.

## 3. Verificación

- [x] 3.1 `check.sh` completo en verde con el ejemplo levantado: los cuatro casos de siempre **más**
      el nuevo. La exclusión NO SHALL llevarse por delante el respaldo.
- [x] 3.2 Medir en el ejemplo levantado que `/metrics` no se sirve y que `/`, `/404` y `/503` en un
      Host desconocido siguen dando la página con el código real, con sus cabeceras.
- [x] 3.3 Las puertas: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
      y `cargo deny check -D warnings`.

## 4. Documentación

- [x] 4.1 `README.md` raíz: el párrafo «No publiques `/metrics` en el router de respaldo» SHALL llevar
      la regla concreta, que es lo que faltaba.
- [x] 4.2 `AGENTS.md`: el mismo dato en el párrafo de `/metrics`.

## 5. Cierre

- [x] 5.1 `openspec validate catchall-metrics-exclusion --strict` limpio.
- [x] 5.2 `openspec archive catchall-metrics-exclusion`, para fusionar la delta en
      `openspec/specs/http-server/`.
      > **Marcada al archivar.** Era la única casilla abierta al correr `openspec archive`
      > —«Task status: 14/15 tasks»—, porque es la acción misma: se marca aquí, en la copia ya
      > archivada como `2026-10-25-catchall-metrics-exclusion`, con la fusión hecha
      > (`http-server: update` · `+ 1 added`).

## Notas de cierre

1. **La fase RED se midió, pero el plan se quedó corto en dos cosas, y las dos las encontró la
   auditoría.** (a) La guarda de lista cerrada **no cubría** un fichero nuevo con la fuga —demostrado
   con un `dynamic.yaml`, que pasaba inadvertido y dejaba la prueba en verde con la fuga presente—, así
   que pasó a **descubrimiento en el árbol** (D3). (b) La guarda se podía **burlar con un comentario de
   cola** en YAML (``rule: HostRegexp(`.+`) # && !Path(`/metrics`)``), que deja la subcadena en la línea
   pero no en el valor —`yaml.safe_load` de esa línea devuelve ``HostRegexp(`.+`)``—. Las dos están
   reparadas y con prueba propia (`r1_…` y `r3_…` en `tests/traefik_catchall.rs`).
2. **Una de las tres aserciones nuevas de `check.sh` no discrimina** entre la fuga y el arreglo: la de
   `X-Robots-Tag`, porque `/metrics` es un endpoint de servicio y nunca lleva esa cabecera, así que se
   cumple en los dos estados. Se queda como **guarda** contra otro modo de regresión —«arreglar» la
   fuga convirtiendo `/metrics` en una página de error—, y el bloque ganó la aserción que sí discrimina
   (el `Content-Type` `text/plain; version=0.0.4`, que solo emite el endpoint de métricas), al lado del
   estado y del cuerpo.
