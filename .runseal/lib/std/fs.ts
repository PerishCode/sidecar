import { path as stdPath } from "@/lib/std/path.ts";

class File {
  static async exists(path: string): Promise<boolean> {
    try {
      const stat = await Deno.stat(path);
      return stat.isFile;
    } catch (err) {
      if (err instanceof Deno.errors.NotFound) {
        return false;
      }
      throw err;
    }
  }

  static async chmod(path: string, mode?: string): Promise<void> {
    if (mode === undefined || Deno.build.os === "windows") {
      return;
    }
    const parsed = Number.parseInt(mode.replace(/^0o/, ""), 8);
    if (!Number.isInteger(parsed) || parsed < 0) {
      throw new Error(`invalid file mode: ${mode}`);
    }
    await Deno.chmod(path, parsed);
  }

  static async write(path: string, text: string, mode?: string): Promise<void> {
    const parent = stdPath.dirname(path);
    if (parent !== "") {
      await Deno.mkdir(parent, { recursive: true });
    }
    await Deno.writeTextFile(path, text);
    await File.chmod(path, mode);
  }

  static async contains(path: string, needles: string[]): Promise<boolean> {
    const text = await File.read(path);
    return needles.some((needle) => text.includes(needle));
  }

  static async backup(path: string): Promise<string> {
    const backup = await Backup.next(path);
    await Deno.rename(path, backup);
    return backup;
  }

  static async read(path: string): Promise<string> {
    try {
      return await Deno.readTextFile(path);
    } catch (err) {
      if (err instanceof Deno.errors.NotFound) {
        return "";
      }
      throw err;
    }
  }
}

class Dir {
  static async exists(path: string): Promise<boolean> {
    try {
      const stat = await Deno.stat(path);
      return stat.isDirectory;
    } catch (err) {
      if (err instanceof Deno.errors.NotFound) {
        return false;
      }
      throw err;
    }
  }

  static async ensure(path: string, mode?: string): Promise<void> {
    await Deno.mkdir(path, { recursive: true });
    await File.chmod(path, mode);
  }
}

class Backup {
  static async next(path: string): Promise<string> {
    const { dir, name } = Route.split(path);
    const first = Route.join(dir, `${name}.bak`);
    if (!(await Route.exists(first))) {
      return first;
    }
    for (let index = 1; index < 1000; index += 1) {
      const candidate = Route.join(dir, `${name}.bak.${index}`);
      if (!(await Route.exists(candidate))) {
        return candidate;
      }
    }
    throw new Error(`too many existing backups for ${path}`);
  }
}

class Route {
  static split(path: string): { dir: string; name: string } {
    const trimmed = path.replace(/[\\/]+$/g, "");
    const slash = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
    const dir = slash < 0 ? "" : trimmed.slice(0, slash);
    const name = slash < 0 ? trimmed : trimmed.slice(slash + 1);
    if (name === "") {
      throw new Error(`invalid path: ${path}`);
    }
    return { dir, name };
  }

  static join(dir: string, name: string): string {
    return dir === "" ? name : stdPath.join(dir, name);
  }

  static async exists(path: string): Promise<boolean> {
    try {
      await Deno.stat(path);
      return true;
    } catch (err) {
      if (err instanceof Deno.errors.NotFound) {
        return false;
      }
      throw err;
    }
  }
}

export const fs = {
  file: {
    exists: File.exists,
    writeText: File.write,
    readTextIfExists: File.read,
    containsAny: File.contains,
    chmodIfUnix: File.chmod,
    backup: {
      numbered: File.backup,
    },
  },
  dir: {
    exists: Dir.exists,
    ensure: Dir.ensure,
  },
};
