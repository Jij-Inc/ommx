const path = require('node:path');
const { glob } = require('glob');
const { loadPyodide } = require("pyodide");

async function test_ommx() {
    // Find the wheel file in dist directory
    const wheels = await glob('dist/ommx-*.whl', { cwd: __dirname });
    if (wheels.length === 0) {
        console.error("No wheel file found in dist directory");
        process.exit(1);
    }
    const wheelPath = path.resolve(__dirname, wheels[0]);
    console.log(`Loading wheel: ${wheels[0]}`);

    console.log("Loading pyodide...");
    let pyodide = await loadPyodide();

    // Load dependencies required by OMMX
    console.log("Loading dependencies...");
    await pyodide.loadPackage(['typing-extensions', 'numpy', 'pandas', 'protobuf']);

    console.log("Loading OMMX wheel...");
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
