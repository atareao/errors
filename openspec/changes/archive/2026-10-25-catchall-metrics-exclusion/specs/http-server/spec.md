# http-server Specification (delta)

## ADDED Requirements

### Requirement: El respaldo no publica el endpoint de servicio `/metrics`

Toda definición de router de respaldo que este repositorio entregue —los labels del ejemplo, los
labels de la plantilla de producción, el fragmento de *provider de fichero* y el bloque de labels del
`README.md` raíz— SHALL excluir
`/metrics` de su regla: `HostRegexp(`.+`) && !Path(`/metrics`)`. Una exclusión que sólo aparezca tras
un `#` de cola NO SHALL contar: en YAML ese `#` abre un comentario, así que el **valor** de la regla
se queda sin ella y el respaldo vuelve a publicar `/metrics`. Un Host que no case con ningún otro
router NO SHALL recibir el contador, ni en el estado ni en el cuerpo, porque `/metrics` publica el
perfil de fallos de la infraestructura y la capacidad `observability` ya exige que sea un endpoint de
servicio y no una página de error.

La exclusión NO SHALL llevarse por delante el respaldo: en un Host desconocido, cualquier otro path
—`/` incluido— SHALL seguir recibiendo la página de error con el código real y sus cabeceras.

Una regla con la sintaxis de Traefik **v2** —un placeholder `{nombre:regexp}` dentro del `HostRegexp`,
como ``HostRegexp(`{host:.+}`)``— NO SHALL contar como definición de router de respaldo: no casa con
ninguna petición, así que no publica `/metrics` ni nada, y exigirle la exclusión sería exigírsela a un
router que no lo es.

Una prueba automática SHALL guardar la propiedad sobre los ficheros versionados, porque la
comprobación de extremo a extremo necesita un motor de contenedores y NO corre en el CI.

#### Scenario: el respaldo no sirve métricas
- **GIVEN** el ejemplo levantado, con Traefik delante y un Host que no case con ningún otro router
- **WHEN** se pide `GET /metrics` a ese Host
- **THEN** el estado SHALL NO ser `200`
- **AND** el cuerpo NO SHALL contener `errors_pages_total`
- **AND** la respuesta NO SHALL llevar `X-Robots-Tag`, que es la firma de una página servida por el
  servicio; se cumple también con la fuga abierta —`/metrics` es un endpoint de servicio y nunca lleva
  esa cabecera—, así que es una guarda contra otro modo de regresión, convertir `/metrics` en una
  página de error, y quien discrimina el arreglo es el estado, el `Content-Type` y el cuerpo
- **AND** la respuesta NO SHALL declarar el `Content-Type` del endpoint de métricas (`text/plain; version=0.0.4`)

#### Scenario: la exclusión no rompe el respaldo
- **GIVEN** el ejemplo levantado
- **WHEN** se pide `GET /` y `GET /503` a un Host que no case con ningún otro router
- **THEN** los estados SHALL ser `404` y `503`
- **AND** el cuerpo SHALL ser la página de ese código
- **AND** el `404` SHALL llevar `X-Robots-Tag` y el `503`, además, `Retry-After`

#### Scenario: ninguna definición de respaldo vuelve a publicar métricas
- **GIVEN** el repositorio versionado, con los tres ficheros que entregan un respaldo desplegable y el bloque de labels del `README.md` raíz
- **WHEN** se inspecciona la regla de cada router de respaldo que el repositorio define, en cualquier fichero versionado
- **THEN** cada una SHALL llevar la exclusión de `/metrics`
- **AND** los tres desplegables y el `README.md` raíz SHALL aportar al menos una definición cada uno, para que un renombrado del router no deje la prueba huérfana y en verde
- **AND** la prueba SHALL fallar nombrando el fichero que la haya perdido

#### Scenario: la exclusión ha de estar en la regla, no en un comentario
- **GIVEN** un fichero de despliegue cuya regla de respaldo lleva la exclusión sólo **después** de un `#` de cola, que en YAML abre un comentario
- **WHEN** se inspecciona el **valor** de la regla
- **THEN** esa definición SHALL considerarse incumplidora
- **AND** la prueba SHALL fallar nombrándola
