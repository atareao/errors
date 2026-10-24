# response-extras Specification

## Purpose
Las dos prestaciones que la respuesta añade sobre el contenido de la página: la compresión gzip cuando el cliente la pide —con la forma correcta de servir un cuerpo comprimido, sin un `Content-Length` que describa lo que no se envía— y la copia de las cuatro cabeceras de correlación de la petición a la respuesta, sin inventar ninguna. Son las que permiten que el servicio atraviese un proxy y una caché sin romper la trazabilidad ni entregar a nadie un cuerpo que no pidió.

## Requirements

### Requirement: La respuesta se comprime cuando el cliente la pide

Cuando la petición traiga `Accept-Encoding: gzip`, la respuesta SHALL ir comprimida con `Content-Encoding: gzip`.
Sin esa cabecera, la respuesta NO SHALL ir comprimida.

Una respuesta comprimida NO SHALL anunciar un `Content-Length` que no corresponda a lo que se envía: omitirlo y responder con codificación por trozos es la forma correcta de servir un cuerpo comprimido, y es lo que hace la capa de compresión. Un `Content-Length` que describa el cuerpo **sin** comprimir sería el fallo que esta cláusula evita.

Toda respuesta que pueda comprimirse SHALL declarar `Vary: Accept-Encoding`, para que ninguna caché intermedia entregue a un cliente un cuerpo comprimido que no pidió.

#### Scenario: con gzip pedido, la respuesta va comprimida
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /404` con `Accept-Encoding: gzip`
- **THEN** la respuesta SHALL llevar `Content-Encoding: gzip`
- **AND** el estado SHALL seguir siendo `404`

#### Scenario: sin gzip pedido, la respuesta va en claro
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /404` sin `Accept-Encoding`
- **THEN** la respuesta NO SHALL llevar `Content-Encoding`
- **AND** el cuerpo SHALL ser el de siempre

#### Scenario: la respuesta declara que varía por codificación
- **GIVEN** una respuesta que puede comprimirse
- **WHEN** se inspeccionan sus cabeceras
- **THEN** SHALL llevar `Vary: Accept-Encoding`

### Requirement: Las cabeceras de correlación se copian

Las cabeceras `X-Request-Id`, `X-Trace-Id`, `X-Correlation-Id` y `X-Amzn-Trace-Id` presentes en la petición SHALL copiarse a la respuesta con el mismo valor, en la página de error y en los endpoints de servicio.

La aplicación NO SHALL inventar un valor cuando la cabecera no venía, ni copiar ninguna otra cabecera de la petición.

#### Scenario: el identificador de petición se propaga
- **GIVEN** una petición con `X-Request-Id: abc123`
- **WHEN** se responde la página de error
- **THEN** la respuesta SHALL llevar `X-Request-Id: abc123`

#### Scenario: las cuatro cabeceras se copian juntas
- **GIVEN** una petición con las cuatro cabeceras de correlación
- **WHEN** se responde
- **THEN** las cuatro SHALL aparecer en la respuesta con sus valores

#### Scenario: una cabecera ausente no se inventa
- **GIVEN** una petición sin `X-Trace-Id`
- **WHEN** se responde
- **THEN** la respuesta NO SHALL llevar `X-Trace-Id`

### Requirement: Las cabeceras de correlación se declaran en Vary

`Vary` SHALL incluir `Accept-Encoding` y los nombres de las cuatro cabeceras de correlación —`X-Request-Id`, `X-Trace-Id`, `X-Correlation-Id` y `X-Amzn-Trace-Id`—, con independencia de si la petición las traía: una lista que cambiase según la petición no serviría a una caché.

El motivo SHALL quedar escrito: la respuesta copia esas cabeceras de la petición, así que su contenido depende de ellas. Sin una caché delante el impacto es nulo —Traefik no cachea por defecto—, pero con un CDN por medio, el identificador de un usuario podría servirse en la respuesta a otro y romper la trazabilidad que esas cabeceras existen para dar.

#### Scenario: la respuesta declara que varía por correlación
- **GIVEN** una petición con `X-Request-Id`
- **WHEN** se responde
- **THEN** `Vary` SHALL incluir `X-Request-Id`
- **AND** SHALL seguir incluyendo `Accept-Encoding`

#### Scenario: sin cabeceras de correlación la lista es la misma
- **GIVEN** una petición sin ninguna cabecera de correlación
- **WHEN** se responde
- **THEN** `Vary` SHALL incluir igualmente los cuatro nombres
