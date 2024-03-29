#!/usr/bin/env -S deno run --allow-all

function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms))
}

console.log(`child.ts: Running. My pid: ${Deno.pid}`);

// Wait a bit, then kill the parent.
await sleep(1000);

console.log(`child.ts: Killing parent: ${Deno.ppid}`);
Deno.kill(Deno.ppid, "SIGKILL");

// Wait a bit.
await sleep(2000);


console.error(`TEST FAILED. ORPHAN STILL ALIVE. Parent: ${Deno.ppid}`);
