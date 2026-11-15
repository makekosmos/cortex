import { test } from "nsh/test";
import { demoState, isRead } from "./state";

/**
 * A page whose data has not arrived must not read as an empty page. These are
 * the decision the pages make (`Pending` draws a skeleton in place of its
 * cards); the pages themselves read the demo state in a test, which is a host
 * that has answered everything.
 */
test("only an answer counts as read", async () => {
  if (!isRead(demoState, "storage")) {
    throw new Error("without a host everything is read: the demo has no skeleton to wait for");
  }
  if (isRead({ ...demoState, read: {} }, "storage")) {
    throw new Error("a host that answered nothing reads as answered");
  }
  if (!isRead({ ...demoState, read: { storage: true } }, "storage")) {
    throw new Error("an answered key does not read as answered");
  }
  if (isRead({ ...demoState, read: { storage: true } }, "backups")) {
    throw new Error("a key answered on its own reads as answered for another page's key");
  }
});
