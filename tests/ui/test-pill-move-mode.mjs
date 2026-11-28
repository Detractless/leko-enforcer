import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { mountPillVm } from "./test-pill-timings.mjs";

const context = (vm) => vm.listeners.get("pill:contextmenu")({ preventDefault() {} });
test("right click toggles move mode", () => {
  const vm = mountPillVm(); context(vm); context(vm);
  assert.deepEqual(vm.posted, ["move_toggle", "move_toggle"]);
});
test("drag moves pill without voice actions", () => {
  const vm = mountPillVm(); context(vm);
  vm.listeners.get("pill:pointerdown")({ button: 0 });
  vm.listeners.get("pill:pointerup")({ button: 0 });
  assert.deepEqual(vm.posted, ["move_toggle", "move_start", "move_end"]);
});
test("pill styles match idle hover and move states", () => {
  const css = readFileSync(new URL("../../ui/src/web/pill.css", import.meta.url), "utf8");
  assert.match(css, /state-idle\.is-hovered \{ width: 72px; height: 24px;/);
  assert.match(css, /move-mode \{\s*width: 112px; height: 30px;/);
  assert.match(css, /move-mode \.dash \{ display: block;/);
});
test("keyboard moves and drops pill", () => {
  const vm = mountPillVm(); vm.ctx.window.__leko_keyboard_move();
  for (const key of ["ArrowRight", "Enter"]) vm.listeners.get("document:keydown")({ key, preventDefault() {} });
  assert.deepEqual(vm.posted, ["move_keyboard_start", "move_key_right", "move_key_commit"]);
});
test("move requests during active voice are ignored", () => {
  for (const state of ["thinking", "speaking"]) {
    const vm = mountPillVm(); vm.ctx.window.__leko_on_event({ state });
    context(vm); vm.ctx.window.__leko_keyboard_move();
    vm.ctx.window.__leko_on_event({ state: "reply_card", text: "hello" });
    context(vm); vm.ctx.window.__leko_keyboard_move();
    vm.ctx.window.__leko_on_event({ state: "speaking" });
    vm.ctx.window.__leko_on_event({ state: "idle" });
    assert.deepEqual(vm.posted, []);
    assert.equal(vm.els.pill.className.includes("move-mode"), false);
  }
});
test("voice starting during movement ends move mode", () => {
  for (const keyboard of [false, true]) {
    const vm = mountPillVm();
    if (keyboard) vm.ctx.window.__leko_keyboard_move(); else context(vm);
    vm.ctx.window.__leko_on_event({ state: "speaking" });
    assert.equal(vm.els.pill.className.includes("move-mode"), false);
    assert.equal(vm.els.pill.className.includes("state-speaking"), true);
    assert.deepEqual(vm.posted, keyboard ? ["move_keyboard_start", "move_key_interrupt"] : ["move_toggle", "move_toggle"]);
  }
});

test("move glow has window space above and below", () => {
  const css = readFileSync(new URL("../../ui/src/web/pill.css", import.meta.url), "utf8");
  assert.match(css, /html, body \{[^}]*padding: 0;/);
  assert.match(css, /body \{ padding: 0 0 44px 0; \}/);
});
