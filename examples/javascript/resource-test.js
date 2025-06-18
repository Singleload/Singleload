// Test resource limits
const used = process.memoryUsage();
console.log("Memory usage:");
for (let key in used) {
    console.log(`${key}: ${Math.round(used[key] / 1024 / 1024 * 100) / 100} MB`);
}

// Try to allocate large array (will fail if memory limit is too low)
try {
    const bigArray = new Array(10000000).fill("x");
    console.log("Successfully allocated large array");
} catch (e) {
    console.error("Failed to allocate memory:", e.message);
}