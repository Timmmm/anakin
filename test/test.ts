#!/usr/bin/env -S deno run --allow-all

// E.g. run `ANAKIN_LOG=info ./test.ts ../target/debug/anakin`

console.log("test.ts: Running: " + Deno.args);

if (Deno.args[0] === undefined) {
    console.error("Pass the path to the anakin binary");
}

const command = new Deno.Command(Deno.args[0], {
    args: [
        "./parent.ts",
    ],
});

await command.spawn().status;

console.log("test.ts: Done");
