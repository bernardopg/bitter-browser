project('bitter-browser', 'rust',
  version: '0.6.0',
  meson_version: '>= 0.59.0',
)

gnome = import('gnome')

cargo = find_program('cargo', required: true)

# Cargo build
custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: ['bitter-ui', 'bitter-cli'],
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    'sh', '-c',
    'CARGO_TARGET_DIR="$1" "$2" build --locked --release --manifest-path "$3" --bin bitter-ui --bin bitter-cli && cp "$1/release/bitter-ui" "$4" && cp "$1/release/bitter-cli" "$5"',
    'sh', meson.project_build_root() / 'cargo-target', cargo,
    meson.project_source_root() / 'Cargo.toml', '@OUTPUT0@', '@OUTPUT1@',
  ]
)

# Install desktop file
install_data(
  'crates/bitter-ui/data/br.com.bebitterbebetter.browser.Bitter.desktop',
  install_dir: get_option('datadir') / 'applications'
)

# Install application icons (scalable, symbolic and PNG sizes)
install_subdir(
  'crates/bitter-ui/data/icons/hicolor',
  install_dir: get_option('datadir') / 'icons',
)

# Register as a web browser helper for Xfce's preferred applications
install_data(
  'crates/bitter-ui/data/xfce4/helpers/bitter-browser.desktop',
  install_dir: get_option('datadir') / 'xfce4' / 'helpers',
)

# License
install_data(
  'LICENSE',
  install_dir: get_option('datadir') / 'licenses' / meson.project_name(),
)

# Install appdata
install_data(
  'crates/bitter-ui/data/br.com.bebitterbebetter.browser.Bitter.metainfo.xml',
  install_dir: get_option('datadir') / 'metainfo'
)

# Install gschema
install_data(
  'crates/bitter-ui/data/br.com.bebitterbebetter.browser.Bitter.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas'
)

# Compile schemas
gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
