const path = require('path');
const fs = require('fs');

function loadBinding() {
  const candidates = [
    path.join(__dirname, 'index.node'),
    path.join(__dirname, 'expanse-node.node'),
    path.join(__dirname, '../../target/release/libexpanse_node.dylib'),
    path.join(__dirname, '../../target/release/libexpanse_node.so'),
    path.join(__dirname, '../../target/release/expanse_node.dll'),
    path.join(__dirname, '../../target/debug/libexpanse_node.dylib'),
    path.join(__dirname, '../../target/debug/libexpanse_node.so'),
    path.join(__dirname, '../../target/debug/expanse_node.dll'),
  ];

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      try {
        if (candidate.endsWith('.node')) {
          return require(candidate);
        }
        const mod = { exports: {} };
        process.dlopen(mod, candidate);
        return mod.exports;
      } catch (err) {
        // try next candidate
      }
    }
  }

  throw new Error(
    'Failed to load @orieg/expanse native addon. Please build with `cargo build -p expanse-node`.'
  );
}

module.exports = loadBinding();
