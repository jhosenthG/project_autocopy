# AutoCopy

**Respaldo automático con versionado para Windows**

> _¿Por qué Rust? Porque me gusta._ 

---

## Descripción

AutoCopy es una herramienta de respaldo para Windows que te permite hacer copias de seguridad de tus archivos con versionado automático. Todo esto con una interfaz gráfica moderna o desde la línea de comandos.

### Características

- **Versionado automático**: Cada respaldo crea una carpeta con timestamp
- **Limpieza automática**: Mantiene solo las últimas N versiones
- **Interfaz gráfica (egui)**: Bonita, rápida y nativa de Windows
- **Modo CLI**: Para automatizaciones y scripts
- **Windows Task Scheduler**: Programa respaldos automáticos
- **Cancelación en tiempo real**: Detén el respaldo cuando quieras
- **Validación de espacio**: Comprueba antes de copiar
- **Filtrado inteligente**: Ignora archivos de sistema (Thumbs.db, desktop.ini)

---

## Construir desde código fuente

### Requisitos

- [Rust](https://rust-lang.org) (1.70+)
- Windows 10/11

### Compilación

```bash
# Clonar el repositorio
git clone https://github.com/tu-usuario/autocopy.git
cd autocopy

# Construir el ejecutable
cargo build --release

# El binario estará en:
# target\release\autocopy.exe
```

### Crear un ejecutable standalone (opcional)

```bash
# Instalar cargo-bundle si lo necesitas
cargo install cargo-bundle

# O simplemente copia el ejecutable de target\release
```

---

## Uso

### Modo Gráfico (GUI)

```bash
.\target\release\autocopy.exe
```

1. Selecciona la **carpeta origen** (lo que quieres respaldar)
2. Selecciona la **carpeta destino** (donde se guardarán los respaldos)
3. Ajusta el **número de versiones** a mantener
4. Clic en **"Iniciar Respaldo"**

### Modo Consola (CLI)

```bash
# Hacer respaldo usando la última configuración
.\target\release\autocopy.exe --backup
.\target\release\autocopy.exe -b
```

> ⚠️ **Nota**: La primera vez debes usar la GUI para configurar origen y destino.

---

## Programar respaldos automáticos

### Opción 1: Task Scheduler de Windows

1. Abre **Programador de tareas** (`taskschd.msc`)
2. Crea una tarea nueva
3. Agrega un disparador (diario, semanal, etc.)
4. En "Acciones", pon:
   ```
   C:\ruta\a\autocopy.exe --backup
   ```

### Opción 2: Desde la propia app

La GUI tiene integración con Windows Task Scheduler para programar respaldos automáticos.

---

## Estructura de respaldo

```
destino/
├── backup_2024-06-03_14-30-00/    ← Versión 1
│   ├── archivo1.txt
│   └── carpeta/
│       └── archivo2.txt
├── backup_2024-06-03_15-45-00/    ← Versión 2
│   ├── archivo1.txt
│   └── carpeta/
│       └── archivo2.txt
└── backup_2024-06-04_09-00-00/    ← Versión 3
    └── ...
```

---

## Testing

```bash
# Ejecutar tests
cargo test

# Con información detallada
cargo test -- --nocapture

# Tests de integración
cargo test --test integration_test
```

---

## Tecnologías

| Componente | Tecnología |
|------------|------------|
| Lenguaje | **Rust**  |
| GUI | **egui** (embedded) |
| Serialización | **serde** + **serde_json** |
| Fecha/Hora | **chrono** |
| Navegación | **walkdir** |
| Windows API | **windows-rs** |
| Errores | **anyhow** + **thiserror** |

---

## Licencia

MIT License - ver [LICENSE](LICENSE)

---

<div align="center">

**Hecho con 💚 y Rust** 🦀

</div>