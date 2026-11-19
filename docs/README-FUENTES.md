# Stretch & Water

Recordatorios de estiramiento y agua para acompañar las horas frente a la computadora. Vive en la barra de menú de macOS, muestra animaciones sin quitar el foco y funciona localmente, sin navegador.

## Descargar e instalar

**Versión preliminar 0.5.1 · Mac con chip Apple Silicon (M1 o posterior).**

[Descargar la aplicación para Mac](https://github.com/melinalazaro/Strech-WaterApp/releases/download/v0.5.1/Stretch-and-Water-0.5.1-macOS-arm64.zip) · [Notas y archivos de la versión](https://github.com/melinalazaro/Strech-WaterApp/releases/tag/v0.5.1)

1. Descargá el ZIP del enlace anterior y descomprimilo. Se crea la carpeta **Stretch and Water**.
2. Mové la carpeta completa a un lugar propio, por ejemplo el Escritorio/Bureau. Conservá todos sus archivos, incluidos los ocultos.
3. Abrí **Stretch & Water.app**, dentro de esa carpeta. Buscá la figura y **SW** junto al reloj; si usás varias pantallas, revisá sus barras de menú.
4. Elegí **Tomar agua → Tomar agua ahora** para comprobar el primer aviso.

La carpeta completa es necesaria: la app guarda allí sus preferencias. El ZIP que GitHub llama **Source code** contiene las fuentes para desarrollar; el enlace de descarga anterior contiene la aplicación lista para abrir.

Esta entrega tiene firma local ad hoc y todavía no está notarizada por Apple. macOS puede bloquear su primera apertura. Si reconocés esta descarga y confiás en ella, consultá la opción por aplicación **Abrir igualmente** en Ajustes del Sistema → Privacidad y seguridad, según la [guía oficial de Apple](https://support.apple.com/es-es/102445). La instalación de una copia descargada en otra Mac sigue pendiente de validación.

El mínimo declarado es macOS 12.0; no se ha probado cada versión de macOS. Actualmente no se incluye una versión para Mac Intel, Windows o Linux. Más detalles en la [guía de instalación y actualización](docs/LEEME-INSTALACION.txt).

## Qué permite hacer

- Activar o desactivar por separado estiramientos y agua.
- Elegir **30 minutos, 60 minutos o 2 horas** para cada tipo.
- Mostrar un aviso manual, pausar temporalmente o pausar hasta reanudar.
- Activar o silenciar el sonido y guardar las preferencias.
- Colocar cada figura abajo a la derecha de la pantalla donde está el cursor al comenzar el aviso. La figura conserva su posición hasta terminar.

Cada aparición dura 12 segundos, con una primera pose de 1,5 segundos antes del movimiento. Hay cinco segundos de separación entre figuras.

## Estado de esta versión

La 0.5.1 pasó **53 pruebas automáticas del perfil normal y 54 del perfil PRUEBA**, además de pruebas de geometría con pantallas simuladas. El comportamiento con varias pantallas fue validado manualmente en PRUEBA. La revisión visual de los nuevos intervalos y su persistencia sigue pendiente.

Las animaciones, los íconos, el sonido y una sesión de cuatro horas se validaron en la versión anterior 0.5.0. El inicio automático después de un reinicio real continúa pendiente (prueba 8 de SPEC-003). Consultá el [registro de validación](docs/VALIDACION-0.5.1.md).

## Compilar desde las fuentes

Se necesita una Mac Apple Silicon, las herramientas de compilación de Apple, Node con npm, Rust con Cargo y Python 3. El entorno usado para esta entrega fue Node 24.15.0, npm 11.12.1 y Rust 1.99.1. Las dependencias están bloqueadas en `package-lock.json` y `src-tauri/Cargo.lock`. También se usan las utilidades de macOS `codesign`, `lipo`, `ditto` y `strip`.

Desde la raíz del repositorio, ejecutá en orden:

```sh
npm ci
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
sh scripts/build-release.sh
python3 -B scripts/package-release.py
```

Los primeros dos pasos pueden necesitar internet. Los siguientes construyen la app normal, firman una copia para entregar y comprueban el ZIP después de extraerlo. El resultado y su archivo SHA-256 se guardan en `entregas/v0.5.1/`. El archivo oculto `.stretch-water-project` debe acompañar a la app en la carpeta instalada.

Para comprobar el código sin iniciar la app:

```sh
npm run build
node --test tests/media.test.mjs
mkdir -p .development
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked --features test-profile
```

Tauri 2 y Rust administran los menús y recordatorios. El reproductor nativo de macOS usa las secuencias PNG incluidas en `src-tauri/assets/films/`. Los archivos React, TypeScript y SVG conservan el prototipo anterior. El perfil PRUEBA, destinado al desarrollo, tiene preferencias separadas e intervalos de 30, 60 y 120 segundos; el ZIP distribuido contiene el perfil normal.

## Código y recursos

La licencia para reutilizar o redistribuir el código y los recursos gráficos está pendiente de definir.
