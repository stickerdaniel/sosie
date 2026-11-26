import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const OUT = new URL("../assets/primer-tokens.json", import.meta.url);
const TOKENS = [
  "bgColor-default",
  "bgColor-muted",
  "bgColor-neutral-muted",
  "bgColor-success-emphasis",
  "bgColor-danger-emphasis",
  "borderColor-default",
  "borderColor-muted",
  "borderColor-emphasis",
  "fgColor-default",
  "fgColor-muted",
  "fgColor-success",
  "fgColor-danger",
];

const dir = await mkdtemp(join(tmpdir(), "primer-"));
try {
  const { stdout } = await run("npm", ["pack", "@primer/primitives@latest", "--silent", "--pack-destination", dir]);
  await run("tar", ["xzf", join(dir, stdout.trim()), "-C", dir]);
  const pkg = JSON.parse(await readFile(join(dir, "package/package.json"), "utf8"));
  const read = async (theme) => {
    const css = await readFile(join(dir, `package/dist/css/functional/themes/${theme}.css`), "utf8");
    return Object.fromEntries(
      TOKENS.map((token) => {
        const match = css.match(new RegExp(`--${token}:\\s*([^;]+);`));
        if (!match) throw new Error(`${theme}.css has no --${token}`);
        return [token, match[1].trim()];
      }),
    );
  };
  const typography = await readFile(join(dir, "package/dist/css/functional/typography/typography.css"), "utf8");
  const fontStack = typography.match(/--fontStack-system:\s*([^;]+);/)?.[1].trim();
  if (!fontStack) throw new Error("typography.css has no --fontStack-system");
  const tokens = {
    package: `@primer/primitives@${pkg.version}`,
    fontStack,
    themes: { light: await read("light"), dark: await read("dark") },
  };
  await writeFile(OUT, `${JSON.stringify(tokens, null, "\t")}\n`);
  console.log(tokens.package);
} finally {
  await rm(dir, { recursive: true, force: true });
}
