#!/usr/bin/env -S deno run --allow-all

function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms))
}

console.log("parent.ts: Running: " + Deno.args);

const command = new Deno.Command("./child.ts");

const child = command.spawn();

await sleep(2000);

console.log("parent.ts: Still running");

await child.status;

console.log("TEST FAILED. PARENT NOT KILLED.");
