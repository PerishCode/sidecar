const decoder = new TextDecoder();
const encoder = new TextEncoder();
export type Options = {
  cwd?: string;
  env?: Record<string, string>;
  stdin?: "inherit" | "null" | "piped";
  stdout?: "inherit" | "null" | "piped";
  stderr?: "inherit" | "null" | "piped";
};
const blocked = new Set([
  "DYLD_FALLBACK_LIBRARY_PATH",
  "DYLD_INSERT_LIBRARIES",
  "DYLD_LIBRARY_PATH",
  "LD_PRELOAD",
  "LD_LIBRARY_PATH",
]);
class Inherited {
  static present(): boolean {
    for (const key of blocked) {
      if (Deno.env.get(key) !== undefined) {
        return true;
      }
    }
    return false;
  }

  static sanitize(extra: Record<string, string> | undefined): Record<string, string> {
    const env = Deno.env.toObject();
    for (const key of blocked) {
      delete env[key];
    }
    return { ...env, ...(extra ?? {}) };
  }

  static options(
    extra: Record<string, string> | undefined,
  ): Pick<Deno.CommandOptions, "clearEnv" | "env"> {
    if (this.present()) {
      return { clearEnv: true, env: this.sanitize(extra) };
    }
    return extra === undefined ? {} : { env: extra };
  }
}
export async function exists(name: string): Promise<boolean> {
  try {
    await new Deno.Command(name, {
      args: ["--version"],
      ...Inherited.options(undefined),
      stdin: "null",
      stdout: "null",
      stderr: "null",
    }).output();
    return true;
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return false;
    }
    throw err;
  }
}

export class Bin {
  constructor(private readonly command: string) {}

  async run(args: string[] = [], options: Options = {}) {
    const code = await this.status(args, options);
    if (code !== 0) {
      Deno.exit(code);
    }
  }

  async status(args: string[] = [], options: Options = {}) {
    const status = await new Deno.Command(this.command, {
      args,
      cwd: options.cwd,
      ...Inherited.options(options.env),
      stdin: options.stdin ?? "inherit",
      stdout: options.stdout ?? "inherit",
      stderr: options.stderr ?? "inherit",
    }).spawn().status;
    return status.code;
  }

  async text(args: string[] = [], options: Omit<Options, "stdout"> = {}): Promise<string> {
    const output = await new Deno.Command(this.command, {
      args,
      cwd: options.cwd,
      ...Inherited.options(options.env),
      stdin: options.stdin ?? "null",
      stdout: "piped",
      stderr: options.stderr ?? "inherit",
    }).output();
    if (!output.success) {
      Deno.exit(output.code);
    }
    return decoder.decode(output.stdout).trimEnd();
  }

  async input(
    args: string[],
    input: string,
    options: Omit<Options, "stdin"> = {},
  ): Promise<string> {
    const child = new Deno.Command(this.command, {
      args,
      cwd: options.cwd,
      ...Inherited.options(options.env),
      stdin: "piped",
      stdout: options.stdout ?? "piped",
      stderr: options.stderr ?? "inherit",
    }).spawn();
    const writer = child.stdin.getWriter();
    await writer.write(encoder.encode(input));
    await writer.close();
    const output = await child.output();
    if (!output.success) {
      Deno.exit(output.code);
    }
    return decoder.decode(output.stdout).trimEnd();
  }
}

export function bin(command: string): Bin {
  return new Bin(command);
}
