# response-extras Specification (delta)

## MODIFIED Requirements

### Requirement: Las cabeceras de correlación se declaran en Vary

`Vary` SHALL incluir `Accept-Encoding`, los nombres de las cuatro cabeceras de correlación —`X-Request-Id`,
`X-Trace-Id`, `X-Correlation-Id` y `X-Amzn-Trace-Id`— y `X-Theme`, con independencia de si la petición las
traía: una lista que cambiase según la petición no serviría a una caché.

El motivo SHALL quedar escrito, y es el mismo para las dos familias: la respuesta **depende** de esas
cabeceras. Las cuatro de correlación se copian de la petición a la respuesta, así que sin declararlas una
caché delante podría servirle a un usuario el identificador de otro y romper la trazabilidad que esas
cabeceras existen para dar. `X-Theme` cambia **el cuerpo** —la hoja que va incrustada en el HTML—, así que
una caché que no la declarase entregaría a un cliente el tema que pidió otro, o el cuerpo entero de otro.
Sin una caché por medio el impacto es nulo —Traefik no cachea por defecto—, pero con un CDN delante sí.

#### Scenario: la respuesta declara que varía por correlación
- **GIVEN** una petición con `X-Request-Id`
- **WHEN** se responde
- **THEN** `Vary` SHALL incluir `X-Request-Id`
- **AND** SHALL seguir incluyendo `Accept-Encoding`

#### Scenario: sin cabeceras de correlación la lista es la misma
- **GIVEN** una petición sin ninguna cabecera de correlación
- **WHEN** se responde
- **THEN** `Vary` SHALL incluir igualmente los cuatro nombres

#### Scenario: la respuesta declara que varía por el tema
- **GIVEN** una petición con `X-Theme`
- **WHEN** se responde
- **THEN** `Vary` SHALL incluir `X-Theme`

#### Scenario: sin X-Theme la lista también lo declara
- **GIVEN** una petición sin `X-Theme`
- **WHEN** se responde
- **THEN** `Vary` SHALL incluir igualmente `X-Theme`
