# observability Specification (delta)

## MODIFIED Requirements

### Requirement: Las páginas servidas se cuentan por código

El servicio SHALL contar las páginas de error servidas, por código **y por el origen del que salió ese código** —el path, la cabecera `X-Code` o el defecto—, y exponerlas en `GET /metrics` en texto Prometheus. `GET /metrics` SHALL responder `200` como endpoint de servicio y NO SHALL pasar por la resolución de páginas de error.

El código contado SHALL ser el de la página **servida**: la serie que se incrementa y el estado con el que sale la respuesta NO SHALL diferir.

El contador NO SHALL crecer sin límite: solo cuenta códigos del rango utilizable.

El origen SHALL distinguirse porque la cabecera `X-Code` la elige quien llama: sin esa etiqueta, un tercero puede inflar a voluntad el contador de un código y fabricar la apariencia de una avalancha justo durante una incidencia.

#### Scenario: el contador distingue de dónde salió el código
- **GIVEN** un servicio recién arrancado
- **WHEN** se piden varias páginas por su path y varias más indicando el código con la cabecera
- **THEN** `/metrics` SHALL declarar las primeras con origen `path`
- **AND** las segundas con origen `header`

#### Scenario: el path gana y la etiqueta lo dice
- **GIVEN** un servicio recién arrancado
- **WHEN** se pide `/404` con la cabecera `X-Code: 503`
- **THEN** el estado SHALL ser `404`
- **AND** `/metrics` SHALL declararlo con origen `path` y NO con origen `header`

#### Scenario: el contador refleja lo servido
- **GIVEN** un servicio recién arrancado
- **WHEN** se piden N páginas de error con código `404` y M con código `503`
- **THEN** `/metrics` SHALL declarar N para el `404` y M para el `503`

#### Scenario: metrics es un endpoint de servicio
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /metrics`
- **THEN** SHALL responder `200`
- **AND** el cuerpo NO SHALL ser una página de error

#### Scenario: las métricas declaran su tipo
- **GIVEN** una respuesta de `/metrics`
- **WHEN** se inspecciona
- **THEN** SHALL incluir la ayuda y el tipo del contador conforme al formato Prometheus
