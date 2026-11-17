import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";
import { setLanguage } from "../i18n";

// the interface tests read English, the key language; a test that wants another language
// switches itself and is switched back here
beforeEach(() => setLanguage("en"));

// Testing Library only cleans up on its own when vitest runs with globals
afterEach(cleanup);
