import { assert, assertArrayEqual, demo } from "../support/index.mjs";

export async function run() {
  assert.equal(demo.echoStatus(demo.Status.Active), demo.Status.Active);
  assert.equal(demo.statusToString(demo.Status.Active), "active");
  assert.equal(demo.isActive(demo.Status.Pending), false);
  assertArrayEqual(demo.echoVecStatus([demo.Status.Active, demo.Status.Pending]), [demo.Status.Active, demo.Status.Pending]);
  assert.equal(demo.echoDirection(demo.Direction.East), demo.Direction.East);
  assert.equal(demo.oppositeDirection(demo.Direction.East), demo.Direction.West);
  assert.equal(demo.Direction.fromRaw(2), demo.Direction.East);
  assert.equal(demo.Direction.cardinal(), demo.Direction.North);
  assert.equal(demo.Direction.fromDegrees(90), demo.Direction.East);
  assert.equal(demo.Direction.opposite(demo.Direction.East), demo.Direction.West);
  assert.equal(demo.Direction.isHorizontal(demo.Direction.East), true);
  assert.equal(demo.Direction.label(demo.Direction.South), "S");
  assert.equal(demo.Direction.count(), 4);
}
