async function delay(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function main() {
    console.log("Starting async operation...");
    await delay(1000);
    console.log("Operation completed after 1 second");
    
    // Demonstrate JSON output
    const result = {
        timestamp: new Date().toISOString(),
        random: Math.random(),
        environment: process.env.USER || "unknown"
    };
    
    console.log("Result:", JSON.stringify(result, null, 2));
}

main().catch(console.error);