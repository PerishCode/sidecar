function join(...parts: string[]): string {
  const separator = Deno.build.os === "windows" ? "\\" : "/";
  const joined = parts
    .filter((part) => part !== "")
    .map((part, index) =>
      index === 0 ? part.replace(/[\\/]+$/g, "") : part.replace(/^[\\/]+|[\\/]+$/g, "")
    )
    .filter((part) => part !== "")
    .join(separator);
  return joined === "" ? "." : joined;
}

function dirname(path: string): string {
  const trimmed = path.replace(/[\\/]+$/g, "");
  const slash = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  if (slash < 0) {
    return "";
  }
  if (slash === 0) {
    return trimmed.slice(0, 1);
  }
  return trimmed.slice(0, slash);
}

function separator(): string {
  return Deno.build.os === "windows" ? ";" : ":";
}

export const path = {
  join,
  dirname,
  listSeparator: separator,
};
