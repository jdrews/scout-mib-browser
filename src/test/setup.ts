import { vi } from "vitest";

// Mock Tauri invoke - it doesn't exist in jsdom
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
