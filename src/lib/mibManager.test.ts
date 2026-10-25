import { describe, it, expect, vi, beforeEach } from "vitest";
import { S } from "$lib/stores.svelte";

vi.mock("$lib/tauriCommands", () => ({
  configRead: vi.fn(),
  configWrite: vi.fn(() => Promise.resolve()),
  mibLoad: vi.fn(),
  mibUnloadMany: vi.fn(),
  openDirectory: vi.fn(),
  openFiles: vi.fn(),
  mibOidNameMap: vi.fn(() => Promise.resolve([["1.3.6.1.2.1.1.1", "sysDescr"]])),
  mibTree: vi.fn(() => Promise.resolve([])),
}));

import { configRead, configWrite, mibLoad, mibUnloadMany, openDirectory, openFiles } from "$lib/tauriCommands";
import { refreshMibState, addMibDirectory, addMibFiles, unloadMibs } from "$lib/mibManager";
import type { LoadDirectoriesStatus } from "$lib/types";

const mockedConfigRead = configRead as ReturnType<typeof vi.fn>;
const mockedConfigWrite = configWrite as ReturnType<typeof vi.fn>;
const mockedMibLoad = mibLoad as ReturnType<typeof vi.fn>;
const mockedMibUnloadMany = mibUnloadMany as ReturnType<typeof vi.fn>;
const mockedOpenDirectory = openDirectory as ReturnType<typeof vi.fn>;
const mockedOpenFiles = openFiles as ReturnType<typeof vi.fn>;

const status: LoadDirectoriesStatus = {
  nodeCount: 42,
  fallbackMibs: ["BROKEN-MIB"],
  filesParsed: 1,
  filesCached: 2,
};

beforeEach(() => {
  vi.clearAllMocks();
  S.nodeCount = 0;
  S.fallbackMibs.length = 0;
  S.treeData.length = 0;
  S.treeVersion = 0;
  S.oidNameMap = new Map();
  S.statusText = "Ready";
  mockedMibLoad.mockResolvedValue(status);
  mockedMibUnloadMany.mockResolvedValue(status);
});

describe("refreshMibState", () => {
  it("maps the load status onto the store and refreshes map + tree", async () => {
    await refreshMibState(status);

    expect(S.nodeCount).toBe(42);
    expect(S.fallbackMibs).toEqual(["BROKEN-MIB"]);
    expect(S.oidNameMap.get("1.3.6.1.2.1.1.1")).toBe("sysDescr");
    expect(S.treeVersion).toBe(1);
  });
});

describe("addMibDirectory", () => {
  it("is a no-op when the picker is cancelled", async () => {
    mockedOpenDirectory.mockResolvedValue(null);

    await addMibDirectory();

    expect(mockedMibLoad).not.toHaveBeenCalled();
    expect(mockedConfigWrite).not.toHaveBeenCalled();
  });

  it("dedups an already-configured directory and loads the full set", async () => {
    mockedOpenDirectory.mockResolvedValue("/existing");
    mockedConfigRead.mockResolvedValue({
      mib: { directories: ["/existing"], files: ["/file.mib"] },
    });

    await addMibDirectory();

    expect(mockedConfigWrite).not.toHaveBeenCalled();
    expect(mockedMibLoad).toHaveBeenCalledWith(["/existing"], ["/file.mib"]);
  });

  it("appends a new directory to the config before loading", async () => {
    mockedOpenDirectory.mockResolvedValue("/new");
    mockedConfigRead.mockResolvedValue({
      mib: { directories: ["/existing"], files: [] },
    });

    await addMibDirectory();

    expect(mockedConfigWrite).toHaveBeenCalledWith("mib.directories", ["/existing", "/new"]);
    expect(mockedMibLoad).toHaveBeenCalledWith(["/existing", "/new"], []);
  });

  it("sets the load status text with parse/cache stats", async () => {
    mockedOpenDirectory.mockResolvedValue("/new");
    mockedConfigRead.mockResolvedValue({ mib: { directories: [], files: [] } });

    await addMibDirectory();

    expect(S.statusText).toBe("Loaded 42 nodes (1 parsed, 2 cached)");
  });
});

describe("addMibFiles", () => {
  it("is a no-op when the picker is cancelled", async () => {
    mockedOpenFiles.mockResolvedValue(null);

    await addMibFiles();

    expect(mockedMibLoad).not.toHaveBeenCalled();
    expect(mockedConfigWrite).not.toHaveBeenCalled();
  });

  it("dedups already-configured files and persists the new list", async () => {
    mockedOpenFiles.mockResolvedValue(["/file.mib", "/new.mib"]);
    mockedConfigRead.mockResolvedValue({
      mib: { directories: ["/dir"], files: ["/file.mib"] },
    });

    await addMibFiles();

    expect(mockedConfigWrite).toHaveBeenCalledWith("mib.files", ["/file.mib", "/new.mib"]);
    expect(mockedMibLoad).toHaveBeenCalledWith(["/dir"], ["/file.mib", "/new.mib"]);
  });
});

describe("unloadMibs", () => {
  it("dispatches the bulk unload by file path and refreshes state", async () => {
    await unloadMibs(["/a.mib", "/b.mib"]);

    expect(mockedMibUnloadMany).toHaveBeenCalledWith(["/a.mib", "/b.mib"]);
    expect(S.nodeCount).toBe(42);
    expect(S.treeVersion).toBe(1);
  });
});
