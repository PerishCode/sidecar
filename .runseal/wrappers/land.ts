import { cli, flags } from "@perish/harness/cli";
import { bin } from "@perish/harness/cmd";
import { io } from "@perish/harness/io";
import { doc } from "@perish/harness/json";

type Options = {
  base: string;
  body: string;
  dry: boolean;
  watch: boolean;
};

const budget = 240_000;
const early = 5_000;
const late = 10_000;
const fatal = ["failure", "cancelled", "timed_out", "action_required", "stale"];
const benign = ["success", "skipped", "neutral"];

const options = parse([...Deno.args]);
if (options.help) {
  usage();
  Deno.exit(0);
}

await bin("git").run(["--version"], { stdout: "null" });
await bin("gh").run(["--version"], { stdout: "null" });

const branch = await current();
if (options.dry) {
  await landable(options.base, branch, { fetch: false });
  plan(options, branch);
  Deno.exit(0);
}

await landable(options.base, branch, { fetch: true });
await bin("git").run(["push", "-u", "origin", branch]);
const sha = await bin("git").text(["rev-parse", "HEAD"]);

const url = await pull(options, branch);
io.print(url);
if (!options.watch) {
  io.print("land: PR is up; guard not awaited (--watch=false)");
  Deno.exit(0);
}
await guarded(sha);
await bin("gh").run([
  "pr",
  "merge",
  branch,
  "--squash",
  "--match-head-commit",
  sha,
  "--delete-branch",
]);
await bin("git").run(["checkout", options.base]);
await bin("git").run(["pull", "--ff-only", "origin", options.base]);
if (await ok(["rev-parse", "--verify", `refs/heads/${branch}`])) {
  await bin("git").run(["branch", "-D", branch]);
}

function usage(): void {
  io.print("Usage: runseal :land [options]");
  io.print("");
  io.print("Land the current clean topic branch on GitHub.");
  io.print("The branch is pushed, a PR is created or reused, the checks on the");
  io.print("exact pushed head commit are awaited, the PR is squash-merged against");
  io.print("that same commit, main is synced, and the topic branch is deleted.");
  io.print("");
  io.print("  --watch=false      stop once the PR exists; skip the guard wait and merge");
  io.print("");
  io.print("Options:");
  io.print("  --base <branch>    base branch (default: main)");
  io.print("  --body <body>      pull request body override");
  io.print("  --dry-run          print planned actions without changing git or GitHub");
}

function parse(args: string[]): Options & { help: boolean } {
  const parsed = cli.parse(args, {
    string: ["base", "body"],
    boolean: ["dry-run", "watch", "help", "h"],
    default: { watch: true },
  });
  flags(parsed).positionals("land", { allowHelp: true });
  return {
    base: flags(parsed).string("base", "main"),
    body: flags(parsed).string("body"),
    dry: flags(parsed).boolean("dry-run"),
    watch: parsed.watch === true,
    help: flags(parsed).help(),
  };
}

async function current(): Promise<string> {
  const branch = await bin("git").text(["branch", "--show-current"]);
  if (branch === "") {
    io.fail("land: detached HEAD is not a landable topic branch");
  }
  return branch;
}

async function landable(
  base: string,
  branch: string,
  options: { fetch: boolean },
): Promise<void> {
  if (branch === base || branch === "main" || branch === "master") {
    io.fail(`land: must run on a topic branch, not ${branch}`);
  }
  const dirty = await bin("git").text(["status", "--short"]);
  if (dirty.trim() !== "") {
    io.fail("land: working tree must be clean; commit or discard changes first");
  }
  if (options.fetch) {
    await bin("git").run(["fetch", "origin", base]);
  }
  const remote = `origin/${base}`;
  if (!await ok(["rev-parse", "--verify", remote])) {
    io.fail(`land: missing ${remote}; fetch or check the base branch name`);
  }
  if (!await ok(["merge-base", "--is-ancestor", remote, "HEAD"])) {
    io.fail(`land: current branch must contain latest ${remote}; rebase onto ${base} first`);
  }
  const ahead = Number(await bin("git").text(["rev-list", "--count", `${remote}..HEAD`]));
  if (!Number.isFinite(ahead) || ahead <= 0) {
    io.fail(`land: current branch has no commits ahead of ${remote}`);
  }
}

async function ok(args: string[]): Promise<boolean> {
  return await bin("git").status(args, {
    stdin: "null",
    stdout: "null",
    stderr: "null",
  }) === 0;
}

async function pull(options: Options, branch: string): Promise<string> {
  const existing = await bin("gh").text([
    "pr",
    "list",
    "--head",
    branch,
    "--base",
    options.base,
    "--state",
    "open",
    "--json",
    "url",
  ]);
  if (!doc(existing).empty()) {
    return doc(existing).get("[0].url");
  }
  return await bin("gh").text([
    "pr",
    "create",
    "--base",
    options.base,
    "--head",
    branch,
    "--title",
    await title(options.base),
    "--body",
    options.body,
  ]);
}

async function title(base: string): Promise<string> {
  const subjects = await bin("git").text([
    "log",
    "--reverse",
    "--format=%s",
    `origin/${base}..HEAD`,
  ]);
  const first = subjects.split(/\r?\n/).find((line) => line.trim() !== "");
  return first ?? "land branch";
}

async function guarded(sha: string): Promise<void> {
  const start = Date.now();
  let registered = false;
  while (Date.now() - start < budget) {
    const payload = await bin("gh").text([
      "api",
      `repos/{owner}/{repo}/commits/${sha}/check-runs`,
    ]);
    const runs = doc(payload).get(".check_runs");
    const total = doc(runs).len();
    if (total > 0) {
      registered = true;
      const broken = doc(doc(runs).filter("conclusion", fatal)).len();
      if (broken > 0) {
        io.fail(`land: checks failed on ${sha}`);
      }
      const finished = doc(doc(runs).filter("status", ["completed"])).len();
      if (finished === total) {
        const passed = doc(doc(runs).filter("conclusion", benign)).len();
        if (passed !== total) {
          io.fail(`land: checks finished with unexpected conclusions on ${sha}`);
        }
        io.print(`checks passed on ${sha}`);
        return;
      }
    }
    await delay(registered ? late : early);
  }
  io.fail(`land: timed out waiting for checks on ${sha}`);
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function plan(options: Options, branch: string): void {
  const creation = options.body === "" ? "--body ''" : "--body <given>";
  const steps = [
    "[dry-run] would run:",
    `  git fetch origin ${options.base}`,
    `  verify ${branch} is clean, not ${options.base}, contains origin/${options.base}, ahead >= 1`,
    `  git push -u origin ${branch}`,
    "  git rev-parse HEAD  # record exact head sha",
    `  gh pr list --head ${branch} --base ${options.base} --state open --json url`,
    `  gh pr create --base ${options.base} --head ${branch} --title <commit> ${creation}  # if missing`,
    "  gh api repos/{owner}/{repo}/commits/<sha>/check-runs  # poll until all succeed",
    `  gh pr merge ${branch} --squash --match-head-commit <sha> --delete-branch`,
    `  git checkout ${options.base}`,
    `  git pull --ff-only origin ${options.base}`,
    `  git branch -D ${branch}  # if still present locally`,
  ];
  io.print(steps.join("\n"));
}
