import { bin, exists } from "@/lib/std/cmd.ts";
import { fs } from "@/lib/std/fs.ts";
import { io } from "@/lib/std/io.ts";

const file = ".runseal/negentropy.version";

async function version(): Promise<string> {
  const value = (await fs.file.readTextIfExists(file)).trim();
  if (value === "") {
    io.fail(`negentropy: missing pinned version in ${file}`);
  }
  return value;
}

async function verify(): Promise<void> {
  if (!(await exists("negentropy"))) {
    io.fail("missing required tool: negentropy");
  }
  const expected = await version();
  const actual = await bin("negentropy").text(["--version"]);
  if (actual !== `negentropy ${expected}`) {
    io.fail(`negentropy: expected ${expected}, got ${actual}`);
  }
}

export const negentropy = { verify, version };
