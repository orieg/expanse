const path = require('path');
const fs = require('fs');

// Map the current runtime to the napi-style platform name used by the
// per-platform prebuilt packages (@orieg/expanse-<platform>), published as
// optionalDependencies of @orieg/expanse.
function platformName() {
  const { platform, arch } = process;
  if (platform === 'linux') {
    if (arch === 'x64') return 'linux-x64-gnu';
    if (arch === 'arm64') return 'linux-arm64-gnu';
  } else if (platform === 'darwin') {
    if (arch === 'x64') return 'darwin-x64';
    if (arch === 'arm64') return 'darwin-arm64';
  } else if (platform === 'win32') {
    if (arch === 'x64') return 'win32-x64-msvc';
  }
  return null;
}

function loadBinding() {
  const plat = platformName();

  // 1. A co-located prebuilt addon (e.g. bundled next to this loader).
  const localCandidates = [];
  if (plat) {
    localCandidates.push(path.join(__dirname, `expanse.${plat}.node`));
  }
  localCandidates.push(path.join(__dirname, 'index.node'));
  for (const candidate of localCandidates) {
    if (fs.existsSync(candidate)) {
      return require(candidate);
    }
  }

  // 2. The published per-platform package installed via optionalDependencies.
  if (plat) {
    try {
      return require(`@orieg/expanse-${plat}`);
    } catch (err) {
      // fall through to the local development fallbacks below
    }
  }

  // 3. Development / CI fallback: load straight from `cargo build` output.
  const devCandidates = [
    path.join(__dirname, '../../target/release/libexpanse_node.dylib'),
    path.join(__dirname, '../../target/release/libexpanse_node.so'),
    path.join(__dirname, '../../target/release/expanse_node.dll'),
    path.join(__dirname, '../../target/debug/libexpanse_node.dylib'),
    path.join(__dirname, '../../target/debug/libexpanse_node.so'),
    path.join(__dirname, '../../target/debug/expanse_node.dll'),
  ];
  for (const candidate of devCandidates) {
    if (fs.existsSync(candidate)) {
      try {
        const mod = { exports: {} };
        process.dlopen(mod, candidate);
        return mod.exports;
      } catch (err) {
        // try next candidate
      }
    }
  }

  throw new Error(
    'Failed to load the @orieg/expanse native addon. ' +
      'Ensure the matching @orieg/expanse-<platform> optionalDependency was installed, ' +
      'or build locally with `cargo build -p expanse-node`.'
  );
}

module.exports = loadBinding();
