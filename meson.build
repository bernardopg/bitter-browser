project('bitter-browser', 'rust',
  version: '0.1.1',
  meson_version: '>= 0.59.0',
)

gnome = import('gnome')

cargo = find_program('cargo', required: true)

# Cargo build
custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: ['bitter-ui'],
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    'sh', '-c',
    'CARGO_TARGET_DIR="$1" "$2" build --locked --release --manifest-path "$3" --bin bitter-ui && cp "$1/release/bitter-ui" "$4"',
    'sh', meson.project_build_root() / 'cargo-target', cargo,
    meson.project_source_root() / 'Cargo.toml', '@OUTPUT@',
  ]
)

# Install desktop file
install_data(
  'crates/bitter-ui/data/com.bitterbrowser.Browser.desktop',
  install_dir: get_option('datadir') / 'applications'
)

# Install application icon
install_data(
  'crates/bitter-ui/data/icons/com.bitterbrowser.Browser.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'apps'
)

# Install appdata
install_data(
  'crates/bitter-ui/data/com.bitterbrowser.Browser.metainfo.xml',
  install_dir: get_option('datadir') / 'metainfo'
)

# Install gschema
install_data(
  'crates/bitter-ui/data/com.bitterbrowser.Browser.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas'
)

# Compile schemas
gnome.post_install(
  glib_compile_schemas: true,
  update_desktop_database: true,
)
