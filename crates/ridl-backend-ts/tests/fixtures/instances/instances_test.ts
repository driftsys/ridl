import { instances } from "./__ridl_instances.ts";

Deno.test("instance constants match the root lock and retain literal types", () => {
  const primary: 1 = instances.veh.adas.cruise.PRIMARY;
  const backup: 2 = instances.veh.adas.cruise.BACKUP;
  const unit: 3 = instances.veh.ego.status.UNIT;
  if (primary !== 1 || backup !== 2 || unit !== 3) {
    throw new Error("Generated instance constants differ from instances.lock");
  }
});
