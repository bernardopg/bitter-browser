project('bitter-browser', 'rust',
  version: '0.1.0',
  meson_version: '>= 0.59.0',
)

i18n = import('i18n')
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
    'env',
    'CARGO_HOME=' + meson.project_build_root() / 'cargo-home',
    cargo, 'build',
    '--release',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    '--bin', 'bitter-ui',
  ]
)

# Install desktop file
install_data(
  'crates/bitter-ui/data/com.bitterbrowser.Browser.desktop',
  install_dir: get_option('datadir') / 'applications'
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
