const assert = require('assert');
const { WasmExpanseSet } = require('../pkg/expanse_wasm.js');

function run() {
    const set = new WasmExpanseSet();
    set.add(42n);
    assert.ok(set.contains(42n));
    assert.strictEqual(set.size(), 1n);
    set.remove(42n);
    assert.ok(!set.contains(42n));
    console.log("Node integration tests passed");
}

run();
