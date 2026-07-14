export function read(object, camelKey, snakeKey = camelKey) {
  if (!object || typeof object !== "object") {
    return undefined;
  }
  if (Object.hasOwn(object, camelKey)) {
    return object[camelKey];
  }
  if (Object.hasOwn(object, snakeKey)) {
    return object[snakeKey];
  }
  return undefined;
}

export function asArray(value) {
  return Array.isArray(value) ? value : [];
}
