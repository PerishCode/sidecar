import { cli, flags } from "@perish/harness/cli";
import { bin, exists } from "@perish/harness/cmd";
import { fs } from "@perish/harness/fs";
import { io } from "@perish/harness/io";
import { negentropy } from "@perish/harness/negentropy";
import { path } from "@perish/harness/path";

class Check {
  static async tool(name: string): Promise<void> {
    if (!(await exists(name))) {
      io.fail(`init: missing required tool: ${name}`);
    }
  }

  static async path(root: string, relative: string): Promise<void> {
    if (!(await fs.file.exists(path.join(root, relative)))) {
      io.fail(`init: missing required path: ${relative}`);
    }
  }
}

function usage(): void {
  io.print("Usage: runseal :init");
  io.print("");
  io.print("Validate the repository toolchain and entrypoints.");
}

const args = cli.parse(Deno.args, { boolean: ["help", "h"] });
flags(args).positionals("init", { allowHelp: true });
if (flags(args).help()) {
  usage();
  Deno.exit(0);
}

io.print("==> resolving repository");
const root = await bin("git").text(["rev-parse", "--show-toplevel"]);
io.print(`repository: ${root}`);

io.print("==> checking required tools");
for (
  const tool of [
    "git",
    "deno",
    "cargo",
    "gh",
    "runseal",
    "sh",
  ]
) {
  await Check.tool(tool);
}
await negentropy.verify();
io.print("ok: git, deno, cargo, gh, runseal, negentropy, sh");

io.print("==> checking repository entrypoints");
for (
  const entry of [
    "Cargo.toml",
    "Cargo.lock",
    "negentropy.toml",
    "vocabulary.toml",
    "manage.sh",
    "runseal.toml",
    ".runseal/deno.json",
    ".runseal/deno.lock",
    ".runseal/negentropy.version",
    ".runseal/wrappers/guard.ts",
    ".runseal/wrappers/init.ts",
    ".runseal/wrappers/land.ts",
    ".github/workflows/guard.yml",
  ]
) {
  await Check.path(root, entry);
}
io.print("ok: repository entrypoints");

await bin("deno").run(["--version"], { stdout: "null" });
io.print("development environment ready");
