const path = require('node:path');
const { loadPyodide } = require("pyodide");
const fs = require('fs');

async function test_ommx() {
    console.log("Loading pyodide...");
    let pyodide = await loadPyodide();

    // Find the wheel file in dist directory
    const distDir = path.join(__dirname, 'dist');
    const files = fs.readdirSync(distDir);
    const wheelFile = files.find(f => f.endsWith('.whl') && f.includes('emscripten'));

    if (!wheelFile) {
        console.error("No wheel file found in dist directory");
        process.exit(1);
    }

    console.log(`Loading wheel: ${wheelFile}`);

    // Load dependencies required by OMMX
    console.log("Loading dependencies...");
    await pyodide.loadPackage(['typing-extensions', 'numpy', 'pandas', 'protobuf']);

    const wheelPath = path.join(distDir, wheelFile);
    await pyodide.loadPackage(wheelPath);

    console.log("Testing OMMX...");
    return pyodide.runPythonAsync(`
import ommx.v1 as v1
import ommx._ommx_rust as rust

# Test basic functionality
print("✓ OMMX imported successfully!")
print("✓ Available modules:", len(dir(v1)), "items")
print("✓ Rust extension loaded:", hasattr(rust, 'DecisionVariable'))

# Test creating decision variables
dv1 = v1.DecisionVariable.integer(id=1, lower=0, upper=10)
print("✓ Created integer decision variable")

dv2 = v1.DecisionVariable.binary(id=2)
print("✓ Created binary decision variable")

dv3 = v1.DecisionVariable.continuous(id=3, lower=0.0, upper=1.0)
print("✓ Created continuous decision variable")

print("\\n🎉 OMMX on pyodide works! 🎉")
print("Rust SDK successfully compiled to WebAssembly and works in Node.js!")
    `);
}

test_ommx()
    .then(() => {
        console.log("\nTest completed successfully!");
        process.exit(0);
    })
    .catch(err => {
        console.error("Test failed:", err);
        process.exit(1);
    });
