function print(value = ""): void {
  console.log(value);
}

function error(value: string): void {
  console.error(value);
}

function fail(message: string, code = 1): never {
  error(message);
  Deno.exit(code);
}

export const io = {
  print,
  error,
  fail,
};
