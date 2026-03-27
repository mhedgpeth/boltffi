import { assert, assertArrayEqual, demo } from "../support/index.mjs";

export async function run() {
  const holder = demo.StateHolder.new("local");
  assert.equal(holder.getLabel(), "local");
  assert.equal(holder.getValue(), 0);
  holder.setValue(5);
  assert.equal(holder.getValue(), 5);
  assert.equal(holder.increment(), 6);
  holder.addItem("a");
  holder.addItem("b");
  assert.equal(holder.itemCount(), 2);
  assertArrayEqual(holder.getItems(), ["a", "b"]);
  assert.equal(holder.removeLast(), "b");
  assert.equal(holder.transformValue((value) => Math.trunc(value / 2)), 3);
  assert.equal(holder.applyValueCallback({ onValue: (value) => value + 3 }), 6);
  assert.equal(await holder.asyncGetValue(), 6);
  await holder.asyncSetValue(9);
  assert.equal(holder.getValue(), 9);
  assert.equal(await holder.asyncAddItem("z"), 2);
  assertArrayEqual(holder.getItems(), ["a", "z"]);
  holder.clear();
  assert.equal(holder.getValue(), 0);
  assertArrayEqual(holder.getItems(), []);
  holder.dispose();
}
