export interface GraphObjectInput {
  id: string;
  typeId: string;
  title?: string;
  deletedAt?: string | null;
}

export interface GraphLinkInput {
  id: string;
  sourceObjectId: string;
  targetObjectId: string;
  linkType: string;
}

export interface GraphTypeInput {
  id: string;
  name: string;
}

export type ColorForType = (typeId: string) => [number, number, number, number];

export interface GraphNodeMeta {
  id: string;
  title: string;
  typeId: string;
  typeName: string;
}

export interface BuiltGraph {
  pointColors: Float32Array;
  pointSizes: Float32Array;
  links: Float32Array;
  nodeMeta: GraphNodeMeta[];
}

export interface BuildGraphOptions {
  baseSize?: number;
  sizeStep?: number;
  hiddenTypeIds?: ReadonlySet<string>;
}

export function buildGraph(
  objects: GraphObjectInput[],
  links: GraphLinkInput[],
  types: GraphTypeInput[],
  colorForType: ColorForType,
  opts?: BuildGraphOptions,
): BuiltGraph {
  const baseSize = opts?.baseSize ?? 4;
  const sizeStep = opts?.sizeStep ?? 2;
  const hiddenTypeIds = opts?.hiddenTypeIds ?? new Set<string>();

  // Create a type map for quick lookups
  const typeMap = new Map<string, string>();
  for (const type of types) {
    typeMap.set(type.id, type.name);
  }

  // First pass: filter surviving objects and build nodeMeta + index map
  const nodeMeta: GraphNodeMeta[] = [];
  const objectIdToNodeIndex = new Map<string, number>();

  for (const object of objects) {
    // Skip deleted objects and hidden types
    if (object.deletedAt || hiddenTypeIds.has(object.typeId)) {
      continue;
    }

    const title = object.title && object.title.length > 0 ? object.title : object.id;
    const typeName = typeMap.get(object.typeId) ?? object.typeId;

    const nodeIndex = nodeMeta.length;
    objectIdToNodeIndex.set(object.id, nodeIndex);

    nodeMeta.push({
      id: object.id,
      title,
      typeId: object.typeId,
      typeName,
    });
  }

  const nodeCount = nodeMeta.length;

  // Initialize degree tracking
  const degree: number[] = Array.from({ length: nodeCount }, () => 0);

  // Second pass: filter links to valid ones and accumulate degrees
  const validLinks: Array<[number, number]> = [];

  for (const link of links) {
    const srcIndex = objectIdToNodeIndex.get(link.sourceObjectId);
    const tgtIndex = objectIdToNodeIndex.get(link.targetObjectId);

    // Check if both endpoints exist and are not a self-link
    if (srcIndex !== undefined && tgtIndex !== undefined && srcIndex !== tgtIndex) {
      validLinks.push([srcIndex, tgtIndex]);
      degree[srcIndex]++;
      degree[tgtIndex]++;
    }
  }

  // Build pointColors
  const pointColors = new Float32Array(nodeCount * 4);
  for (let i = 0; i < nodeCount; i++) {
    const color = colorForType(nodeMeta[i].typeId);
    pointColors[i * 4] = color[0];
    pointColors[i * 4 + 1] = color[1];
    pointColors[i * 4 + 2] = color[2];
    pointColors[i * 4 + 3] = color[3];
  }

  // Build pointSizes
  const pointSizes = new Float32Array(nodeCount);
  for (let i = 0; i < nodeCount; i++) {
    pointSizes[i] = baseSize + sizeStep * Math.sqrt(degree[i]);
  }

  // Build links as flattened Float32Array
  const linksArray = new Float32Array(validLinks.length * 2);
  for (let i = 0; i < validLinks.length; i++) {
    linksArray[i * 2] = validLinks[i][0];
    linksArray[i * 2 + 1] = validLinks[i][1];
  }

  return {
    pointColors,
    pointSizes,
    links: linksArray,
    nodeMeta,
  };
}
