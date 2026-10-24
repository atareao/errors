# response-extras Specification (delta)

## ADDED Requirements

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
