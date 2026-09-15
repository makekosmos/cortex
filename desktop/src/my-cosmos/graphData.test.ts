import { describe, it, expect } from "../../test-support/node-test.mjs";
import {
  buildGraph,
  type GraphObjectInput,
  type GraphLinkInput,
  type GraphTypeInput,
} from "./graphData";

describe("buildGraph", () => {
  it("empty objects returns empty graph", () => {
    const result = buildGraph([], [], [], () => [1, 1, 1, 1]);

    expect(result.pointColors.length).toBe(0);
    expect(result.pointSizes.length).toBe(0);
    expect(result.links.length).toBe(0);
    expect(result.nodeMeta).toEqual([]);
  });

  it("basic 3 objects across 2 types with colors", () => {
    const objects: GraphObjectInput[] = [
      { id: "obj1", typeId: "A", title: "Object 1" },
      { id: "obj2", typeId: "B", title: "Object 2" },
      { id: "obj3", typeId: "A", title: "Object 3" },
    ];

    const types: GraphTypeInput[] = [
      { id: "A", name: "TypeA" },
      { id: "B", name: "TypeB" },
    ];

    const colorForType = (typeId: string) => (typeId === "A" ? [1, 0, 0, 1] : [0, 1, 0, 1]);

    const result = buildGraph(objects, [], types, colorForType);

    expect(result.nodeMeta).toHaveLength(3);
    expect(result.nodeMeta[0]).toEqual({
      id: "obj1",
      title: "Object 1",
      typeId: "A",
      typeName: "TypeA",
    });
    expect(result.nodeMeta[1]).toEqual({
      id: "obj2",
      title: "Object 2",
      typeId: "B",
      typeName: "TypeB",
    });
    expect(result.nodeMeta[2]).toEqual({
      id: "obj3",
      title: "Object 3",
      typeId: "A",
      typeName: "TypeA",
    });

    expect(Array.from(result.pointColors)).toEqual([
      1,
      0,
      0,
      1, // obj1: typeA red
      0,
      1,
      0,
      1, // obj2: typeB green
      1,
      0,
      0,
      1, // obj3: typeA red
    ]);
  });

  it("title fallback to id when title is missing or empty", () => {
    const objects: GraphObjectInput[] = [
      { id: "obj1", typeId: "A" }, // no title
      { id: "obj2", typeId: "A", title: "" }, // empty title
      { id: "obj3", typeId: "A", title: "Has Title" }, // non-empty title
    ];

    const types: GraphTypeInput[] = [{ id: "A", name: "TypeA" }];

    const result = buildGraph(objects, [], types, () => [1, 1, 1, 1]);

    expect(result.nodeMeta[0].title).toBe("obj1");
    expect(result.nodeMeta[1].title).toBe("obj2");
    expect(result.nodeMeta[2].title).toBe("Has Title");
  });

  it("typeName fallback to typeId when type not in types list", () => {
    const objects: GraphObjectInput[] = [
      { id: "obj1", typeId: "A", title: "Obj 1" },
      { id: "obj2", typeId: "UnknownType", title: "Obj 2" },
    ];

    const types: GraphTypeInput[] = [{ id: "A", name: "TypeA" }];

    const result = buildGraph(objects, [], types, () => [1, 1, 1, 1]);

    expect(result.nodeMeta[0].typeName).toBe("TypeA");
    expect(result.nodeMeta[1].typeName).toBe("UnknownType");
  });

  it("deleted object is excluded and its links are dropped", () => {
    const objects: GraphObjectInput[] = [
      { id: "obj1", typeId: "A", title: "Obj 1" },
      { id: "obj2", typeId: "A", title: "Obj 2", deletedAt: "2026-06-18" },
      { id: "obj3", typeId: "A", title: "Obj 3" },
    ];

    const links: GraphLinkInput[] = [
      { id: "link1", sourceObjectId: "obj1", targetObjectId: "obj2", linkType: "rel" },
      { id: "link2", sourceObjectId: "obj1", targetObjectId: "obj3", linkType: "rel" },
      { id: "link3", sourceObjectId: "obj2", targetObjectId: "obj3", linkType: "rel" },
    ];

    const types: GraphTypeInput[] = [{ id: "A", name: "TypeA" }];

    const result = buildGraph(objects, links, types, () => [1, 1, 1, 1]);

    expect(result.nodeMeta).toHaveLength(2);
    expect(result.nodeMeta[0].id).toBe("obj1");
    expect(result.nodeMeta[1].id).toBe("obj3");
    expect(Array.from(result.links)).toEqual([0, 1]); // only obj1->obj3 (nodeIndex 0 to 1)
  });

  it("hiddenTypeIds excludes object and drops its links", () => {
    const objects: GraphObjectInput[] = [
      { id: "obj1", typeId: "A", title: "Obj 1" },
      { id: "obj2", typeId: "B", title: "Obj 2" },
      { id: "obj3", typeId: "A", title: "Obj 3" },
    ];

    const links: GraphLinkInput[] = [
      { id: "link1", sourceObjectId: "obj1", targetObjectId: "obj2", linkType: "rel" },
      { id: "link2", sourceObjectId: "obj1", targetObjectId: "obj3", linkType: "rel" },
      { id: "link3", sourceObjectId: "obj2", targetObjectId: "obj3", linkType: "rel" },
    ];

    const types: GraphTypeInput[] = [
      { id: "A", name: "TypeA" },
      { id: "B", name: "TypeB" },
    ];

    const result = buildGraph(objects, links, types, () => [1, 1, 1, 1], {
      hiddenTypeIds: new Set(["B"]),
    });

    expect(result.nodeMeta).toHaveLength(2);
    expect(result.nodeMeta[0].id).toBe("obj1");
    expect(result.nodeMeta[1].id).toBe("obj3");
    expect(Array.from(result.links)).toEqual([0, 1]); // only obj1->obj3
  });

  it("link referencing non-existent object id is dropped", () => {
    const objects: GraphObjectInput[] = [
      { id: "obj1", typeId: "A", title: "Obj 1" },
      { id: "obj2", typeId: "A", title: "Obj 2" },
    ];

    const links: GraphLinkInput[] = [
      { id: "link1", sourceObjectId: "obj1", targetObjectId: "nonexistent", linkType: "rel" },
      { id: "link2", sourceObjectId: "obj1", targetObjectId: "obj2", linkType: "rel" },
      { id: "link3", sourceObjectId: "nonexistent", targetObjectId: "obj2", linkType: "rel" },
    ];

    const types: GraphTypeInput[] = [{ id: "A", name: "TypeA" }];

    const result = buildGraph(objects, links, types, () => [1, 1, 1, 1]);

    expect(result.nodeMeta).toHaveLength(2);
    expect(Array.from(result.links)).toEqual([0, 1]); // only obj1->obj2
  });

  it("self-link is dropped", () => {
    const objects: GraphObjectInput[] = [
      { id: "obj1", typeId: "A", title: "Obj 1" },
      { id: "obj2", typeId: "A", title: "Obj 2" },
    ];

    const links: GraphLinkInput[] = [
      { id: "link1", sourceObjectId: "obj1", targetObjectId: "obj1", linkType: "rel" },
      { id: "link2", sourceObjectId: "obj1", targetObjectId: "obj2", linkType: "rel" },
      { id: "link3", sourceObjectId: "obj2", targetObjectId: "obj2", linkType: "rel" },
    ];

    const types: GraphTypeInput[] = [{ id: "A", name: "TypeA" }];

    const result = buildGraph(objects, links, types, () => [1, 1, 1, 1]);

    expect(result.nodeMeta).toHaveLength(2);
    expect(Array.from(result.links)).toEqual([0, 1]); // only obj1->obj2
  });

  it("degree->size calculation: hub with 4 leaves", () => {
    const objects: GraphObjectInput[] = [
      { id: "hub", typeId: "A", title: "Hub" },
      { id: "leaf1", typeId: "A", title: "Leaf 1" },
      { id: "leaf2", typeId: "A", title: "Leaf 2" },
      { id: "leaf3", typeId: "A", title: "Leaf 3" },
      { id: "leaf4", typeId: "A", title: "Leaf 4" },
    ];

    const links: GraphLinkInput[] = [
      { id: "l1", sourceObjectId: "hub", targetObjectId: "leaf1", linkType: "rel" },
      { id: "l2", sourceObjectId: "hub", targetObjectId: "leaf2", linkType: "rel" },
      { id: "l3", sourceObjectId: "hub", targetObjectId: "leaf3", linkType: "rel" },
      { id: "l4", sourceObjectId: "hub", targetObjectId: "leaf4", linkType: "rel" },
    ];

    const types: GraphTypeInput[] = [{ id: "A", name: "TypeA" }];

    const result = buildGraph(objects, links, types, () => [1, 1, 1, 1], {
      baseSize: 4,
      sizeStep: 2,
    });

    expect(result.nodeMeta).toHaveLength(5);

    // Hub has degree 4, size = 4 + 2 * sqrt(4) = 4 + 2 * 2 = 8
    const hubSize = result.pointSizes[0];
    expect(hubSize).toBeCloseTo(8, 5);

    // Each leaf has degree 1, size = 4 + 2 * sqrt(1) = 4 + 2 * 1 = 6
    const leafSize = result.pointSizes[1];
    expect(leafSize).toBeCloseTo(6, 5);
    expect(result.pointSizes[2]).toBeCloseTo(6, 5);
    expect(result.pointSizes[3]).toBeCloseTo(6, 5);
    expect(result.pointSizes[4]).toBeCloseTo(6, 5);

    // 4 edges, each represented as [srcIndex, tgtIndex], so 8 floats total
    expect(result.links.length).toBe(8);
    expect(Array.from(result.links)).toEqual([0, 1, 0, 2, 0, 3, 0, 4]);
  });

  it("custom baseSize and sizeStep are respected", () => {
    const objects: GraphObjectInput[] = [
      { id: "obj1", typeId: "A", title: "Obj 1" },
      { id: "obj2", typeId: "A", title: "Obj 2" },
    ];

    const links: GraphLinkInput[] = [
      { id: "link1", sourceObjectId: "obj1", targetObjectId: "obj2", linkType: "rel" },
    ];

    const types: GraphTypeInput[] = [{ id: "A", name: "TypeA" }];

    const result = buildGraph(objects, links, types, () => [1, 1, 1, 1], {
      baseSize: 10,
      sizeStep: 5,
    });

    expect(result.nodeMeta).toHaveLength(2);

    // obj1 has degree 1, size = 10 + 5 * sqrt(1) = 15
    const size1 = result.pointSizes[0];
    expect(size1).toBeCloseTo(15, 5);

    // obj2 has degree 1, size = 10 + 5 * sqrt(1) = 15
    const size2 = result.pointSizes[1];
    expect(size2).toBeCloseTo(15, 5);
  });
});
