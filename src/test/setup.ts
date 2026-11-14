import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
// German texts with their placeholders filled in, as in the app
import "../i18n";

// Testing Library only cleans up on its own when vitest runs with globals
afterEach(cleanup);
