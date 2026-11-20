# Private Mewing Launcher — Rune port

Port 1:1 de la UI del launcher Flet (`../private-mewing-launcher`) a
[Rune](https://github.com/anomalyco/rune) (Rust), fijado al revision
`8fbc395190dbff430db9d9fc515bb6ac8cc7d13d`.

La referencia visual es la captura del original que muestra el evento CobblePass 5
con 162 mods, imagen de fondo submarina, selector de variante en el header,
miniaturas reales de mods con su nombre de archivo `.jar`, badges `INSTALLED`,
descripción `cp5`, usuario `bobby_fisher` y botón `Launch`.

## Datos y funcionalidad real

El puerto ya no usa datos estáticos: al arrancar consulta la misma API que el
original (`utils/launcher.py`):

- `GET /events/` — lista de eventos para el selector de variantes.
- `GET /events/<id>` (o `/currentEvent`) — evento activo: título, versión,
  descripción, imagen de fondo, mods, resource packs, etc.
- Descarga y decodifica la imagen del hero y las miniaturas de los mods
  (`cdn.modrinth.com`) con `ImageCache` de Rune.
- Lee y escribe `%APPDATA%\.mewing-launcher\mewing-launcher-settings\settings.json`
  con los mismos campos (`username`, `maxRam`, `minRam`, `hideLauncherOnStart`,
  `selectedEventId`).
- Selector de variante funcional: al elegir otra variante se descarga su evento,
  se actualiza la imagen y se persiste la selección.
- Búsqueda en vivo sobre los 162 mods / resource packs.
- Navegación Home/Settings, pestañas, modales (logs, downloads, variantes),
  RAM, switch "hide on start", estado Launch/Stop y minimizar la ventana.

Si la API no responde se usa un snapshot de respaldo y la imagen incluida en
`assets/`, así que la UI nunca queda vacía.

## Ejecutar

```powershell
cd private-mewing-launcher-rune
$env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH
cargo run
```

## Instalador y releases

`scripts/build_installer.ps1` compila en modo release, copia el binario a
`dist\PrivateMewingLauncher.exe` y genera el instalador de Windows con Inno
Setup 6 (`installer\mewing.iss`):

```powershell
./scripts/build_installer.ps1 -Version 2.0.0
```

El workflow `.github/workflows/windows-release.yml` corre en cada tag `v*`
(por ejemplo `v2.0.0`): compila en Windows, genera el instalador, calcula su
SHA-256 y publica ambos archivos como release de GitHub. También puede
ejecutarse manualmente para obtener el artefacto sin publicar release.

## Actualizaciones automáticas

Al arrancar, el launcher consulta
`GET https://private-mewing-launcher-api.vercel.app/launcher/latest`. La API
resuelve el último release de GitHub y responde `version`, `url`, `sha256` y
`notes`. Si la versión es mayor que la compilada, el header muestra
«Actualizar vX»: el launcher descarga el instalador a `%TEMP%`, verifica su
SHA-256, lo ejecuta en modo silencioso y se cierra para dejar reemplazar el
ejecutable.

## Verificación headless

```powershell
cargo test
cargo run -- --snap snap.png --scene home --size 1536x864 --scale 1.25
cargo run -- --snap settings.png --scene settings --size 1536x864 --scale 1.25
cargo run -- --tree tree.json --scene home --size 1536x864 --scale 1.25
```

Escenas: `home`, `settings`, `logs`, `downloads`, `resources`.

## Estructura

| Flet | Rune |
| --- | --- |
| `main.py` | `src/main.rs` (AppSpec, escenas, setup con API) |
| `utils/launcher.py`, `minecraft_event/variants.py` | `src/data.rs` (API y settings.json) |
| `utils/colors.py`, `DESIGN.md` | `src/theme.rs` |
| `layouts/layout.py`, `sections/left_bar.py`, `footer_bar.py` | `src/widgets.rs` |
| `views/main_view.py`, `sections/mods_and_resources.py`, `mods_list.py` | `src/home.rs` |
| `views/settings_view.py`, `ram_selector.py`, `hide_launcher_setting.py` | `src/settings.rs` |
| `ft.Icons` | `src/icons.rs` (SVG rasterizados por Rune) |

## Alcance

El launcher funciona sin Python: sincroniza mods, resource packs, shader packs
y configuraciones por instancia, instala Fabric/Forge y el runtime Java,
lanza Minecraft con la RAM y usuario configurados y controla su proceso.
La instalación se completa antes del lanzamiento: vuelve a pulsar Launch para jugar.

Las descargas y llamadas a la API se ejecutan en segundo plano. Los logs muestran
salida real, con copia, borrado y auto-scroll. Se confirma la retirada de archivos
locales y la migración de instalaciones antiguas; los configs editados se conservan.
Si la API falla, el respaldo es solo una vista previa: no se sincronizan archivos.

Discord RPC está implementado en Windows y es opcional; en otros sistemas no se
publica presencia. No existe un actualizador del launcher en el original.

Para generar el portable y el instalador Windows:

```powershell
.\scripts\build_installer.ps1
```

El script compila en release y utiliza Inno Setup si ya está instalado; no instala
herramientas. Usa `-SkipInstaller` para generar solo el portable en `dist/`.
El instalador conserva el AppId del original.
