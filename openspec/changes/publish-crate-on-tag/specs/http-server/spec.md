# http-server Specification (delta)

## ADDED Requirements

### Requirement: Un tag publica el crate además de la imagen

Empujar una etiqueta `v*` SHALL publicar la imagen en GHCR **y** el crate en crates.io. La versión de
`Cargo.toml` SHALL coincidir con la de la etiqueta: si no coinciden, la publicación SHALL fallar
nombrando el desajuste en vez de publicar una versión que no es la que se anuncia. Publicar en
crates.io es **irreversible**, así que el crate NO SHALL publicarse antes que la imagen: SHALL depender
de que la imagen se haya construido, probado y publicado. Publicar NO SHALL romperse al re-ejecutarse
sobre una etiqueta cuya versión ya está en el registro: SHALL detectarlo y continuar. Y el paquete
SHALL verificarse como publicable en cada propuesta de cambio, no por primera vez al publicar.

#### Scenario: una etiqueta publica las dos cosas
- **GIVEN** una etiqueta `v*` cuya versión coincide con la de `Cargo.toml`
- **WHEN** se empuja la etiqueta
- **THEN** la imagen SHALL quedar publicada en GHCR
- **AND** el crate SHALL quedar publicado en crates.io

#### Scenario: la versión y la etiqueta no coinciden
- **GIVEN** `Cargo.toml` en `0.4.0` y una etiqueta `v0.4.1`
- **WHEN** se ejecuta la publicación
- **THEN** SHALL fallar nombrando el desajuste
- **AND** NO SHALL publicarse nada irreversible

#### Scenario: el crate no se publica si la imagen falla
- **GIVEN** un fallo al construir o al probar la imagen
- **WHEN** el workflow termina
- **THEN** el crate NO SHALL quedar publicado

#### Scenario: republicar una etiqueta ya publicada no falla
- **GIVEN** una etiqueta cuya versión ya está en crates.io
- **WHEN** se republica con el disparador manual
- **THEN** el paso SHALL detectarlo y continuar sin error

#### Scenario: el paquete se verifica en cada propuesta de cambio
- **GIVEN** una propuesta de cambio
- **WHEN** corre el CI
- **THEN** el paquete SHALL verificarse como publicable **sin subirlo**
