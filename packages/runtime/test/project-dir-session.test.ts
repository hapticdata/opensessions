import { describe, expect, test } from "bun:test";
import { buildDirSessionMap, resolveSessionForProjectDir } from "../src/server/project-dir-session";

describe("resolveSessionForProjectDir", () => {
  test("returns the exact unique session match", () => {
    const sessions = buildDirSessionMap([
      { name: "api", dir: "/projects/myapp" },
      { name: "web", dir: "/projects/web" },
    ]);

    expect(resolveSessionForProjectDir("/projects/myapp", sessions)).toBe("api");
  });

  test("treats duplicate session directories as ambiguous", () => {
    const sessions = buildDirSessionMap([
      { name: "api", dir: "/projects/myapp" },
      { name: "web", dir: "/projects/myapp" },
    ]);

    expect(resolveSessionForProjectDir("/projects/myapp", sessions)).toBeNull();
  });

  test("prefers an exact match over parent or child directory matches", () => {
    const sessions = buildDirSessionMap([
      { name: "root", dir: "/projects/myapp" },
      { name: "pkg", dir: "/projects/myapp/packages/ui" },
    ]);

    expect(resolveSessionForProjectDir("/projects/myapp", sessions)).toBe("root");
  });

  test("returns null when only related directory matches are ambiguous", () => {
    const sessions = buildDirSessionMap([
      { name: "root", dir: "/projects" },
      { name: "pkg", dir: "/projects/myapp/packages/ui" },
    ]);

    expect(resolveSessionForProjectDir("/projects/myapp", sessions)).toBeNull();
  });

  test("supports encoded Claude project directories when the match is unique", () => {
    const sessions = buildDirSessionMap([
      { name: "api", dir: "/projects/my.app" },
    ]);

    expect(resolveSessionForProjectDir("__encoded__:-projects-my-app", sessions)).toBe("api");
  });
});
