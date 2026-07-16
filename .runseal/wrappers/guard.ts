import { cache } from "@perish/harness/cache";
import { cli, flags } from "@perish/harness/cli";
import { bin } from "@perish/harness/cmd";
import { io } from "@perish/harness/io";
import { negentropy } from "@perish/harness/negentropy";

function usage(): void {
  io.print("Usage: runseal :guard [--fresh]");
  io.print("");
  io.print("Run repository guard checks.");
  io.print("");
  io.print("  --fresh   ignore the guard cache and run the full gauntlet");
}

const args = cli.parse(Deno.args, { boolean: ["help", "h", "fresh"] });
flags(args).positionals("guard", { allowHelp: true });
if (flags(args).help()) {
  usage();
  Deno.exit(0);
}

const mark = await cache.key();
if (args.fresh !== true && (await cache.hit(mark))) {
  io.print(`guard: clean (cached ${mark.slice(0, 12)})`);
  Deno.exit(0);
}

io.print("==> cargo fmt");
await bin("cargo").run(["fmt", "--all", "--check"]);

io.print("==> cargo clippy");
await bin("cargo").run([
  "clippy",
  "--locked",
  "--workspace",
  "--all-targets",
  "--",
  "-D",
  "warnings",
]);

io.print("==> cargo test");
await bin("cargo").run(["test", "--locked", "--workspace"]);

io.print("==> deno fmt");
await bin("deno").run(["fmt", "--check", ".runseal"]);

io.print("==> deno check");
await bin("deno").run([
  "check",
  "--config",
  ".runseal/deno.json",
  "--lock",
  ".runseal/deno.lock",
  "--frozen=true",
  ".runseal/wrappers/guard.ts",
  ".runseal/wrappers/init.ts",
  ".runseal/wrappers/land.ts",
]);

io.print("==> negentropy");
await negentropy.verify();
await bin("negentropy").run(["--strict", "."]);

await cache.keep(mark);
